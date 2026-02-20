use crate::mesh::types::TriangleMesh;
use std::f64::consts::PI;

/// Generate a cone mesh (frustum with linearly varying inner/outer radii).
/// GDML cone: z is full length, rmin1/rmax1 at -z/2, rmin2/rmax2 at +z/2.
pub fn tessellate_cone(
    rmin1: f64,
    rmax1: f64,
    rmin2: f64,
    rmax2: f64,
    z: f64,
    startphi: f64,
    deltaphi: f64,
    segments: u32,
) -> TriangleMesh {
    let hz = (z / 2.0) as f32;
    let has_hole = rmin1 > 1e-10 || rmin2 > 1e-10;
    let full_circle = (deltaphi - 2.0 * PI).abs() < 1e-6;
    let seg = segments.max(3);

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    let phi_step = deltaphi / seg as f64;

    // Outer surface: sloped normals
    let outer_base = 0u32;
    let dr_outer = rmax2 - rmax1;
    let slope_outer = if z.abs() > 1e-10 {
        (dr_outer / z) as f32
    } else {
        0.0
    };
    for i in 0..=seg {
        let phi = startphi + phi_step * i as f64;
        let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
        let r1 = rmax1 as f32;
        let r2 = rmax2 as f32;

        // Normal for a cone surface: outward + upward component
        let nz = -slope_outer;
        let nr = 1.0_f32;
        let len = (nz * nz + nr * nr).sqrt();
        let nz = nz / len;
        let nr = nr / len;

        // Bottom vertex
        positions.extend_from_slice(&[r1 * cp, r1 * sp, -hz]);
        normals.extend_from_slice(&[nr * cp, nr * sp, nz]);
        // Top vertex
        positions.extend_from_slice(&[r2 * cp, r2 * sp, hz]);
        normals.extend_from_slice(&[nr * cp, nr * sp, nz]);
    }
    for i in 0..seg {
        let b = outer_base + i * 2;
        indices.extend_from_slice(&[b, b + 2, b + 3]);
        indices.extend_from_slice(&[b, b + 3, b + 1]);
    }

    // Inner surface
    if has_hole {
        let inner_base = positions.len() as u32 / 3;
        let dr_inner = rmin2 - rmin1;
        let slope_inner = if z.abs() > 1e-10 {
            (dr_inner / z) as f32
        } else {
            0.0
        };
        for i in 0..=seg {
            let phi = startphi + phi_step * i as f64;
            let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
            let r1 = rmin1 as f32;
            let r2 = rmin2 as f32;

            let nz = slope_inner;
            let nr = 1.0_f32;
            let len = (nz * nz + nr * nr).sqrt();
            let nz = nz / len;
            let nr = nr / len;

            positions.extend_from_slice(&[r1 * cp, r1 * sp, -hz]);
            normals.extend_from_slice(&[-nr * cp, -nr * sp, nz]);
            positions.extend_from_slice(&[r2 * cp, r2 * sp, hz]);
            normals.extend_from_slice(&[-nr * cp, -nr * sp, nz]);
        }
        for i in 0..seg {
            let b = inner_base + i * 2;
            indices.extend_from_slice(&[b, b + 3, b + 2]);
            indices.extend_from_slice(&[b, b + 1, b + 3]);
        }
    }

    // Top cap (z = +hz)
    {
        let cap_base = positions.len() as u32 / 3;
        if has_hole {
            for i in 0..=seg {
                let phi = startphi + phi_step * i as f64;
                let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
                positions.extend_from_slice(&[rmin2 as f32 * cp, rmin2 as f32 * sp, hz]);
                normals.extend_from_slice(&[0.0, 0.0, 1.0]);
                positions.extend_from_slice(&[rmax2 as f32 * cp, rmax2 as f32 * sp, hz]);
                normals.extend_from_slice(&[0.0, 0.0, 1.0]);
            }
            for i in 0..seg {
                let b = cap_base + i * 2;
                indices.extend_from_slice(&[b, b + 1, b + 3]);
                indices.extend_from_slice(&[b, b + 3, b + 2]);
            }
        } else if rmax2 > 1e-10 {
            positions.extend_from_slice(&[0.0, 0.0, hz]);
            normals.extend_from_slice(&[0.0, 0.0, 1.0]);
            for i in 0..=seg {
                let phi = startphi + phi_step * i as f64;
                let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
                positions.extend_from_slice(&[rmax2 as f32 * cp, rmax2 as f32 * sp, hz]);
                normals.extend_from_slice(&[0.0, 0.0, 1.0]);
            }
            for i in 0..seg {
                indices.extend_from_slice(&[cap_base, cap_base + 1 + i, cap_base + 2 + i]);
            }
        }
    }

    // Bottom cap (z = -hz)
    {
        let cap_base = positions.len() as u32 / 3;
        if has_hole {
            for i in 0..=seg {
                let phi = startphi + phi_step * i as f64;
                let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
                positions.extend_from_slice(&[rmin1 as f32 * cp, rmin1 as f32 * sp, -hz]);
                normals.extend_from_slice(&[0.0, 0.0, -1.0]);
                positions.extend_from_slice(&[rmax1 as f32 * cp, rmax1 as f32 * sp, -hz]);
                normals.extend_from_slice(&[0.0, 0.0, -1.0]);
            }
            for i in 0..seg {
                let b = cap_base + i * 2;
                indices.extend_from_slice(&[b, b + 3, b + 1]);
                indices.extend_from_slice(&[b, b + 2, b + 3]);
            }
        } else if rmax1 > 1e-10 {
            positions.extend_from_slice(&[0.0, 0.0, -hz]);
            normals.extend_from_slice(&[0.0, 0.0, -1.0]);
            for i in 0..=seg {
                let phi = startphi + phi_step * i as f64;
                let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
                positions.extend_from_slice(&[rmax1 as f32 * cp, rmax1 as f32 * sp, -hz]);
                normals.extend_from_slice(&[0.0, 0.0, -1.0]);
            }
            for i in 0..seg {
                indices.extend_from_slice(&[cap_base, cap_base + 2 + i, cap_base + 1 + i]);
            }
        }
    }

    // Side caps for partial phi
    if !full_circle {
        // Start phi face
        add_cone_wedge_face(
            &mut positions, &mut normals, &mut indices,
            startphi, rmin1, rmax1, rmin2, rmax2, hz, has_hole, true,
        );
        // End phi face
        add_cone_wedge_face(
            &mut positions, &mut normals, &mut indices,
            startphi + deltaphi, rmin1, rmax1, rmin2, rmax2, hz, has_hole, false,
        );
    }

    TriangleMesh { positions, normals, indices }
}

#[allow(clippy::too_many_arguments)]
fn add_cone_wedge_face(
    positions: &mut Vec<f32>,
    normals: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    phi: f64,
    rmin1: f64,
    rmax1: f64,
    rmin2: f64,
    rmax2: f64,
    hz: f32,
    has_hole: bool,
    is_start: bool,
) {
    let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
    let (nx, ny) = if is_start { (-sp, cp) } else { (sp, -cp) };
    let base = positions.len() as u32 / 3;

    if has_hole {
        // Quad: inner-bottom, outer-bottom, outer-top, inner-top
        positions.extend_from_slice(&[rmin1 as f32 * cp, rmin1 as f32 * sp, -hz]);
        normals.extend_from_slice(&[nx, ny, 0.0]);
        positions.extend_from_slice(&[rmax1 as f32 * cp, rmax1 as f32 * sp, -hz]);
        normals.extend_from_slice(&[nx, ny, 0.0]);
        positions.extend_from_slice(&[rmax2 as f32 * cp, rmax2 as f32 * sp, hz]);
        normals.extend_from_slice(&[nx, ny, 0.0]);
        positions.extend_from_slice(&[rmin2 as f32 * cp, rmin2 as f32 * sp, hz]);
        normals.extend_from_slice(&[nx, ny, 0.0]);
        if is_start {
            indices.extend_from_slice(&[base, base + 2, base + 1]);
            indices.extend_from_slice(&[base, base + 3, base + 2]);
        } else {
            indices.extend_from_slice(&[base, base + 1, base + 2]);
            indices.extend_from_slice(&[base, base + 2, base + 3]);
        }
    } else {
        // Triangle: center, outer-bottom, outer-top
        positions.extend_from_slice(&[0.0, 0.0, -hz]);
        normals.extend_from_slice(&[nx, ny, 0.0]);
        positions.extend_from_slice(&[rmax1 as f32 * cp, rmax1 as f32 * sp, -hz]);
        normals.extend_from_slice(&[nx, ny, 0.0]);
        positions.extend_from_slice(&[rmax2 as f32 * cp, rmax2 as f32 * sp, hz]);
        normals.extend_from_slice(&[nx, ny, 0.0]);
        positions.extend_from_slice(&[0.0, 0.0, hz]);
        normals.extend_from_slice(&[nx, ny, 0.0]);
        if is_start {
            indices.extend_from_slice(&[base, base + 2, base + 1]);
            indices.extend_from_slice(&[base, base + 3, base + 2]);
        } else {
            indices.extend_from_slice(&[base, base + 1, base + 2]);
            indices.extend_from_slice(&[base, base + 2, base + 3]);
        }
    }
}
