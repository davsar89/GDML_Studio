use super::emit_quad;
use crate::mesh::types::TriangleMesh;

/// Generate a twisted general trapezoid mesh; twistedbox and twistedtrd are
/// special cases of it, exactly as G4TwistedBox/G4TwistedTrd construct a
/// G4VTwistedFaceted.
/// All length parameters are FULL dimensions (halved internally).
/// All angles are in radians.
pub fn tessellate_twisted_trap(
    phi_twist: f64,
    z: f64,
    theta: f64,
    phi_angle: f64,
    y1: f64,
    x1: f64,
    x2: f64,
    y2: f64,
    x3: f64,
    x4: f64,
    alph: f64,
    segments: u32,
) -> TriangleMesh {
    let dz = z / 2.0;
    let dy1 = y1 / 2.0;
    let dx1 = x1 / 2.0;
    let dx2 = x2 / 2.0;
    let dy2 = y2 / 2.0;
    let dx3 = x3 / 2.0;
    let dx4 = x4 / 2.0;

    let tan_theta = theta.tan();
    let (shift_x, shift_y) = (tan_theta * phi_angle.cos(), tan_theta * phi_angle.sin());
    let tan_a = alph.tan();

    let z_segs = segments.max(3);
    let dz_step = z / z_segs as f64;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // The 4 corners of the slice at height z (G4TwistBoxSide::SurfacePoint):
    // the trapezoid interpolated in z, rotated by the twist phi = PhiTwist *
    // z / (2 dz), which is zero at mid-height, THEN shifted by fdeltaX * phi /
    // PhiTwist = z tan(theta) cos(phi_angle) (G4VTwistedFaceted.cc), unrotated.
    let corners_at = |z_pos: f64| -> [[f32; 3]; 4] {
        // Interpolation parameter: 0 at bottom, 1 at top
        let t = (z_pos + dz) / (2.0 * dz);

        // Interpolate half-dimensions
        let dy = dy1 + t * (dy2 - dy1);
        let dx_lo = dx1 + t * (dx3 - dx1); // x at -y edge
        let dx_hi = dx2 + t * (dx4 - dx2); // x at +y edge

        // 4 corners (as in arb8_mesh::tessellate_trap), about the slice centre
        let corners = [
            [-dy * tan_a - dx_lo, -dy],
            [-dy * tan_a + dx_lo, -dy],
            [dy * tan_a + dx_hi, dy],
            [dy * tan_a - dx_hi, dy],
        ];
        let (st, ct) = (phi_twist * (t - 0.5)).sin_cos();
        corners.map(|[x, y]| {
            [
                (x * ct - y * st + z_pos * shift_x) as f32,
                (x * st + y * ct + z_pos * shift_y) as f32,
                z_pos as f32,
            ]
        })
    };

    // 4 side faces, Z-segmented
    for j in 0..z_segs {
        let z0 = -dz + j as f64 * dz_step;
        let z1 = z0 + dz_step;
        let bot = corners_at(z0);
        let top = corners_at(z1);

        // Side 0 (-Y): bot[0]->bot[1] to top[0]->top[1]
        emit_quad(
            &mut positions,
            &mut normals,
            &mut indices,
            bot[0],
            bot[1],
            top[1],
            top[0],
            false,
        );
        // Side 1 (+X): bot[1]->bot[2] to top[1]->top[2]
        emit_quad(
            &mut positions,
            &mut normals,
            &mut indices,
            bot[1],
            bot[2],
            top[2],
            top[1],
            false,
        );
        // Side 2 (+Y): bot[2]->bot[3] to top[2]->top[3]
        emit_quad(
            &mut positions,
            &mut normals,
            &mut indices,
            bot[2],
            bot[3],
            top[3],
            top[2],
            false,
        );
        // Side 3 (-X): bot[3]->bot[0] to top[3]->top[0]
        emit_quad(
            &mut positions,
            &mut normals,
            &mut indices,
            bot[3],
            bot[0],
            top[0],
            top[3],
            false,
        );
    }

    // Top cap (z = +dz)
    {
        let c = corners_at(dz);
        emit_quad(
            &mut positions,
            &mut normals,
            &mut indices,
            c[0],
            c[1],
            c[2],
            c[3],
            false,
        );
    }

    // Bottom cap (z = -dz)
    {
        let c = corners_at(-dz);
        emit_quad(
            &mut positions,
            &mut normals,
            &mut indices,
            c[3],
            c[2],
            c[1],
            c[0],
            false,
        );
    }

    TriangleMesh {
        positions,
        normals,
        indices,
    }
}
