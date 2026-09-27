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
    engine.set("i", 2.0);
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
    assert!(
        (lo + 5.0).abs() < 1e-4 && (hi - 5.0).abs() < 1e-4,
        "z {lo}..{hi}"
    );
    let (lo, hi) = bounds(&t, 1);
    assert!((lo + hi).abs() < 1e-4 && hi > 7.0, "y {lo}..{hi}");
    let (c, s) = (15f64.to_radians().cos(), 15f64.to_radians().sin());
    let expected = std::f64::consts::FRAC_PI_2 * 5.0 * 75.0 * (c * c + s * s / 3.0);
    assert!((volume(&t) / expected - 1.0).abs() < 0.01, "{}", volume(&t));
}

/// x, y of the mesh vertices lying in the plane at height `z`.
fn slice(mesh: &TriangleMesh, z: f64) -> Vec<[f64; 2]> {
    mesh.positions
        .chunks_exact(3)
        .filter(|p| (p[2] as f64 - z).abs() < 1e-4)
        .map(|p| [p[0] as f64, p[1] as f64])
        .collect()
}

#[tokio::test]
async fn twistedbox_is_untwisted_at_mid_height() {
    // G4TwistBoxSide::SurfacePoint: the slice at height z is turned by
    // PhiTwist * z / (2 dz), so the square is axis-aligned at z = 0 and turned
    // by -+30 deg at the ends, where max x = 7.5 (cos 30 + sin 30).
    let b = solid_mesh(
        r#"<twistedbox name="B" PhiTwist="60" x="15" y="15" z="40" aunit="deg"/>"#,
        "B",
    )
    .await;
    let max_x = |z| slice(&b, z).iter().map(|p| p[0]).fold(f64::MIN, f64::max);
    let end = 7.5 * (30f64.to_radians().cos() + 30f64.to_radians().sin());
    for (z, expected) in [(-20.0, end), (0.0, 7.5), (20.0, end)] {
        assert!((max_x(z) - expected).abs() < 1e-4, "z {z}: {}", max_x(z));
    }
    // Each twisted side quad is split into two flat triangles that sit inside
    // the ruled surface (by area * warp / 12), -1.1% here at 32 segments.
    assert!((volume(&b) / 9000.0 - 1.0).abs() < 0.02, "{}", volume(&b));
}

#[tokio::test]
async fn twistedtrap_theta_shift_is_not_twisted() {
    // The theta/phi centre line, fdeltaX * phi / PhiTwist = z tan(theta)
    // cos(Phi) (G4VTwistedFaceted.cc, G4TwistBoxSide::SurfacePoint), is added
    // after the twist rotation, so the end-cap centroids stay on the x axis.
    // Twisting keeps every slice's area: G4VTwistedFaceted::GetCubicVolume
    // ((dx1+dx2+dx3+dx4)(dy1+dy2) + (dx4+dx3-dx2-dx1)(dy2-dy1)/3) dz = 5760.
    let t = solid_mesh(
        r#"<twistedtrap name="T" PhiTwist="60" z="40" Theta="30" Phi="0" y1="10" x1="8"
          x2="12" y2="16" x3="10" x4="14" Alph="10" aunit="deg"/>"#,
        "T",
    )
    .await;
    for z in [-20.0, 20.0] {
        let s = slice(&t, z);
        let n = s.len() as f64;
        let c = s
            .iter()
            .fold([0.0, 0.0], |c, p| [c[0] + p[0] / n, c[1] + p[1] / n]);
        let expected = z * 30f64.to_radians().tan();
        assert!(
            (c[0] - expected).abs() < 1e-4 && c[1].abs() < 1e-4,
            "z {z}: {c:?}"
        );
    }
    assert!((volume(&t) / 5760.0 - 1.0).abs() < 0.02, "{}", volume(&t));
}

#[tokio::test]
async fn tessellated_relative_facets_and_lunit_follow_geant4() {
    // A 10 x 10 x 10 square pyramid (1000/3 mm3) whose base quad and one side
    // are RELATIVE: vertices after the first are offsets from it. Each facet's
    // own lunit, and the tet's, scale the named positions (G4GDMLReadSolids.cc
    // TriangularRead/QuadrangularRead/TetRead: GetPosition(..) * lunit).
    // Volume about the origin cannot see RELATIVE (det(v0, v0 + d1, v0 + d2) =
    // det(v0, d1, d2)), so the x extent is checked too.
    let defines = r#"<position name="o" x="20" y="30" z="40"/><position name="a" x="30" y="30" z="40"/>
        <position name="b" x="30" y="40" z="40"/><position name="c" x="20" y="40" z="40"/>
        <position name="apex" x="25" y="35" z="50"/><position name="d1" x="-10" y="0" z="0"/>
        <position name="d2" x="-10" y="10" z="0"/><position name="d3" x="0" y="10" z="0"/>
        <position name="e1" x="0" y="-10" z="0"/><position name="e2" x="5" y="-5" z="10"/>"#;
    let pyramid = |name: &str, unit: &str| {
        format!(
            r#"<tessellated name="{name}">
            <quadrangular vertex1="a" vertex2="d1" vertex3="d2" vertex4="d3" type="RELATIVE"{unit}/>
            <triangular vertex1="o" vertex2="a" vertex3="apex"{unit}/>
            <triangular vertex1="a" vertex2="b" vertex3="apex" type="ABSOLUTE"{unit}/>
            <triangular vertex1="b" vertex2="c" vertex3="apex"{unit}/>
            <triangular vertex1="c" vertex2="e1" vertex3="e2" type="RELATIVE"{unit}/></tessellated>"#
        )
    };
    let solids = format!(
        r#"{}{}<tet name="T" vertex1="o" vertex2="a" vertex3="c" vertex4="apex" lunit="cm"/>"#,
        pyramid("P", ""),
        pyramid("Pcm", r#" lunit="cm""#)
    );
    let xml = document(
        defines,
        &solids,
        r#"<volume name="W"><materialref ref="M"/><solidref ref="P"/></volume>"#,
    );
    let expected = [
        ("P", 1000.0 / 3.0, (20.0, 30.0)),
        ("Pcm", 1e6 / 3.0, (200.0, 300.0)),
        ("T", 1e6 / 6.0, (200.0, 300.0)),
    ];
    for xml in [xml.clone(), {
        let state = load(&xml).await;
        let saved = serialize_gdml(&state.read().await.loaded.as_ref().unwrap().document);
        saved.unwrap()
    }] {
        let state = load(&xml).await;
        let guard = state.read().await;
        let loaded = guard.loaded.as_ref().unwrap();
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
        for (name, v, x) in expected {
            let got = volume(&loaded.meshes[name]);
            assert!((got / v - 1.0).abs() < 1e-5, "{name}: {got}, expected {v}");
            assert_eq!(x_bounds(&loaded.meshes[name]), x, "{name}");
        }
    }
}

#[tokio::test]
async fn xtru_with_duplicate_closing_and_collinear_vertices_is_closed() {
    // G4ExtrudedSolid drops coincident and collinear vertices before building
    // (G4ExtrudedSolid.cc:119-135, G4GeomTools::RemoveRedundantVertices), so
    // this is the 10 x 10 square extruded over z = -10..10: 2000 mm3.
    let vertices: String = [(0, 0), (5, 0), (10, 0), (10, 10), (10, 10), (0, 10), (0, 0)]
        .iter()
        .map(|(x, y)| format!(r#"<twoDimVertex x="{x}" y="{y}"/>"#))
        .collect();
    let x = solid_mesh(
        &format!(
            r#"<xtru name="X">{vertices}
            <section zOrder="0" zPosition="-10" xOffset="0" yOffset="0" scalingFactor="1"/>
            <section zOrder="1" zPosition="10" xOffset="0" yOffset="0" scalingFactor="1"/></xtru>"#
        ),
        "X",
    )
    .await;
    // About a point off the solid too: an open or inverted cap cancels out
    // about the origin when its plane passes through it.
    let shifted = TriangleMesh {
        positions: x.positions.iter().map(|c| c + 50.0).collect(),
        ..x.clone()
    };
    for m in [&x, &shifted] {
        assert!((volume(m) / 2000.0 - 1.0).abs() < 1e-3, "{}", volume(m));
    }
}

#[tokio::test]
async fn ellipsoid_zcut_defaults_follow_geant4() {
    // A missing cut is 0 (G4GDMLReadSolids.cc:472-473); 0 and 0 mean uncut
    // (G4Ellipsoid.cc:191), otherwise each cut is clamped into [-cz, cz].
    // V between cuts = pi a b [z - z^3 / (3 c^2)].
    let slab = |lo: f64, hi: f64| {
        let f = |z: f64| z - z.powi(3) / (3.0 * 400.0);
        std::f64::consts::PI * 150.0 * (f(hi) - f(lo))
    };
    for (cuts, lo, hi) in [
        ("", -20.0, 20.0),
        (r#"zcut2="4""#, 0.0, 4.0),
        (r#"zcut1="-5""#, -5.0, 0.0),
        (r#"zcut1="-30" zcut2="30""#, -20.0, 20.0),
    ] {
        let e = solid_mesh(
            &format!(r#"<ellipsoid name="E" ax="10" by="15" cz="20" {cuts}/>"#),
            "E",
        )
        .await;
        let (zlo, zhi) = bounds(&e, 2);
        assert!(
            (zlo - lo).abs() < 1e-4 && (zhi - hi).abs() < 1e-4,
            "{cuts}: z {zlo}..{zhi}"
        );
        let v = volume(&e);
        // 32 x 16 facets cut the uncut ellipsoid's volume by 1.6%.
        assert!((v / slab(lo, hi) - 1.0).abs() < 0.02, "{cuts}: {v}");
    }
}

#[tokio::test]
async fn sphere_deltatheta_stops_at_the_pole() {
    // G4Sphere::CheckThetaAngles: dTheta + sTheta >= pi gives pi - sTheta.
    let ball = 4.0 / 3.0 * std::f64::consts::PI * 1000.0;
    for (angles, fraction, zlo) in [
        (r#"deltatheta="360""#, 1.0, -10.0),
        (r#"starttheta="90" deltatheta="180""#, 0.5, -10.0),
    ] {
        let s = solid_mesh(
            &format!(r#"<sphere name="S" rmax="10" {angles} aunit="deg"/>"#),
            "S",
        )
        .await;
        let (lo, hi) = bounds(&s, 2);
        let zhi = if fraction < 1.0 { 0.0 } else { 10.0 };
        assert!(
            (lo - zlo).abs() < 1e-4 && (hi - zhi).abs() < 1e-4,
            "{angles}: z {lo}..{hi}"
        );
        let v = volume(&s);
        assert!((v / (fraction * ball) - 1.0).abs() < 0.02, "{angles}: {v}");
    }
}

#[tokio::test]
async fn invalid_ellipsoid_and_its_boolean_are_reported() {
    // Crossed cuts are fatal in G4Ellipsoid::CheckParameters (G4Ellipsoid.cc:196).
    // The empty mesh used to be kept, and the union silently became the box.
    let state = load(&document(
        "",
        r#"<ellipsoid name="E" ax="10" by="10" cz="10" zcut1="5" zcut2="-5"/>
        <box name="B" x="10" y="10" z="10"/>
        <union name="U"><first ref="B"/><second ref="E"/></union>"#,
        r#"<volume name="W"><materialref ref="M"/><solidref ref="U"/></volume>"#,
    ))
    .await;
    let guard = state.read().await;
    let loaded = guard.loaded.as_ref().unwrap();
    for name in ["E", "U"] {
        assert!(
            loaded
                .warnings
                .iter()
                .any(|w| w.contains(&format!("solid '{name}'"))),
            "{name}: {:?}",
            loaded.warnings
        );
        assert!(!loaded.meshes.contains_key(name), "{name}");
    }
}

async fn export(state: &SharedState) -> String {
    export_gdml(State(state.clone())).await.unwrap().0["gdml"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn structure_loop_stays_before_the_volume_that_uses_it() {
    // Geant4 reads <structure> in one pass and GetVolume is fatal on a name it
    // has not built yet (G4GDMLReadStructure.cc 1084-1096). The loop defining
    // the slabs was written after W, which places one of them.
    let xml = document(
        r#"<variable name="i" value="0"/>"#,
        r#"<box name="B" x="1" y="1" z="1"/><box name="Big" x="100" y="100" z="100"/>"#,
        r#"<loop for="i" from="1" to="2" step="1"><volume name="Slab[i]"><materialref ref="M"/><solidref ref="B"/></volume></loop>
      <volume name="W"><materialref ref="M"/><solidref ref="Big"/><physvol><volumeref ref="Slab_0"/></physvol></volume>"#,
    );
    let saved = export(&load(&xml).await).await;
    assert!(
        saved.find("<loop").unwrap() < saved.find(r#"<volume name="W""#).unwrap(),
        "the loop moved after the volume that uses it:\n{saved}"
    );
    let scene = get_scene(State(load(&saved).await)).await.unwrap().0;
    assert_eq!(
        scene["scene_graph"]["children"][0]["volume_name"], "Slab_0",
        "the reloaded export lost the placement"
    );
    assert!(
        !scene["warnings"].to_string().contains("undefined volume"),
        "{}",
        scene["warnings"]
    );
}

fn mixture_of(mut material: Material, component: &str) -> Material {
    material.z = None;
    material.atom_value = None;
    material.components = vec![MaterialComponent::Fraction {
        n: "1".into(),
        ref_name: component.into(),
    }];
    material
}

async fn edit_material(state: &SharedState, name: &str, component: &str) -> bool {
    let guard = state.read().await;
    let materials = &guard.loaded.as_ref().unwrap().document.materials.materials;
    let material = mixture_of(
        materials.iter().find(|m| m.name == name).unwrap().clone(),
        component,
    );
    drop(guard);
    update_material(
        State(state.clone()),
        Json(UpdateMaterialRequest {
            name: name.into(),
            material,
            elements: vec![],
        }),
    )
    .await
    .is_ok()
}

fn material_names(xml: &str) -> Vec<String> {
    let doc = parse_gdml_from_bytes(xml.as_bytes(), "saved.gdml".into()).unwrap();
    doc.materials
        .materials
        .into_iter()
        .map(|m| m.name)
        .collect()
}

#[tokio::test]
async fn edited_forward_material_reference_is_reordered() {
    // G4GDMLReadMaterials resolves a <fraction ref> while reading the material,
    // so a mixture must follow what it is made of. M is declared before N;
    // making M a mixture of N used to export M first, which Geant4 rejects.
    let state = load(&document("", r#"<box name="B" x="1" y="1" z="1"/>"#, "")).await;
    assert!(edit_material(&state, "M", "N").await);
    assert_eq!(material_names(&export(&state).await), ["N", "M"]);
}

#[tokio::test]
async fn material_component_cycle_is_rejected() {
    // No order satisfies Geant4's single pass once N is made of M and M of N.
    let state = load(&document("", r#"<box name="B" x="1" y="1" z="1"/>"#, "")).await;
    assert!(edit_material(&state, "M", "N").await);
    assert!(
        !edit_material(&state, "N", "M").await,
        "a component cycle was accepted"
    );
    assert_eq!(material_names(&export(&state).await), ["N", "M"]);
}

#[tokio::test]
async fn xtru_sections_follow_document_order_not_zorder() {
    // XtruRead pushes the sections in document order and SectionRead never
    // reads zOrder (G4GDMLReadSolids.cc 684-690, 1904-1954). Sorting on zOrder
    // put z = +10 first here and turned the 10 x 10 x 20 prism inside out.
    let x = solid_mesh(
        r#"<xtru name="X"><twoDimVertex x="0" y="0"/><twoDimVertex x="10" y="0"/>
        <twoDimVertex x="10" y="10"/><twoDimVertex x="0" y="10"/>
        <section zOrder="1" zPosition="-10" xOffset="0" yOffset="0" scalingFactor="1"/>
        <section zOrder="0" zPosition="10" xOffset="0" yOffset="0" scalingFactor="1"/></xtru>"#,
        "X",
    )
    .await;
    assert!((volume(&x) / 2000.0 - 1.0).abs() < 1e-3, "{}", volume(&x));
}

#[tokio::test]
async fn nist_material_ref_is_accepted() {
    // Geant4's GetMaterial falls back to G4NistManager::FindOrBuildMaterial, so
    // a volume may name G4_WATER without the document defining it.
    let state = load(&document(
        "",
        r#"<box name="B" x="1" y="1" z="1"/>"#,
        r#"<volume name="W"><materialref ref="M"/><solidref ref="B"/></volume>"#,
    ))
    .await;
    let _ = update_volume_material_ref(
        State(state.clone()),
        Json(UpdateMaterialRefRequest {
            volume_name: "W".into(),
            material_ref: "G4_WATER".into(),
        }),
    )
    .await
    .expect("a NIST material was refused");
    assert!(export(&state)
        .await
        .contains(r#"<materialref ref="G4_WATER"/>"#));
}

#[tokio::test]
async fn polyhedra_without_numsides_is_reported_not_invented() {
    // G4GDMLReadSolids defaults numsides to 0 and G4Polyhedra refuses it. The
    // parser invented numsides="6": the preview drew a hexagonal prism Geant4
    // never builds, and the save wrote the invented value into the file.
    let xml = document(
        "",
        r#"<box name="B" x="100" y="100" z="100"/><polyhedra name="P" startphi="0" deltaphi="360" aunit="deg">
        <zplane z="-5" rmax="10"/><zplane z="5" rmax="10"/></polyhedra>"#,
        r#"<volume name="W"><materialref ref="M"/><solidref ref="B"/></volume>"#,
    );
    let state = load(&xml).await;
    {
        let guard = state.read().await;
        let loaded = guard.loaded.as_ref().unwrap();
        assert!(
            !loaded.meshes.contains_key("P"),
            "a sideless polyhedra was drawn"
        );
        assert!(
            loaded
                .warnings
                .iter()
                .any(|w| w.contains("'P'") && w.contains("numsides")),
            "{:?}",
            loaded.warnings
        );
    }
    assert!(
        !export(&state).await.contains("numsides"),
        "numsides was invented on save"
    );
}

#[tokio::test]
async fn delete_material_used_only_inside_a_loop_is_refused() {
    // N is named only by the volumes a structure <loop> declares. Export
    // writes the loop back verbatim, so deleting N saved a loop whose
    // materialref Geant4 cannot resolve.
    let xml = document(
        r#"<variable name="i" value="0"/>"#,
        r#"<box name="B" x="1" y="1" z="1"/><box name="Big" x="100" y="100" z="100"/>"#,
        r#"<loop for="i" from="1" to="2" step="1"><volume name="Slab[i]"><materialref ref="N"/><solidref ref="B"/></volume></loop>
      <volume name="W"><materialref ref="M"/><solidref ref="Big"/><physvol><volumeref ref="Slab_0"/></physvol></volume>"#,
    );
    let state = load(&xml).await;
    let refused = delete_material(
        State(state.clone()),
        Json(DeleteMaterialRequest { name: "N".into() }),
    )
    .await;
    assert!(refused.is_err(), "a material used by a loop was deleted");
    assert!(material_names(&export(&state).await).contains(&"N".to_string()));
}

#[tokio::test]
async fn bad_position_define_warns_instead_of_rejecting() {
    // One bad <position> define rejected the whole upload, although the same
    // mistake in a <constant> only warns. The rest of the file should load and
    // the warning name the define.
    let xml = document(
        r#"<position name="bad" x="1/0"/><position name="good" x="5"/>"#,
        r#"<box name="B" x="1" y="1" z="1"/><box name="Big" x="100" y="100" z="100"/>"#,
        r#"<volume name="V"><materialref ref="M"/><solidref ref="B"/></volume>
      <volume name="W"><materialref ref="M"/><solidref ref="Big"/>
      <physvol><volumeref ref="V"/><positionref ref="good"/></physvol></volume>"#,
    );
    let state = load(&xml).await;
    let warnings = state.read().await.loaded.as_ref().unwrap().warnings.clone();
    assert!(
        warnings.iter().any(|w| w.contains("position \"bad\"")),
        "{warnings:?}"
    );
    let scene = get_scene(State(state)).await.unwrap().0;
    assert_eq!(scene["scene_graph"]["children"][0]["position"][0], 5.0);
}

#[tokio::test]
async fn clhep_units_evaluate_like_geant4() {
    // G4GDMLEvaluator loads every CLHEP unit in internal units, where mm, ns
    // and MeV are 1 (setSystemOfUnits.cc, SystemOfUnits.h), so g/cm3 is
    // 6.2415e18. kg, g, s, ns, perCent and milliradian were undefined here and
    // evaluated to 0. A define named like a builtin (`e`) is the one used, so
    // `w` must wait for it; the exponent in `30e-1` does not name it.
    let xml = document(
        r#"<constant name="w" value="2*e"/><constant name="e" value="k"/><constant name="k" value="30e-1"/>"#,
        r#"<box name="U" x="kg/g*milliradian" y="50*perCent*(s/ns)*1e-9*cm" z="1e-18*g/cm3"/>
        <box name="E" x="w" y="1" z="1"/>"#,
        r#"<volume name="W"><materialref ref="M"/><solidref ref="U"/></volume>"#,
    );
    let state = load(&xml).await;
    let guard = state.read().await;
    let loaded = guard.loaded.as_ref().unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    for (axis, half) in [(0, 0.5), (1, 2.5), (2, 0.5 * 6.241509074)] {
        let (lo, hi) = bounds(&loaded.meshes["U"], axis);
        assert!(
            (hi / half - 1.0).abs() < 1e-6 && (lo / half + 1.0).abs() < 1e-6,
            "axis {axis}: {lo}..{hi}"
        );
    }
    assert_eq!(x_bounds(&loaded.meshes["E"]), (-3.0, 3.0));
}

#[tokio::test]
async fn scene_density_uses_the_evaluator() {
    // DRead evaluates `value` as an expression and multiplies by `unit`
    // (G4GDMLReadMaterials.cc:147-187). The scene parsed the value as a bare
    // number, so a density given through a define had none.
    let xml = document(
        r#"<constant name="rho" value="2700"/>"#,
        r#"<box name="B" x="1" y="1" z="1"/>"#,
        r#"<volume name="W"><materialref ref="Al"/><solidref ref="B"/></volume>"#,
    )
    .replace(
        "</materials>",
        r#"<material name="Al" Z="13"><D value="rho" unit="kg/m3"/><atom value="26.98"/></material></materials>"#,
    );
    let scene = get_scene(State(load(&xml).await)).await.unwrap().0;
    let density = scene["scene_graph"]["density"].as_f64();
    assert!(
        density.is_some_and(|d| (d - 2.7).abs() < 1e-12),
        "{density:?}"
    );
}

fn element(name: &str, z: Option<&str>, fractions: Vec<Fraction>) -> Element {
    Element {
        name: name.into(),
        formula: None,
        z: z.map(Into::into),
        atom_value: z.map(|_| "1.008".into()),
        atom_unit: None,
        atom_type: None,
        fractions,
    }
}

async fn elements(state: &SharedState) -> Vec<(String, Option<String>)> {
    let guard = state.read().await;
    let materials = &guard.loaded.as_ref().unwrap().document.materials;
    materials
        .elements
        .iter()
        .map(|e| (e.name.clone(), e.z.clone()))
        .collect()
}

#[tokio::test]
async fn add_element_with_unknown_isotope_is_rejected() {
    // GetIsotope is fatal on a name that does not exist
    // (G4GDMLReadMaterials.cc:738, 958-963). Adding one element skipped the
    // checks a staged NIST element goes through.
    let state = load(&document("", r#"<box name="B" x="1" y="1" z="1"/>"#, "")).await;
    let fraction = Fraction {
        n: "1".into(),
        ref_name: "U235".into(),
    };
    let refused = add_element(
        State(state.clone()),
        Json(AddElementRequest {
            element: element("Enriched", None, vec![fraction]),
        }),
    )
    .await;
    assert_eq!(refused.err().map(|e| e.status.as_u16()), Some(400));
    assert!(elements(&state).await.is_empty());
}

#[tokio::test]
async fn update_element_rejects_fractional_z() {
    // A staged NIST element with a non-integer Z is refused; editing an
    // element skipped that check and stored Z = 1.5.
    let xml = document("", r#"<box name="B" x="1" y="1" z="1"/>"#, "").replace(
        "<materials>",
        r#"<materials><element name="Hyd" Z="1"><atom value="1.008"/></element>"#,
    );
    let state = load(&xml).await;
    let refused = update_element(
        State(state.clone()),
        Json(UpdateElementRequest {
            name: "Hyd".into(),
            element: element("Hyd", Some("1.5"), vec![]),
        }),
    )
    .await;
    assert_eq!(refused.err().map(|e| e.status.as_u16()), Some(400));
    assert_eq!(
        elements(&state).await,
        [("Hyd".to_string(), Some("1".to_string()))]
    );
}

#[tokio::test]
async fn material_without_composition_is_rejected() {
    // With no <fraction> or <composite>, MaterialRead builds the material from
    // Z and <atom> (G4GDMLReadMaterials.cc:673-676), and G4Element refuses
    // Z < 1. The Materials "+" stored exactly that: a name and a density.
    let state = load(&document("", r#"<box name="B" x="1" y="1" z="1"/>"#, "")).await;
    let mut empty = state
        .read()
        .await
        .loaded
        .as_ref()
        .unwrap()
        .document
        .materials
        .materials[0]
        .clone();
    empty.z = None;
    empty.atom_value = None;
    let added = add_material(
        State(state.clone()),
        Json(AddMaterialRequest {
            material: Material {
                name: "Empty".into(),
                ..empty.clone()
            },
            elements: vec![],
        }),
    )
    .await;
    assert_eq!(added.err().map(|e| e.status.as_u16()), Some(400));
    let updated = update_material(
        State(state.clone()),
        Json(UpdateMaterialRequest {
            name: "M".into(),
            material: empty,
            elements: vec![],
        }),
    )
    .await;
    assert_eq!(updated.err().map(|e| e.status.as_u16()), Some(400));
    let saved = export(&state).await;
    assert_eq!(material_names(&saved), ["M", "N"]);
    assert!(saved.contains(r#"<material name="M" Z="1">"#), "{saved}");
}

#[tokio::test]
async fn quantity_is_listed_in_its_declared_unit() {
    // The Defines tab prints `evaluated` next to the declared unit, so a
    // quantity of 10 cm has to come back as 10, not the 100 mm used inside.
    let state = load(&document(
        r#"<quantity name="q" value="10" unit="cm"/><quantity name="qa" value="90" unit="deg"/>"#,
        r#"<box name="B" x="q" y="1" z="1"/>"#,
        "",
    ))
    .await;
    let defines = get_defines(State(state)).await.unwrap().0;
    let evaluated = |name: &str| {
        defines["defines"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["name"] == name)
            .and_then(|d| d["evaluated"].as_f64())
            .unwrap()
    };
    assert_eq!(evaluated("q"), 10.0);
    assert!(
        (evaluated("qa") - 90.0).abs() < 1e-12,
        "{}",
        evaluated("qa")
    );
}

#[tokio::test]
async fn material_named_like_an_element_can_be_edited() {
    // Elements and materials are separate tables, and MixtureRead resolves a
    // component name as an element first (G4GDMLReadMaterials.cc:770-779); a
    // <composite> can only name an element (795). Both large samples name
    // materials like elements. Editing either one was refused, making the
    // material a mixture of its element was a "self-reference", and renaming
    // it rewrote every <composite ref> to the element into a name Geant4
    // cannot find.
    let xml = document("", r#"<box name="B" x="1" y="1" z="1"/>"#, "").replace(
        "<materials>",
        r#"<materials><element name="Al" Z="13"><atom value="26.98"/></element>
        <element name="O" Z="8"><atom value="16"/></element>
        <material name="Al" Z="13"><D value="2.7"/><atom value="26.98"/></material>
        <material name="Oxide"><D value="3.9"/><composite n="2" ref="Al"/><composite n="3" ref="O"/></material>"#,
    );
    let state = load(&xml).await;
    let element = state
        .read()
        .await
        .loaded
        .as_ref()
        .unwrap()
        .document
        .materials
        .elements[0]
        .clone();
    let edited = update_element(
        State(state.clone()),
        Json(UpdateElementRequest {
            name: "Al".into(),
            element,
        }),
    )
    .await;
    assert!(edited.is_ok(), "{:?}", edited.err());
    let aluminium = state
        .read()
        .await
        .loaded
        .as_ref()
        .unwrap()
        .document
        .materials
        .materials[0]
        .clone();
    for (name, material) in [
        (
            "Al",
            Material {
                density: Some(Density {
                    value: "2.71".into(),
                    unit: None,
                }),
                ..aluminium.clone()
            },
        ),
        ("Al", mixture_of(aluminium.clone(), "Al")),
        (
            "Al",
            Material {
                name: "AlMat".into(),
                ..aluminium
            },
        ),
    ] {
        let result = update_material(
            State(state.clone()),
            Json(UpdateMaterialRequest {
                name: name.into(),
                material: material.clone(),
                elements: vec![],
            }),
        )
        .await;
        assert!(result.is_ok(), "{}: {:?}", material.name, result.err());
    }
    let saved = export(&state).await;
    assert!(saved.contains(r#"<composite n="2" ref="Al"/>"#), "{saved}");
    assert_eq!(material_names(&saved), ["AlMat", "Oxide", "M", "N"]);
}

#[tokio::test]
async fn loop_inside_materials_survives_export() {
    // Preserved elements go back after the element they followed; the
    // materials section never wrote its own, so a <loop> there was deleted by
    // the first save. (Geant4 10.7 refuses a loop in <materials>,
    // G4GDMLReadMaterials.cc:922-927, but the file is the user's to fix.)
    let xml = document(
        r#"<variable name="i" value="0"/>"#,
        r#"<box name="B" x="1" y="1" z="1"/>"#,
        "",
    )
    .replace(
        "<materials>",
        r#"<materials><element name="H" Z="1"><atom value="1.008"/></element>
        <loop for="i" from="1" to="2" step="1"><material name="L[i]" Z="1"><D value="i"/><atom value="1"/></material></loop>"#,
    );
    let saved = export(&load(&xml).await).await;
    let at = |needle: &str| {
        saved
            .find(needle)
            .unwrap_or_else(|| panic!("{needle}:\n{saved}"))
    };
    assert!(at(r#"<element name="H""#) < at("<loop") && at("<loop") < at(r#"<material name="M""#));
}

#[tokio::test]
async fn comment_before_a_preserved_element_survives_export() {
    // A comment anchors to the next named element, and the writer only
    // replays anchors for the elements it models, so one before an
    // <opticalsurface> was lost on save. It now travels with the element.
    let xml = document(
        "",
        r#"<box name="B" x="1" y="1" z="1"/><!-- polished foil -->
        <opticalsurface name="S" model="glisur" finish="polished" type="dielectric_metal" value="1"/>"#,
        "",
    );
    let saved = export(&load(&xml).await).await;
    let comment = saved.find("<!-- polished foil -->");
    assert!(
        comment.is_some_and(|at| at < saved.find("<opticalsurface").unwrap()),
        "{saved}"
    );
}
