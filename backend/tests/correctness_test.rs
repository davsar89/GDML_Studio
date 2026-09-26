use axum::{extract::State, Json};
use gdml_studio_backend::{
    api::handlers::*,
    eval::engine::EvalEngine,
    gdml::{materials::serialize_gdml, model::*, parser::parse_gdml_from_bytes},
    mesh::types::TriangleMesh,
    state::app_state::{create_shared_state, SharedState},
};
use std::collections::HashMap;

const MATERIALS: &str = r#"<materials>
  <material name="M" Z="1"><D value="1"/><atom value="1"/></material>
  <material name="N" Z="1"><D value="2"/><atom value="1"/></material>
</materials>"#;

#[test]
fn invalid_loop_bounds_are_not_silently_replaced_with_zero() {
    use gdml_studio_backend::gdml::loops::expand_loops;
    let mut engine = EvalEngine::new();
    assert!(expand_loops(
        r#"<loop for="i" from="1" to="missing" step="1"><position name="p[i]"/></loop>"#,
        &engine
    )
    .is_err());
    assert!(expand_loops(
        r#"<loop for="i" from="1" to="2" step="1"><position name="p[missing]"/></loop>"#,
        &engine
    )
    .is_err());
    engine.context.set("i", 2.0);
    let expanded = expand_loops(
        r#"<loop for="i" to="3" step="1"><position name="p[i]" x="i"/></loop>"#,
        &engine,
    )
    .unwrap();
    assert_eq!(expanded.matches("<position").count(), 2);
    assert!(expanded.contains("p_1") && expanded.contains("p_2"));
}

fn document(defines: &str, solids: &str, structure: &str) -> String {
    format!(
        r#"<gdml><define>{defines}</define>{MATERIALS}<solids>{solids}</solids>
      <structure>{structure}</structure><setup name="Default" version="1"><world ref="W"/></setup></gdml>"#
    )
}

async fn load(xml: &str) -> SharedState {
    let state = create_shared_state();
    let _ = upload_file(
        State(state.clone()),
        Json(UploadFileRequest {
            filename: "check.gdml".into(),
            content: xml.into(),
            segments: Some(32),
        }),
    )
    .await
    .unwrap();
    state
}

fn bounds(mesh: &TriangleMesh, axis: usize) -> (f64, f64) {
    mesh.positions
        .chunks_exact(3)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p[axis] as f64), hi.max(p[axis] as f64))
        })
}

fn x_bounds(mesh: &TriangleMesh) -> (f64, f64) {
    bounds(mesh, 0)
}

fn volume(mesh: &TriangleMesh) -> f64 {
    mesh.indices
        .chunks_exact(3)
        .map(|tri| {
            let p = |i: u32| {
                let k = i as usize * 3;
                [
                    mesh.positions[k] as f64,
                    mesh.positions[k + 1] as f64,
                    mesh.positions[k + 2] as f64,
                ]
            };
            let [a, b, c] = [p(tri[0]), p(tri[1]), p(tri[2])];
            (a[0] * (b[1] * c[2] - b[2] * c[1])
                + a[1] * (b[2] * c[0] - b[0] * c[2])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0
        })
        .sum()
}

#[tokio::test]
async fn real_arithmetic_and_declared_units_reach_geometry() {
    let engine = EvalEngine::new();
    for (expr, expected) in [
        ("1/2", 0.5),
        ("-3/2", -1.5),
        ("1e-3/2", 0.0005),
        ("2**(-2)", 0.25),
        ("2+3/2", 3.5),
    ] {
        assert!(
            (engine.eval_expr(expr).unwrap() - expected).abs() < 1e-12,
            "{expr}"
        );
    }
    for expr in ["1/0", "NaN", "inf", "sqrt(-1)"] {
        assert!(engine.eval_expr(expr).is_err(), "{expr}");
    }
    let xml = document(
        r#"
        <quantity name="a" value="5" unit="cm"/><quantity name="b" value="10" unit="cm"/>
        <constant name="k" value="a/b"/>
        <quantity name="q" value="90" unit="deg"/><quantity name="t" value="180" unit="deg"/>
        <constant name="angle_ratio" value="q/t"/>
        <position name="p" x="k" unit="cm"/><rotation name="r" z="angle_ratio" unit="deg"/>"#,
        r#"<box name="half" x="1/2" y="2" z="2"/><box name="ratio" x="k" y="1" z="1" lunit="cm"/>
        <box name="unit" x="1" y="1" z="1"/>
        <union name="offset"><first ref="unit"/><second ref="unit"/><position x="k" unit="cm"/>
          <rotation z="angle_ratio" unit="deg"/></union>"#,
        r#"<volume name="V"><materialref ref="M"/><solidref ref="ratio"/></volume>
        <volume name="W"><materialref ref="M"/><solidref ref="ratio"/>
          <physvol><volumeref ref="V"/><positionref ref="p"/><rotationref ref="r"/></physvol>
          <physvol><volumeref ref="V"/><position x="k" unit="cm"/><rotation z="angle_ratio" unit="deg"/></physvol>
        </volume>"#,
    );
    let state = load(&xml).await;
    let guard = state.read().await;
    let loaded = guard.loaded.as_ref().unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    assert_eq!(x_bounds(&loaded.meshes["half"]), (-0.25, 0.25));
    assert!((volume(&loaded.meshes["half"]) - 2.0).abs() < 1e-6);
    assert_eq!(x_bounds(&loaded.meshes["ratio"]), (-2.5, 2.5));
    assert!((volume(&loaded.meshes["ratio"]) - 500.0).abs() < 1e-5);
    let angle = 0.5_f64.to_radians();
    assert!(
        (x_bounds(&loaded.meshes["offset"]).1 - (5.0 + 0.5 * (angle.cos() + angle.sin()))).abs()
            < 1e-5
    );
    drop(guard);
    let scene = get_scene(State(state)).await.unwrap().0;
    let children = scene["scene_graph"]["children"].as_array().unwrap();
    assert_eq!(children.len(), 2, "Both placement forms must appear");
    for child in children {
        assert_eq!(child["position"][0], 5.0);
        assert!((child["rotation"][2].as_f64().unwrap() - angle).abs() < 1e-12);
    }
}

#[tokio::test]
async fn generated_defines_and_material_edits_agree_after_reload() {
    let xml = document(
        r#"<variable name="i" value="1"/>
        <loop for="i" from="1" to="2" step="1"><position name="p[i]" x="i*10"/>
        <rotation name="r[i]" z="i*10" unit="deg"/><scale name="s[i]" x="i"/></loop>"#,
        r#"<box name="box" x="1" y="1" z="1"/>
        <scaledSolid name="scaled"><solidref ref="box"/><scaleref ref="s_1"/></scaledSolid>"#,
        r#"<volume name="V"><materialref ref="M"/><solidref ref="scaled"/></volume>
        <loop for="i" from="1" to="1" step="1"><volume name="G[i]"><materialref ref="M"/><solidref ref="box"/></volume></loop>
        <volume name="W"><materialref ref="M"/><solidref ref="box"/>
        <loop for="i" from="1" to="2" step="1"><physvol><volumeref ref="V"/><positionref ref="p[i]"/><rotationref ref="r[i]"/></physvol></loop>
        <physvol><volumeref ref="G_0"/></physvol></volume>"#,
    );
    let state = load(&xml).await;
    let scene = get_scene(State(state.clone())).await.unwrap().0;
    let children = scene["scene_graph"]["children"].as_array().unwrap();
    assert_eq!(children.len(), 3);
    assert_eq!(
        children.iter().filter(|v| v["volume_name"] == "V").count(),
        2
    );
    for (i, child) in children
        .iter()
        .filter(|v| v["volume_name"] == "V")
        .enumerate()
    {
        assert_eq!(child["position"][0], (i + 1) as f64 * 10.0);
        assert!(
            (child["rotation"][2].as_f64().unwrap() - ((i + 1) as f64 * 10.0).to_radians()).abs()
                < 1e-12
        );
    }
    let mesh_before = state.read().await.loaded.as_ref().unwrap().meshes["scaled"]
        .positions
        .clone();
    assert_eq!(
        x_bounds(&state.read().await.loaded.as_ref().unwrap().meshes["scaled"]),
        (-1.0, 1.0)
    );
    let _ = update_volume_material_ref(
        State(state.clone()),
        Json(UpdateMaterialRefRequest {
            volume_name: "V".into(),
            material_ref: "N".into(),
        }),
    )
    .await
    .unwrap();
    let mut mat = state
        .read()
        .await
        .loaded
        .as_ref()
        .unwrap()
        .document
        .materials
        .materials[0]
        .clone();
    mat.name = "Renamed & M".into();
    let _ = update_material(
        State(state.clone()),
        Json(UpdateMaterialRequest {
            name: "M".into(),
            material: mat,
            elements: vec![],
        }),
    )
    .await
    .unwrap();
    let saved = export_gdml(State(state.clone())).await.unwrap().0["gdml"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(saved.contains("<loop"));
    assert!(saved.contains("Renamed &amp; M"));
    let reloaded = load(&saved).await;
    for state in [state, reloaded] {
        let scene = get_scene(State(state.clone())).await.unwrap().0;
        let children = scene["scene_graph"]["children"].as_array().unwrap();
        assert_eq!(
            children.len(),
            3,
            "Material edits must preserve all placements"
        );
        for child in children {
            assert_eq!(
                child["material_name"],
                if child["volume_name"] == "V" {
                    "N"
                } else {
                    "Renamed & M"
                }
            );
        }
        assert_eq!(
            state.read().await.loaded.as_ref().unwrap().meshes["scaled"].positions,
            mesh_before
        );
    }
}

#[tokio::test]
async fn missing_references_omit_geometry_with_a_warning() {
    let xml = document(
        "",
        r#"<box name="B" x="1" y="1" z="1"/>
      <union name="bad"><first ref="B"/><second ref="B"/><positionref ref="absent"/></union>
      <scaledSolid name="badscale"><solidref ref="B"/><scaleref ref="absentscale"/></scaledSolid>"#,
        r#"<volume name="V"><materialref ref="M"/><solidref ref="B"/></volume><volume name="W"><materialref ref="M"/><solidref ref="B"/>
      <physvol><volumeref ref="V"/><positionref ref="absent"/></physvol>
      <physvol><volumeref ref="V"/><rotationref ref="absentrot"/></physvol></volume>"#,
    );
    let state = load(&xml).await;
    let guard = state.read().await;
    let loaded = guard.loaded.as_ref().unwrap();
    assert!(loaded.meshes.contains_key("B"), "Keep the valid solid");
    assert!(!loaded.meshes.contains_key("bad"));
    assert!(!loaded.meshes.contains_key("badscale"));
    assert!(loaded.warnings.iter().any(|w| w.contains("absent")));
    drop(guard);
    let scene = get_scene(State(state)).await.unwrap().0;
    assert!(scene["scene_graph"]["children"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(scene["warnings"].to_string().contains("absentrot"));
}

#[tokio::test]
async fn module_export_preserves_definitions_and_dependency_order() {
    let parent = r#"<gdml><define/><materials/><solids><box name="world" x="100" y="100" z="100"/></solids>
    <structure><volume name="W"><materialref ref="G4_Galactic"/><solidref ref="world"/><physvol><file name="child.gdml"/></physvol></volume></structure>
    <setup name="Default" version="1"><world ref="W"/></setup></gdml>"#;
    let child = r#"<gdml><define><quantity name="side" value="2" unit="mm"/><constant name="derived" value="side*2"/><scale name="s" x="2"/></define>
    <materials><define><constant name="rho" value="1"/></define><isotope name="H1" Z="1" N="1"><atom value="1.0078"/></isotope>
    <element name="Hydrogen"><fraction n="1" ref="H1"/></element><material name="M"><D value="rho"/><fraction n="1" ref="Hydrogen"/></material></materials>
    <solids><box name="box" x="derived" y="2" z="2"/><scaledSolid name="scaled"><solidref ref="box"/><scaleref ref="s"/></scaledSolid></solids>
    <structure><volume name="child"><materialref ref="M"/><solidref ref="scaled"/></volume></structure><setup name="Default" version="1"><world ref="child"/></setup></gdml>"#;
    let state = create_shared_state();
    let _ = upload_files(
        State(state.clone()),
        Json(UploadFilesRequest {
            main_file: "main.gdml".into(),
            segments: None,
            files: HashMap::from([
                ("main.gdml".into(), parent.into()),
                ("child.gdml".into(), child.into()),
            ]),
        }),
    )
    .await
    .unwrap();
    let saved = export_gdml(State(state.clone())).await.unwrap().0["gdml"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        saved.find("<volume name=\"child\"").unwrap() < saved.find("<volume name=\"W\"").unwrap()
    );
    assert!(
        saved.find("<quantity name=\"side\"").unwrap()
            < saved.find("<constant name=\"derived\"").unwrap()
    );
    let doc = parse_gdml_from_bytes(saved.as_bytes(), "saved.gdml".into()).unwrap();
    assert_eq!(doc.materials.isotopes.len(), 1);
    assert_eq!(doc.defines.scales.len(), 1);
    assert_eq!(doc.materials_define, Some(vec!["rho".into()]));
    assert_eq!(
        x_bounds(
            &load(&saved)
                .await
                .read()
                .await
                .loaded
                .as_ref()
                .unwrap()
                .meshes["scaled"]
        ),
        (-4.0, 4.0)
    );

    let mut cyc = doc.clone();
    cyc.structure.volumes[0].physvols.push(PhysVol {
        name: None,
        copynumber: None,
        file_ref: None,
        volume_ref: "W".into(),
        position: None,
        rotation: None,
    });
    assert!(serialize_gdml(&cyc)
        .unwrap_err()
        .to_string()
        .contains("Cyclic volume"));

    let state = create_shared_state();
    let files = HashMap::from([
        (
            "test_modular_mother.gdml".into(),
            include_str!("../../sample_data/test_modular_mother.gdml").into(),
        ),
        (
            "test_modular_child.gdml".into(),
            include_str!("../../sample_data/test_modular_child.gdml").into(),
        ),
    ]);
    let _ = upload_files(
        State(state.clone()),
        Json(UploadFilesRequest {
            main_file: "test_modular_mother.gdml".into(),
            files,
            segments: None,
        }),
    )
    .await
    .unwrap();
    let saved = export_gdml(State(state)).await.unwrap().0["gdml"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        saved.find("<volume name=\"ChildWorld\"").unwrap()
            < saved.find("<volume name=\"MotherWorld\"").unwrap()
    );
}

#[tokio::test]
async fn material_and_nist_elements_commit_atomically() {
    let state = load(&document(
        "",
        r#"<box name="B" x="1" y="1" z="1"/>"#,
        r#"<volume name="W"><materialref ref="M"/><solidref ref="B"/></volume>"#,
    ))
    .await;
    let mut material = state
        .read()
        .await
        .loaded
        .as_ref()
        .unwrap()
        .document
        .materials
        .materials[0]
        .clone();
    let element = Element {
        name: "NewElement".into(),
        formula: None,
        z: Some("1".into()),
        atom_value: Some("1".into()),
        atom_unit: None,
        atom_type: None,
        fractions: vec![],
    };
    material.components = vec![MaterialComponent::Fraction {
        n: "1".into(),
        ref_name: element.name.clone(),
    }];
    let mut invalid_element = element.clone();
    invalid_element.z = Some("not_a_number".into());
    assert!(update_material(
        State(state.clone()),
        Json(UpdateMaterialRequest {
            name: "M".into(),
            material: material.clone(),
            elements: vec![invalid_element],
        })
    )
    .await
    .is_err());
    assert!(state
        .read()
        .await
        .loaded
        .as_ref()
        .unwrap()
        .document
        .materials
        .elements
        .is_empty());
    material.density.as_mut().unwrap().value = "-1".into();
    assert!(update_material(
        State(state.clone()),
        Json(UpdateMaterialRequest {
            name: "M".into(),
            material: material.clone(),
            elements: vec![element.clone()]
        })
    )
    .await
    .is_err());
    assert!(state
        .read()
        .await
        .loaded
        .as_ref()
        .unwrap()
        .document
        .materials
        .elements
        .is_empty());
    material.density.as_mut().unwrap().value = "3.25".into();
    let _ = update_material(
        State(state.clone()),
        Json(UpdateMaterialRequest {
            name: "M".into(),
            material: material.clone(),
            elements: vec![element.clone()],
        }),
    )
    .await
    .unwrap();
    assert_eq!(
        state
            .read()
            .await
            .loaded
            .as_ref()
            .unwrap()
            .document
            .materials
            .elements
            .len(),
        1
    );
    let mut conflicting = element;
    conflicting.z = Some("2".into());
    material.density.as_mut().unwrap().value = "4".into();
    assert!(update_material(
        State(state.clone()),
        Json(UpdateMaterialRequest {
            name: "M".into(),
            material,
            elements: vec![conflicting]
        })
    )
    .await
    .is_err());
    assert_eq!(
        state
            .read()
            .await
            .loaded
            .as_ref()
            .unwrap()
            .document
            .materials
            .materials[0]
            .density
            .as_ref()
            .unwrap()
            .value,
        "3.25"
    );
}

async fn solid_mesh(solids: &str, solid: &str) -> TriangleMesh {
    let xml = document(
        "",
        solids,
        &format!(r#"<volume name="W"><materialref ref="M"/><solidref ref="{solid}"/></volume>"#),
    );
    let state = load(&xml).await;
    let guard = state.read().await;
    let loaded = guard.loaded.as_ref().unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    loaded.meshes[solid].clone()
}

#[tokio::test]
async fn boolean_with_a_finely_meshed_orb_has_the_right_volume() {
    // A 960-face orb made a BSP chain far past the old depth cap of 192, whose
    // plane-less leaves kept everything below them. The box's face
    // x = 0 is the orb's mirror plane, so exactly half its mesh must remain.
    let orb = r#"<orb name="O" r="50"/><box name="B" x="200" y="200" z="200"/>"#;
    let half = solid_mesh(
        &format!(r#"{orb}<intersection name="H"><first ref="O"/><second ref="B"/><position x="100"/></intersection>"#),
        "H",
    )
    .await;
    let whole = volume(&solid_mesh(orb, "O").await);
    assert!((volume(&half) / (0.5 * whole) - 1.0).abs() < 1e-3);
    let (lo, hi) = x_bounds(&half);
    assert!(lo.abs() < 1e-3 && (hi - 50.0).abs() < 1e-3, "{lo}..{hi}");
}

#[tokio::test]
async fn box_and_offset_orb_intersection_is_bounded_by_both() {
    // The browser baseline: this came out from x = -44.4 with volume 618,583,
    // more than the half of the orb that fits in the box at all (452,389).
    // Analytic: that half ball minus four half caps of height 10 beyond
    // |y|, |z| = 50, i.e. 2/3 pi 60^3 - 2 pi 10^2 (180 - 10) / 3 = 416,784.
    let bo = solid_mesh(
        r#"<box name="B" x="100" y="100" z="100"/><orb name="O" r="60"/>
        <intersection name="BO"><first ref="B"/><second ref="O"/><position x="50"/></intersection>"#,
        "BO",
    )
    .await;
    let (lo, hi) = x_bounds(&bo);
    assert!(
        (lo + 10.0).abs() < 1e-3 && (hi - 50.0).abs() < 1e-3,
        "{lo}..{hi}"
    );
    let v = volume(&bo);
    assert!(v < 452_389.0 && (v / 416_784.0 - 1.0).abs() < 0.02, "{v}");
}

#[tokio::test]
async fn chained_subtractions_stay_linear() {
    // Every plane of each hole used to split the whole plate, and each later
    // hole split those slivers again: 10 holes gave 148x the triangles of one.
    let plate = |n: usize| {
        let mut solids = String::from(
            r#"<box name="s0" x="200" y="80" z="10"/><tube name="T" rmax="10" z="20" deltaphi="360" aunit="deg"/>"#,
        );
        for i in 0..n {
            let (x, y) = (-80 + 40 * (i % 5) as i32, if i < 5 { -20 } else { 20 });
            solids += &format!(
                r#"<subtraction name="s{}"><first ref="s{i}"/><second ref="T"/><position x="{x}" y="{y}"/></subtraction>"#,
                i + 1
            );
        }
        solids
    };
    let one = solid_mesh(&plate(1), "s1").await;
    let ten = solid_mesh(&plate(10), "s10").await;
    let expected = 10.0 * (200.0 * 80.0 - 10.0 * std::f64::consts::PI * 100.0);
    assert!(
        (volume(&ten) / expected - 1.0).abs() < 0.01,
        "{}",
        volume(&ten)
    );
    assert!(
        ten.triangle_count() <= 12 * one.triangle_count(),
        "{} vs {}",
        ten.triangle_count(),
        one.triangle_count()
    );
}

#[tokio::test]
async fn get_meshes_sends_only_placed_solids() {
    let state = load(&document(
        "",
        r#"<box name="S" x="12.34" y="1" z="1"/><box name="A" x="1" y="1" z="1"/>
        <union name="U"><first ref="A"/><second ref="A"/><position x="0.5"/></union>
        <box name="unused" x="1" y="1" z="1"/>"#,
        r#"<volume name="V"><materialref ref="M"/><solidref ref="U"/></volume>
        <volume name="W"><materialref ref="M"/><solidref ref="S"/><physvol><volumeref ref="V"/></physvol></volume>"#,
    ))
    .await;
    let response = get_meshes(State(state)).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = std::str::from_utf8(&body).unwrap();
    let json: serde_json::Value = serde_json::from_str(body).unwrap();
    let mut keys: Vec<_> = json["meshes"].as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["S", "U"]);
    // f32 written as f32, not widened through f64 to 6.170000076293945.
    assert!(body.contains("6.17") && !body.contains("6.1700000"));
}

#[tokio::test]
async fn twistedtubs_zlen_is_the_full_length() {
    // The reader halves zlen (G4GDMLReadSolids.cc:3323) and the phi sweep is
    // centred, [-dphi/2, dphi/2] (G4TwistTubsFlatSide), so z spans +-zlen/2 and
    // y is symmetric. Each z slice is an annular sector between hyperboloids
    // r^2 = r_mid^2 (1 + (z tan(tw/2) / h)^2) with r_mid = r_end cos(tw/2), so
    // V = dphi h (Ro^2 - Ri^2)_end (cos^2(tw/2) + sin^2(tw/2) / 3).
    let t = solid_mesh(
        r#"<twistedtubs name="T" twistedangle="30" endinnerrad="5" endouterrad="10"
          zlen="10" phi="90" aunit="deg"/>"#,
        "T",
    )
    .await;
    let (lo, hi) = bounds(&t, 2);
    assert!((lo + 5.0).abs() < 1e-4 && (hi - 5.0).abs() < 1e-4, "z {lo}..{hi}");
    let (lo, hi) = bounds(&t, 1);
    assert!((lo + hi).abs() < 1e-4 && hi > 7.0, "y {lo}..{hi}");
    let (c, s) = (15f64.to_radians().cos(), 15f64.to_radians().sin());
    let expected = std::f64::consts::FRAC_PI_2 * 5.0 * 75.0 * (c * c + s * s / 3.0);
    assert!((volume(&t) / expected - 1.0).abs() < 0.01, "{}", volume(&t));
}
