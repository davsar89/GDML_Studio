use super::xtru_mesh::{ear_clip_triangulate, polygon_signed_area};
use super::{clean_polygon, face_normal};
use crate::mesh::types::TriangleMesh;
use std::f64::consts::PI;

/// `n` steps along the curve `f(t)`, t from 0 to 1, as outline points: the two
/// ends are corners, the samples between them lie on a smooth surface.
pub fn curve(n: u32, f: impl Fn(f64) -> (f64, f64)) -> impl Iterator<Item = (f64, f64, bool)> {
    (0..=n).map(move |k| {
        let (r, z) = f(k as f64 / n as f64);
        (r, z, k > 0 && k < n)
    })
}

/// Tessellate a solid of revolution: the closed (r,z) outline swept about the
/// z axis. Geant4 builds G4Polycone and G4Polyhedra this way (`Create` takes a
/// G4ReduciblePolygon), and every other revolved solid is one such outline.
///
/// Each point is `(r, z, smooth)`. `smooth` marks a sample inside a curve
/// (`curve`), where the two edges meeting there share the mean of their
/// normals, so a sphere or a hyperboloid shades round instead of banded; any
/// other point is a crease.
///
/// The outline is a **closed polygon** in the (r,z) half-plane, not a list of
/// z-planes: `G4Polycone`'s generic constructor calls `Area()`,
/// `RemoveDuplicateVertices()` and `CrossesItself()` on it. Either winding is
/// accepted, since `Create` reverses a clockwise one.
///
/// `sides` selects the surface:
/// - `None` — smooth revolution in `segments` steps.
/// - `Some(n)` — `n` flat facets whose corners sit at the given radii. The
///   radii are corner radii: `<genericPolyhedra>` hands them to G4Polyhedra
///   unscaled (`G4Polyhedra.cc:154`, from `G4GDMLReadSolids.cc:1629`), and the
///   `<polyhedra>` caller converts its apothems first.
pub fn tessellate_generic_polycone(
    outline: impl IntoIterator<Item = (f64, f64, bool)>,
    startphi: f64,
    deltaphi: f64,
    segments: u32,
    sides: Option<u32>,
) -> TriangleMesh {
    let mut m = TriangleMesh::default();

    // Matches G4Polycone::Create — a non-positive or over-full sweep is a
    // complete revolution.
    let full_circle = deltaphi <= 0.0 || deltaphi >= 2.0 * PI - 1e-6;
    let deltaphi = if full_circle { 2.0 * PI } else { deltaphi };
    let steps = sides.unwrap_or(segments).max(3);

    // Radii below the axis, or within kCarTolerance of it, go onto it; then
    // coincident and collinear points are dropped, including across the wrap,
    // as G4Polycone::Create does.
    let snapped: Vec<_> = outline
        .into_iter()
        .map(|(r, z, s)| (if r > 1e-9 { r } else { 0.0 }, z, s))
        .collect();
    let mut pts = clean_polygon(&snapped, |p| (p.0, p.1));
    if polygon_signed_area(&xy_of(&pts)) < 0.0 {
        pts.reverse();
    }
    let n_pts = pts.len();

    // Outward (r,z) normal of edge i -> i+1: the edge turned by -90 degrees,
    // out of the counter-clockwise outline. An edge on the axis sweeps no
    // surface and is skipped, as G4Polycone::Create skips it.
    let edge_n: Vec<Option<(f64, f64)>> = (0..n_pts)
        .map(|i| {
            let ((ri, zi, _), (rj, zj, _)) = (pts[i], pts[(i + 1) % n_pts]);
            let len = (rj - ri).hypot(zj - zi);
            (ri > 0.0 || rj > 0.0).then(|| ((zj - zi) / len, (ri - rj) / len))
        })
        .collect();
    // The shared normal at a smooth point, off the axis.
    let point_n: Vec<Option<(f64, f64)>> = (0..n_pts)
        .map(|i| {
            let (a, b) = (edge_n[(i + n_pts - 1) % n_pts]?, edge_n[i]?);
            let len = (a.0 + b.0).hypot(a.1 + b.1);
            (pts[i].2 && pts[i].0 > 0.0 && len > 1e-12)
                .then(|| ((a.0 + b.0) / len, (a.1 + b.1) / len))
        })
        .collect();

    let sc: Vec<(f64, f64)> = (0..=steps)
        .map(|k| (startphi + deltaphi * k as f64 / steps as f64).sin_cos())
        .collect();
    let at = |(r, z, _): (f64, f64, bool), (s, c): (f64, f64)| {
        [(r * c) as f32, (r * s) as f32, z as f32]
    };

    // ─── Bands of revolution, one per outline edge including the closing one ─
    // Corners of step k: a = (i, k), b = (j, k), c = (j, k+1), d = (i, k+1).
    // A band touching the axis is a fan: drop the triangle collapsed there.
    let quad = |indices: &mut Vec<u32>, [a, b, c, d]: [u32; 4], ri: f64, rj: f64| {
        if ri > 0.0 {
            indices.extend([a, d, c]);
        }
        if rj > 0.0 {
            indices.extend([a, c, b]);
        }
    };
    let mut shared: Vec<Option<(u32, u32)>> = vec![None; n_pts];
    for (i, en) in edge_n.iter().enumerate() {
        let Some(en) = *en else { continue };
        let j = (i + 1) % n_pts;
        let (ri, rj) = (pts[i].0, pts[j].0);
        if sides.is_some() {
            // Flat facets: each quad is planar and carries its own normal.
            for k in 0..steps as usize {
                let q = [(i, k), (j, k), (j, k + 1), (i, k + 1)].map(|(p, k)| at(pts[p], sc[k]));
                let d = |a: [f32; 3], b: [f32; 3]| [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let n = face_normal([0.0; 3], d(q[0], q[2]), d(q[3], q[1]));
                let base = m.positions.len() as u32 / 3;
                for p in q {
                    m.positions.extend(p);
                    m.normals.extend(n);
                }
                quad(&mut m.indices, [0, 1, 2, 3].map(|v| base + v), ri, rj);
            }
            continue;
        }
        // Smooth revolution: a ring of vertices per end, shared with the next
        // band where the outline is smooth. The centre of a flat face on the
        // axis has one normal at every phi, so it is one vertex (stride 0).
        let mut ring = |p: usize| {
            let mut push = |(nr, nz): (f64, f64)| {
                let one = pts[p].0 == 0.0 && nr == 0.0;
                let base = m.positions.len() as u32 / 3;
                for &(s, c) in &sc[..if one { 1 } else { sc.len() }] {
                    m.positions.extend(at(pts[p], (s, c)));
                    m.normals
                        .extend([(nr * c) as f32, (nr * s) as f32, nz as f32]);
                }
                (base, u32::from(!one))
            };
            match point_n[p] {
                Some(n) => *shared[p].get_or_insert_with(|| push(n)),
                None => push(en),
            }
        };
        let ((a, sa), (b, sb)) = (ring(i), ring(j));
        for k in 0..steps {
            let (a0, a1, b0, b1) = (a + sa * k, a + sa * (k + 1), b + sb * k, b + sb * (k + 1));
            quad(&mut m.indices, [a0, b0, b1, a1], ri, rj);
        }
    }

    // ─── Flat faces closing a partial sweep ──────────────────────────────────
    if !full_circle {
        // An outline that touches the axis mid-way (a polycone pinched to a
        // point) leaves a zero-area ear along the axis: drop it.
        let xy = xy_of(&pts);
        let mut tris = ear_clip_triangulate(&xy, true);
        tris.retain(|&[a, b, c]| {
            let (u, v) = (
                (xy[b].0 - xy[a].0, xy[b].1 - xy[a].1),
                (xy[c].0 - xy[a].0, xy[c].1 - xy[a].1),
            );
            (u.0 * v.1 - u.1 * v.0).abs() > 1e-12
        });
        for (is_start, k) in [(true, 0), (false, steps as usize)] {
            let (s, c) = sc[k];
            // -phi_hat on the start face, +phi_hat on the end face.
            let n = if is_start { [s, -c, 0.0] } else { [-s, c, 0.0] }.map(|v| v as f32);
            let base = m.positions.len() as u32 / 3;
            for &p in &pts {
                m.positions.extend(at(p, sc[k]));
                m.normals.extend(n);
            }
            // A counter-clockwise triangle in (r,z) faces -phi_hat, which is
            // what the start face wants; the end face is the same reversed.
            for &t in &tris {
                let t = t.map(|v| base + v as u32);
                m.indices
                    .extend(if is_start { t } else { [t[0], t[2], t[1]] });
            }
        }
    }
    m
}

fn xy_of(pts: &[(f64, f64, bool)]) -> Vec<(f64, f64)> {
    pts.iter().map(|p| (p.0, p.1)).collect()
}
