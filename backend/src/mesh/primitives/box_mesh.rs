use crate::mesh::types::TriangleMesh;

/// Generate a box mesh centered at origin.
/// GDML box x/y/z are FULL dimensions (not half), so we divide by 2.
pub fn tessellate_box(x: f64, y: f64, z: f64) -> TriangleMesh {
    let hx = (x / 2.0) as f32;
    let hy = (y / 2.0) as f32;
    let hz = (z / 2.0) as f32;

    // 6 faces, each face has 4 vertices (for correct normals), 2 triangles
    let mut positions = Vec::with_capacity(24 * 3);
    let mut normals = Vec::with_capacity(24 * 3);
    let mut indices = Vec::with_capacity(12 * 3);

    let faces: [([f32; 3], [f32; 3], [[f32; 3]; 4]); 6] = [
        // +Z face
        (
            [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
            [[-hx, -hy, hz], [hx, -hy, hz], [hx, hy, hz], [-hx, hy, hz]],
        ),
        // -Z face
        (
            [0.0, 0.0, -1.0], [0.0, 0.0, -1.0],
            [[-hx, hy, -hz], [hx, hy, -hz], [hx, -hy, -hz], [-hx, -hy, -hz]],
        ),
        // +X face
        (
            [1.0, 0.0, 0.0], [1.0, 0.0, 0.0],
            [[hx, -hy, -hz], [hx, hy, -hz], [hx, hy, hz], [hx, -hy, hz]],
        ),
        // -X face
        (
            [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0],
            [[-hx, -hy, hz], [-hx, hy, hz], [-hx, hy, -hz], [-hx, -hy, -hz]],
        ),
        // +Y face
        (
            [0.0, 1.0, 0.0], [0.0, 1.0, 0.0],
            [[-hx, hy, hz], [hx, hy, hz], [hx, hy, -hz], [-hx, hy, -hz]],
        ),
        // -Y face
        (
            [0.0, -1.0, 0.0], [0.0, -1.0, 0.0],
            [[-hx, -hy, -hz], [hx, -hy, -hz], [hx, -hy, hz], [-hx, -hy, hz]],
        ),
    ];

    for (i, (_n, normal, verts)) in faces.iter().enumerate() {
        let base = (i * 4) as u32;
        for v in verts {
            positions.extend_from_slice(v);
            normals.extend_from_slice(normal);
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
