use super::emit_quad;
use crate::mesh::types::TriangleMesh;

/// A GDML box: full lengths, centred at the origin.
pub fn tessellate_box(x: f64, y: f64, z: f64) -> TriangleMesh {
    tessellate_trd(x, y, x, y, z)
}

/// A GDML trd: full lengths, x1/y1 at -z/2 and x2/y2 at +z/2.
pub fn tessellate_trd(x1: f64, y1: f64, x2: f64, y2: f64, z: f64) -> TriangleMesh {
    tessellate_trap(z, 0.0, 0.0, y1, x1, x1, 0.0, y2, x2, x2, 0.0)
}

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
    let dz = z / 2.0;
    // The face centres sit at -/+ dz*tan(theta)*(cos(phi), sin(phi)).
    let tan_theta = theta.tan();
    let x_shift = dz * tan_theta * phi.cos();
    let y_shift = dz * tan_theta * phi.sin();
    // One z face, counter-clockwise: half-height `dy`, half-widths `dx_lo` at
    // -y and `dx_hi` at +y, sheared by `alpha` about its centre (cx, cy).
    let face = |cx: f64, cy: f64, dy: f64, dx_lo: f64, dx_hi: f64, alpha: f64| {
        let shear = dy * alpha.tan();
        [
            [cx - shear - dx_lo, cy - dy],
            [cx - shear + dx_lo, cy - dy],
            [cx + shear + dx_hi, cy + dy],
            [cx + shear - dx_hi, cy + dy],
        ]
    };
    let [v0, v1, v2, v3] = face(-x_shift, -y_shift, y1 / 2.0, x1 / 2.0, x2 / 2.0, alpha1);
    let [v4, v5, v6, v7] = face(x_shift, y_shift, y2 / 2.0, x3 / 2.0, x4 / 2.0, alpha2);
    tessellate_arb8(dz, [v0, v1, v2, v3, v4, v5, v6, v7])
}

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
