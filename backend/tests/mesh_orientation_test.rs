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

use gdml_studio_backend::eval::engine::EvalEngine;
use gdml_studio_backend::gdml::parser::parse_gdml_from_bytes;
use gdml_studio_backend::mesh::primitives::{
    arb8_mesh::{tessellate_arb8, tessellate_box, tessellate_trap, tessellate_trd},
    cone_mesh::tessellate_cone,
    cut_tube_mesh::tessellate_cut_tube,
    elcone_mesh::tessellate_elcone,
    ellipsoid_mesh::tessellate_ellipsoid,
    eltube_mesh::tessellate_eltube,
    generic_polycone_mesh::tessellate_generic_polycone,
    hype_mesh::tessellate_hype,
    paraboloid_mesh::tessellate_paraboloid,
    polycone_mesh::tessellate_polycone,
    polyhedra_mesh::tessellate_polyhedra,
    sphere_mesh::tessellate_sphere,
    torus_mesh::tessellate_torus,
    tube_mesh::tessellate_tube,
    twisted_trap_mesh::tessellate_twisted_trap,
    twisted_tubs_mesh::tessellate_twisted_tubs,
    xtru_mesh::tessellate_xtru,
};
use gdml_studio_backend::mesh::tessellator::tessellate_all_solids;
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

/// An (r,z) contour whose every point is a corner.
fn corners(rz: &[(f64, f64)]) -> impl Iterator<Item = (f64, f64, bool)> + '_ {
    rz.iter().map(|&(r, z)| (r, z, false))
}

/// Solid `S` declared by `xml`, through the real path: parser, evaluator and
/// tessellator; and the warnings.
fn solid(xml: &str) -> (Option<TriangleMesh>, Vec<String>) {
    let gdml = format!("<gdml><solids>{xml}</solids></gdml>");
    let doc = parse_gdml_from_bytes(gdml.as_bytes(), "t.gdml".into()).unwrap();
    let mut engine = EvalEngine::new();
    engine.evaluate_all(&doc.defines).unwrap();
    let (mut meshes, warnings) = tessellate_all_solids(&doc.solids, &engine, SEG).unwrap();
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
    // Inner radii from the top down, then outer radii back up.
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
            // G4Polycone.cc:232: a zero opening angle is a full turn.
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
            "cone, apex",
            r#"<cone name="S" rmax1="10" rmax2="0" z="20"/>"#,
            PI * frustum(20.0, 10.0, 0.0),
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
            "sphere, wide theta cut",
            r#"<sphere name="S" rmax="50" deltatheta="120" aunit="deg"/>"#,
            2.0 * PI * sector(50.0, 0.0, 120.0 * d),
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
fn parameters_geant4_rejects_are_reported() {
    for xml in [
        // G4Tubs: rmin >= rmax.
        r#"<tube name="S" rmin="10" rmax="5" z="20"/>"#,
        // G4Sphere: rmin >= rmax.
        r#"<sphere name="S" rmin="10" rmax="5"/>"#,
        // G4Polycone.cc:80: rInner > rOuter at a plane.
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
    // The shape both shipped samples use: a bicone whose closing edge lies on
    // the z axis. This was already right under the old z-plane mapping, so it
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
    let m = tessellate_twisted_trap(
        0.6, 30.0, 0.0, 0.0, 20.0, 10.0, 10.0, 20.0, 10.0, 10.0, 0.0, SEG,
    );
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

#[test]
fn twisted_tubs_is_a_hyperboloid_not_a_cylinder() {
    // G4TwistedTubs::SetFields (inline in G4TwistedTubs.hh):
    //   tanStereo    = |r_mid * tan(twist/2)| / zHalfLength
    //   endRadius[i] = sqrt(r_mid^2 + endZ[i]^2 * tanStereo^2)
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
    // Every stored normal must be a unit vector within 60 deg of its facet's
    // winding normal, also where a wall collapses: a polycone/polyhedra step
    // at one z (a flat annulus), a polyhedra closing on the axis, an arb8
    // wedge with coincident corners, an xtru with a repeated vertex.
    let step = [
        (-15.0, 0.0, 0.0),
        (-10.0, 5.0, 10.0),
        (0.0, 5.0, 10.0),
        (0.0, 2.0, 20.0),
        (10.0, 2.0, 20.0),
        (15.0, 0.0, 0.0),
    ];
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
        ("tube", tessellate_tube(3.0, 6.0, 20.0, 0.3, 2.0, SEG)),
        (
            "cone",
            tessellate_cone(2.0, 6.0, 0.0, 3.0, 20.0, 0.0, 2.0, SEG),
        ),
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
        (
            "sphere",
            tessellate_sphere(3.0, 6.0, 0.0, 2.0, 0.3, 2.0, SEG),
        ),
        ("torus", tessellate_torus(1.0, 3.0, 10.0, 0.0, 2.0, SEG)),
        ("polycone step", tessellate_polycone(&step, 0.0, 2.0, SEG)),
        ("polyhedra step", tessellate_polyhedra(&step, 0.0, 2.0, 5)),
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
        (
            "ellipsoid",
            tessellate_ellipsoid(4.0, 6.0, 8.0, -3.0, 5.0, SEG),
        ),
        ("eltube", tessellate_eltube(4.0, 6.0, 8.0, SEG)),
        ("elcone", tessellate_elcone(0.5, 0.8, 10.0, 5.0, SEG)),
        ("paraboloid", tessellate_paraboloid(2.0, 5.0, 10.0, SEG)),
        ("hype", tessellate_hype(3.0, 6.0, 0.2, 0.3, 20.0, SEG)),
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
    // The first bad normal of each solid, so one run lists every offender.
    let bad: Vec<String> = meshes
        .iter()
        .filter_map(|(name, m)| {
            let p = |i: u32| [0, 1, 2].map(|k| m.positions[i as usize * 3 + k] as f64);
            let n = |i: u32| [0, 1, 2].map(|k| m.normals[i as usize * 3 + k] as f64);
            m.indices.chunks_exact(3).find_map(|tri| {
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
                tri.iter().map(|&i| n(i)).find_map(|ni| {
                    let len = ni.iter().map(|c| c * c).sum::<f64>().sqrt();
                    let cos = (0..3).map(|k| geo[k] * ni[k]).sum::<f64>() / area2;
                    ((len - 1.0).abs() > 1e-3 || (area2 > 1e-6 && cos < 0.5))
                        .then(|| format!("{name}: normal {ni:?} on facet {geo:?}"))
                })
            })
        })
        .collect();
    assert!(meshes.iter().all(|(_, m)| m.triangle_count() > 0));
    assert!(bad.is_empty(), "{bad:#?}");
}
