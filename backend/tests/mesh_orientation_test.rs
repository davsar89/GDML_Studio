//! Mesh orientation tests: verify that primitive tessellations form closed,
//! consistently outward-oriented surfaces.
//!
//! Two complementary checks:
//!
//! 1. **Signed volume about a shifted reference point.** By the divergence
//!    theorem, the signed volume of a closed, consistently outward-wound mesh
//!    is independent of the reference point and equals the analytic volume.
//!    A face with inverted winding breaks that invariance: its (wrong-sign)
//!    flux is weighted by its distance to the reference point, so evaluating
//!    the sum about a point far from the solid makes the error visible even
//!    for faces whose plane passes through the origin (e.g. the phi-wedge
//!    faces of a sector, which contribute zero moment about the origin).
//!
//! 2. **Stored normals agree with winding.** Each triangle's geometric normal
//!    (from vertex order) must point the same way as its stored vertex
//!    normals, otherwise lighting and CSG classification disagree.

use std::f64::consts::PI;

use gdml_studio_backend::mesh::primitives::{
    arb8_mesh::tessellate_arb8, box_mesh::tessellate_box, cone_mesh::tessellate_cone,
    cut_tube_mesh::tessellate_cut_tube, elcone_mesh::tessellate_elcone,
    ellipsoid_mesh::tessellate_ellipsoid, eltube_mesh::tessellate_eltube,
    generic_polycone_mesh::tessellate_generic_polycone, hype_mesh::tessellate_hype,
    paraboloid_mesh::tessellate_paraboloid, polycone_mesh::tessellate_polycone,
    polyhedra_mesh::tessellate_polyhedra, sphere_mesh::tessellate_sphere,
    torus_mesh::tessellate_torus, trap_mesh::tessellate_trap, trd_mesh::tessellate_trd,
    tube_mesh::tessellate_tube, twisted_box_mesh::tessellate_twisted_box,
    twisted_trap_mesh::tessellate_twisted_trap, twisted_tubs_mesh::tessellate_twisted_tubs,
    xtru_mesh::tessellate_xtru,
};
use gdml_studio_backend::mesh::types::TriangleMesh;

/// Signed volume of the mesh about reference point `o` (tetrahedron sum).
fn signed_volume_about(mesh: &TriangleMesh, o: [f64; 3]) -> f64 {
    let v = |i: usize| -> [f64; 3] {
        [
            mesh.positions[i * 3] as f64 - o[0],
            mesh.positions[i * 3 + 1] as f64 - o[1],
            mesh.positions[i * 3 + 2] as f64 - o[2],
        ]
    };
    let mut vol = 0.0;
    for t in 0..mesh.triangle_count() {
        let a = v(mesh.indices[t * 3] as usize);
        let b = v(mesh.indices[t * 3 + 1] as usize);
        let c = v(mesh.indices[t * 3 + 2] as usize);
        // (a x b) . c / 6
        vol += (a[1] * b[2] - a[2] * b[1]) * c[0]
            + (a[2] * b[0] - a[0] * b[2]) * c[1]
            + (a[0] * b[1] - a[1] * b[0]) * c[2];
    }
    vol / 6.0
}

/// Assert the mesh volume matches `expected` (relative tolerance) about both
/// the origin and a far-away reference point. The second check is what
/// catches faces with inverted winding whose plane passes through the origin.
fn assert_volume(mesh: &TriangleMesh, expected: f64, rel_tol: f64, name: &str) {
    for o in [[0.0, 0.0, 0.0], [137.0, -89.0, 53.0]] {
        let vol = signed_volume_about(mesh, o);
        let rel_err = (vol - expected).abs() / expected.abs();
        assert!(
            rel_err < rel_tol,
            "{name}: signed volume about {o:?} = {vol:.4}, expected {expected:.4} \
             (rel err {rel_err:.4} > {rel_tol})"
        );
    }
}

/// Assert each triangle's stored vertex normals point the same way as its
/// geometric (winding) normal.
fn assert_normals_match_winding(mesh: &TriangleMesh, name: &str) {
    let p = |i: usize| -> [f64; 3] {
        [
            mesh.positions[i * 3] as f64,
            mesh.positions[i * 3 + 1] as f64,
            mesh.positions[i * 3 + 2] as f64,
        ]
    };
    let n = |i: usize| -> [f64; 3] {
        [
            mesh.normals[i * 3] as f64,
            mesh.normals[i * 3 + 1] as f64,
            mesh.normals[i * 3 + 2] as f64,
        ]
    };
    for t in 0..mesh.triangle_count() {
        let (i0, i1, i2) = (
            mesh.indices[t * 3] as usize,
            mesh.indices[t * 3 + 1] as usize,
            mesh.indices[t * 3 + 2] as usize,
        );
        let (a, b, c) = (p(i0), p(i1), p(i2));
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let geo = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let geo_len = (geo[0] * geo[0] + geo[1] * geo[1] + geo[2] * geo[2]).sqrt();
        if geo_len < 1e-9 {
            continue; // degenerate triangle
        }
        let (n0, n1, n2) = (n(i0), n(i1), n(i2));
        let avg = [
            n0[0] + n1[0] + n2[0],
            n0[1] + n1[1] + n2[1],
            n0[2] + n1[2] + n2[2],
        ];
        let avg_len = (avg[0] * avg[0] + avg[1] * avg[1] + avg[2] * avg[2]).sqrt();
        if avg_len < 1e-9 {
            continue;
        }
        let dot = geo[0] * avg[0] + geo[1] * avg[1] + geo[2] * avg[2];
        assert!(
            dot > 0.0,
            "{name}: triangle {t} stored normals oppose winding normal \
             (geo {geo:?}, stored avg {avg:?})"
        );
    }
}

/// Generic orientation check for solids without a convenient analytic volume:
/// the signed volume must be positive (outward winding) and agree between two
/// reference points (closed, consistently oriented surface), and stored
/// normals must agree with winding.
fn assert_closed_outward(mesh: &TriangleMesh, name: &str) {
    assert!(mesh.triangle_count() > 0, "{name}: empty mesh");
    let v1 = signed_volume_about(mesh, [0.0, 0.0, 0.0]);
    let v2 = signed_volume_about(mesh, [137.0, -89.0, 53.0]);
    assert!(
        v1 > 0.0,
        "{name}: signed volume {v1:.4} not positive (inverted winding?)"
    );
    let rel = (v1 - v2).abs() / v1.abs();
    assert!(
        rel < 0.01,
        "{name}: volume depends on reference point ({v1:.4} vs {v2:.4}) — \
         mesh is not consistently oriented/closed"
    );
    assert_normals_match_winding(mesh, name);
}

const SEG: u32 = 64;

#[test]
fn box_volume_sanity() {
    // Control: validates the helper itself. GDML box takes full lengths.
    let m = tessellate_box(10.0, 20.0, 30.0);
    assert_volume(&m, 6000.0, 1e-9, "box");
    assert_normals_match_winding(&m, "box");
}

#[test]
fn tube_full_circle() {
    let m = tessellate_tube(0.0, 10.0, 20.0, 0.0, 2.0 * PI, SEG);
    assert_volume(&m, PI * 100.0 * 20.0, 0.01, "tube full");
    assert_normals_match_winding(&m, "tube full");
}

#[test]
fn tube_quarter_solid() {
    let m = tessellate_tube(0.0, 10.0, 20.0, 0.0, PI / 2.0, SEG);
    assert_volume(&m, 0.5 * (PI / 2.0) * 100.0 * 20.0, 0.01, "tube quarter");
    assert_normals_match_winding(&m, "tube quarter");
}

#[test]
fn tube_quarter_hollow_offset_start() {
    // Non-zero startphi exercises wedge faces away from the coordinate axes.
    let m = tessellate_tube(5.0, 10.0, 20.0, 0.7, PI / 2.0, SEG);
    let expected = 0.5 * (PI / 2.0) * (100.0 - 25.0) * 20.0;
    assert_volume(&m, expected, 0.01, "tube quarter hollow");
    assert_normals_match_winding(&m, "tube quarter hollow");
}

#[test]
fn cone_quarter() {
    // Frustum sector: V = (h * dphi / 6) * (R1^2 + R1*R2 + R2^2)
    let m = tessellate_cone(0.0, 10.0, 0.0, 5.0, 20.0, 0.0, PI / 2.0, SEG);
    let expected = (20.0 * (PI / 2.0) / 6.0) * (100.0 + 50.0 + 25.0);
    assert_volume(&m, expected, 0.01, "cone quarter");
    assert_normals_match_winding(&m, "cone quarter");
}

#[test]
fn cone_quarter_hollow() {
    let m = tessellate_cone(2.0, 10.0, 2.0, 5.0, 20.0, 0.3, PI / 2.0, SEG);
    let outer = (20.0 * (PI / 2.0) / 6.0) * (100.0 + 50.0 + 25.0);
    let inner = (20.0 * (PI / 2.0) / 6.0) * (4.0 + 4.0 + 4.0);
    assert_volume(&m, outer - inner, 0.01, "cone quarter hollow");
    assert_normals_match_winding(&m, "cone quarter hollow");
}

#[test]
fn sphere_quarter_phi() {
    let m = tessellate_sphere(0.0, 10.0, 0.0, PI / 2.0, 0.0, PI, SEG);
    let expected = (4.0 / 3.0) * PI * 1000.0 / 4.0;
    assert_volume(&m, expected, 0.02, "sphere quarter");
    assert_normals_match_winding(&m, "sphere quarter");
}

#[test]
fn sphere_shell_segment() {
    // Hollow shell, partial phi AND partial theta.
    let m = tessellate_sphere(5.0, 10.0, 0.5, PI / 2.0, 0.4, PI / 3.0, SEG);
    // V = (dphi) * (r2^3 - r1^3)/3 * (cos(t0) - cos(t0+dt))
    let expected =
        (PI / 2.0) * (1000.0 - 125.0) / 3.0 * ((0.4f64).cos() - (0.4f64 + PI / 3.0).cos());
    assert_volume(&m, expected, 0.02, "sphere shell segment");
    assert_normals_match_winding(&m, "sphere shell segment");
}

/// Volume of a solid spherical sector: dphi * r^3/3 * (cos(t0) - cos(t0+dt)).
fn solid_sector_volume(r: f64, dphi: f64, t0: f64, dt: f64) -> f64 {
    dphi * r.powi(3) / 3.0 * (t0.cos() - (t0 + dt).cos())
}

// The theta cap of a solid sphere is a CONE with its apex at the origin. It was
// previously meshed as a flat disk at z = rmax*cos(theta), making the solid a
// spherical zone rather than a sector. The resulting mesh stays closed and
// consistently wound, so assert_closed_outward passes on it — only a volume
// check catches this. The error vanishes at exactly theta = PI/2 (the cut plane
// passes through the origin) and flips sign above it, so both a narrow and a
// wide cut are pinned here. rmin = 0 is essential: the hollow branch was always
// correct.

#[test]
fn sphere_solid_narrow_theta_cut() {
    // Previously -60% (the disk sat above the missing cone).
    let m = tessellate_sphere(0.0, 50.0, 0.0, 2.0 * PI, 0.0, PI / 4.0, SEG);
    let expected = solid_sector_volume(50.0, 2.0 * PI, 0.0, PI / 4.0);
    assert_volume(&m, expected, 0.02, "sphere solid narrow theta cut");
    assert_normals_match_winding(&m, "sphere solid narrow theta cut");
}

#[test]
fn sphere_solid_wide_theta_cut() {
    // Past PI/2 the sign flips: previously +12.5%.
    let m = tessellate_sphere(0.0, 50.0, 0.0, 2.0 * PI, 0.0, 2.0 * PI / 3.0, SEG);
    let expected = solid_sector_volume(50.0, 2.0 * PI, 0.0, 2.0 * PI / 3.0);
    assert_volume(&m, expected, 0.02, "sphere solid wide theta cut");
    assert_normals_match_winding(&m, "sphere solid wide theta cut");
}

#[test]
fn sphere_solid_theta_and_phi_cut() {
    // Exercises the phi wedge face too: with rmin = 0 the wedge is a sector with
    // its apex at the origin, not a strip against the z-axis. The two coincide
    // only when theta spans the full 0..PI, which every prior test did.
    let (t0, dt, dphi) = (PI / 6.0, PI / 2.0, PI / 2.0);
    let m = tessellate_sphere(0.0, 50.0, 0.0, dphi, t0, dt, SEG);
    let expected = solid_sector_volume(50.0, dphi, t0, dt);
    assert_volume(&m, expected, 0.02, "sphere solid theta+phi cut");
    assert_normals_match_winding(&m, "sphere solid theta+phi cut");
}

#[test]
fn polycone_quarter() {
    // Two z-planes forming a quarter tube.
    let planes = [(-10.0, 0.0, 10.0), (10.0, 0.0, 10.0)];
    let m = tessellate_polycone(&planes, 0.0, PI / 2.0, SEG);
    assert_volume(
        &m,
        0.5 * (PI / 2.0) * 100.0 * 20.0,
        0.01,
        "polycone quarter",
    );
    assert_normals_match_winding(&m, "polycone quarter");
}

#[test]
fn polycone_decreasing_z_is_not_inside_out() {
    // Decreasing-z plane lists are legal GDML -- G4Polycone::Create reverses the
    // (r,z) polygon when its signed area is negative. The mesher assumed
    // planes[0] was the bottom, so every triangle came out inverted (measured
    // -94238 against +94238). Double-sided rendering hides it, but any boolean
    // using the solid as an operand is then wrong.
    let up = [(-50.0, 10.0, 20.0), (50.0, 10.0, 20.0)];
    let down = [(50.0, 10.0, 20.0), (-50.0, 10.0, 20.0)];
    let expected = PI * (400.0 - 100.0) * 100.0;

    let m_up = tessellate_polycone(&up, 0.0, 2.0 * PI, SEG);
    let m_down = tessellate_polycone(&down, 0.0, 2.0 * PI, SEG);

    assert_volume(&m_up, expected, 0.01, "polycone increasing z");
    assert_volume(&m_down, expected, 0.01, "polycone decreasing z");
    assert_normals_match_winding(&m_down, "polycone decreasing z");
}

#[test]
fn zero_deltaphi_means_full_circle() {
    // G4Polycone::Create: `if ((phiTotal <= 0) || (phiTotal > twopi*(...)))`
    // means a complete revolution. A zero sweep previously gave a zero-volume
    // mesh instead of a full solid.
    let planes = [(-50.0, 0.0, 20.0), (50.0, 0.0, 20.0)];
    let expected = PI * 400.0 * 100.0;

    let m = tessellate_polycone(&planes, 0.0, 0.0, SEG);
    assert_volume(&m, expected, 0.01, "polycone deltaphi=0");

    let p = tessellate_polyhedra(&planes, 0.0, 0.0, 64);
    assert_volume(&p, expected, 0.02, "polyhedra deltaphi=0");
}

#[test]
fn generic_polycone_revolves_the_closing_edge() {
    // A hollow tube written as a closed (r,z) contour. Treating the rzpoints as
    // z-planes never revolves the last-to-first edge, so the bore is lost and
    // full disks are emitted at both ends: measured 33.3% too large.
    let hollow = [(10.0, -50.0), (20.0, -50.0), (20.0, 50.0), (10.0, 50.0)];
    let expected = PI * (400.0 - 100.0) * 100.0;
    let m = tessellate_generic_polycone(&hollow, 0.0, 2.0 * PI, SEG, None);
    assert_volume(&m, expected, 0.01, "generic polycone hollow tube");
    assert_normals_match_winding(&m, "generic polycone hollow tube");

    // Reversed contour winding is legal input -- G4Polycone::Create normalises
    // it by signed area.
    let cw: Vec<(f64, f64)> = hollow.iter().rev().copied().collect();
    let r = tessellate_generic_polycone(&cw, 0.0, 2.0 * PI, SEG, None);
    assert_volume(&r, expected, 0.01, "generic polycone reversed contour");
}

#[test]
fn generic_polycone_bicone_still_correct() {
    // The shape both shipped samples use: a bicone whose closing edge lies on
    // the z axis. This was already right under the old z-plane mapping, so it
    // guards against the fix regressing the common case.
    let bicone = [(0.0, 10.0), (5.0, 20.0), (0.0, 30.0)];
    // Two cones joined base to base, each h=10, r=5.
    let expected = 2.0 * PI * 25.0 * 10.0 / 3.0;
    let m = tessellate_generic_polycone(&bicone, 0.0, 2.0 * PI, SEG, None);
    assert_volume(&m, expected, 0.01, "generic polycone bicone");
}

#[test]
fn generic_polycone_partial_sweep_is_closed() {
    // Exercises the flat phi faces produced by ear-clipping the contour.
    let hollow = [(10.0, -50.0), (20.0, -50.0), (20.0, 50.0), (10.0, 50.0)];
    let expected = 0.25 * PI * (400.0 - 100.0) * 100.0;
    let m = tessellate_generic_polycone(&hollow, 0.3, PI / 2.0, SEG, None);
    assert_volume(&m, expected, 0.01, "generic polycone quarter sweep");
    assert_closed_outward(&m, "generic polycone quarter sweep");
}

#[test]
fn polyhedra_quarter() {
    // 4 flat sides over a quarter turn; rmax is the apothem.
    let planes = [(-10.0, 0.0, 10.0), (10.0, 0.0, 10.0)];
    let numsides = 4u32;
    let deltaphi = PI / 2.0;
    let m = tessellate_polyhedra(&planes, 0.0, deltaphi, numsides);
    let alpha = deltaphi / numsides as f64; // angle per side
    let r_vertex = 10.0 / (alpha / 2.0).cos();
    let area = numsides as f64 * 0.5 * r_vertex * r_vertex * alpha.sin();
    assert_volume(&m, area * 20.0, 0.01, "polyhedra quarter");
    assert_normals_match_winding(&m, "polyhedra quarter");
}

#[test]
fn torus_quarter() {
    let m = tessellate_torus(0.0, 3.0, 10.0, 0.0, PI / 2.0, SEG);
    // Pappus: V = (pi * r^2) * (R * dphi)
    let expected = PI * 9.0 * 10.0 * (PI / 2.0);
    assert_volume(&m, expected, 0.02, "torus quarter");
    assert_normals_match_winding(&m, "torus quarter");
}

#[test]
fn cut_tube_quarter_straight_cuts() {
    // Cut normals along -z/+z make it an ordinary tube segment.
    let m = tessellate_cut_tube(
        0.0,
        10.0,
        20.0,
        0.0,
        PI / 2.0,
        [0.0, 0.0, -1.0],
        [0.0, 0.0, 1.0],
        SEG,
    );
    assert_volume(
        &m,
        0.5 * (PI / 2.0) * 100.0 * 20.0,
        0.01,
        "cut_tube quarter",
    );
    assert_normals_match_winding(&m, "cut_tube quarter");
}

#[test]
fn twisted_tubs_quarter_no_twist() {
    let m = tessellate_twisted_tubs(0.0, 10.0, 20.0, PI / 2.0, 0.0, SEG);
    assert_volume(
        &m,
        0.5 * (PI / 2.0) * 100.0 * 20.0,
        0.01,
        "twisted_tubs quarter",
    );
    assert_normals_match_winding(&m, "twisted_tubs quarter");
}

#[test]
fn ellipsoid_full_and_cut() {
    let m = tessellate_ellipsoid(5.0, 8.0, 10.0, -10.0, 10.0, SEG);
    assert_volume(
        &m,
        (4.0 / 3.0) * PI * 5.0 * 8.0 * 10.0,
        0.02,
        "ellipsoid full",
    );
    assert_normals_match_winding(&m, "ellipsoid full");

    // Cut ellipsoid: V = pi*a*b * [(z2 - z1) - (z2^3 - z1^3)/(3 c^2)]
    let (z1, z2) = (-5.0_f64, 7.0_f64);
    let m = tessellate_ellipsoid(5.0, 8.0, 10.0, z1, z2, SEG);
    let expected = PI * 5.0 * 8.0 * ((z2 - z1) - (z2.powi(3) - z1.powi(3)) / (3.0 * 100.0));
    assert_volume(&m, expected, 0.02, "ellipsoid cut");
    assert_normals_match_winding(&m, "ellipsoid cut");
}

#[test]
fn eltube_volume() {
    // dx, dy are semi-axes; dz is the half-length.
    let m = tessellate_eltube(5.0, 8.0, 10.0, SEG);
    assert_volume(&m, PI * 5.0 * 8.0 * 20.0, 0.01, "eltube");
    assert_normals_match_winding(&m, "eltube");
}

#[test]
fn trd_volume() {
    // Full widths; prismatoid: V = h/6 * (A1 + 4*Am + A2)
    let m = tessellate_trd(10.0, 12.0, 6.0, 4.0, 20.0);
    let (a1, a2) = (10.0 * 12.0, 6.0 * 4.0);
    let am = ((10.0 + 6.0) / 2.0) * ((12.0 + 4.0) / 2.0);
    assert_volume(&m, 20.0 / 6.0 * (a1 + 4.0 * am + a2), 1e-6, "trd");
    assert_normals_match_winding(&m, "trd");
}

#[test]
fn twisted_box_volume() {
    // Twisting rotates each z-slice rigidly, preserving volume.
    let m = tessellate_twisted_box(0.6, 10.0, 20.0, 30.0, SEG);
    assert_volume(&m, 6000.0, 0.01, "twisted_box");
    assert_normals_match_winding(&m, "twisted_box");
}

#[test]
fn remaining_primitives_closed_and_outward() {
    assert_closed_outward(
        &tessellate_hype(3.0, 6.0, 0.2, 0.3, 20.0, SEG),
        "hype hollow",
    );
    assert_closed_outward(
        &tessellate_hype(0.0, 6.0, 0.0, 0.3, 20.0, SEG),
        "hype solid",
    );
    assert_closed_outward(&tessellate_paraboloid(2.0, 5.0, 10.0, SEG), "paraboloid");
    assert_closed_outward(&tessellate_elcone(0.5, 0.8, 10.0, 5.0, SEG), "elcone");
    assert_closed_outward(
        &tessellate_trap(20.0, 0.1, 0.2, 12.0, 10.0, 8.0, 0.05, 10.0, 9.0, 7.0, 0.05),
        "trap",
    );
    assert_closed_outward(
        &tessellate_twisted_trap(
            0.4, 20.0, 0.1, 0.2, 12.0, 10.0, 8.0, 10.0, 9.0, 7.0, 0.05, SEG,
        ),
        "twisted_trap",
    );
    // arb8 as a simple box: 4 bottom + 4 top vertices, dz is the half-length.
    // Clockwise XY order (the Geant4 G4GenericTrap convention)...
    let sq_cw = [
        [-5.0, -5.0],
        [-5.0, 5.0],
        [5.0, 5.0],
        [5.0, -5.0],
        [-5.0, -5.0],
        [-5.0, 5.0],
        [5.0, 5.0],
        [5.0, -5.0],
    ];
    assert_closed_outward(&tessellate_arb8(10.0, sq_cw), "arb8 box cw");
    // ...and counter-clockwise, which Geant4 silently accepts by reordering.
    let sq_ccw = [
        [-5.0, -5.0],
        [5.0, -5.0],
        [5.0, 5.0],
        [-5.0, 5.0],
        [-5.0, -5.0],
        [5.0, -5.0],
        [5.0, 5.0],
        [-5.0, 5.0],
    ];
    assert_closed_outward(&tessellate_arb8(10.0, sq_ccw), "arb8 box ccw");
    // xtru: square extruded between two sections.
    let verts = [(-5.0, -5.0), (5.0, -5.0), (5.0, 5.0), (-5.0, 5.0)];
    let sections = [(-10.0, 0.0, 0.0, 1.0), (10.0, 0.0, 0.0, 1.0)];
    assert_closed_outward(&tessellate_xtru(&verts, &sections), "xtru square");
}

#[test]
fn generic_polyhedra_radii_are_corner_radii_not_apothems() {
    // Geant4 has two G4Polyhedra constructors and the GDML reader picks a
    // different one per solid:
    //
    //   <polyhedra>        G4GDMLReadSolids.cc:1509 -> G4Polyhedra.cc:71
    //                      the GEANT3 z-plane form. Computes
    //                      convertRad = cos(0.5*phiTotal/numSide) and stores
    //                      rInner/convertRad, so its radii are APOTHEMS.
    //
    //   <genericPolyhedra> G4GDMLReadSolids.cc:1629 -> G4Polyhedra.cc:154
    //                      the (r,z) form. Hands r[] straight to
    //                      G4ReduciblePolygon and Create with NO scaling, so
    //                      its radii are already CORNER radii.
    //
    // Both were being treated as apothems, inflating every genericPolyhedra.
    let n = 6u32;
    let r = 10.0;
    let half_height = 5.0;

    // A closed (r,z) rectangle: a solid prism of corner radius r.
    let contour = [
        (0.0, -half_height),
        (r, -half_height),
        (r, half_height),
        (0.0, half_height),
    ];
    let m = tessellate_generic_polycone(&contour, 0.0, 2.0 * PI, SEG, Some(n));

    // Regular n-gon of CIRCUMradius r: area = (n/2) r^2 sin(2*pi/n).
    let area = 0.5 * n as f64 * r * r * (2.0 * PI / n as f64).sin();
    let expected = area * 2.0 * half_height;
    assert_volume(&m, expected, 0.01, "genericPolyhedra prism");
    assert_normals_match_winding(&m, "genericPolyhedra prism");

    // Guard the specific regression: treating r as an apothem scales every
    // radius by 1/cos(pi/n), which for n=6 is a 33.3% volume error.
    let inflated = expected / (PI / n as f64).cos().powi(2);
    assert!(
        (inflated - expected).abs() / expected > 0.3,
        "test would not distinguish the two conventions"
    );
}

#[test]
fn polyhedra_radii_are_still_apothems() {
    // The sibling convention, asserted here so the two cannot be conflated
    // again: <polyhedra> really does take inscribed-circle radii.
    let n = 6u32;
    let apothem = 10.0;
    let planes = [(-5.0, 0.0, apothem), (5.0, 0.0, apothem)];
    let m = tessellate_polyhedra(&planes, 0.0, 2.0 * PI, n);

    let r_vertex = apothem / (PI / n as f64).cos();
    let area = 0.5 * n as f64 * r_vertex * r_vertex * (2.0 * PI / n as f64).sin();
    assert_volume(&m, area * 10.0, 0.01, "polyhedra hexagonal prism");
}
