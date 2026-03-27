use crate::mesh::types::TriangleMesh;

fn face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let w = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let nx = u[1] * w[2] - u[2] * w[1];
    let ny = u[2] * w[0] - u[0] * w[2];
    let nz = u[0] * w[1] - u[1] * w[0];
    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    if len > 1e-10 {
        [nx / len, ny / len, nz / len]
    } else {
        [0.0, 0.0, 1.0]
    }
}

fn emit_quad(
    positions: &mut Vec<f32>,
    normals: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    p0: [f32; 3], p1: [f32; 3], p2: [f32; 3], p3: [f32; 3],
    flip: bool,
) {
    let n = if flip {
        let n = face_normal(p0, p1, p2);
        [-n[0], -n[1], -n[2]]
    } else {
        face_normal(p0, p1, p2)
    };
    let base = (positions.len() / 3) as u32;
    for p in &[p0, p1, p2, p3] {
        positions.extend_from_slice(p);
        normals.extend_from_slice(&n);
    }
    if flip {
        indices.extend_from_slice(&[base, base + 2, base + 1]);
        indices.extend_from_slice(&[base, base + 3, base + 2]);
    } else {
        indices.extend_from_slice(&[base, base + 1, base + 2]);
        indices.extend_from_slice(&[base, base + 2, base + 3]);
    }
}

/// Generate a twisted box mesh.
/// All dimensions are FULL lengths (halved internally).
/// `phi_twist` is the total twist angle in radians.
pub fn tessellate_twisted_box(
    phi_twist: f64,
    x: f64,
    y: f64,
    z: f64,
    segments: u32,
) -> TriangleMesh {
    let hx = (x / 2.0) as f32;
    let hy = (y / 2.0) as f32;
    let hz = (z / 2.0) as f32;

    let z_segs = segments.max(3);
    let dz = z / z_segs as f64;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // Compute 4 rotated corners at a given z-level
    let corners_at = |z_pos: f64| -> [[f32; 3]; 4] {
        let t = phi_twist * (z_pos + hz as f64) / (2.0 * hz as f64);
        let (st, ct) = (t.sin() as f32, t.cos() as f32);
        let zf = z_pos as f32;
        // Unrotated corners: (-hx,-hy), (+hx,-hy), (+hx,+hy), (-hx,+hy)
        // Rotated: (x*cos - y*sin, x*sin + y*cos)
        [
            [(-hx) * ct - (-hy) * st, (-hx) * st + (-hy) * ct, zf],
            [hx * ct - (-hy) * st,    hx * st + (-hy) * ct,    zf],
            [hx * ct - hy * st,       hx * st + hy * ct,       zf],
            [(-hx) * ct - hy * st,    (-hx) * st + hy * ct,    zf],
        ]
    };

    // 4 side faces, each subdivided along Z
    // Side edges: 0-1 (-Y), 1-2 (+X), 2-3 (+Y), 3-0 (-X)
    for j in 0..z_segs {
        let z0 = -(hz as f64) + j as f64 * dz;
        let z1 = z0 + dz;
        let bot = corners_at(z0);
        let top = corners_at(z1);

        // Side 0 (-Y): bot[0]->bot[1] to top[0]->top[1]
        emit_quad(&mut positions, &mut normals, &mut indices,
            bot[0], bot[1], top[1], top[0], false);
        // Side 1 (+X): bot[1]->bot[2] to top[1]->top[2]
        emit_quad(&mut positions, &mut normals, &mut indices,
            bot[1], bot[2], top[2], top[1], false);
        // Side 2 (+Y): bot[2]->bot[3] to top[2]->top[3]
        emit_quad(&mut positions, &mut normals, &mut indices,
            bot[2], bot[3], top[3], top[2], false);
        // Side 3 (-X): bot[3]->bot[0] to top[3]->top[0]
        emit_quad(&mut positions, &mut normals, &mut indices,
            bot[3], bot[0], top[0], top[3], false);
    }

    // Top cap (z = +hz)
    {
        let c = corners_at(hz as f64);
        emit_quad(&mut positions, &mut normals, &mut indices,
            c[0], c[1], c[2], c[3], false);
    }

    // Bottom cap (z = -hz)
    {
        let c = corners_at(-(hz as f64));
        emit_quad(&mut positions, &mut normals, &mut indices,
            c[3], c[2], c[1], c[0], false);
    }

    TriangleMesh {
        positions,
        normals,
        indices,
    }
}
