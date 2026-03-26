use crate::mesh::types::TriangleMesh;
use std::f64::consts::PI;

/// Generate a polycone mesh from a series of z-planes with rmin/rmax at each level.
/// Z-plane z values are absolute positions (not half-lengths).
/// startphi and deltaphi are in radians.
pub fn tessellate_polycone(
    planes: &[(f64, f64, f64)], // (z, rmin, rmax) per z-plane
    startphi: f64,
    deltaphi: f64,
    segments: u32,
) -> TriangleMesh {
    let seg = segments.max(3);
    let full_circle = (deltaphi - 2.0 * PI).abs() < 1e-6;
    let phi_step = deltaphi / seg as f64;

    let mut positions: Vec<f32> = Vec::new();
    let mut normals: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let n = planes.len();
    if n < 2 {
        return TriangleMesh {
            positions,
            normals,
            indices,
        };
    }

    // For each pair of adjacent z-planes, generate a frustum segment
    for p in 0..n - 1 {
        let (z1, rmin1, rmax1) = planes[p];
        let (z2, rmin2, rmax2) = planes[p + 1];
        let z1f = z1 as f32;
        let z2f = z2 as f32;
        let dz = z2 - z1;
        let has_hole = rmin1 > 1e-10 || rmin2 > 1e-10;

        // Outer surface
        let outer_base = positions.len() as u32 / 3;
        let dr_outer = rmax2 - rmax1;
        let slope_outer = if dz.abs() > 1e-10 {
            (dr_outer / dz) as f32
        } else {
            0.0
        };
        let nz_o = -slope_outer;
        let nr_o = 1.0_f32;
        let len_o = (nz_o * nz_o + nr_o * nr_o).sqrt();
        let nz_o = nz_o / len_o;
        let nr_o = nr_o / len_o;

        for i in 0..=seg {
            let phi = startphi + phi_step * i as f64;
            let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
            let r1 = rmax1 as f32;
            let r2 = rmax2 as f32;

            positions.extend_from_slice(&[r1 * cp, r1 * sp, z1f]);
            normals.extend_from_slice(&[nr_o * cp, nr_o * sp, nz_o]);
            positions.extend_from_slice(&[r2 * cp, r2 * sp, z2f]);
            normals.extend_from_slice(&[nr_o * cp, nr_o * sp, nz_o]);
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
            let slope_inner = if dz.abs() > 1e-10 {
                (dr_inner / dz) as f32
            } else {
                0.0
            };
            let nz_i = slope_inner;
            let nr_i = 1.0_f32;
            let len_i = (nz_i * nz_i + nr_i * nr_i).sqrt();
            let nz_i = nz_i / len_i;
            let nr_i = nr_i / len_i;

            for i in 0..=seg {
                let phi = startphi + phi_step * i as f64;
                let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
                let r1 = rmin1 as f32;
                let r2 = rmin2 as f32;

                positions.extend_from_slice(&[r1 * cp, r1 * sp, z1f]);
                normals.extend_from_slice(&[-nr_i * cp, -nr_i * sp, nz_i]);
                positions.extend_from_slice(&[r2 * cp, r2 * sp, z2f]);
                normals.extend_from_slice(&[-nr_i * cp, -nr_i * sp, nz_i]);
            }
            for i in 0..seg {
                let b = inner_base + i * 2;
                indices.extend_from_slice(&[b, b + 3, b + 2]);
                indices.extend_from_slice(&[b, b + 1, b + 3]);
            }
        }
    }

    // First z-plane cap (bottom)
    {
        let (z, rmin, rmax) = planes[0];
        let zf = z as f32;
        let has_hole = rmin > 1e-10;
        let cap_base = positions.len() as u32 / 3;

        if has_hole {
            for i in 0..=seg {
                let phi = startphi + phi_step * i as f64;
                let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
                positions.extend_from_slice(&[rmin as f32 * cp, rmin as f32 * sp, zf]);
                normals.extend_from_slice(&[0.0, 0.0, -1.0]);
                positions.extend_from_slice(&[rmax as f32 * cp, rmax as f32 * sp, zf]);
                normals.extend_from_slice(&[0.0, 0.0, -1.0]);
            }
            for i in 0..seg {
                let b = cap_base + i * 2;
                indices.extend_from_slice(&[b, b + 3, b + 1]);
                indices.extend_from_slice(&[b, b + 2, b + 3]);
            }
        } else if rmax > 1e-10 {
            positions.extend_from_slice(&[0.0, 0.0, zf]);
            normals.extend_from_slice(&[0.0, 0.0, -1.0]);
            for i in 0..=seg {
                let phi = startphi + phi_step * i as f64;
                let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
                positions.extend_from_slice(&[rmax as f32 * cp, rmax as f32 * sp, zf]);
                normals.extend_from_slice(&[0.0, 0.0, -1.0]);
            }
            for i in 0..seg {
                indices.extend_from_slice(&[cap_base, cap_base + 2 + i, cap_base + 1 + i]);
            }
        }
    }

    // Last z-plane cap (top)
    {
        let (z, rmin, rmax) = planes[n - 1];
        let zf = z as f32;
        let has_hole = rmin > 1e-10;
        let cap_base = positions.len() as u32 / 3;

        if has_hole {
            for i in 0..=seg {
                let phi = startphi + phi_step * i as f64;
                let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
                positions.extend_from_slice(&[rmin as f32 * cp, rmin as f32 * sp, zf]);
                normals.extend_from_slice(&[0.0, 0.0, 1.0]);
                positions.extend_from_slice(&[rmax as f32 * cp, rmax as f32 * sp, zf]);
                normals.extend_from_slice(&[0.0, 0.0, 1.0]);
            }
            for i in 0..seg {
                let b = cap_base + i * 2;
                indices.extend_from_slice(&[b, b + 1, b + 3]);
                indices.extend_from_slice(&[b, b + 3, b + 2]);
            }
        } else if rmax > 1e-10 {
            positions.extend_from_slice(&[0.0, 0.0, zf]);
            normals.extend_from_slice(&[0.0, 0.0, 1.0]);
            for i in 0..=seg {
                let phi = startphi + phi_step * i as f64;
                let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
                positions.extend_from_slice(&[rmax as f32 * cp, rmax as f32 * sp, zf]);
                normals.extend_from_slice(&[0.0, 0.0, 1.0]);
            }
            for i in 0..seg {
                indices.extend_from_slice(&[cap_base, cap_base + 1 + i, cap_base + 2 + i]);
            }
        }
    }

    // Phi wedge faces for partial arc
    if !full_circle {
        for &phi in &[startphi, startphi + deltaphi] {
            let is_start = (phi - startphi).abs() < 1e-10;
            let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
            let (nx, ny) = if is_start { (-sp, cp) } else { (sp, -cp) };

            // Build a strip of quads along z-planes at this phi angle
            for p in 0..n - 1 {
                let (z1, rmin1, rmax1) = planes[p];
                let (z2, rmin2, rmax2) = planes[p + 1];
                let z1f = z1 as f32;
                let z2f = z2 as f32;
                let has_hole = rmin1 > 1e-10 || rmin2 > 1e-10;
                let base = positions.len() as u32 / 3;

                if has_hole {
                    positions.extend_from_slice(&[rmin1 as f32 * cp, rmin1 as f32 * sp, z1f]);
                    normals.extend_from_slice(&[nx, ny, 0.0]);
                    positions.extend_from_slice(&[rmax1 as f32 * cp, rmax1 as f32 * sp, z1f]);
                    normals.extend_from_slice(&[nx, ny, 0.0]);
                    positions.extend_from_slice(&[rmax2 as f32 * cp, rmax2 as f32 * sp, z2f]);
                    normals.extend_from_slice(&[nx, ny, 0.0]);
                    positions.extend_from_slice(&[rmin2 as f32 * cp, rmin2 as f32 * sp, z2f]);
                    normals.extend_from_slice(&[nx, ny, 0.0]);
                    if is_start {
                        indices.extend_from_slice(&[base, base + 2, base + 1]);
                        indices.extend_from_slice(&[base, base + 3, base + 2]);
                    } else {
                        indices.extend_from_slice(&[base, base + 1, base + 2]);
                        indices.extend_from_slice(&[base, base + 2, base + 3]);
                    }
                } else {
                    positions.extend_from_slice(&[0.0, 0.0, z1f]);
                    normals.extend_from_slice(&[nx, ny, 0.0]);
                    positions.extend_from_slice(&[rmax1 as f32 * cp, rmax1 as f32 * sp, z1f]);
                    normals.extend_from_slice(&[nx, ny, 0.0]);
                    positions.extend_from_slice(&[rmax2 as f32 * cp, rmax2 as f32 * sp, z2f]);
                    normals.extend_from_slice(&[nx, ny, 0.0]);
                    positions.extend_from_slice(&[0.0, 0.0, z2f]);
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
        }
    }

    TriangleMesh {
        positions,
        normals,
        indices,
    }
}
