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

    fn face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let nx = u[1] * v[2] - u[2] * v[1];
        let ny = u[2] * v[0] - u[0] * v[2];
        let nz = u[0] * v[1] - u[1] * v[0];
        let len = (nx * nx + ny * ny + nz * nz).sqrt();
        if len > 1e-10 {
            [nx / len, ny / len, nz / len]
        } else {
            [0.0, 0.0, 1.0]
        }
    }

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

    for (i, verts) in faces.iter().enumerate() {
        let n = face_normal(verts[0], verts[1], verts[2]);
        let base = (i * 4) as u32;
        for v in verts {
            positions.extend_from_slice(v);
            normals.extend_from_slice(&n);
        }
        indices.extend_from_slice(&[base, base + 1, base + 2]);
        indices.extend_from_slice(&[base, base + 2, base + 3]);
    }

    TriangleMesh {
        positions,
        normals,
        indices,
    }
}
