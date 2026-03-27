use crate::mesh::types::TriangleMesh;
use std::f64::consts::PI;

/// Tessellate a hyperbolic tube (G4Hype).
///
/// - `rmin`, `rmax`: base inner/outer radii at z=0
/// - `inst`, `outst`: inner/outer stereo angles (radians)
/// - `hz`: half-length along Z (already halved by caller)
///
/// Radius varies as: `r(z) = sqrt(r0² + (z * tan(stereo))²)`
pub fn tessellate_hype(
    rmin: f64,
    rmax: f64,
    inst: f64,
    outst: f64,
    hz: f64,
    segments: u32,
) -> TriangleMesh {
    let mut positions: Vec<f32> = Vec::new();
    let mut normals: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    if rmax <= 0.0 || hz <= 0.0 {
        return TriangleMesh { positions, normals, indices };
    }

    let phi_seg = segments;
    let z_seg = (segments / 2).max(2);
    let dphi = 2.0 * PI / phi_seg as f64;
    let dz = 2.0 * hz / z_seg as f64;

    let tan_out = outst.tan();
    let tan_in = inst.tan();

    let r_outer = |z: f64| -> f64 { (rmax * rmax + (z * tan_out).powi(2)).sqrt() };
    let r_inner = |z: f64| -> f64 {
        if rmin > 1e-10 {
            (rmin * rmin + (z * tan_in).powi(2)).sqrt()
        } else {
            0.0
        }
    };

    let has_inner = rmin > 1e-10;

    // Generate a surface of revolution (outer or inner)
    let gen_surface = |r_fn: &dyn Fn(f64) -> f64, flip: bool,
                       positions: &mut Vec<f32>, normals: &mut Vec<f32>, indices: &mut Vec<u32>| {
        for j in 0..z_seg {
            let z0 = -hz + j as f64 * dz;
            let z1 = -hz + (j + 1) as f64 * dz;
            let rad0 = r_fn(z0);
            let rad1 = r_fn(z1);

            for i in 0..phi_seg {
                let phi0 = i as f64 * dphi;
                let phi1 = (i + 1) as f64 * dphi;

                let c0 = phi0.cos();
                let s0 = phi0.sin();
                let c1 = phi1.cos();
                let s1 = phi1.sin();

                let base = (positions.len() / 3) as u32;

                // 4 vertices of the quad
                let verts = [
                    (rad0 * c0, rad0 * s0, z0),
                    (rad0 * c1, rad0 * s1, z0),
                    (rad1 * c1, rad1 * s1, z1),
                    (rad1 * c0, rad1 * s0, z1),
                ];

                // Compute face normal from cross product
                let u = [verts[1].0 - verts[0].0, verts[1].1 - verts[0].1, verts[1].2 - verts[0].2];
                let v = [verts[3].0 - verts[0].0, verts[3].1 - verts[0].1, verts[3].2 - verts[0].2];
                let mut nx = u[1] * v[2] - u[2] * v[1];
                let mut ny = u[2] * v[0] - u[0] * v[2];
                let mut nz = u[0] * v[1] - u[1] * v[0];
                let len = (nx * nx + ny * ny + nz * nz).sqrt();
                if len > 1e-12 {
                    nx /= len; ny /= len; nz /= len;
                }
                if flip {
                    nx = -nx; ny = -ny; nz = -nz;
                }

                for (vx, vy, vz) in &verts {
                    positions.push(*vx as f32);
                    positions.push(*vy as f32);
                    positions.push(*vz as f32);
                    normals.push(nx as f32);
                    normals.push(ny as f32);
                    normals.push(nz as f32);
                }

                if !flip {
                    indices.push(base); indices.push(base + 1); indices.push(base + 2);
                    indices.push(base); indices.push(base + 2); indices.push(base + 3);
                } else {
                    indices.push(base); indices.push(base + 2); indices.push(base + 1);
                    indices.push(base); indices.push(base + 3); indices.push(base + 2);
                }
            }
        }
    };

    // Outer surface
    gen_surface(&r_outer, false, &mut positions, &mut normals, &mut indices);

    // Inner surface
    if has_inner {
        gen_surface(&r_inner, true, &mut positions, &mut normals, &mut indices);
    }

    // Top and bottom annular caps
    let gen_cap = |z: f64, is_top: bool,
                   positions: &mut Vec<f32>, normals: &mut Vec<f32>, indices: &mut Vec<u32>| {
        let r_out = r_outer(z);
        let r_in = r_inner(z);
        let nz_val: f32 = if is_top { 1.0 } else { -1.0 };

        for i in 0..phi_seg {
            let phi0 = i as f64 * dphi;
            let phi1 = (i + 1) as f64 * dphi;
            let c0 = phi0.cos();
            let s0 = phi0.sin();
            let c1 = phi1.cos();
            let s1 = phi1.sin();

            let base = (positions.len() / 3) as u32;

            let verts = [
                (r_out * c0, r_out * s0),
                (r_out * c1, r_out * s1),
                (r_in * c1, r_in * s1),
                (r_in * c0, r_in * s0),
            ];

            for (vx, vy) in &verts {
                positions.push(*vx as f32);
                positions.push(*vy as f32);
                positions.push(z as f32);
                normals.push(0.0);
                normals.push(0.0);
                normals.push(nz_val);
            }

            if is_top {
                indices.push(base); indices.push(base + 1); indices.push(base + 2);
                indices.push(base); indices.push(base + 2); indices.push(base + 3);
            } else {
                indices.push(base); indices.push(base + 2); indices.push(base + 1);
                indices.push(base); indices.push(base + 3); indices.push(base + 2);
            }
        }
    };

    gen_cap(-hz, false, &mut positions, &mut normals, &mut indices);
    gen_cap(hz, true, &mut positions, &mut normals, &mut indices);

    TriangleMesh { positions, normals, indices }
}
