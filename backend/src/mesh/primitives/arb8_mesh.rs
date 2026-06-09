use crate::mesh::types::TriangleMesh;

/// Generate an arbitrary 8-vertex trapezoid (arb8/GenTrap) mesh.
/// `dz` is the half-length in Z. `vertices` are [x,y] pairs:
///   vertices[0..4] at z = -dz (bottom quad),
///   vertices[4..8] at z = +dz (top quad).
pub fn tessellate_arb8(dz: f64, vertices: [[f64; 2]; 8]) -> TriangleMesh {
    let hz = dz as f32;

    let v: [[f32; 3]; 8] = [
        [vertices[0][0] as f32, vertices[0][1] as f32, -hz],
        [vertices[1][0] as f32, vertices[1][1] as f32, -hz],
        [vertices[2][0] as f32, vertices[2][1] as f32, -hz],
        [vertices[3][0] as f32, vertices[3][1] as f32, -hz],
        [vertices[4][0] as f32, vertices[4][1] as f32, hz],
        [vertices[5][0] as f32, vertices[5][1] as f32, hz],
        [vertices[6][0] as f32, vertices[6][1] as f32, hz],
        [vertices[7][0] as f32, vertices[7][1] as f32, hz],
    ];

    let mut positions = Vec::with_capacity(24 * 3);
    let mut normals = Vec::with_capacity(24 * 3);
    let mut indices = Vec::with_capacity(12 * 3);

    fn face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
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

    // Geant4's G4GenericTrap expects vertices in clockwise XY order (and
    // silently reorders counter-clockwise input). Detect the input orientation
    // via the shoelace area of bottom+top quads so both conventions produce an
    // outward-wound mesh.
    let shoelace = |o: usize| -> f64 {
        (0..4).fold(0.0, |acc, i| {
            let (a, b) = (vertices[o + i], vertices[o + (i + 1) % 4]);
            acc + a[0] * b[1] - b[0] * a[1]
        })
    };
    let ccw = shoelace(0) + shoelace(4) > 0.0;

    // 6 faces wound CCW when viewed from outside (assuming CCW input order)
    let mut faces: [[[f32; 3]; 4]; 6] = [
        [v[3], v[2], v[1], v[0]], // Bottom (-Z)
        [v[4], v[5], v[6], v[7]], // Top (+Z)
        [v[0], v[1], v[5], v[4]], // Side: v1-v2-v6-v5
        [v[1], v[2], v[6], v[5]], // Side: v2-v3-v7-v6
        [v[2], v[3], v[7], v[6]], // Side: v3-v4-v8-v7
        [v[3], v[0], v[4], v[7]], // Side: v4-v1-v5-v8
    ];
    if !ccw {
        // Clockwise input (the Geant4 convention): mirror every face.
        for f in &mut faces {
            f.reverse();
        }
    }

    for (i, verts) in faces.iter().enumerate() {
        let n = face_normal(verts[0], verts[1], verts[2]);
        let base = (i * 4) as u32;
        for vert in verts {
            positions.extend_from_slice(vert);
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
