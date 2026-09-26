use super::emit_quad;
use crate::mesh::types::TriangleMesh;

/// Generate a trapezoid (trd) mesh centered at origin.
/// GDML trd: x1/y1 are full lengths at -z/2, x2/y2 at +z/2, z is full length.
pub fn tessellate_trd(x1: f64, y1: f64, x2: f64, y2: f64, z: f64) -> TriangleMesh {
    let hx1 = (x1 / 2.0) as f32;
    let hy1 = (y1 / 2.0) as f32;
    let hx2 = (x2 / 2.0) as f32;
    let hy2 = (y2 / 2.0) as f32;
    let hz = (z / 2.0) as f32;

    // 8 vertices of the trapezoid
    // Bottom face (z = -hz)
    let v0 = [-hx1, -hy1, -hz];
    let v1 = [hx1, -hy1, -hz];
    let v2 = [hx1, hy1, -hz];
    let v3 = [-hx1, hy1, -hz];
    // Top face (z = +hz)
    let v4 = [-hx2, -hy2, hz];
    let v5 = [hx2, -hy2, hz];
    let v6 = [hx2, hy2, hz];
    let v7 = [-hx2, hy2, hz];

    let mut positions = Vec::with_capacity(24 * 3);
    let mut normals = Vec::with_capacity(24 * 3);
    let mut indices = Vec::with_capacity(12 * 3);

    // 6 faces wound CCW when viewed from outside
    let faces: [[[f32; 3]; 4]; 6] = [
        // +Z face (top)
        [v4, v5, v6, v7],
        // -Z face (bottom)
        [v3, v2, v1, v0],
        // -Y face (front)
        [v0, v1, v5, v4],
        // +Y face (back)
        [v2, v3, v7, v6],
        // -X face (left)
        [v3, v0, v4, v7],
        // +X face (right)
        [v1, v2, v6, v5],
    ];

    for f in &faces {
        emit_quad(
            &mut positions,
            &mut normals,
            &mut indices,
            f[0],
            f[1],
            f[2],
            f[3],
            false,
        );
    }

    TriangleMesh {
        positions,
        normals,
        indices,
    }
}
