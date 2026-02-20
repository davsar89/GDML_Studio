use crate::mesh::types::TriangleMesh;
use std::f64::consts::PI;

/// Generate a sphere mesh with optional theta/phi ranges and optional inner radius.
/// startphi/deltaphi control azimuthal range.
/// starttheta/deltatheta control polar range.
pub fn tessellate_sphere(
    rmin: f64,
    rmax: f64,
    startphi: f64,
    deltaphi: f64,
    starttheta: f64,
    deltatheta: f64,
    segments: u32,
) -> TriangleMesh {
    let seg = segments.max(4);
    let phi_seg = seg;
    let theta_seg = seg / 2;
    let has_hole = rmin > 1e-10;
    let full_phi = (deltaphi - 2.0 * PI).abs() < 1e-6;
    let full_theta = starttheta.abs() < 1e-6 && (deltatheta - PI).abs() < 1e-6;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // Outer surface
    add_sphere_surface(
        &mut positions, &mut normals, &mut indices,
        rmax, startphi, deltaphi, starttheta, deltatheta,
        phi_seg, theta_seg, false,
    );

    // Inner surface (if hollow)
    if has_hole {
        add_sphere_surface(
            &mut positions, &mut normals, &mut indices,
            rmin, startphi, deltaphi, starttheta, deltatheta,
            phi_seg, theta_seg, true,
        );
    }

    // Phi-cut wedge faces (when not a full circle in phi)
    if !full_phi {
        add_phi_wedge_face(
            &mut positions, &mut normals, &mut indices,
            startphi, rmin, rmax, starttheta, deltatheta,
            theta_seg, has_hole, true,
        );
        add_phi_wedge_face(
            &mut positions, &mut normals, &mut indices,
            startphi + deltaphi, rmin, rmax, starttheta, deltatheta,
            theta_seg, has_hole, false,
        );
    }

    // Theta-cut cap faces (when theta doesn't cover full 0..PI)
    if !full_theta {
        let theta_start_cut = starttheta.abs() > 1e-6;
        let theta_end_cut = ((starttheta + deltatheta) - PI).abs() > 1e-6;

        if theta_start_cut {
            add_theta_cap(
                &mut positions, &mut normals, &mut indices,
                starttheta, startphi, deltaphi,
                rmin, rmax, phi_seg, has_hole, true,
            );
        }
        if theta_end_cut {
            add_theta_cap(
                &mut positions, &mut normals, &mut indices,
                starttheta + deltatheta, startphi, deltaphi,
                rmin, rmax, phi_seg, has_hole, false,
            );
        }
    }

    TriangleMesh { positions, normals, indices }
}

fn add_sphere_surface(
    positions: &mut Vec<f32>,
    normals: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    r: f64,
    startphi: f64,
    deltaphi: f64,
    starttheta: f64,
    deltatheta: f64,
    phi_seg: u32,
    theta_seg: u32,
    invert: bool,
) {
    let base = positions.len() as u32 / 3;
    let r = r as f32;

    let phi_step = deltaphi / phi_seg as f64;
    let theta_step = deltatheta / theta_seg as f64;

    // Generate vertices
    for j in 0..=theta_seg {
        let theta = starttheta + theta_step * j as f64;
        let st = theta.sin() as f32;
        let ct = theta.cos() as f32;

        for i in 0..=phi_seg {
            let phi = startphi + phi_step * i as f64;
            let sp = phi.sin() as f32;
            let cp = phi.cos() as f32;

            let nx = st * cp;
            let ny = st * sp;
            let nz = ct;

            positions.extend_from_slice(&[r * nx, r * ny, r * nz]);
            if invert {
                normals.extend_from_slice(&[-nx, -ny, -nz]);
            } else {
                normals.extend_from_slice(&[nx, ny, nz]);
            }
        }
    }

    // Generate indices
    let cols = phi_seg + 1;
    for j in 0..theta_seg {
        for i in 0..phi_seg {
            let a = base + j * cols + i;
            let b = a + cols;
            let c = a + 1;
            let d = b + 1;

            if invert {
                indices.extend_from_slice(&[a, b, c]);
                indices.extend_from_slice(&[c, b, d]);
            } else {
                indices.extend_from_slice(&[a, c, b]);
                indices.extend_from_slice(&[c, d, b]);
            }
        }
    }
}

/// Add a radial wedge face at a given phi angle, connecting outer to inner surface
/// (or outer to the polar axis if solid). Sweeps along theta.
#[allow(clippy::too_many_arguments)]
fn add_phi_wedge_face(
    positions: &mut Vec<f32>,
    normals: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    phi: f64,
    rmin: f64,
    rmax: f64,
    starttheta: f64,
    deltatheta: f64,
    theta_seg: u32,
    has_hole: bool,
    is_start: bool,
) {
    let (sp, cp) = (phi.sin() as f32, phi.cos() as f32);
    // Normal is perpendicular to the radial plane at this phi
    let (nx, ny) = if is_start { (-sp, cp) } else { (sp, -cp) };

    let base = positions.len() as u32 / 3;
    let theta_step = deltatheta / theta_seg as f64;

    if has_hole {
        // Quad strip: pairs of (inner, outer) vertices along theta
        for j in 0..=theta_seg {
            let theta = starttheta + theta_step * j as f64;
            let st = theta.sin() as f32;
            let ct = theta.cos() as f32;

            // Inner vertex
            let ri = rmin as f32;
            positions.extend_from_slice(&[ri * st * cp, ri * st * sp, ri * ct]);
            normals.extend_from_slice(&[nx, ny, 0.0]);

            // Outer vertex
            let ro = rmax as f32;
            positions.extend_from_slice(&[ro * st * cp, ro * st * sp, ro * ct]);
            normals.extend_from_slice(&[nx, ny, 0.0]);
        }

        for j in 0..theta_seg {
            let b = base + j * 2;
            if is_start {
                indices.extend_from_slice(&[b, b + 1, b + 3]);
                indices.extend_from_slice(&[b, b + 3, b + 2]);
            } else {
                indices.extend_from_slice(&[b, b + 3, b + 1]);
                indices.extend_from_slice(&[b, b + 2, b + 3]);
            }
        }
    } else {
        // Triangle fan from polar axis to outer surface along theta.
        // For a solid sphere, the "inner edge" collapses to the z-axis (r=0),
        // which means points at (0, 0, r*cos(theta)). We use a strip with
        // center-axis vertices paired with outer vertices.
        for j in 0..=theta_seg {
            let theta = starttheta + theta_step * j as f64;
            let st = theta.sin() as f32;
            let ct = theta.cos() as f32;

            // Center axis vertex (r=0 -> point on z-axis)
            let ro = rmax as f32;
            positions.extend_from_slice(&[0.0, 0.0, ro * ct]);
            normals.extend_from_slice(&[nx, ny, 0.0]);

            // Outer vertex
            positions.extend_from_slice(&[ro * st * cp, ro * st * sp, ro * ct]);
            normals.extend_from_slice(&[nx, ny, 0.0]);
        }

        for j in 0..theta_seg {
            let b = base + j * 2;
            if is_start {
                indices.extend_from_slice(&[b, b + 1, b + 3]);
                indices.extend_from_slice(&[b, b + 3, b + 2]);
            } else {
                indices.extend_from_slice(&[b, b + 3, b + 1]);
                indices.extend_from_slice(&[b, b + 2, b + 3]);
            }
        }
    }
}

/// Add an annular or disk cap at a given theta angle.
/// This cap is a ring (or disk) in the plane at the given theta,
/// sweeping phi from startphi to startphi+deltaphi.
#[allow(clippy::too_many_arguments)]
fn add_theta_cap(
    positions: &mut Vec<f32>,
    normals: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    theta: f64,
    startphi: f64,
    deltaphi: f64,
    rmin: f64,
    rmax: f64,
    phi_seg: u32,
    has_hole: bool,
    is_start: bool,
) {
    let st = theta.sin();
    let ct = theta.cos();

    // The cap normal points along the theta direction (conical surface normal).
    // For theta cuts: at starttheta the outward normal points "up" (toward theta=0),
    // at endtheta the outward normal points "down" (toward theta=PI).
    // The normal to the cone at angle theta is (cos(theta)*cos(phi), cos(theta)*sin(phi), -sin(theta)).
    // For is_start (starttheta cap), outward is toward smaller theta, so we negate.
    let sign: f32 = if is_start { -1.0 } else { 1.0 };

    let base = positions.len() as u32 / 3;
    let phi_step = deltaphi / phi_seg as f64;

    if has_hole {
        // Annular cap: pairs of (inner, outer) vertices along phi
        for i in 0..=phi_seg {
            let phi = startphi + phi_step * i as f64;
            let sp = phi.sin() as f32;
            let cp = phi.cos() as f32;

            let nx = sign * ct as f32 * cp;
            let ny = sign * ct as f32 * sp;
            let nz = sign * -(st as f32);

            // Inner vertex
            let ri = rmin as f32;
            positions.extend_from_slice(&[
                ri * st as f32 * cp,
                ri * st as f32 * sp,
                ri * ct as f32,
            ]);
            normals.extend_from_slice(&[nx, ny, nz]);

            // Outer vertex
            let ro = rmax as f32;
            positions.extend_from_slice(&[
                ro * st as f32 * cp,
                ro * st as f32 * sp,
                ro * ct as f32,
            ]);
            normals.extend_from_slice(&[nx, ny, nz]);
        }

        for i in 0..phi_seg {
            let b = base + i * 2;
            if is_start {
                // Normal points inward (toward theta=0), winding flipped
                indices.extend_from_slice(&[b, b + 3, b + 1]);
                indices.extend_from_slice(&[b, b + 2, b + 3]);
            } else {
                indices.extend_from_slice(&[b, b + 1, b + 3]);
                indices.extend_from_slice(&[b, b + 3, b + 2]);
            }
        }
    } else {
        // Disk cap: fan from center (on z-axis) to outer edge
        let ro = rmax as f32;
        let center_nz = sign * -(st as f32);

        // Center vertex at the pole of the cone (on z-axis at this theta's z)
        positions.extend_from_slice(&[0.0, 0.0, ro * ct as f32]);
        normals.extend_from_slice(&[0.0, 0.0, center_nz]);

        for i in 0..=phi_seg {
            let phi = startphi + phi_step * i as f64;
            let sp = phi.sin() as f32;
            let cp = phi.cos() as f32;

            let nx = sign * ct as f32 * cp;
            let ny = sign * ct as f32 * sp;
            let nz = sign * -(st as f32);

            positions.extend_from_slice(&[
                ro * st as f32 * cp,
                ro * st as f32 * sp,
                ro * ct as f32,
            ]);
            normals.extend_from_slice(&[nx, ny, nz]);
        }

        for i in 0..phi_seg {
            if is_start {
                indices.extend_from_slice(&[base, base + 2 + i, base + 1 + i]);
            } else {
                indices.extend_from_slice(&[base, base + 1 + i, base + 2 + i]);
            }
        }
    }
}
