//! CAD import through the HTTP handlers. The committed STL fixtures give the
//! outcomes listed in `tests/data/cad/README.md`, whose volumes come from
//! formulas; an imported document saves and reopens with exactly the
//! coordinates the import computed; re-imports keep the user's materials.

use axum::{
    body::Bytes,
    extract::{Query, State},
    Json,
};
use gdml_studio_backend::{
    api::handlers::*,
    cad::{
        self,
        ir::{Assembly, Child, Target},
        transform::Affine,
    },
    eval::engine::EvalEngine,
    gdml::materials::serialize_gdml,
    gdml::model::GdmlDocument,
    gdml::parser::parse_gdml_from_bytes,
    state::app_state::{create_shared_state, SharedState},
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

fn fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/cad")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

async fn import_bytes(
    state: &SharedState,
    filename: &str,
    bytes: Vec<u8>,
    options: Value,
) -> Result<Value, String> {
    let query = ImportQuery {
        filename: filename.to_string(),
        options: Some(options.to_string()),
    };
    import_cad(State(state.clone()), Query(query), Bytes::from(bytes))
        .await
        .map(|json| json.0)
        .map_err(|e| e.message)
}

async fn import(state: &SharedState, name: &str) -> Result<Value, String> {
    import_bytes(state, name, fixture(name), json!({})).await
}

async fn report(state: &SharedState) -> Value {
    get_import_report(State(state.clone())).await.unwrap().0
}

async fn update(state: &SharedState, options: Value) -> Result<Value, String> {
    let request = serde_json::from_value(json!({ "options": options })).unwrap();
    update_import(State(state.clone()), Json(request))
        .await
        .map(|json| json.0)
        .map_err(|e| e.message)
}

fn close(a: &Value, b: f64) -> bool {
    (a.as_f64().unwrap() - b).abs() <= 1e-9 * b.abs()
}

#[tokio::test]
async fn stl_fixtures_give_the_documented_outcomes() {
    let boxes = [
        "box_ascii.stl",
        "box_binary.stl",
        "binary_solid_header.stl",
        "unwelded.stl",
        "one_flipped.stl",
        "all_flipped.stl",
        "with_degenerate.stl",
    ];
    for name in boxes {
        let state = create_shared_state();
        let summary = import(&state, name).await.unwrap();
        assert_eq!(summary["imported"], true);
        assert_eq!(summary["unsaved"], true);
        let r = report(&state).await;
        let [d] = r["definitions"].as_array().unwrap().as_slice() else {
            panic!("{name}: {r}")
        };
        assert_eq!(
            (d["facets"].as_u64(), d["vertices"].as_u64()),
            (Some(12), Some(8)),
            "{name}"
        );
        assert!(
            close(&d["volume_mm3"], 6000.0),
            "{name}: {}",
            d["volume_mm3"]
        );
        assert_eq!(
            d["bounds_mm"],
            json!([[-5.0, -10.0, -15.0], [5.0, 10.0, 15.0]]),
            "{name}"
        );
        let reversed = match name {
            "one_flipped.stl" => 1,
            "all_flipped.stl" => 12,
            _ => 0,
        };
        assert_eq!(d["reversed_facets"], reversed, "{name}");
        let dropped = if name == "with_degenerate.stl" { 1 } else { 0 };
        assert_eq!(d["repairs"]["dropped_facets"], dropped, "{name}");
    }

    let state = create_shared_state();
    import(&state, "multi_solid.stl").await.unwrap();
    let r = report(&state).await;
    let labels: Vec<&str> = r["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["label"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["one", "two"]);

    import(&state, "closed_cavity.stl").await.unwrap();
    let r = report(&state).await;
    let [d] = r["definitions"].as_array().unwrap().as_slice() else {
        panic!("{r}")
    };
    assert_eq!(d["shells"], 2);
    assert!(close(&d["volume_mm3"], 784.0), "{}", d["volume_mm3"]);

    // Rejected, and the previous document stays open.
    let error = import(&state, "open.stl").await.unwrap_err();
    assert!(error.contains("open surface"), "{error}");
    assert_eq!(report(&state).await["filename"], "closed_cavity.stl");
}

#[tokio::test]
async fn step_import_needs_the_worker() {
    if std::env::var_os("GDML_CAD_WORKER").is_some() {
        return;
    }
    let state = create_shared_state();
    let error = import(&state, "box.step").await.unwrap_err();
    assert_eq!(
        error,
        "box.step: STEP import is not available in this build."
    );
}

// The STEP tests run the worker (cad-worker/README.md): after building it,
// GDML_CAD_WORKER=$PWD/cad-worker/dist/cad/gdml-cad-worker \
//     cargo test --test cad_import_test -- --include-ignored

#[tokio::test]
#[ignore = "needs the STEP worker in GDML_CAD_WORKER"]
async fn step_parts_have_their_formula_volumes() {
    let expected: Value = serde_json::from_slice(&fixture("expected.json")).unwrap();
    let mut checked = 0;
    for (name, e) in expected.as_object().unwrap() {
        let file = format!("{name}.step");
        if name == "assembly" || e["bbox_mm"].is_null() {
            continue;
        }
        let state = create_shared_state();
        import(&state, &file).await.unwrap();
        let r = report(&state).await;
        assert_eq!(r["rejected"], json!([]), "{name}");
        let [d] = r["definitions"].as_array().unwrap().as_slice() else {
            panic!("{name}: {r}")
        };
        let volume = e["volume_mm3"].as_f64().unwrap();
        assert!(close(&d["exact"]["volume_mm3"], volume), "{name}: {d}");
        let mesh_error = d["volume_mm3"].as_f64().unwrap() / volume - 1.0;
        assert!(
            mesh_error.abs() <= 1e-3,
            "{name}: mesh volume off by {mesh_error}"
        );
        // The worker's bounds are exact; mesh vertices lie on the surface, at
        // most a chord inside them.
        let chord = d["exact"]["chord_mm"].as_f64().unwrap();
        for k in 0..6 {
            let mesh = d["bounds_mm"][k / 3][k % 3].as_f64().unwrap();
            let exact = d["exact"]["bounds_mm"][k].as_f64().unwrap();
            let formula = e["bbox_mm"][k].as_f64().unwrap();
            assert!(
                (exact - formula).abs() <= 1e-6 && (mesh - formula).abs() <= chord + 1e-9,
                "{name}: bound {k} {exact} and {mesh} vs {formula}"
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 15);
}

/// The rotation Geant4 gives a physvol with angles `[x, y, z]` in rad:
/// `(Rz Ry Rx)^-1` (G4GDMLReadStructure::PhysvolRead).
fn physvol_rotation([x, y, z]: [f64; 3]) -> [[f64; 3]; 3] {
    let ((sa, ca), (sb, cb), (sg, cg)) = (x.sin_cos(), y.sin_cos(), z.sin_cos());
    let r = [
        [cg * cb, cg * sb * sa - sg * ca, cg * sb * ca + sg * sa],
        [sg * cb, sg * sb * sa + cg * ca, sg * sb * ca - cg * sa],
        [-sb, cb * sa, cb * ca],
    ];
    [0, 1, 2].map(|i| [0, 1, 2].map(|j| r[j][i]))
}

/// A 3 x 4 affine matrix: rotation (times reflection) and translation in mm.
type M34 = [[f64; 4]; 3];

/// Every logical volume placed under volume `name`, through any assemblies,
/// with the transform Geant4 gives it: `[R, t] S` for each physvol
/// (G4GDMLReadStructure::PhysvolRead), composed down from `at`.
fn placements(doc: &GdmlDocument, engine: &EvalEngine, name: &str, at: M34) -> Vec<(String, M34)> {
    let volume = |name: &str| {
        doc.structure
            .volumes
            .iter()
            .find(|v| v.name == name)
            .unwrap()
    };
    let mut out = Vec::new();
    for pv in &volume(name).physvols {
        let t = engine.resolve_position(&pv.position).unwrap();
        let r = physvol_rotation(engine.resolve_rotation(&pv.rotation).unwrap());
        let s = engine.resolve_scale(&pv.scale).unwrap();
        let local: M34 =
            std::array::from_fn(|i| [r[i][0] * s[0], r[i][1] * s[1], r[i][2] * s[2], t[i]]);
        let world: M34 = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                let own = if j == 3 { at[i][3] } else { 0.0 };
                own + (0..3).map(|k| at[i][k] * local[k][j]).sum::<f64>()
            })
        });
        match volume(&pv.volume_ref).assembly {
            true => out.extend(placements(doc, engine, &pv.volume_ref, world)),
            false => out.push((pv.volume_ref.clone(), world)),
        }
    }
    out
}

#[tokio::test]
#[ignore = "needs the STEP worker in GDML_CAD_WORKER"]
async fn a_step_assembly_places_every_occurrence() {
    let expected = &serde_json::from_slice::<Value>(&fixture("expected.json")).unwrap()["assembly"];
    let mut placed_by = Vec::new();
    for hierarchy in ["keep", "flatten"] {
        let state = create_shared_state();
        let options = json!({ "hierarchy": hierarchy });
        import_bytes(&state, "assembly.step", fixture("assembly.step"), options)
            .await
            .unwrap();
        let r = report(&state).await;
        let defs = r["definitions"].as_array().unwrap();
        assert_eq!(defs.len(), 2);
        let instances = |d: &Value| d["instances"].as_f64().unwrap();
        assert_eq!(defs.iter().map(instances).sum::<f64>(), 3.0);
        let volume: f64 = defs
            .iter()
            .map(|d| d["exact"]["volume_mm3"].as_f64().unwrap() * instances(d))
            .sum();
        assert!((volume - 13500.0).abs() <= 1e-9 * 13500.0, "{volume}");
        let part = defs.iter().find(|d| instances(d) == 2.0).unwrap();
        assert_eq!(part["label"], "part 0x bad & name");

        // As Geant4 reads the saved document, each box occurrence sits at its
        // composed world matrix, and the mirrored L (a separate definition in
        // the file) at its translation. The file writes directions to 12
        // significant digits, so the matrices it holds differ from the
        // generator's by up to about 1e-12, and 1e-10 mm over 40 mm lever arms.
        let saved = export_gdml(State(state.clone())).await.unwrap().0;
        let doc =
            parse_gdml_from_bytes(saved["gdml"].as_str().unwrap().as_bytes(), "a.gdml".into())
                .unwrap();
        let assemblies = doc.structure.volumes.iter().filter(|v| v.assembly).count();
        assert_eq!(assemblies, if hierarchy == "keep" { 2 } else { 0 });
        let mut engine = EvalEngine::new();
        engine.evaluate_all(&doc.defines).unwrap();
        let identity = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ];
        let mut placed = placements(&doc, &engine, "World", identity);
        placed.sort_by(|a, b| a.1[0][3].total_cmp(&b.1[0][3]));
        let lv = part["volume"].as_str().unwrap();
        let boxes = placed.iter().filter(|p| p.0 == lv);
        for ((_, m), e) in boxes.zip(expected["box_world_matrices"].as_array().unwrap()) {
            for i in 0..3 {
                for j in 0..4 {
                    let tolerance = if j == 3 { 1e-10 } else { 2e-12 };
                    assert!(
                        (m[i][j] - e[i][j].as_f64().unwrap()).abs() <= tolerance,
                        "{m:?} vs {e}"
                    );
                }
            }
        }
        let [(_, l)] = placed.iter().filter(|p| p.0 != lv).collect::<Vec<_>>()[..] else {
            panic!("{placed:?}")
        };
        assert_eq!(
            *l,
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, -40.0],
                [0.0, 0.0, 1.0, 0.0]
            ]
        );
        placed_by.push(placed);
    }
    // Kept or flattened, Geant4 places the same volumes at the same transforms.
    let [keep, flat] = &placed_by[..] else {
        unreachable!()
    };
    for ((a, m), (b, n)) in keep.iter().zip(flat) {
        assert_eq!(a, b);
        for i in 0..3 {
            for j in 0..4 {
                assert!((m[i][j] - n[i][j]).abs() <= 1e-12, "{m:?} vs {n:?}");
            }
        }
    }
}

#[test]
fn reflected_placements_in_nested_assemblies_keep_their_transforms() {
    // A box placed twice in an assembly, once reflected, which is placed
    // twice in another, once reflected, which the world places rotated.
    // The outer assembly also holds one whose only part is excluded, which
    // is left out.
    let mut ir = cad::read(
        &fixture("box_ascii.stl"),
        "box.stl",
        &cad::Options::default(),
        &cad::Control::default(),
    )
    .unwrap();
    let (s, c) = 0.3f64.sin_cos();
    let turn = Affine::from_rows(&[
        c,
        -s,
        0.0,
        5.0,
        s * c,
        c * c,
        -s,
        1.0,
        s * s,
        s * c,
        c,
        -2.0,
    ]);
    let mirror = Affine::from_rows(&[-1.0, 0.0, 0.0, 30.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    let child = |target, at| Child { target, at };
    let mut spare = ir.definitions[0].clone();
    spare.id = "spare".into();
    ir.definitions.push(spare);
    ir.assemblies = vec![
        Assembly {
            label: "outer".into(),
            children: vec![
                child(Target::Assembly(1), mirror.after(&turn)),
                child(Target::Assembly(2), turn),
                child(Target::Assembly(1), turn),
            ],
        },
        Assembly {
            label: "inner".into(),
            children: vec![
                child(Target::Definition(0), turn),
                child(Target::Definition(0), mirror),
            ],
        },
        Assembly {
            label: "spare".into(),
            children: vec![child(Target::Definition(1), turn)],
        },
    ];
    ir.roots = vec![child(Target::Assembly(0), turn)];
    let identity = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ];
    let placed: Vec<Vec<(String, M34)>> = ["keep", "flatten"]
        .map(|hierarchy| {
            let options = serde_json::from_value(json!({
                "hierarchy": hierarchy,
                "overrides": { "spare": { "representation": "exclude" } },
            }))
            .unwrap();
            let built = cad::build::document("box.stl", &ir, &options, Default::default(), |_| {
                "G4_AIR".into()
            })
            .unwrap();
            let xml = serialize_gdml(&built.document).unwrap();
            let doc = parse_gdml_from_bytes(xml.as_bytes(), "box.gdml".into()).unwrap();
            let assemblies = doc.structure.volumes.iter().filter(|v| v.assembly).count();
            assert_eq!(assemblies, if hierarchy == "keep" { 2 } else { 0 });
            let mut engine = EvalEngine::new();
            engine.evaluate_all(&doc.defines).unwrap();
            placements(&doc, &engine, "World", identity)
        })
        .into();
    let expected: Vec<Affine> = [mirror.after(&turn), turn]
        .iter()
        .flat_map(|outer| [turn, mirror].map(|inner| turn.after(outer).after(&inner)))
        .collect();
    for list in &placed {
        assert_eq!(list.len(), 4);
        for ((_, m), e) in list.iter().zip(&expected) {
            for i in 0..3 {
                for j in 0..4 {
                    let e = if j == 3 { e.t[i] } else { e.r[i][j] };
                    assert!((m[i][j] - e).abs() <= 1e-12, "{m:?} vs {e:?}");
                }
            }
        }
    }
}

#[tokio::test]
async fn an_import_saves_and_reopens_with_its_exact_coordinates() {
    let state = create_shared_state();
    import(&state, "closed_cavity.stl").await.unwrap();
    let mesh = {
        let s = state.read().await;
        let session = s.loaded.as_ref().unwrap().import.as_ref().unwrap();
        session.ir.definitions[0].mesh.clone()
    };
    let saved = export_gdml(State(state.clone())).await.unwrap().0;
    assert_eq!(saved["filename"], "closed_cavity.gdml");
    let xml = saved["gdml"].as_str().unwrap();
    assert!(
        xml.contains(r#"<material name="CAD_UNASSIGNED" Z="1" state="gas">"#),
        "{xml}"
    );
    assert!(xml.contains(r#"<materialref ref="G4_Galactic"/>"#));

    // Every vertex reads back as the same f64; every facet as the same triangle.
    let doc = parse_gdml_from_bytes(xml.as_bytes(), "closed_cavity.gdml".into()).unwrap();
    let mut engine = EvalEngine::new();
    engine.evaluate_all(&doc.defines).unwrap();
    for (i, p) in mesh.positions.iter().enumerate() {
        assert_eq!(engine.named_position(&format!("t0_{i}")).unwrap(), *p);
    }
    let gdml_studio_backend::gdml::model::Solid::Tessellated(solid) = &doc.solids.solids[0] else {
        panic!("not tessellated")
    };
    for (facet, t) in solid.facets.iter().zip(&mesh.triangles) {
        let (_, names, kind, lunit) = facet.parts();
        let expected: Vec<String> = t.iter().map(|v| format!("t0_{v}")).collect();
        assert_eq!(
            (names, kind, lunit),
            (expected.iter().map(String::as_str).collect(), None, None)
        );
    }

    // Reopening it as GDML gives an ordinary document.
    let upload = Json(UploadFilesRequest {
        files: HashMap::from([("closed_cavity.gdml".to_string(), xml.to_string())]),
        main_file: "closed_cavity.gdml".to_string(),
        segments: None,
    });
    let summary = upload_files(State(state.clone()), upload).await.unwrap().0;
    assert_eq!(
        (summary["imported"].clone(), summary["unsaved"].clone()),
        (json!(false), json!(false))
    );
    // The part and the world box.
    assert_eq!(summary["meshes_count"], 2);
    assert_eq!(summary["warnings"], json!([]));
}

#[tokio::test]
async fn reimports_keep_materials_and_apply_new_options() {
    let state = create_shared_state();
    import(&state, "multi_solid.stl").await.unwrap();
    let _assigned = update_volume_material_ref(
        State(state.clone()),
        Json(UpdateMaterialRefRequest {
            volume_name: "one_lv".into(),
            material_ref: "G4_WATER".into(),
        }),
    )
    .await
    .unwrap();
    let materials = |s: &Value| -> Vec<(String, String)> {
        s["volumes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                (
                    v["name"].as_str().unwrap().into(),
                    v["material_ref"].as_str().unwrap().into(),
                )
            })
            .collect()
    };
    let structure =
        |state: SharedState| async move { get_structure(State(state)).await.unwrap().0 };

    // Excluding part "two" keeps the assignment of part "one".
    update(
        &state,
        json!({ "overrides": { "1.0": { "representation": "exclude" } } }),
    )
    .await
    .unwrap();
    assert_eq!(
        materials(&structure(state.clone()).await),
        [
            ("one_lv".into(), "G4_WATER".into()),
            ("World".into(), "G4_Galactic".into())
        ]
    );
    let r = report(&state).await;
    assert_eq!(
        (
            r["definitions"][1]["excluded"].clone(),
            r["definitions"][1]["volume"].clone()
        ),
        (json!(true), Value::Null)
    );

    // A new unit reads the file again: 10 mm per unit scales volumes by 1000.
    update(&state, json!({ "stl_unit_mm": 10.0 }))
        .await
        .unwrap();
    let r = report(&state).await;
    let small: f64 = 10.0 * 20.0 * 30.0;
    assert!(
        close(&r["definitions"][0]["volume_mm3"], 1000.0 * small),
        "{r}"
    );
    assert_eq!(materials(&structure(state.clone()).await)[0].1, "G4_WATER");

    // A part material applies to every part.
    let lead = json!({
        "material": { "name": "Lead", "formula": null, "z": "82", "state": "solid",
            "density": { "value": "11.35", "unit": "g/cm3" }, "density_ref": null,
            "temperature": null, "temperature_ref": null, "pressure": null, "pressure_ref": null,
            "mee": null, "mee_ref": null, "atom_value": "207.2", "atom_unit": "g/mole",
            "atom_type": null, "properties": [], "rl": null, "al": null, "components": [] },
        "elements": [],
    });
    update(&state, json!({ "part_material": lead }))
        .await
        .unwrap();
    assert_eq!(
        materials(&structure(state.clone()).await),
        [
            ("one_lv".into(), "Lead".into()),
            ("two_lv".into(), "Lead".into()),
            ("World".into(), "G4_Galactic".into())
        ]
    );
    // Excluding everything is refused and leaves the document as it was.
    let all_out = json!({ "overrides": { "0.0": { "representation": "exclude" }, "1.0": { "representation": "exclude" } } });
    assert!(update(&state, all_out)
        .await
        .unwrap_err()
        .contains("Every part is excluded"));
    assert_eq!(materials(&structure(state.clone()).await).len(), 3);
}

/// A closed UV sphere of `rings` x `segments` facets (fans at the poles), as
/// binary STL with f32 coordinates.
fn sphere_stl(radius: f64, rings: usize, segments: usize) -> Vec<u8> {
    // Exactly shared poles and seam, as a mesher writes them.
    let point = |i: usize, j: usize| match i {
        0 => [0.0, 0.0, radius],
        i if i == rings => [0.0, 0.0, -radius],
        _ => {
            let theta = std::f64::consts::PI * i as f64 / rings as f64;
            let phi = 2.0 * std::f64::consts::PI * (j % segments) as f64 / segments as f64;
            [
                radius * theta.sin() * phi.cos(),
                radius * theta.sin() * phi.sin(),
                radius * theta.cos(),
            ]
        }
    };
    let mut facets = Vec::new();
    for i in 0..rings {
        for j in 0..segments {
            let (a, b) = (point(i, j), point(i, j + 1));
            let (c, d) = (point(i + 1, j), point(i + 1, j + 1));
            if i > 0 {
                facets.push([a, c, b]);
            }
            if i + 1 < rings {
                facets.push([b, c, d]);
            }
        }
    }
    let mut out = vec![b' '; 80];
    out.extend((facets.len() as u32).to_le_bytes());
    for f in &facets {
        out.extend([0u8; 12]);
        for c in f.iter().flatten() {
            out.extend((*c as f32).to_le_bytes());
        }
        out.extend([0u8; 2]);
    }
    out
}

#[tokio::test]
async fn a_sphere_imports_as_one_closed_body() {
    let state = create_shared_state();
    import_bytes(&state, "sphere.stl", sphere_stl(50.0, 40, 80), json!({}))
        .await
        .unwrap();
    let r = report(&state).await;
    let d = &r["definitions"][0];
    assert_eq!(d["facets"], 2 * 80 * 39);
    let exact = 4.0 / 3.0 * std::f64::consts::PI * 50f64.powi(3);
    let v = d["volume_mm3"].as_f64().unwrap();
    // An inscribed polyhedron: a little less than the sphere.
    assert!(v < exact && v > 0.99 * exact, "{v} vs {exact}");
}

/// Plan acceptance: 250k facets import, preview, save and reopen. Run with
/// `cargo test --release --test cad_import_test -- --ignored --nocapture`.
#[tokio::test]
#[ignore]
async fn a_250k_facet_stl_imports_saves_and_reopens() {
    let bytes = sphere_stl(100.0, 250, 500);
    let state = create_shared_state();
    let t = std::time::Instant::now();
    import_bytes(&state, "big.stl", bytes, json!({}))
        .await
        .unwrap();
    let imported = t.elapsed();
    let facets = report(&state).await["facets"].clone();
    let t = std::time::Instant::now();
    let saved = export_gdml(State(state.clone())).await.unwrap().0;
    let exported = t.elapsed();
    let xml = saved["gdml"].as_str().unwrap().to_string();
    let t = std::time::Instant::now();
    let upload = Json(UploadFilesRequest {
        files: HashMap::from([("big.gdml".to_string(), xml.clone())]),
        main_file: "big.gdml".to_string(),
        segments: None,
    });
    let summary = upload_files(State(state.clone()), upload).await.unwrap().0;
    let reopened = t.elapsed();
    assert_eq!(summary["meshes_count"], 2);
    println!(
        "{facets} facets: import {imported:?}, export {exported:?} ({} MB), reopen {reopened:?}",
        xml.len() / 1_000_000
    );
}

// Public SO-ARM100 models (TheRobotStudio, Apache-2.0), downloaded by
// tests/data/real/fetch.sh; their counts come from the investigation
// (doc/cad-import/INVESTIGATION.md). With the worker built:
// tests/data/real/fetch.sh && GDML_CAD_WORKER=$PWD/../cad-worker/dist/cad/gdml-cad-worker \
//     cargo test --release --test cad_import_test -- --ignored so10 cancelling

fn real_model(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/real")
        .join(name);
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("{}: {e}; run tests/data/real/fetch.sh", path.display()))
}

/// The solid a definition id "<label entry>.<solid>.<body>" belongs to.
fn solid_of(definition: &Value) -> &str {
    let id = definition["id"].as_str().unwrap();
    id.rsplit_once('.').unwrap().0
}

#[tokio::test]
#[ignore = "needs GDML_CAD_WORKER and tests/data/real/fetch.sh"]
async fn so101_upper_arm_imports_closed_within_the_volume_target() {
    let state = create_shared_state();
    let bytes = real_model("upper_arm.step");
    import_bytes(&state, "upper_arm.step", bytes, json!({}))
        .await
        .unwrap();
    let r = report(&state).await;
    // Its two wiring holders are empty labels.
    for rejected in r["rejected"].as_array().unwrap() {
        assert_eq!(rejected["reason"], "it holds no geometry", "{rejected}");
    }
    for d in r["definitions"].as_array().unwrap() {
        let error = d["volume_mm3"].as_f64().unwrap() / d["exact"]["volume_mm3"].as_f64().unwrap();
        assert!((error - 1.0).abs() <= 1e-3, "{}: {error}", d["label"]);
    }
}

#[tokio::test]
#[ignore = "needs GDML_CAD_WORKER and tests/data/real/fetch.sh"]
async fn so100_accounts_for_every_solid_and_reports_its_interferences() {
    let state = create_shared_state();
    let bytes = real_model("so100.step");
    import_bytes(&state, "so100.step", bytes, json!({}))
        .await
        .unwrap();
    let r = report(&state).await;
    let definitions = r["definitions"].as_array().unwrap();
    let rejected = r["rejected"].as_array().unwrap();
    let solids: std::collections::BTreeSet<&str> = definitions.iter().map(solid_of).collect();
    assert_eq!(solids.len() + rejected.len(), 15, "{rejected:?}");
    // Each rejected part says why, naming the face that could not be meshed.
    let reason = |label: &str| {
        let r = rejected.iter().find(|r| r["label"] == label).unwrap();
        r["reason"].as_str().unwrap().to_string()
    };
    assert!(reason("Wrist_Roll_Pitch_08i").contains("a cone around"));
    assert!(reason("Rotation_Pitch_08i").contains("open surface"));
    // The investigation's 12th interference overlaps by 0.0006 mm3, less
    // than 1e-6 of the smaller solid: a contact, not an interference.
    assert_eq!(r["unchecked"], 0);
    let volumes: Vec<f64> = (r["interferences"].as_array().unwrap().iter())
        .map(|i| i["volume_mm3"].as_f64().unwrap())
        .collect();
    assert_eq!(volumes.len(), 11);
    let largest = volumes.iter().copied().fold(0.0, f64::max);
    assert!((largest / 187.457 - 1.0).abs() < 1e-5, "{largest}");
}

#[tokio::test]
#[ignore = "needs GDML_CAD_WORKER and tests/data/real/fetch.sh"]
async fn so101_assembly_accounts_for_every_solid_and_empty_part() {
    use cad::worker::{Label, Meshing, Output};
    let bytes = real_model("so101.step");
    // The worker reads every solid and occurrence (meshing none of them).
    let none = Default::default();
    let meshing = Meshing {
        volume_target: 1e-3,
        chords: &none,
        only: true,
    };
    let tree: Output = cad::worker::run(&bytes, &meshing, &Default::default(), None).unwrap();
    let label: HashMap<&str, &Label> = tree.labels.iter().map(|l| (l.entry.as_str(), l)).collect();
    fn occurrences(label: &HashMap<&str, &Label>, entry: &str) -> usize {
        let l = label[entry];
        match l.assembly {
            true => l
                .components
                .iter()
                .map(|c| occurrences(label, &c.target))
                .sum(),
            false => l.solids.len(),
        }
    }
    assert_eq!(
        tree.labels.iter().map(|l| l.solids.len()).sum::<usize>(),
        90
    );
    assert_eq!(
        tree.roots
            .iter()
            .map(|r| occurrences(&label, r))
            .sum::<usize>(),
        266
    );

    // The import imports each solid or says why not, and lists the 12 leaves
    // that hold no solid.
    let state = create_shared_state();
    import_bytes(&state, "so101.step", bytes, json!({}))
        .await
        .unwrap();
    let r = report(&state).await;
    let rejected = r["rejected"].as_array().unwrap();
    let reason = |r: &Value| r["reason"].as_str().unwrap().to_string();
    let empty = (rejected.iter())
        .filter(|r| {
            reason(r) == "it holds no geometry" || reason(r).contains("form no closed surface")
        })
        .count();
    assert_eq!(empty, 12, "{rejected:?}");
    let solids: std::collections::BTreeSet<&str> = r["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .map(solid_of)
        .collect();
    assert_eq!(solids.len() + rejected.len() - empty, 90);
    let invalid = rejected
        .iter()
        .find(|r| r["label"] == "XG-ZIJI_16")
        .unwrap();
    assert!(reason(invalid).contains("B-rep invalid"), "{invalid}");
}

#[tokio::test]
#[ignore = "needs GDML_CAD_WORKER and tests/data/real/fetch.sh"]
async fn cancelling_an_import_stops_it_at_once_and_keeps_the_document() {
    use std::time::{Duration, Instant};
    let state = create_shared_state();
    import(&state, "box.step").await.unwrap();
    let running = tokio::spawn({
        let state = state.clone();
        async move { import_bytes(&state, "so100.step", real_model("so100.step"), json!({})).await }
    });
    while get_import_progress(State(state.clone())).await.0["phase"] != "mesh" {
        assert!(!running.is_finished());
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let t = Instant::now();
    assert_eq!(
        cancel_import(State(state.clone())).await.0["cancelled"],
        true
    );
    let result = running.await.unwrap();
    assert!(t.elapsed() < Duration::from_secs(1), "{:?}", t.elapsed());
    assert_eq!(result.unwrap_err(), "Import cancelled");
    assert_eq!(report(&state).await["filename"], "box.step");
    assert!(get_import_progress(State(state.clone())).await.0.is_null());
}
