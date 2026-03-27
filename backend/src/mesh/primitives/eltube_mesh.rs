use crate::mesh::types::TriangleMesh;
use std::f64::consts::PI;

/// Tessellate an elliptical tube (G4EllipticalTube).
///
/// - `dx`, `dy`: semi-axes of the elliptical cross-section
/// - `dz`: half-length along Z axis
pub fn tessellate_eltube(dx: f64, dy: f64, dz: f64, segments: u32) -> TriangleMesh {
    let mut positions: Vec<f32> = Vec::new();
    let mut normals: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    if dx <= 0.0 || dy <= 0.0 || dz <= 0.0 {
        return TriangleMesh { positions, normals, indices };
    }

    let seg = segments;
    let dphi = 2.0 * PI / seg as f64;

    // Side wall: quads connecting bottom and top ellipse rings
    for i in 0..seg {
        let phi0 = i as f64 * dphi;
        let phi1 = (i + 1) as f64 * dphi;

        let c0 = phi0.cos();
        let s0 = phi0.sin();
        let c1 = phi1.cos();
        let s1 = phi1.sin();

        let x0 = dx * c0;
        let y0 = dy * s0;
        let x1 = dx * c1;
        let y1 = dy * s1;

        // Ellipse outward normal: gradient of (x/dx)^2 + (y/dy)^2 = 1
        // n = (cos(phi)/dx, sin(phi)/dy, 0) normalized
        let norm = |c: f64, s: f64| -> [f64; 3] {
            let nx = c / dx;
            let ny = s / dy;
            let len = (nx * nx + ny * ny).sqrt();
            if len > 1e-12 { [nx / len, ny / len, 0.0] } else { [1.0, 0.0, 0.0] }
        };

        let n0 = norm(c0, s0);
        let n1 = norm(c1, s1);

        let base = (positions.len() / 3) as u32;

        // v0: bottom-left, v1: bottom-right, v2: top-right, v3: top-left
        let verts = [
            (x0, y0, -dz, n0),
            (x1, y1, -dz, n1),
            (x1, y1, dz, n1),
            (x0, y0, dz, n0),
        ];

        for (vx, vy, vz, n) in &verts {
            positions.push(*vx as f32);
            positions.push(*vy as f32);
            positions.push(*vz as f32);
            normals.push(n[0] as f32);
            normals.push(n[1] as f32);
            normals.push(n[2] as f32);
        }

        indices.push(base);
        indices.push(base + 1);
        indices.push(base + 2);
        indices.push(base);
        indices.push(base + 2);
        indices.push(base + 3);
    }

    // Top cap (z = +dz, normal pointing up)
    {
        let center_idx = (positions.len() / 3) as u32;
        positions.push(0.0);
        positions.push(0.0);
        positions.push(dz as f32);
        normals.push(0.0);
        normals.push(0.0);
        normals.push(1.0);

        for i in 0..=seg {
            let phi = i as f64 * dphi;
            positions.push((dx * phi.cos()) as f32);
            positions.push((dy * phi.sin()) as f32);
            positions.push(dz as f32);
            normals.push(0.0);
            normals.push(0.0);
            normals.push(1.0);
        }

        for i in 0..seg {
            indices.push(center_idx);
            indices.push(center_idx + 1 + i);
            indices.push(center_idx + 2 + i);
        }
    }

    // Bottom cap (z = -dz, normal pointing down)
    {
        let center_idx = (positions.len() / 3) as u32;
        positions.push(0.0);
        positions.push(0.0);
        positions.push(-dz as f32);
        normals.push(0.0);
        normals.push(0.0);
        normals.push(-1.0);

        for i in 0..=seg {
            let phi = i as f64 * dphi;
            positions.push((dx * phi.cos()) as f32);
            positions.push((dy * phi.sin()) as f32);
            positions.push(-dz as f32);
            normals.push(0.0);
            normals.push(0.0);
            normals.push(-1.0);
        }

        for i in 0..seg {
            indices.push(center_idx);
            indices.push(center_idx + 2 + i);
            indices.push(center_idx + 1 + i);
        }
    }

    TriangleMesh { positions, normals, indices }
}
