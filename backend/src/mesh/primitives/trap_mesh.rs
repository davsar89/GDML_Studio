use crate::mesh::types::TriangleMesh;

/// Tessellate a general trapezoid (G4Trap).
///
/// All length parameters are FULL dimensions (halved internally).
/// Angles are in radians.
///
/// - `z`: full length along z
/// - `theta`, `phi`: polar/azimuthal tilt of line connecting face centers
/// - `y1`: full y-length at -z face
/// - `x1`, `x2`: full x-lengths at -y and +y edges of -z face
/// - `alpha1`: shear angle at -z face
/// - `y2`: full y-length at +z face
/// - `x3`, `x4`: full x-lengths at -y and +y edges of +z face
/// - `alpha2`: shear angle at +z face
pub fn tessellate_trap(
    z: f64,
    theta: f64,
    phi: f64,
    y1: f64,
    x1: f64,
    x2: f64,
    alpha1: f64,
    y2: f64,
    x3: f64,
    x4: f64,
    alpha2: f64,
) -> TriangleMesh {
    // Convert to half-dimensions (Geant4 convention)
    let dz = z / 2.0;
    let dy1 = y1 / 2.0;
    let dx1 = x1 / 2.0;
    let dx2 = x2 / 2.0;
    let dy2 = y2 / 2.0;
    let dx3 = x3 / 2.0;
    let dx4 = x4 / 2.0;

    // Face center offsets from theta/phi tilt
    let tan_theta = theta.tan();
    let x_shift = dz * tan_theta * phi.cos();
    let y_shift = dz * tan_theta * phi.sin();

    let tan_a1 = alpha1.tan();
    let tan_a2 = alpha2.tan();

    // Bottom face (z = -dz): center at (-x_shift, -y_shift, -dz)
    let bx = -x_shift;
    let by = -y_shift;
    let v0 = [(bx - dy1 * tan_a1 - dx1) as f32, (by - dy1) as f32, (-dz) as f32];
    let v1 = [(bx - dy1 * tan_a1 + dx1) as f32, (by - dy1) as f32, (-dz) as f32];
    let v2 = [(bx + dy1 * tan_a1 + dx2) as f32, (by + dy1) as f32, (-dz) as f32];
    let v3 = [(bx + dy1 * tan_a1 - dx2) as f32, (by + dy1) as f32, (-dz) as f32];

    // Top face (z = +dz): center at (+x_shift, +y_shift, +dz)
    let tx = x_shift;
    let ty = y_shift;
    let v4 = [(tx - dy2 * tan_a2 - dx3) as f32, (ty - dy2) as f32, dz as f32];
    let v5 = [(tx - dy2 * tan_a2 + dx3) as f32, (ty - dy2) as f32, dz as f32];
    let v6 = [(tx + dy2 * tan_a2 + dx4) as f32, (ty + dy2) as f32, dz as f32];
    let v7 = [(tx + dy2 * tan_a2 - dx4) as f32, (ty + dy2) as f32, dz as f32];

    // Same face layout as trd_mesh
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
        [v4, v5, v6, v7], // +Z face (top)
        [v3, v2, v1, v0], // -Z face (bottom)
        [v0, v1, v5, v4], // -Y face (front)
        [v2, v3, v7, v6], // +Y face (back)
        [v3, v0, v4, v7], // -X face (left)
        [v1, v2, v6, v5], // +X face (right)
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
