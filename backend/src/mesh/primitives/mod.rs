// Tessellators take one parameter per GDML attribute of the solid they build —
// a trap has 11, a twisted trap 12 — and the emit helpers thread the three
// output buffers plus a vertex quad. Grouping them into structs would only
// rename the arguments, so the lint is turned off for this module rather than
// silenced function by function.
#![allow(clippy::too_many_arguments)]

pub mod arb8_mesh;
pub mod box_mesh;
pub mod cone_mesh;
pub mod cut_tube_mesh;
pub mod elcone_mesh;
pub mod ellipsoid_mesh;
pub mod eltube_mesh;
pub mod generic_polycone_mesh;
pub mod hype_mesh;
pub mod paraboloid_mesh;
pub mod polycone_mesh;
pub mod polyhedra_mesh;
pub mod sphere_mesh;
pub mod torus_mesh;
pub mod trap_mesh;
pub mod trd_mesh;
pub mod tube_mesh;
pub mod twisted_box_mesh;
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
