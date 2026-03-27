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

/// Generate a twisted general trapezoid mesh.
/// All length parameters are FULL dimensions (halved internally).
/// All angles are in radians.
pub fn tessellate_twisted_trap(
    phi_twist: f64,
    z: f64,
    theta: f64,
    phi_angle: f64,
    y1: f64,
    x1: f64,
    x2: f64,
    y2: f64,
    x3: f64,
    x4: f64,
    alph: f64,
    segments: u32,
) -> TriangleMesh {
    let dz = z / 2.0;
    let dy1 = y1 / 2.0;
    let dx1 = x1 / 2.0;
    let dx2 = x2 / 2.0;
    let dy2 = y2 / 2.0;
    let dx3 = x3 / 2.0;
    let dx4 = x4 / 2.0;

    let tan_theta = theta.tan();
    let x_shift = dz * tan_theta * phi_angle.cos();
    let y_shift = dz * tan_theta * phi_angle.sin();
    let tan_a = alph.tan();

    let z_segs = segments.max(3);
    let dz_step = z / z_segs as f64;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // Compute 4 trap corners at a given z-level, with twist applied
    let corners_at = |z_pos: f64| -> [[f32; 3]; 4] {
        // Interpolation parameter: 0 at bottom, 1 at top
        let t = (z_pos + dz) / (2.0 * dz);

        // Interpolate half-dimensions
        let dy = dy1 + t * (dy2 - dy1);
        let dx_lo = dx1 + t * (dx3 - dx1); // x at -y edge
        let dx_hi = dx2 + t * (dx4 - dx2); // x at +y edge

        // Interpolate center offset
        let cx = -x_shift + t * (2.0 * x_shift);
        let cy = -y_shift + t * (2.0 * y_shift);

        // 4 corners (same formula as trap_mesh.rs)
        let c0 = [cx - dy * tan_a - dx_lo, cy - dy];
        let c1 = [cx - dy * tan_a + dx_lo, cy - dy];
        let c2 = [cx + dy * tan_a + dx_hi, cy + dy];
        let c3 = [cx + dy * tan_a - dx_hi, cy + dy];

        // Apply twist rotation
        let twist = phi_twist * t;
        let (st, ct) = (twist.sin() as f32, twist.cos() as f32);
        let zf = z_pos as f32;

        let rotate = |xy: [f64; 2]| -> [f32; 3] {
            let x = xy[0] as f32;
            let y = xy[1] as f32;
            [x * ct - y * st, x * st + y * ct, zf]
        };

        [rotate(c0), rotate(c1), rotate(c2), rotate(c3)]
    };

    // 4 side faces, Z-segmented
    for j in 0..z_segs {
        let z0 = -dz + j as f64 * dz_step;
        let z1 = z0 + dz_step;
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

    // Top cap (z = +dz)
    {
        let c = corners_at(dz);
        emit_quad(&mut positions, &mut normals, &mut indices,
            c[0], c[1], c[2], c[3], false);
    }

    // Bottom cap (z = -dz)
    {
        let c = corners_at(-dz);
        emit_quad(&mut positions, &mut normals, &mut indices,
            c[3], c[2], c[1], c[0], false);
    }

    TriangleMesh {
        positions,
        normals,
        indices,
    }
}
