use crate::mesh::types::TriangleMesh;
use std::f64::consts::PI;

/// Tessellate an elliptical cone (G4EllipticalCone).
///
/// - `dx`, `dy`: dimensionless semi-axis ratios
/// - `zmax`: cone height (at z=zmax, radius=0)
/// - `zcut`: z-cut half-height (solid extends from -zcut to +zcut)
///
/// At height z, the elliptical cross-section has semi-axes:
///   rx(z) = dx * (zmax - z), ry(z) = dy * (zmax - z)
pub fn tessellate_elcone(
    dx: f64,
    dy: f64,
    zmax: f64,
    zcut: f64,
    segments: u32,
) -> TriangleMesh {
    let mut positions: Vec<f32> = Vec::new();
    let mut normals: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    if dx <= 0.0 || dy <= 0.0 || zmax <= 0.0 || zcut <= 0.0 {
        return TriangleMesh { positions, normals, indices };
    }

    let zcut = zcut.min(zmax);
    let phi_seg = segments;
    let z_seg = (segments / 2).max(2);
    let dphi = 2.0 * PI / phi_seg as f64;
    let dz = 2.0 * zcut / z_seg as f64;

    let rx_at = |z: f64| -> f64 { dx * (zmax - z) };
    let ry_at = |z: f64| -> f64 { dy * (zmax - z) };

    // Side surface: quads between z-slices
    for j in 0..z_seg {
        let z0 = -zcut + j as f64 * dz;
        let z1 = -zcut + (j + 1) as f64 * dz;
        let rx0 = rx_at(z0);
        let ry0 = ry_at(z0);
        let rx1 = rx_at(z1);
        let ry1 = ry_at(z1);

        for i in 0..phi_seg {
            let phi0 = i as f64 * dphi;
            let phi1 = (i + 1) as f64 * dphi;
            let c0 = phi0.cos();
            let s0 = phi0.sin();
            let c1 = phi1.cos();
            let s1 = phi1.sin();

            let base = (positions.len() / 3) as u32;

            let verts = [
                (rx0 * c0, ry0 * s0, z0),
                (rx0 * c1, ry0 * s1, z0),
                (rx1 * c1, ry1 * s1, z1),
                (rx1 * c0, ry1 * s0, z1),
            ];

            // Face normal from cross product
            let u = [verts[1].0 - verts[0].0, verts[1].1 - verts[0].1, verts[1].2 - verts[0].2];
            let v = [verts[3].0 - verts[0].0, verts[3].1 - verts[0].1, verts[3].2 - verts[0].2];
            let mut nx = u[1] * v[2] - u[2] * v[1];
            let mut ny = u[2] * v[0] - u[0] * v[2];
            let mut nz = u[0] * v[1] - u[1] * v[0];
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            if len > 1e-12 {
                nx /= len;
                ny /= len;
                nz /= len;
            }

            for (vx, vy, vz) in &verts {
                positions.push(*vx as f32);
                positions.push(*vy as f32);
                positions.push(*vz as f32);
                normals.push(nx as f32);
                normals.push(ny as f32);
                normals.push(nz as f32);
            }

            indices.push(base);
            indices.push(base + 1);
            indices.push(base + 2);
            indices.push(base);
            indices.push(base + 2);
            indices.push(base + 3);
        }
    }

    // Bottom cap at z = -zcut (larger ellipse)
    {
        let z = -zcut;
        let rx = rx_at(z);
        let ry = ry_at(z);
        let center_idx = (positions.len() / 3) as u32;
        positions.push(0.0);
        positions.push(0.0);
        positions.push(z as f32);
        normals.push(0.0);
        normals.push(0.0);
        normals.push(-1.0);

        for i in 0..=phi_seg {
            let phi = i as f64 * dphi;
            positions.push((rx * phi.cos()) as f32);
            positions.push((ry * phi.sin()) as f32);
            positions.push(z as f32);
            normals.push(0.0);
            normals.push(0.0);
            normals.push(-1.0);
        }

        for i in 0..phi_seg {
            indices.push(center_idx);
            indices.push(center_idx + 2 + i);
            indices.push(center_idx + 1 + i);
        }
    }

    // Top cap at z = +zcut (smaller ellipse)
    {
        let z = zcut;
        let rx = rx_at(z);
        let ry = ry_at(z);
        if rx > 1e-10 && ry > 1e-10 {
            let center_idx = (positions.len() / 3) as u32;
            positions.push(0.0);
            positions.push(0.0);
            positions.push(z as f32);
            normals.push(0.0);
            normals.push(0.0);
            normals.push(1.0);

            for i in 0..=phi_seg {
                let phi = i as f64 * dphi;
                positions.push((rx * phi.cos()) as f32);
                positions.push((ry * phi.sin()) as f32);
                positions.push(z as f32);
                normals.push(0.0);
                normals.push(0.0);
                normals.push(1.0);
            }

            for i in 0..phi_seg {
                indices.push(center_idx);
                indices.push(center_idx + 1 + i);
                indices.push(center_idx + 2 + i);
            }
        }
    }

    TriangleMesh { positions, normals, indices }
}
