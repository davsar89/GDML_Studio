// Tessellators take one parameter per GDML attribute of the solid they build —
// a trap has 11, a twisted trap 12 — and the emit helpers thread the three
// output buffers plus a vertex quad. Grouping them into structs would only
// rename the arguments, so the lint is turned off for this module rather than
// silenced function by function.
#![allow(clippy::too_many_arguments)]

pub mod arb8_mesh;
pub mod cut_tube_mesh;
pub mod generic_polycone_mesh;
pub mod torus_mesh;
pub mod twisted_trap_mesh;
pub mod twisted_tubs_mesh;
pub mod xtru_mesh;

/// Unit normal of triangle (a, b, c) by the right-hand rule; +z if degenerate.
pub(super) fn face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
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

/// Append quad p0-p1-p2-p3 as two flat-shaded triangles; `flip` reverses the winding.
/// The normal is that of the diagonals, (p2 - p0) x (p3 - p1), so a quad with a
/// collapsed edge (a triangle, like an arb8 wedge face) still gets its own.
pub(super) fn emit_quad(
    positions: &mut Vec<f32>,
    normals: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    p0: [f32; 3],
    p1: [f32; 3],
    p2: [f32; 3],
    p3: [f32; 3],
    flip: bool,
) {
    let d = |a: [f32; 3], b: [f32; 3]| [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let n = face_normal([0.0; 3], d(p0, p2), d(p1, p3));
    let n = if flip { n.map(|c| -c) } else { n };
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

/// Drop the coincident and collinear vertices of a closed polygon, across the
/// wrap too, in one pass as G4GeomTools::RemoveRedundantVertices does for
/// G4ExtrudedSolid (G4ExtrudedSolid.cc:119-135) and G4Polycone (l.202-203):
/// a vertex goes when its triangle with the neighbours kept so far has an
/// edge, or area / longest edge, within 2*kCarTolerance.
/// Non-finite points are dropped; fewer than 3 survivors mean no polygon.
/// `xy` reads a point's coordinates, so points can carry data along.
pub(super) fn clean_polygon<T: Copy>(pts: &[T], xy: impl Fn(T) -> (f64, f64)) -> Vec<T> {
    const TOL: f64 = 2e-9; // mm
    let pts: Vec<_> = pts
        .iter()
        .copied()
        .filter(|&p| xy(p).0.is_finite() && xy(p).1.is_finite())
        .collect();
    let mut kept: Vec<T> = Vec::with_capacity(pts.len());
    for (i, &p) in pts.iter().enumerate() {
        let cur = xy(p);
        let prev = xy(*kept.last().unwrap_or(&pts[pts.len() - 1]));
        let next = xy(pts.get(i + 1).or(kept.first()).copied().unwrap_or(p));
        let e1 = (prev.0 - cur.0, prev.1 - cur.1);
        let e2 = (next.0 - cur.0, next.1 - cur.1);
        let lengths = [e1, e2, (e2.0 - e1.0, e2.1 - e1.1)].map(|e| e.0.hypot(e.1));
        let longest = lengths.iter().copied().fold(0.0, f64::max);
        let area = 0.5 * (e1.0 * e2.1 - e1.1 * e2.0).abs();
        if lengths.iter().all(|&l| l > TOL) && area / longest > TOL {
            kept.push(p);
        }
    }
    if kept.len() < 3 {
        kept.clear();
    }
    kept
}
