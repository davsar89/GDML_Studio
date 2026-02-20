use crate::mesh::types::TriangleMesh;
use std::f64::consts::PI;

/// Generate a tube mesh (cylinder with optional inner hole and partial phi arc).
/// GDML tube z is FULL length (not half), rmin/rmax are radii.
/// startphi and deltaphi are in radians.
pub fn tessellate_tube(
    rmin: f64,
    rmax: f64,
    z: f64,
    startphi: f64,
    deltaphi: f64,
    segments: u32,
) -> TriangleMesh {
    let hz = (z / 2.0) as f32;
    let has_hole = rmin > 1e-10;
    let full_circle = (deltaphi - 2.0 * PI).abs() < 1e-6;

    let seg = segments.max(3);
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // Generate ring of points at top and bottom for outer and inner surfaces
    let phi_step = deltaphi / seg as f64;

    // Outer surface
    let outer_base = 0u32;
    for i in 0..=seg {
        let phi = startphi + phi_step * i as f64;
        let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
        let r = rmax as f32;

        // Bottom vertex
        positions.extend_from_slice(&[r * cp, r * sp, -hz]);
        normals.extend_from_slice(&[cp, sp, 0.0]);
        // Top vertex
        positions.extend_from_slice(&[r * cp, r * sp, hz]);
        normals.extend_from_slice(&[cp, sp, 0.0]);
    }
    for i in 0..seg {
        let b = outer_base + i * 2;
        indices.extend_from_slice(&[b, b + 2, b + 3]);
        indices.extend_from_slice(&[b, b + 3, b + 1]);
    }

    // Inner surface (if hollow)
    if has_hole {
        let inner_base = positions.len() as u32 / 3;
        for i in 0..=seg {
            let phi = startphi + phi_step * i as f64;
            let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
            let r = rmin as f32;

            // Bottom vertex
            positions.extend_from_slice(&[r * cp, r * sp, -hz]);
            normals.extend_from_slice(&[-cp, -sp, 0.0]);
            // Top vertex
            positions.extend_from_slice(&[r * cp, r * sp, hz]);
            normals.extend_from_slice(&[-cp, -sp, 0.0]);
        }
        for i in 0..seg {
            let b = inner_base + i * 2;
            // Wind opposite direction for inward-facing normals
            indices.extend_from_slice(&[b, b + 3, b + 2]);
            indices.extend_from_slice(&[b, b + 1, b + 3]);
        }
    }

    // Top cap (z = +hz)
    {
        let cap_base = positions.len() as u32 / 3;
        if has_hole {
            // Annular cap
            for i in 0..=seg {
                let phi = startphi + phi_step * i as f64;
                let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
                // Inner edge
                positions.extend_from_slice(&[rmin as f32 * cp, rmin as f32 * sp, hz]);
                normals.extend_from_slice(&[0.0, 0.0, 1.0]);
                // Outer edge
                positions.extend_from_slice(&[rmax as f32 * cp, rmax as f32 * sp, hz]);
                normals.extend_from_slice(&[0.0, 0.0, 1.0]);
            }
            for i in 0..seg {
                let b = cap_base + i * 2;
                indices.extend_from_slice(&[b, b + 1, b + 3]);
                indices.extend_from_slice(&[b, b + 3, b + 2]);
            }
        } else {
            // Solid disk cap with fan
            // Center vertex
            positions.extend_from_slice(&[0.0, 0.0, hz]);
            normals.extend_from_slice(&[0.0, 0.0, 1.0]);
            for i in 0..=seg {
                let phi = startphi + phi_step * i as f64;
                let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
                positions.extend_from_slice(&[rmax as f32 * cp, rmax as f32 * sp, hz]);
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
                // Inner edge
                positions.extend_from_slice(&[rmin as f32 * cp, rmin as f32 * sp, -hz]);
                normals.extend_from_slice(&[0.0, 0.0, -1.0]);
                // Outer edge
                positions.extend_from_slice(&[rmax as f32 * cp, rmax as f32 * sp, -hz]);
                normals.extend_from_slice(&[0.0, 0.0, -1.0]);
            }
            for i in 0..seg {
                let b = cap_base + i * 2;
                indices.extend_from_slice(&[b, b + 3, b + 1]);
                indices.extend_from_slice(&[b, b + 2, b + 3]);
            }
        } else {
            positions.extend_from_slice(&[0.0, 0.0, -hz]);
            normals.extend_from_slice(&[0.0, 0.0, -1.0]);
            for i in 0..=seg {
                let phi = startphi + phi_step * i as f64;
                let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
                positions.extend_from_slice(&[rmax as f32 * cp, rmax as f32 * sp, -hz]);
                normals.extend_from_slice(&[0.0, 0.0, -1.0]);
            }
            for i in 0..seg {
                indices.extend_from_slice(&[cap_base, cap_base + 2 + i, cap_base + 1 + i]);
            }
        }
    }

    // Side caps for partial phi (wedge faces)
    if !full_circle {
        // Start phi face
        {
            let phi = startphi;
            let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
            let nx = -sp;
            let ny = cp;
            let base = positions.len() as u32 / 3;

            if has_hole {
                positions.extend_from_slice(&[rmin as f32 * cp, rmin as f32 * sp, -hz]);
                normals.extend_from_slice(&[nx, ny, 0.0]);
                positions.extend_from_slice(&[rmax as f32 * cp, rmax as f32 * sp, -hz]);
                normals.extend_from_slice(&[nx, ny, 0.0]);
                positions.extend_from_slice(&[rmax as f32 * cp, rmax as f32 * sp, hz]);
                normals.extend_from_slice(&[nx, ny, 0.0]);
                positions.extend_from_slice(&[rmin as f32 * cp, rmin as f32 * sp, hz]);
                normals.extend_from_slice(&[nx, ny, 0.0]);
                indices.extend_from_slice(&[base, base + 2, base + 1]);
                indices.extend_from_slice(&[base, base + 3, base + 2]);
            } else {
                positions.extend_from_slice(&[0.0, 0.0, -hz]);
                normals.extend_from_slice(&[nx, ny, 0.0]);
                positions.extend_from_slice(&[rmax as f32 * cp, rmax as f32 * sp, -hz]);
                normals.extend_from_slice(&[nx, ny, 0.0]);
                positions.extend_from_slice(&[rmax as f32 * cp, rmax as f32 * sp, hz]);
                normals.extend_from_slice(&[nx, ny, 0.0]);
                positions.extend_from_slice(&[0.0, 0.0, hz]);
                normals.extend_from_slice(&[nx, ny, 0.0]);
                indices.extend_from_slice(&[base, base + 2, base + 1]);
                indices.extend_from_slice(&[base, base + 3, base + 2]);
            }
        }

        // End phi face
        {
            let phi = startphi + deltaphi;
            let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
            let nx = sp;
            let ny = -cp;
            let base = positions.len() as u32 / 3;

            if has_hole {
                positions.extend_from_slice(&[rmin as f32 * cp, rmin as f32 * sp, -hz]);
                normals.extend_from_slice(&[nx, ny, 0.0]);
                positions.extend_from_slice(&[rmax as f32 * cp, rmax as f32 * sp, -hz]);
                normals.extend_from_slice(&[nx, ny, 0.0]);
                positions.extend_from_slice(&[rmax as f32 * cp, rmax as f32 * sp, hz]);
                normals.extend_from_slice(&[nx, ny, 0.0]);
                positions.extend_from_slice(&[rmin as f32 * cp, rmin as f32 * sp, hz]);
                normals.extend_from_slice(&[nx, ny, 0.0]);
                indices.extend_from_slice(&[base, base + 1, base + 2]);
                indices.extend_from_slice(&[base, base + 2, base + 3]);
            } else {
                positions.extend_from_slice(&[0.0, 0.0, -hz]);
                normals.extend_from_slice(&[nx, ny, 0.0]);
                positions.extend_from_slice(&[rmax as f32 * cp, rmax as f32 * sp, -hz]);
                normals.extend_from_slice(&[nx, ny, 0.0]);
                positions.extend_from_slice(&[rmax as f32 * cp, rmax as f32 * sp, hz]);
                normals.extend_from_slice(&[nx, ny, 0.0]);
                positions.extend_from_slice(&[0.0, 0.0, hz]);
                normals.extend_from_slice(&[nx, ny, 0.0]);
                indices.extend_from_slice(&[base, base + 1, base + 2]);
                indices.extend_from_slice(&[base, base + 2, base + 3]);
            }
        }
    }

    TriangleMesh {
        positions,
        normals,
        indices,
    }
}
