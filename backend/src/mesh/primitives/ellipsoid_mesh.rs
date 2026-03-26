use crate::mesh::types::TriangleMesh;
use std::f64::consts::PI;

/// Tessellate an ellipsoid (G4Ellipsoid) with optional z-cuts.
///
/// - `ax`, `by`, `cz`: semi-axes along X, Y, Z
/// - `zcut1`: lower z-cut plane (default -cz = no cut)
/// - `zcut2`: upper z-cut plane (default +cz = no cut)
pub fn tessellate_ellipsoid(
    ax: f64,
    by: f64,
    cz: f64,
    zcut1: f64,
    zcut2: f64,
    segments: u32,
) -> TriangleMesh {
    let mut positions: Vec<f32> = Vec::new();
    let mut normals: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    if ax <= 0.0 || by <= 0.0 || cz <= 0.0 {
        return TriangleMesh { positions, normals, indices };
    }

    // Clamp z-cuts to valid range
    let zcut1 = zcut1.max(-cz).min(cz);
    let zcut2 = zcut2.max(-cz).min(cz);
    if zcut1 >= zcut2 {
        return TriangleMesh { positions, normals, indices };
    }

    // Convert z-cuts to theta range: z = cz * cos(theta)
    // theta = acos(z / cz)
    let theta_start = (zcut2 / cz).clamp(-1.0, 1.0).acos(); // top cut -> smaller theta
    let theta_end = (zcut1 / cz).clamp(-1.0, 1.0).acos(); // bottom cut -> larger theta

    let phi_segs = segments;
    let theta_segs = segments / 2;
    let dphi = 2.0 * PI / phi_segs as f64;
    let dtheta = (theta_end - theta_start) / theta_segs as f64;

    let has_top_cut = (zcut2 - cz).abs() > 1e-10;
    let has_bot_cut = (zcut1 + cz).abs() > 1e-10;

    // Precompute inverse squares for normal calculation
    let ax2 = ax * ax;
    let by2 = by * by;
    let cz2 = cz * cz;

    // Helper: push a vertex and its ellipsoid normal
    let push_vert = |theta: f64, phi: f64, positions: &mut Vec<f32>, normals: &mut Vec<f32>| {
        let st = theta.sin();
        let ct = theta.cos();
        let sp = phi.sin();
        let cp = phi.cos();

        let x = ax * st * cp;
        let y = by * st * sp;
        let z = cz * ct;

        // Gradient of x²/ax² + y²/by² + z²/cz² = 1
        let mut nx = x / ax2;
        let mut ny = y / by2;
        let mut nz = z / cz2;
        let len = (nx * nx + ny * ny + nz * nz).sqrt();
        if len > 1e-12 {
            nx /= len;
            ny /= len;
            nz /= len;
        }

        positions.push(x as f32);
        positions.push(y as f32);
        positions.push(z as f32);
        normals.push(nx as f32);
        normals.push(ny as f32);
        normals.push(nz as f32);
    };

    // Generate outer surface quads
    for j in 0..theta_segs {
        let theta0 = theta_start + j as f64 * dtheta;
        let theta1 = theta_start + (j + 1) as f64 * dtheta;

        for i in 0..phi_segs {
            let phi0 = i as f64 * dphi;
            let phi1 = (i + 1) as f64 * dphi;

            let base = (positions.len() / 3) as u32;

            push_vert(theta0, phi0, &mut positions, &mut normals);
            push_vert(theta0, phi1, &mut positions, &mut normals);
            push_vert(theta1, phi1, &mut positions, &mut normals);
            push_vert(theta1, phi0, &mut positions, &mut normals);

            indices.push(base);
            indices.push(base + 1);
            indices.push(base + 2);
            indices.push(base);
            indices.push(base + 2);
            indices.push(base + 3);
        }
    }

    // Top cap (flat disk at z = zcut2) if z-cut is active
    if has_top_cut {
        let z_cap = zcut2 as f32;
        let st = theta_start.sin();
        let rx = ax * st; // elliptical radius at this theta
        let ry = by * st;

        // Center vertex
        let center_idx = (positions.len() / 3) as u32;
        positions.push(0.0);
        positions.push(0.0);
        positions.push(z_cap);
        normals.push(0.0);
        normals.push(0.0);
        normals.push(1.0);

        // Rim vertices
        for i in 0..=phi_segs {
            let phi = i as f64 * dphi;
            let x = rx * phi.cos();
            let y = ry * phi.sin();
            positions.push(x as f32);
            positions.push(y as f32);
            positions.push(z_cap);
            normals.push(0.0);
            normals.push(0.0);
            normals.push(1.0);
        }

        for i in 0..phi_segs {
            indices.push(center_idx);
            indices.push(center_idx + 1 + i);
            indices.push(center_idx + 2 + i);
        }
    }

    // Bottom cap (flat disk at z = zcut1) if z-cut is active
    if has_bot_cut {
        let z_cap = zcut1 as f32;
        let st = theta_end.sin();
        let rx = ax * st;
        let ry = by * st;

        let center_idx = (positions.len() / 3) as u32;
        positions.push(0.0);
        positions.push(0.0);
        positions.push(z_cap);
        normals.push(0.0);
        normals.push(0.0);
        normals.push(-1.0);

        for i in 0..=phi_segs {
            let phi = i as f64 * dphi;
            let x = rx * phi.cos();
            let y = ry * phi.sin();
            positions.push(x as f32);
            positions.push(y as f32);
            positions.push(z_cap);
            normals.push(0.0);
            normals.push(0.0);
            normals.push(-1.0);
        }

        for i in 0..phi_segs {
            indices.push(center_idx);
            indices.push(center_idx + 2 + i);
            indices.push(center_idx + 1 + i);
        }
    }

    TriangleMesh { positions, normals, indices }
}
