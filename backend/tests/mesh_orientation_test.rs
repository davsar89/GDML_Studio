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

use std::collections::HashMap;
use std::f64::consts::PI;

use gdml_studio_backend::mesh::primitives::{
    arb8_mesh::{tessellate_arb8, tessellate_box, tessellate_trap, tessellate_trd},
    cut_tube_mesh::tessellate_cut_tube,
    generic_polycone_mesh::tessellate_generic_polycone,
    torus_mesh::tessellate_torus,
    twisted_trap_mesh::tessellate_twisted_trap,
    twisted_tubs_mesh::tessellate_twisted_tubs,
    xtru_mesh::tessellate_xtru,
};
use gdml_studio_backend::mesh::types::TriangleMesh;

mod common;
use common::signed_volume_about;

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

/// An (r,z) contour whose every point is a corner.
fn corners(rz: &[(f64, f64)]) -> impl Iterator<Item = (f64, f64, bool)> + '_ {
    rz.iter().map(|&(r, z)| (r, z, false))
}

/// Solid `S` declared by `xml`, and the warnings.
fn solid(xml: &str) -> (Option<TriangleMesh>, Vec<String>) {
    let (mut meshes, warnings) = common::tessellate(xml, SEG);
    (meshes.remove("S"), warnings)
}

/// Once vertices within 1e-4 mm are welded, every edge bounds exactly two
/// triangles, once in each direction: a closed, consistently wound surface.
fn assert_closed(m: &TriangleMesh, name: &str) {
    const TOL: f64 = 1e-4;
    let mut cells: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
    let mut welded: Vec<[f64; 3]> = Vec::new();
    let id: Vec<usize> = m
        .positions
        .chunks_exact(3)
        .map(|p| {
            let p = [0, 1, 2].map(|k| p[k] as f64);
            let cell = p.map(|c| (c / TOL).round() as i64);
            let near = (0..27).find_map(|n: i64| {
                let key = [0, 1, 2].map(|k| cell[k] + n / 3_i64.pow(k as u32) % 3 - 1);
                let close = |&w: &usize| (0..3).all(|k| (welded[w][k] - p[k]).abs() <= TOL);
                cells.get(&key)?.iter().copied().find(close)
            });
            near.unwrap_or_else(|| {
                welded.push(p);
                cells.entry(cell).or_default().push(welded.len() - 1);
                welded.len() - 1
            })
        })
        .collect();
    let mut edges: HashMap<(usize, usize), u32> = HashMap::new();
    for t in m.indices.chunks_exact(3) {
        let v = [0, 1, 2].map(|k| id[t[k] as usize]);
        // A triangle collapsed onto an edge by the weld has no area.
        if v[0] != v[1] && v[1] != v[2] && v[2] != v[0] {
            for k in 0..3 {
                *edges.entry((v[k], v[(k + 1) % 3])).or_default() += 1;
            }
        }
    }
    let bad = edges
        .iter()
        .filter(|&(&(a, b), &n)| n != 1 || edges.get(&(b, a)) != Some(&1))
        .count();
    assert!(
        bad == 0,
        "{name}: {bad} of {} edges open or reused",
        edges.len()
    );
}

/// Every stored normal is a unit vector within 60 degrees of the winding
/// normal of each facet it shades, also where a wall collapses to a point.
fn assert_unit_normals(m: &TriangleMesh, name: &str) {
    let p = |i: u32| [0, 1, 2].map(|k| m.positions[i as usize * 3 + k] as f64);
    let n = |i: u32| [0, 1, 2].map(|k| m.normals[i as usize * 3 + k] as f64);
    for tri in m.indices.chunks_exact(3) {
        let (a, b, c) = (p(tri[0]), p(tri[1]), p(tri[2]));
        let (u, v) = (
            [0, 1, 2].map(|k| b[k] - a[k]),
            [0, 1, 2].map(|k| c[k] - a[k]),
        );
        let geo = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let area2 = geo.iter().map(|g| g * g).sum::<f64>().sqrt();
        for ni in tri.iter().map(|&i| n(i)) {
            let len = ni.iter().map(|c| c * c).sum::<f64>().sqrt();
            let cos = (0..3).map(|k| geo[k] * ni[k]).sum::<f64>() / area2;
            assert!(
                (len - 1.0).abs() < 1e-3 && (area2 < 1e-6 || cos > 0.5),
                "{name}: normal {ni:?} on facet {geo:?}"
            );
        }
    }
}

#[test]
fn revolved_and_elliptical_solids_match_geant4() {
    // Analytic volumes of the Geant4 solids. A revolved solid is dphi times
    // INT INT r dr dz over its (r,z) outline (Pappus); a polyhedra of n sides
    // over dphi is n sin(dphi/n) times it, at its corner radii.
    let moment = |p: &[(f64, f64)]| {
        let terms = (0..p.len()).map(|i| {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            (a.0 + b.0) * (a.0 * b.1 - b.0 * a.1)
        });
        (terms.sum::<f64>() / 6.0).abs()
    };
    let frustum = |h: f64, r1: f64, r2: f64| h * (r1 * r1 + r1 * r2 + r2 * r2) / 3.0;
    let sector = |r: f64, t0: f64, t1: f64| r.powi(3) / 3.0 * (t0.cos() - t1.cos());
    let d = PI / 180.0;
    // Inner radii from the bottom up, then outer radii back down.
    let steps = [
        (0.0, -15.0),
        (5.0, -10.0),
        (5.0, 0.0),
        (2.0, 0.0),
        (2.0, 10.0),
        (0.0, 15.0),
        (20.0, 10.0),
        (20.0, 0.0),
        (10.0, 0.0),
        (10.0, -10.0),
    ];
    let c5 = 1.0 / (270.0 * d / 10.0).cos(); // apothem -> corner, 5 sides over 270 deg
    let c6 = 1.0 / (PI / 6.0).cos();
    let hype = |r: f64, st: f64| 40.0 * r * r + 2.0 * 20f64.powi(3) / 3.0 * st.tan().powi(2);
    let ellipsoid = |z: f64| z - z.powi(3) / 300.0;
    let cases = [
        (
            "tube, hollow, 270 deg",
            r#"<tube name="S" rmin="5" rmax="10" z="20" startphi="40" deltaphi="270" aunit="deg"/>"#,
            135.0 * d * 75.0 * 20.0,
        ),
        (
            // G4Polycone.cc:232's rule, a zero opening angle is a full turn;
            // G4Tubs itself refuses it (see `resolve_delta_phi`).
            "tube, deltaphi 0",
            r#"<tube name="S" rmax="10" z="20" deltaphi="0"/>"#,
            PI * 100.0 * 20.0,
        ),
        (
            "cone, hollow quarter",
            r#"<cone name="S" rmin1="2" rmax1="10" rmin2="2" rmax2="5" z="20" startphi="17" deltaphi="90" aunit="deg"/>"#,
            45.0 * d * (frustum(20.0, 10.0, 5.0) - frustum(20.0, 2.0, 2.0)),
        ),
        (
            // G4Cons::Inside keeps rmin(z) <= r <= rmax(z): of walls that
            // cross, only the part where rmin < rmax is solid. Was -7%, with
            // the rest inside out.
            "cone, crossing walls, quarter",
            r#"<cone name="S" rmin1="8" rmax1="6" rmin2="2" rmax2="10" z="20" deltaphi="90" aunit="deg"/>"#,
            45.0 * d * (frustum(16.0, 6.8, 10.0) - frustum(16.0, 6.8, 2.0)),
        ),
        (
            "cone, apex, quarter",
            r#"<cone name="S" rmax1="10" rmax2="0" z="20" deltaphi="90" aunit="deg"/>"#,
            45.0 * d * frustum(20.0, 10.0, 0.0),
        ),
        (
            "sphere, shell cut in phi and theta",
            r#"<sphere name="S" rmin="5" rmax="10" startphi="30" deltaphi="90" starttheta="23" deltatheta="60" aunit="deg"/>"#,
            90.0 * d * (sector(10.0, 23.0 * d, 83.0 * d) - sector(5.0, 23.0 * d, 83.0 * d)),
        ),
        (
            // A solid sphere's theta cut is a cone to the origin: a flat disk
            // made it a zone, -60% here and +12.5% at 120 deg.
            "sphere, narrow theta cut",
            r#"<sphere name="S" rmax="50" deltatheta="45" aunit="deg"/>"#,
            2.0 * PI * sector(50.0, 0.0, 45.0 * d),
        ),
        (
            "sphere, wide theta cut, 270 deg",
            r#"<sphere name="S" rmax="50" deltaphi="270" deltatheta="120" aunit="deg"/>"#,
            270.0 * d * sector(50.0, 0.0, 120.0 * d),
        ),
        (
            "sphere, theta and phi cut",
            r#"<sphere name="S" rmax="50" deltaphi="90" starttheta="30" deltatheta="90" aunit="deg"/>"#,
            90.0 * d * sector(50.0, 30.0 * d, 120.0 * d),
        ),
        ("orb", r#"<orb name="S" r="10"/>"#, 4.0 / 3.0 * PI * 1000.0),
        (
            // Decreasing z is legal (G4Polycone::Create reverses the outline);
            // the dz = 0 steps are flat annuli, the ends close on the axis.
            "polycone, decreasing z with steps",
            r#"<polycone name="S" deltaphi="2"><zplane z="15" rmax="0"/><zplane z="10" rmin="2" rmax="20"/><zplane z="0" rmin="2" rmax="20"/><zplane z="0" rmin="5" rmax="10"/><zplane z="-10" rmin="5" rmax="10"/><zplane z="-15" rmax="0"/></polycone>"#,
            2.0 * moment(&steps),
        ),
        (
            "polyhedra, apothems, hollow, 270 deg",
            r#"<polyhedra name="S" numsides="5" startphi="20" deltaphi="270" aunit="deg"><zplane z="-10" rmin="4" rmax="10"/><zplane z="10" rmin="6" rmax="12"/></polyhedra>"#,
            5.0 * (54.0 * d).sin()
                * moment(&[(4.0, -10.0), (10.0, -10.0), (12.0, 10.0), (6.0, 10.0)])
                * c5
                * c5,
        ),
        (
            "polyhedra, steps closing on the axis",
            r#"<polyhedra name="S" numsides="5" deltaphi="2"><zplane z="-15" rmax="0"/><zplane z="-10" rmin="5" rmax="10"/><zplane z="0" rmin="5" rmax="10"/><zplane z="0" rmin="2" rmax="20"/><zplane z="10" rmin="2" rmax="20"/><zplane z="15" rmax="0"/></polyhedra>"#,
            5.0 * 0.4f64.sin() * moment(&steps) / 0.2f64.cos().powi(2),
        ),
        (
            "polyhedra, hexagonal prism, deltaphi 0",
            r#"<polyhedra name="S" numsides="6" deltaphi="0"><zplane z="-5" rmax="10"/><zplane z="5" rmax="10"/></polyhedra>"#,
            3.0 * (10.0 * c6).powi(2) * (60.0 * d).sin() * 10.0,
        ),
        (
            // A contour, not z-planes: z-plane meshing lost the bore (+33%).
            "genericPolycone, clockwise hollow tube, quarter",
            r#"<genericPolycone name="S" startphi="17" deltaphi="90" aunit="deg"><rzpoint r="10" z="50"/><rzpoint r="20" z="50"/><rzpoint r="20" z="-50"/><rzpoint r="10" z="-50"/></genericPolycone>"#,
            45.0 * d * 300.0 * 100.0,
        ),
        (
            "genericPolycone, bicone",
            r#"<genericPolycone name="S"><rzpoint r="0" z="10"/><rzpoint r="5" z="20"/><rzpoint r="0" z="30"/></genericPolycone>"#,
            2.0 * PI * frustum(10.0, 5.0, 0.0),
        ),
        (
            // Corner radii, unlike <polyhedra> (G4Polyhedra.cc:154): +33% as apothems.
            "genericPolyhedra, hexagonal prism",
            r#"<genericPolyhedra name="S" numsides="6"><rzpoint r="0" z="-5"/><rzpoint r="10" z="-5"/><rzpoint r="10" z="5"/><rzpoint r="0" z="5"/></genericPolyhedra>"#,
            3.0 * 100.0 * (60.0 * d).sin() * 10.0,
        ),
        (
            "hype, hollow",
            r#"<hype name="S" rmin="3" rmax="6" inst="0.2" outst="0.3" z="40"/>"#,
            PI * (hype(6.0, 0.3) - hype(3.0, 0.2)),
        ),
        (
            "hype, solid",
            r#"<hype name="S" rmax="6" outst="0.5" z="40"/>"#,
            PI * hype(6.0, 0.5),
        ),
        (
            // With rmin = 0 but inst != 0 Geant4 keeps the inner surface, a
            // double cone (G4Hype::InnerSurfaceExists). Was 14% too large.
            "hype, inner stereo without inner radius",
            r#"<hype name="S" rmax="6" inst="0.3" outst="0.6" z="40"/>"#,
            PI * (hype(6.0, 0.6) - hype(0.0, 0.3)),
        ),
        (
            // The apex band's normals pointed inward.
            "paraboloid, apex",
            r#"<paraboloid name="S" rlo="0" rhi="5" dz="10"/>"#,
            PI * 25.0 * 10.0,
        ),
        (
            "paraboloid",
            r#"<paraboloid name="S" rlo="2" rhi="5" dz="10"/>"#,
            PI * (4.0 + 25.0) * 10.0,
        ),
        (
            "eltube",
            r#"<eltube name="S" dx="5" dy="8" dz="10"/>"#,
            PI * 40.0 * 20.0,
        ),
        (
            // dx, dy are slopes: the section at z has semi-axes dx (zmax - z).
            "elcone",
            r#"<elcone name="S" dx="0.5" dy="0.8" zmax="10" zcut="5"/>"#,
            PI * 0.4 * (15f64.powi(3) - 5f64.powi(3)) / 3.0,
        ),
        (
            "ellipsoid",
            r#"<ellipsoid name="S" ax="5" by="8" cz="10"/>"#,
            PI * 40.0 * (ellipsoid(10.0) - ellipsoid(-10.0)),
        ),
        (
            "ellipsoid, cut",
            r#"<ellipsoid name="S" ax="5" by="8" cz="10" zcut1="-5" zcut2="7"/>"#,
            PI * 40.0 * (ellipsoid(7.0) - ellipsoid(-5.0)),
        ),
    ];
    for (name, xml, volume) in cases {
        let (m, warnings) = solid(xml);
        assert!(warnings.is_empty(), "{name}: {warnings:?}");
        let m = m.unwrap();
        assert_volume(&m, volume, 0.01, name);
        assert_closed(&m, name);
        assert_unit_normals(&m, name);
    }
}

#[test]
fn a_curve_ending_on_the_axis_shades_smooth_there() {
    // A sphere, ellipsoid or paraboloid crosses the axis square, so at its
    // pole the one normal is along the axis; the first chord's normal there
    // drew a pinched star (26 deg at a paraboloid apex). A cone's apex, and
    // hype's double cone at z = 0, are true points and keep their edge normals.
    for (xml, z, axial) in [
        (r#"<orb name="S" r="10"/>"#, 10.0, true),
        (r#"<orb name="S" r="10"/>"#, -10.0, true),
        (
            r#"<ellipsoid name="S" ax="5" by="8" cz="10"/>"#,
            -10.0,
            true,
        ),
        (
            r#"<paraboloid name="S" rlo="0" rhi="7" dz="3"/>"#,
            -3.0,
            true,
        ),
        (
            r#"<sphere name="S" rmax="50" deltatheta="45" aunit="deg"/>"#,
            50.0,
            true,
        ),
        (
            r#"<sphere name="S" rmax="50" deltatheta="45" aunit="deg"/>"#,
            0.0,
            false,
        ),
        (
            r#"<cone name="S" rmax1="10" rmax2="0" z="20"/>"#,
            10.0,
            false,
        ),
        (
            r#"<hype name="S" rmax="6" inst="0.3" outst="0.6" z="40"/>"#,
            0.0,
            false,
        ),
    ] {
        let m = solid(xml).0.unwrap();
        let on_axis: Vec<bool> = (0..m.positions.len() / 3)
            .filter(|&i| {
                let p = &m.positions[i * 3..i * 3 + 3];
                p[0].hypot(p[1]) < 1e-6 && (p[2] as f64 - z).abs() < 1e-4
            })
            .map(|i| m.normals[i * 3 + 2].abs() > 0.9999)
            .collect();
        assert!(
            !on_axis.is_empty() && on_axis.iter().all(|&a| a == axial),
            "{xml} at z = {z}: normal along the axis {on_axis:?}, want {axial}"
        );
    }
}

#[test]
fn parameters_geant4_rejects_are_reported() {
    for xml in [
        // G4Tubs: rmin >= rmax.
        r#"<tube name="S" rmin="10" rmax="5" z="20"/>"#,
        // G4Sphere: rmin >= rmax.
        r#"<sphere name="S" rmin="10" rmax="5"/>"#,
        // G4Hype: rmin >= rmax. G4Paraboloid: rhi <= rlo.
        r#"<hype name="S" rmin="6" rmax="3" z="20"/>"#,
        r#"<paraboloid name="S" rlo="5" rhi="2" dz="10"/>"#,
        // G4Polycone.cc:81: rInner > rOuter at a plane.
        r#"<polycone name="S"><zplane z="-5" rmin="12" rmax="10"/><zplane z="5" rmax="10"/></polycone>"#,
    ] {
        let (m, warnings) = solid(xml);
        assert!(m.is_none() && warnings.len() == 1, "{xml}: {warnings:?}");
    }
}

#[test]
fn box_volume_sanity() {
    // Control: validates the helper itself. GDML box takes full lengths.
    let m = tessellate_box(10.0, 20.0, 30.0);
    assert_volume(&m, 6000.0, 1e-9, "box");
    assert_normals_match_winding(&m, "box");
}

#[test]
fn generic_polycone_revolves_the_closing_edge() {
    // A hollow tube written as a closed (r,z) contour. Treating the rzpoints as
    // z-planes never revolves the last-to-first edge, so the bore is lost and
    // full disks are emitted at both ends: measured 33.3% too large.
    let hollow = [(10.0, -50.0), (20.0, -50.0), (20.0, 50.0), (10.0, 50.0)];
    let expected = PI * (400.0 - 100.0) * 100.0;
    let m = tessellate_generic_polycone(corners(&hollow), 0.0, 2.0 * PI, SEG, None);
    assert_volume(&m, expected, 0.01, "generic polycone hollow tube");
    assert_normals_match_winding(&m, "generic polycone hollow tube");

    // Reversed contour winding is legal input -- G4Polycone::Create normalises
    // it by signed area.
    let cw: Vec<(f64, f64)> = hollow.iter().rev().copied().collect();
    let r = tessellate_generic_polycone(corners(&cw), 0.0, 2.0 * PI, SEG, None);
    assert_volume(&r, expected, 0.01, "generic polycone reversed contour");
}

#[test]
fn generic_polycone_bicone_still_correct() {
    // The shape both generic solids in solids.gdml use: a bicone whose
    // closing edge lies on the z axis. This was already right under the old z-plane mapping, so it
    // guards against the fix regressing the common case.
    let bicone = [(0.0, 10.0), (5.0, 20.0), (0.0, 30.0)];
    // Two cones joined base to base, each h=10, r=5.
    let expected = 2.0 * PI * 25.0 * 10.0 / 3.0;
    let m = tessellate_generic_polycone(corners(&bicone), 0.0, 2.0 * PI, SEG, None);
    assert_volume(&m, expected, 0.01, "generic polycone bicone");
}

#[test]
fn generic_polycone_partial_sweep_is_closed() {
    // Exercises the flat phi faces produced by ear-clipping the contour.
    let hollow = [(10.0, -50.0), (20.0, -50.0), (20.0, 50.0), (10.0, 50.0)];
    let expected = 0.25 * PI * (400.0 - 100.0) * 100.0;
    let m = tessellate_generic_polycone(corners(&hollow), 0.3, PI / 2.0, SEG, None);
    assert_volume(&m, expected, 0.01, "generic polycone quarter sweep");
    assert_closed_outward(&m, "generic polycone quarter sweep");
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
    // Zero twist collapses the hyperboloid to a plain tube: the exact limit,
    // since tanStereo is proportional to tan(twist/2).
    let m = tessellate_twisted_tubs(0.0, 10.0, -10.0, 10.0, PI / 2.0, 0.0, SEG);
    assert_volume(
        &m,
        0.5 * (PI / 2.0) * 100.0 * 20.0,
        0.01,
        "twisted_tubs quarter",
    );
    assert_normals_match_winding(&m, "twisted_tubs quarter");
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
    let m = tessellate_twisted_trap(
        0.6, 30.0, 0.0, 0.0, 20.0, 10.0, 10.0, 20.0, 10.0, 10.0, 0.0, SEG,
    );
    assert_volume(&m, 6000.0, 0.01, "twisted_box");
    assert_normals_match_winding(&m, "twisted_box");
}

#[test]
fn remaining_primitives_closed_and_outward() {
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
    // ...and counter-clockwise, which Geant4 reorders with a warning.
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
    let m = tessellate_generic_polycone(corners(&contour), 0.0, 2.0 * PI, SEG, Some(n));

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
fn twisted_tubs_is_a_hyperboloid_not_a_cylinder() {
    // The G4TwistedTubs::SetFields hyperboloid (see `twisted_tubs_mesh`),
    // which closes to r_end = r_mid / cos(twist/2).
    //
    // Meshing at constant radius made the twist geometrically INVISIBLE for a
    // full 2*pi sweep -- rotating a constant-radius circle about its own axis
    // is the identity, so the mesh was bit-for-bit a plain tube.
    let r_mid = 10.0;
    let z_half = 10.0;
    let twist = PI / 3.0; // 60 degrees

    let m = tessellate_twisted_tubs(0.0, r_mid, -z_half, z_half, 2.0 * PI, twist, 256);

    // Solid of revolution of r(z)^2 = r_mid^2 + z^2 tan^2:
    //   V = pi * INT(-h..h) r(z)^2 dz = 2*pi*h*(r_mid^2 + h^2 tan^2 / 3)
    let tan_stereo = (r_mid * (0.5 * twist).tan()).abs() / z_half;
    let expected =
        2.0 * PI * z_half * (r_mid * r_mid + z_half * z_half * tan_stereo * tan_stereo / 3.0);
    assert_volume(&m, expected, 0.01, "twistedtubs hyperboloid");
    assert_normals_match_winding(&m, "twistedtubs hyperboloid");

    // The regression guard: a straight tube of the waist radius is 12.2% smaller
    // here, so the assertion above genuinely distinguishes the two surfaces.
    let cylinder = PI * r_mid * r_mid * 2.0 * z_half;
    assert!(
        (expected - cylinder) / cylinder > 0.1,
        "fixture does not distinguish a hyperboloid from a cylinder"
    );

    // End radius must match the closed form r_mid / cos(twist/2).
    let want_end = r_mid / (0.5 * twist).cos();
    let max_r = (0..m.positions.len() / 3)
        .map(|i| {
            let (x, y) = (m.positions[i * 3] as f64, m.positions[i * 3 + 1] as f64);
            (x * x + y * y).sqrt()
        })
        .fold(0.0f64, f64::max);
    assert!(
        (max_r - want_end).abs() / want_end < 0.01,
        "end radius {max_r:.4}, expected r_mid/cos(twist/2) = {want_end:.4}"
    );
}

#[test]
fn all_primitive_normals_are_unit_length() {
    // Unit normals facing out of their facets, also where a wall collapses:
    // an arb8 wedge with coincident corners, an xtru with a repeated vertex.
    // The revolved solids, steps and apexes included, are checked in
    // revolved_and_elliptical_solids_match_geant4.
    let wedge = [
        [-5.0, -5.0],
        [-5.0, 5.0],
        [5.0, 5.0],
        [5.0, -5.0],
        [-5.0, -5.0],
        [-5.0, -5.0],
        [5.0, -5.0],
        [5.0, -5.0],
    ];
    let meshes = [
        ("box", tessellate_box(10.0, 20.0, 30.0)),
        (
            "cut tube",
            tessellate_cut_tube(
                2.0,
                6.0,
                20.0,
                0.0,
                2.0,
                [0.0, -0.5, -1.0],
                [0.3, 0.0, 1.0],
                SEG,
            ),
        ),
        ("torus", tessellate_torus(1.0, 3.0, 10.0, 0.0, 2.0, SEG)),
        (
            "generic polycone",
            tessellate_generic_polycone(
                corners(&[(0.0, -5.0), (5.0, -5.0), (5.0, 0.0), (8.0, 0.0), (0.0, 5.0)]),
                0.0,
                2.0,
                SEG,
                Some(5),
            ),
        ),
        ("trd", tessellate_trd(10.0, 12.0, 6.0, 0.0, 20.0)),
        (
            "trap",
            tessellate_trap(20.0, 0.1, 0.2, 12.0, 10.0, 8.0, 0.05, 10.0, 9.0, 7.0, 0.05),
        ),
        ("arb8 wedge", tessellate_arb8(10.0, wedge)),
        (
            "twisted trap",
            tessellate_twisted_trap(
                0.4, 20.0, 0.1, 0.2, 12.0, 10.0, 8.0, 10.0, 9.0, 7.0, 0.05, SEG,
            ),
        ),
        (
            "twisted tubs",
            tessellate_twisted_tubs(3.0, 6.0, -10.0, 10.0, 2.0, 0.8, SEG),
        ),
        (
            "xtru repeat",
            tessellate_xtru(
                &[
                    (0.0, 0.0),
                    (10.0, 0.0),
                    (10.0, 0.0),
                    (10.0, 10.0),
                    (0.0, 10.0),
                ],
                &[(-5.0, 0.0, 0.0, 1.0), (5.0, 1.0, 0.0, 0.5)],
            ),
        ),
    ];
    for (name, m) in &meshes {
        assert!(m.triangle_count() > 0, "{name}");
        assert_unit_normals(m, name);
    }
}
