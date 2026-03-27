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

/// Generate a twisted tube mesh.
/// `zlen` is the FULL Z-length (halved internally). `deltaphi` is the azimuthal
/// span in radians (start is always 0). `twist_angle` is the total twist over
/// the full Z-length in radians.
pub fn tessellate_twisted_tubs(
    rmin: f64,
    rmax: f64,
    zlen: f64,
    deltaphi: f64,
    twist_angle: f64,
    segments: u32,
) -> TriangleMesh {
    let hz = (zlen / 2.0) as f32;
    let has_hole = rmin > 1e-10;
    let full_circle = (deltaphi - 2.0 * PI).abs() < 1e-6;

    let phi_segs = segments.max(3);
    let z_segs = segments.max(3);
    let dphi = deltaphi / phi_segs as f64;
    let dz = zlen / z_segs as f64;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    let twist_at = |z: f64| -> f64 {
        twist_angle * (z + hz as f64) / (2.0 * hz as f64)
    };

    let rmax_f = rmax as f32;
    let rmin_f = rmin as f32;

    // Outer surface: z_segs x phi_segs quads
    for j in 0..z_segs {
        let z0 = -(hz as f64) + j as f64 * dz;
        let z1 = z0 + dz;
        let tw0 = twist_at(z0);
        let tw1 = twist_at(z1);

        for i in 0..phi_segs {
            let phi0 = i as f64 * dphi;
            let phi1 = phi0 + dphi;

            let a0 = (phi0 + tw0) as f32;
            let a1 = (phi1 + tw0) as f32;
            let a2 = (phi1 + tw1) as f32;
            let a3 = (phi0 + tw1) as f32;

            let p0 = [rmax_f * a0.cos(), rmax_f * a0.sin(), z0 as f32];
            let p1 = [rmax_f * a1.cos(), rmax_f * a1.sin(), z0 as f32];
            let p2 = [rmax_f * a2.cos(), rmax_f * a2.sin(), z1 as f32];
            let p3 = [rmax_f * a3.cos(), rmax_f * a3.sin(), z1 as f32];

            emit_quad(&mut positions, &mut normals, &mut indices, p0, p1, p2, p3, false);
        }
    }

    // Inner surface (if hollow)
    if has_hole {
        for j in 0..z_segs {
            let z0 = -(hz as f64) + j as f64 * dz;
            let z1 = z0 + dz;
            let tw0 = twist_at(z0);
            let tw1 = twist_at(z1);

            for i in 0..phi_segs {
                let phi0 = i as f64 * dphi;
                let phi1 = phi0 + dphi;

                let a0 = (phi0 + tw0) as f32;
                let a1 = (phi1 + tw0) as f32;
                let a2 = (phi1 + tw1) as f32;
                let a3 = (phi0 + tw1) as f32;

                let p0 = [rmin_f * a0.cos(), rmin_f * a0.sin(), z0 as f32];
                let p1 = [rmin_f * a1.cos(), rmin_f * a1.sin(), z0 as f32];
                let p2 = [rmin_f * a2.cos(), rmin_f * a2.sin(), z1 as f32];
                let p3 = [rmin_f * a3.cos(), rmin_f * a3.sin(), z1 as f32];

                emit_quad(&mut positions, &mut normals, &mut indices, p0, p1, p2, p3, true);
            }
        }
    }

    // Top cap (z = +hz, twist = twist_angle)
    {
        let tw = twist_angle;
        if has_hole {
            for i in 0..phi_segs {
                let phi0 = i as f64 * dphi + tw;
                let phi1 = phi0 + dphi;

                let base = (positions.len() / 3) as u32;
                let verts = [
                    [rmin_f * (phi0 as f32).cos(), rmin_f * (phi0 as f32).sin(), hz],
                    [rmax_f * (phi0 as f32).cos(), rmax_f * (phi0 as f32).sin(), hz],
                    [rmax_f * (phi1 as f32).cos(), rmax_f * (phi1 as f32).sin(), hz],
                    [rmin_f * (phi1 as f32).cos(), rmin_f * (phi1 as f32).sin(), hz],
                ];
                let n = [0.0f32, 0.0, 1.0];
                for v in &verts {
                    positions.extend_from_slice(v);
                    normals.extend_from_slice(&n);
                }
                indices.extend_from_slice(&[base, base + 1, base + 2]);
                indices.extend_from_slice(&[base, base + 2, base + 3]);
            }
        } else {
            let center_base = (positions.len() / 3) as u32;
            positions.extend_from_slice(&[0.0, 0.0, hz]);
            normals.extend_from_slice(&[0.0, 0.0, 1.0]);
            for i in 0..=phi_segs {
                let phi = i as f64 * dphi + tw;
                positions.extend_from_slice(&[rmax_f * (phi as f32).cos(), rmax_f * (phi as f32).sin(), hz]);
                normals.extend_from_slice(&[0.0, 0.0, 1.0]);
            }
            for i in 0..phi_segs {
                indices.extend_from_slice(&[center_base, center_base + 1 + i, center_base + 2 + i]);
            }
        }
    }

    // Bottom cap (z = -hz, twist = 0)
    {
        if has_hole {
            for i in 0..phi_segs {
                let phi0 = i as f64 * dphi;
                let phi1 = phi0 + dphi;

                let base = (positions.len() / 3) as u32;
                let verts = [
                    [rmin_f * (phi0 as f32).cos(), rmin_f * (phi0 as f32).sin(), -hz],
                    [rmax_f * (phi0 as f32).cos(), rmax_f * (phi0 as f32).sin(), -hz],
                    [rmax_f * (phi1 as f32).cos(), rmax_f * (phi1 as f32).sin(), -hz],
                    [rmin_f * (phi1 as f32).cos(), rmin_f * (phi1 as f32).sin(), -hz],
                ];
                let n = [0.0f32, 0.0, -1.0];
                for v in &verts {
                    positions.extend_from_slice(v);
                    normals.extend_from_slice(&n);
                }
                indices.extend_from_slice(&[base, base + 2, base + 1]);
                indices.extend_from_slice(&[base, base + 3, base + 2]);
            }
        } else {
            let center_base = (positions.len() / 3) as u32;
            positions.extend_from_slice(&[0.0, 0.0, -hz]);
            normals.extend_from_slice(&[0.0, 0.0, -1.0]);
            for i in 0..=phi_segs {
                let phi = i as f64 * dphi;
                positions.extend_from_slice(&[rmax_f * (phi as f32).cos(), rmax_f * (phi as f32).sin(), -hz]);
                normals.extend_from_slice(&[0.0, 0.0, -1.0]);
            }
            for i in 0..phi_segs {
                indices.extend_from_slice(&[center_base, center_base + 2 + i, center_base + 1 + i]);
            }
        }
    }

    // Wedge faces for partial phi
    if !full_circle {
        // Start-phi wedge (phi = 0 + twist)
        for j in 0..z_segs {
            let z0 = -(hz as f64) + j as f64 * dz;
            let z1 = z0 + dz;
            let tw0 = twist_at(z0);
            let tw1 = twist_at(z1);
            let a0 = tw0 as f32;
            let a1 = tw1 as f32;

            if has_hole {
                let p0 = [rmin_f * a0.cos(), rmin_f * a0.sin(), z0 as f32];
                let p1 = [rmax_f * a0.cos(), rmax_f * a0.sin(), z0 as f32];
                let p2 = [rmax_f * a1.cos(), rmax_f * a1.sin(), z1 as f32];
                let p3 = [rmin_f * a1.cos(), rmin_f * a1.sin(), z1 as f32];
                emit_quad(&mut positions, &mut normals, &mut indices, p0, p1, p2, p3, true);
            } else {
                let p0 = [0.0, 0.0, z0 as f32];
                let p1 = [rmax_f * a0.cos(), rmax_f * a0.sin(), z0 as f32];
                let p2 = [rmax_f * a1.cos(), rmax_f * a1.sin(), z1 as f32];
                let p3 = [0.0, 0.0, z1 as f32];
                emit_quad(&mut positions, &mut normals, &mut indices, p0, p1, p2, p3, true);
            }
        }

        // End-phi wedge (phi = deltaphi + twist)
        for j in 0..z_segs {
            let z0 = -(hz as f64) + j as f64 * dz;
            let z1 = z0 + dz;
            let tw0 = twist_at(z0);
            let tw1 = twist_at(z1);
            let a0 = (deltaphi + tw0) as f32;
            let a1 = (deltaphi + tw1) as f32;

            if has_hole {
                let p0 = [rmin_f * a0.cos(), rmin_f * a0.sin(), z0 as f32];
                let p1 = [rmax_f * a0.cos(), rmax_f * a0.sin(), z0 as f32];
                let p2 = [rmax_f * a1.cos(), rmax_f * a1.sin(), z1 as f32];
                let p3 = [rmin_f * a1.cos(), rmin_f * a1.sin(), z1 as f32];
                emit_quad(&mut positions, &mut normals, &mut indices, p0, p1, p2, p3, false);
            } else {
                let p0 = [0.0, 0.0, z0 as f32];
                let p1 = [rmax_f * a0.cos(), rmax_f * a0.sin(), z0 as f32];
                let p2 = [rmax_f * a1.cos(), rmax_f * a1.sin(), z1 as f32];
                let p3 = [0.0, 0.0, z1 as f32];
                emit_quad(&mut positions, &mut normals, &mut indices, p0, p1, p2, p3, false);
            }
        }
    }

    TriangleMesh {
        positions,
        normals,
        indices,
    }
}
