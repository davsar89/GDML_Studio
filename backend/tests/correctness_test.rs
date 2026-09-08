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

fn x_bounds(mesh: &TriangleMesh) -> (f64, f64) {
    mesh.positions
        .chunks_exact(3)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p[0] as f64), hi.max(p[0] as f64))
        })
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
