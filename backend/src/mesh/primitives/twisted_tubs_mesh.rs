use crate::mesh::types::TriangleMesh;
use std::f64::consts::PI;

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
    p0: [f32; 3],
    p1: [f32; 3],
    p2: [f32; 3],
    p3: [f32; 3],
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

/// Tessellate a `<twistedtubs>`.
///
/// Both radii are the **z = 0** (waist) values, matching `G4TwistedTubs`'s
/// `fInnerRadius`/`fOuterRadius`. The lateral surfaces are hyperboloids, not
/// cylinders: `G4TwistedTubs::SetFields` (inline in `G4TwistedTubs.hh`) sets
///
/// ```text
/// tanStereo       = |r_mid * tan(twist/2)| / zHalfLength
/// endRadius[i]    = sqrt(r_mid^2 + endZ[i]^2 * tanStereo^2)
/// endPhi[i]       = atan2(endZ[i] * tan(twist/2), zHalfLength)
/// ```
///
/// so radius grows away from the waist and reaches `r_mid / cos(twist/2)` at the
/// ends. That is why the GDML reader offers `endinnerrad` *and* `midinnerrad` —
/// they are different surfaces of the same solid.
///
/// A zero twist makes `tanStereo` zero and the profile collapses to a plain
/// tube, which is the correct limit.
pub fn tessellate_twisted_tubs(
    rmin_mid: f64,
    rmax_mid: f64,
    z_neg: f64,
    z_pos: f64,
    deltaphi: f64,
    twist_angle: f64,
    segments: u32,
) -> TriangleMesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    let z_half = z_neg.abs().max(z_pos.abs());
    if z_half < 1e-12 || rmax_mid <= 1e-12 {
        return TriangleMesh {
            positions,
            normals,
            indices,
        };
    }

    let has_hole = rmin_mid > 1e-10;
    let full_circle = (deltaphi - 2.0 * PI).abs() < 1e-6;

    let phi_segs = segments.max(3);
    let z_segs = segments.max(3);
    let dphi = deltaphi / phi_segs as f64;
    let dz = (z_pos - z_neg) / z_segs as f64;

    // The hyperboloid, straight out of SetFields.
    let tan_half_twist = (0.5 * twist_angle).tan();
    let tan_in = (rmin_mid * tan_half_twist).abs() / z_half;
    let tan_out = (rmax_mid * tan_half_twist).abs() / z_half;
    let r_in = |z: f64| (rmin_mid * rmin_mid + z * z * tan_in * tan_in).sqrt();
    let r_out = |z: f64| (rmax_mid * rmax_mid + z * z * tan_out * tan_out).sqrt();
    // Azimuth of the twisted surface at height z. Note this is NOT linear in z;
    // it is the arctangent law above, which still totals `twist_angle` across
    // the full length.
    let twist_at = |z: f64| (z * tan_half_twist).atan2(z_half);

    let lateral = |positions: &mut Vec<f32>,
                   normals: &mut Vec<f32>,
                   indices: &mut Vec<u32>,
                   radius: &dyn Fn(f64) -> f64,
                   flip: bool| {
        for j in 0..z_segs {
            let za = z_neg + j as f64 * dz;
            let zb = za + dz;
            let (ra, rb) = (radius(za), radius(zb));
            let (twa, twb) = (twist_at(za), twist_at(zb));

            for i in 0..phi_segs {
                let phi0 = i as f64 * dphi;
                let phi1 = phi0 + dphi;
                let a0 = (phi0 + twa) as f32;
                let a1 = (phi1 + twa) as f32;
                let a2 = (phi1 + twb) as f32;
                let a3 = (phi0 + twb) as f32;
                let (ra, rb) = (ra as f32, rb as f32);

                emit_quad(
                    positions,
                    normals,
                    indices,
                    [ra * a0.cos(), ra * a0.sin(), za as f32],
                    [ra * a1.cos(), ra * a1.sin(), za as f32],
                    [rb * a2.cos(), rb * a2.sin(), zb as f32],
                    [rb * a3.cos(), rb * a3.sin(), zb as f32],
                    flip,
                );
            }
        }
    };

    lateral(&mut positions, &mut normals, &mut indices, &r_out, false);
    if has_hole {
        lateral(&mut positions, &mut normals, &mut indices, &r_in, true);
    }

    // ─── End caps, at the end radii rather than the waist radii ──────────────
    for (z, outward) in [(z_pos, 1.0f32), (z_neg, -1.0f32)] {
        let tw = twist_at(z);
        let (ro, ri) = (r_out(z) as f32, r_in(z) as f32);
        let zf = z as f32;
        let n = [0.0f32, 0.0, outward];

        if has_hole {
            for i in 0..phi_segs {
                let phi0 = i as f64 * dphi + tw;
                let phi1 = phi0 + dphi;
                let (c0, s0) = ((phi0 as f32).cos(), (phi0 as f32).sin());
                let (c1, s1) = ((phi1 as f32).cos(), (phi1 as f32).sin());
                let base = (positions.len() / 3) as u32;
                for v in &[
                    [ri * c0, ri * s0, zf],
                    [ro * c0, ro * s0, zf],
                    [ro * c1, ro * s1, zf],
                    [ri * c1, ri * s1, zf],
                ] {
                    positions.extend_from_slice(v);
                    normals.extend_from_slice(&n);
                }
                if outward > 0.0 {
                    indices.extend_from_slice(&[base, base + 1, base + 2]);
                    indices.extend_from_slice(&[base, base + 2, base + 3]);
                } else {
                    indices.extend_from_slice(&[base, base + 2, base + 1]);
                    indices.extend_from_slice(&[base, base + 3, base + 2]);
                }
            }
        } else {
            let center = (positions.len() / 3) as u32;
            positions.extend_from_slice(&[0.0, 0.0, zf]);
            normals.extend_from_slice(&n);
            for i in 0..=phi_segs {
                let phi = (i as f64 * dphi + tw) as f32;
                positions.extend_from_slice(&[ro * phi.cos(), ro * phi.sin(), zf]);
                normals.extend_from_slice(&n);
            }
            for i in 0..phi_segs {
                if outward > 0.0 {
                    indices.extend_from_slice(&[center, center + 1 + i, center + 2 + i]);
                } else {
                    indices.extend_from_slice(&[center, center + 2 + i, center + 1 + i]);
                }
            }
        }
    }

    // ─── Flat faces closing a partial sweep ──────────────────────────────────
    if !full_circle {
        for (phi_off, flip) in [(0.0, false), (deltaphi, true)] {
            for j in 0..z_segs {
                let za = z_neg + j as f64 * dz;
                let zb = za + dz;
                let a0 = (phi_off + twist_at(za)) as f32;
                let a1 = (phi_off + twist_at(zb)) as f32;
                let (roa, rob) = (r_out(za) as f32, r_out(zb) as f32);

                let (inner_a, inner_b) = if has_hole {
                    let (ria, rib) = (r_in(za) as f32, r_in(zb) as f32);
                    (
                        [ria * a0.cos(), ria * a0.sin(), za as f32],
                        [rib * a1.cos(), rib * a1.sin(), zb as f32],
                    )
                } else {
                    ([0.0, 0.0, za as f32], [0.0, 0.0, zb as f32])
                };

                emit_quad(
                    &mut positions,
                    &mut normals,
                    &mut indices,
                    inner_a,
                    [roa * a0.cos(), roa * a0.sin(), za as f32],
                    [rob * a1.cos(), rob * a1.sin(), zb as f32],
                    inner_b,
                    flip,
                );
            }
        }
    }

    TriangleMesh {
        positions,
        normals,
        indices,
    }
}
