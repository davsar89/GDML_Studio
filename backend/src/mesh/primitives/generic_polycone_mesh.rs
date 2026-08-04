use super::xtru_mesh::{ear_clip_triangulate, polygon_signed_area};
use crate::mesh::types::TriangleMesh;
use std::f64::consts::PI;

/// Tessellate `<genericPolycone>` / `<genericPolyhedra>` by revolving a closed
/// (r,z) contour about the z axis.
///
/// The `<rzpoint>` list is a **closed polygon** in the (r,z) half-plane, not a
/// list of z-planes. `G4Polycone`'s generic constructor builds a
/// `G4ReduciblePolygon(r, z, numRZ)` and then calls `Area()`,
/// `RemoveDuplicateVertices()` and `CrossesItself()` on it — all polygon
/// operations, none of which make sense for an ordered stack of planes.
///
/// Routing these solids through the z-plane mesher never revolved the closing
/// edge from the last point back to the first. For a contour that is a monotone
/// function of z with r=0 at both ends — which both shipped samples are — the
/// result happens to be correct to within 0.01%. For a contour with a bore or a
/// re-entrant profile it is not: the inner wall is dropped and full disks of the
/// inner radius are emitted at each end, plugging the hole. A hollow-tube
/// contour came out 33.3% too large.
///
/// `sides` selects the two GDML solids:
/// - `None` — smooth revolution in `segments` steps (`genericPolycone`).
/// - `Some(n)` — `n` flat facets (`genericPolyhedra`). As with `<polyhedra>`,
///   the radii are apothems and are converted to corner radii.
pub fn tessellate_generic_polycone(
    rz: &[(f64, f64)], // (r, z) contour points
    startphi: f64,
    deltaphi: f64,
    segments: u32,
    sides: Option<u32>,
) -> TriangleMesh {
    let mut positions: Vec<f32> = Vec::new();
    let mut normals: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let empty = |p, n, i| TriangleMesh {
        positions: p,
        normals: n,
        indices: i,
    };

    // Matches G4Polycone::Create — a non-positive or over-full sweep is a
    // complete revolution.
    let full_circle = deltaphi <= 0.0 || deltaphi >= 2.0 * PI - 1e-6;
    let deltaphi = if full_circle { 2.0 * PI } else { deltaphi };

    let steps = match sides {
        Some(n) => n.max(3),
        None => segments.max(3),
    };

    // Drop consecutive duplicates, including across the wrap, mirroring
    // G4ReduciblePolygon::RemoveDuplicateVertices.
    let mut pts: Vec<(f64, f64)> = Vec::with_capacity(rz.len());
    for &(r, z) in rz {
        if !r.is_finite() || !z.is_finite() {
            continue;
        }
        if let Some(&(pr, pz)) = pts.last() {
            if (r - pr).abs() < 1e-12 && (z - pz).abs() < 1e-12 {
                continue;
            }
        }
        pts.push((r.max(0.0), z));
    }
    while pts.len() >= 2 {
        let (fr, fz) = pts[0];
        let (lr, lz) = pts[pts.len() - 1];
        if (fr - lr).abs() < 1e-12 && (fz - lz).abs() < 1e-12 {
            pts.pop();
        } else {
            break;
        }
    }
    if pts.len() < 3 {
        return empty(positions, normals, indices);
    }

    // G4Polycone::Create reverses the contour when its signed area is negative,
    // so either winding is legal input.
    if polygon_signed_area(&pts) < 0.0 {
        pts.reverse();
    }

    // Apothem -> corner radius, the same convention `<polyhedra>` uses.
    let radius_scale = match sides {
        Some(n) => 1.0 / (deltaphi / (2.0 * n as f64)).cos(),
        None => 1.0,
    };

    let phi_at = |k: u32| startphi + deltaphi * (k as f64) / (steps as f64);
    let n_pts = pts.len();

    // ─── Bands of revolution, one per contour edge including the closing one ──
    for i in 0..n_pts {
        let j = (i + 1) % n_pts;
        let (ri, zi) = pts[i];
        let (rj, zj) = pts[j];
        let (ri, rj) = (ri * radius_scale, rj * radius_scale);

        // Outward normal in the (r,z) plane is the edge direction rotated by
        // -90 degrees, which points out of a counter-clockwise contour.
        let (dr, dz) = (rj - ri, zj - zi);
        let len = (dr * dr + dz * dz).sqrt();
        if len < 1e-12 {
            continue;
        }
        let (nr, nz) = (dz / len, -dr / len);

        let base = positions.len() as u32 / 3;
        for k in 0..=steps {
            let phi = phi_at(k);
            let (sp, cp) = phi.sin_cos();
            // Flat facets share one normal per side; a smooth revolution
            // interpolates per phi step.
            let (fnr_x, fnr_y) = match sides {
                Some(_) => {
                    let mid = phi_at(k.min(steps.saturating_sub(1))) + deltaphi / (2.0 * steps as f64);
                    (nr * mid.cos(), nr * mid.sin())
                }
                None => (nr * cp, nr * sp),
            };
            positions.extend_from_slice(&[(ri * cp) as f32, (ri * sp) as f32, zi as f32]);
            normals.extend_from_slice(&[fnr_x as f32, fnr_y as f32, nz as f32]);
            positions.extend_from_slice(&[(rj * cp) as f32, (rj * sp) as f32, zj as f32]);
            normals.extend_from_slice(&[fnr_x as f32, fnr_y as f32, nz as f32]);
        }

        for k in 0..steps {
            let a = base + k * 2; // point i at phi_k
            let b = a + 1; // point j at phi_k
            let d = a + 2; // point i at phi_k+1
            let c = a + 3; // point j at phi_k+1
            indices.extend_from_slice(&[a, d, c]);
            indices.extend_from_slice(&[a, c, b]);
        }
    }

    // ─── Flat faces closing a partial sweep ──────────────────────────────────
    if !full_circle {
        let contour: Vec<(f64, f64)> = pts.iter().map(|&(r, z)| (r * radius_scale, z)).collect();
        let tris = ear_clip_triangulate(&contour, true);

        for (is_start, phi) in [(true, startphi), (false, startphi + deltaphi)] {
            let (sp, cp) = phi.sin_cos();
            // -phi_hat on the start face, +phi_hat on the end face.
            let (nx, ny) = if is_start { (sp, -cp) } else { (-sp, cp) };

            let base = positions.len() as u32 / 3;
            for &(r, z) in &contour {
                positions.extend_from_slice(&[(r * cp) as f32, (r * sp) as f32, z as f32]);
                normals.extend_from_slice(&[nx as f32, ny as f32, 0.0]);
            }
            for t in &tris {
                // A counter-clockwise triangle in (r,z) maps to a face whose
                // normal is -phi_hat, which is what the start face wants; the
                // end face is the same triangle reversed.
                if is_start {
                    indices.extend_from_slice(&[
                        base + t[0] as u32,
                        base + t[1] as u32,
                        base + t[2] as u32,
                    ]);
                } else {
                    indices.extend_from_slice(&[
                        base + t[0] as u32,
                        base + t[2] as u32,
                        base + t[1] as u32,
                    ]);
                }
            }
        }
    }

    empty(positions, normals, indices)
}
