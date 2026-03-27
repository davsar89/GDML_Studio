use crate::mesh::types::TriangleMesh;
use std::f64::consts::PI;

/// Tessellate a paraboloid (G4Paraboloid).
///
/// - `rlo`: radius at z = -dz (lower face)
/// - `rhi`: radius at z = +dz (upper face)
/// - `dz`: half-height
///
/// Parabolic profile: r²(z) = k1 + k2*z
/// where k1 = (rhi²+rlo²)/2, k2 = (rhi²-rlo²)/(2*dz)
pub fn tessellate_paraboloid(rlo: f64, rhi: f64, dz: f64, segments: u32) -> TriangleMesh {
    let mut positions: Vec<f32> = Vec::new();
    let mut normals: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    if dz <= 0.0 || (rlo <= 0.0 && rhi <= 0.0) {
        return TriangleMesh { positions, normals, indices };
    }

    let k1 = (rhi * rhi + rlo * rlo) / 2.0;
    let k2 = if dz > 1e-12 { (rhi * rhi - rlo * rlo) / (2.0 * dz) } else { 0.0 };

    let r_at = |z: f64| -> f64 {
        let r2 = k1 + k2 * z;
        if r2 > 0.0 { r2.sqrt() } else { 0.0 }
    };

    let phi_seg = segments;
    let z_seg = (segments / 2).max(2);
    let dphi = 2.0 * PI / phi_seg as f64;
    let dz_step = 2.0 * dz / z_seg as f64;

    // Side surface
    for j in 0..z_seg {
        let z0 = -dz + j as f64 * dz_step;
        let z1 = -dz + (j + 1) as f64 * dz_step;
        let rad0 = r_at(z0);
        let rad1 = r_at(z1);

        for i in 0..phi_seg {
            let phi0 = i as f64 * dphi;
            let phi1 = (i + 1) as f64 * dphi;
            let c0 = phi0.cos();
            let s0 = phi0.sin();
            let c1 = phi1.cos();
            let s1 = phi1.sin();

            let base = (positions.len() / 3) as u32;

            let verts = [
                (rad0 * c0, rad0 * s0, z0),
                (rad0 * c1, rad0 * s1, z0),
                (rad1 * c1, rad1 * s1, z1),
                (rad1 * c0, rad1 * s0, z1),
            ];

            let u = [verts[1].0 - verts[0].0, verts[1].1 - verts[0].1, verts[1].2 - verts[0].2];
            let v = [verts[3].0 - verts[0].0, verts[3].1 - verts[0].1, verts[3].2 - verts[0].2];
            let mut nx = u[1] * v[2] - u[2] * v[1];
            let mut ny = u[2] * v[0] - u[0] * v[2];
            let mut nz = u[0] * v[1] - u[1] * v[0];
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            if len > 1e-12 {
                nx /= len; ny /= len; nz /= len;
            }

            for (vx, vy, vz) in &verts {
                positions.push(*vx as f32);
                positions.push(*vy as f32);
                positions.push(*vz as f32);
                normals.push(nx as f32);
                normals.push(ny as f32);
                normals.push(nz as f32);
            }

            indices.push(base); indices.push(base + 1); indices.push(base + 2);
            indices.push(base); indices.push(base + 2); indices.push(base + 3);
        }
    }

    // Bottom cap at z = -dz
    if rlo > 1e-10 {
        let center_idx = (positions.len() / 3) as u32;
        positions.push(0.0); positions.push(0.0); positions.push(-dz as f32);
        normals.push(0.0); normals.push(0.0); normals.push(-1.0);

        for i in 0..=phi_seg {
            let phi = i as f64 * dphi;
            positions.push((rlo * phi.cos()) as f32);
            positions.push((rlo * phi.sin()) as f32);
            positions.push(-dz as f32);
            normals.push(0.0); normals.push(0.0); normals.push(-1.0);
        }
        for i in 0..phi_seg {
            indices.push(center_idx);
            indices.push(center_idx + 2 + i);
            indices.push(center_idx + 1 + i);
        }
    }

    // Top cap at z = +dz
    if rhi > 1e-10 {
        let center_idx = (positions.len() / 3) as u32;
        positions.push(0.0); positions.push(0.0); positions.push(dz as f32);
        normals.push(0.0); normals.push(0.0); normals.push(1.0);

        for i in 0..=phi_seg {
            let phi = i as f64 * dphi;
            positions.push((rhi * phi.cos()) as f32);
            positions.push((rhi * phi.sin()) as f32);
            positions.push(dz as f32);
            normals.push(0.0); normals.push(0.0); normals.push(1.0);
        }
        for i in 0..phi_seg {
            indices.push(center_idx);
            indices.push(center_idx + 1 + i);
            indices.push(center_idx + 2 + i);
        }
    }

    TriangleMesh { positions, normals, indices }
}
