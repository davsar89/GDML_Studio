//! Integration tests for the GDML Studio backend pipeline.
//!
//! Exercises the full parse -> evaluate -> tessellate pipeline on each of the
//! sample GDML files under sample_data/.

use std::path::Path;

use gdml_studio_backend::config::DEFAULT_MESH_SEGMENTS;
use gdml_studio_backend::eval::engine::EvalEngine;
use gdml_studio_backend::gdml::parser::{parse_gdml, parse_gdml_from_bytes};
use gdml_studio_backend::mesh::tessellator::tessellate_all_solids;
use gdml_studio_backend::mesh::types::TriangleMesh;

mod common;
use common::signed_volume_about;

/// Locate the project root (two levels up from the backend/tests directory).
fn project_root() -> &'static Path {
    // The integration test binary is run from the workspace root by default,
    // but we use an absolute-ish anchor based on the Cargo manifest dir.
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

// ─── Per-file helpers ────────────────────────────────────────────────────────

struct PipelineResult {
    num_constants: usize,
    num_quantities: usize,
    num_variables: usize,
    num_elements: usize,
    num_materials: usize,
    num_solids: usize,
    num_volumes: usize,
    num_meshes: usize,
    total_triangles: usize,
}

/// Run the full pipeline on a single GDML file and return summary counts.
fn run_pipeline(gdml_path: &Path) -> PipelineResult {
    // 1. Parse
    let doc = parse_gdml(gdml_path)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", gdml_path.display(), e));

    assert!(
        !doc.setup.world_ref.is_empty(),
        "Missing world reference in {}",
        gdml_path.display()
    );
    assert!(
        !doc.materials.materials.is_empty() || !doc.materials.elements.is_empty(),
        "Missing material data in {}",
        gdml_path.display()
    );

    // 2. Evaluate all defines / expressions
    let mut engine = EvalEngine::new();
    engine.evaluate_all(&doc.defines).unwrap_or_else(|e| {
        panic!(
            "Failed to evaluate defines for {}: {}",
            gdml_path.display(),
            e
        )
    });

    // 3. Tessellate all solids
    let (meshes, _warnings) = tessellate_all_solids(&doc.solids, &engine, DEFAULT_MESH_SEGMENTS)
        .unwrap_or_else(|e| {
            panic!(
                "Failed to tessellate solids for {}: {}",
                gdml_path.display(),
                e
            )
        });

    let total_triangles: usize = meshes.values().map(|m| m.triangle_count()).sum();

    PipelineResult {
        num_constants: doc.defines.constants.len(),
        num_quantities: doc.defines.quantities.len(),
        num_variables: doc.defines.variables.len(),
        num_elements: doc.materials.elements.len(),
        num_materials: doc.materials.materials.len(),
        num_solids: doc.solids.solids.len(),
        num_volumes: doc.structure.volumes.len(),
        num_meshes: meshes.len(),
        total_triangles,
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[test]
fn test_pod_asm_pipeline() {
    let path = project_root().join("sample_data/pod_asm.gdml");

    let result = run_pipeline(&path);

    // 9 solids: box, 4 tubes, polycone, 3 xtru
    assert_eq!(result.num_solids, 9, "Expected 9 solids");
    assert_eq!(result.num_volumes, 9, "Expected 9 volumes");
    assert_eq!(
        result.num_meshes, result.num_solids,
        "Every solid should be tessellated into a mesh"
    );
    assert!(result.total_triangles > 0, "Expected at least one triangle");
}

#[test]
fn test_sample_detector_pipelines() {
    for file in [
        "BgoDetModel_v2_00.gdml",
        "fermi_simple_elements_satellite.gdml",
        "NaiDetModelWithMLI_v2_00.gdml",
        "pod_asm_tessellated.gdml",
    ] {
        let result = run_pipeline(&project_root().join("sample_data").join(file));
        // The file should parse something non-trivial
        assert!(result.num_solids > 0, "{file}: expected at least one solid");
        assert!(
            result.num_volumes > 0,
            "{file}: expected at least one volume"
        );
        assert!(result.num_meshes > 0, "{file}: expected at least one mesh");
        assert!(
            result.total_triangles > 0,
            "{file}: expected at least one triangle"
        );
        assert_eq!(
            result.num_meshes, result.num_solids,
            "{file}: every solid should be tessellated into a mesh"
        );
    }
}

#[test]
fn test_solids_gdml_pipeline() {
    let path = project_root().join("sample_data/solids.gdml");

    let result = run_pipeline(&path);

    // This file has many solid types; two (multiunion1, reflectedsphere1)
    // reference an undefined `s100` and get no mesh. Just verify that meshes
    // come out and nothing panics.
    assert!(result.num_solids > 0, "Expected at least one solid");
    assert!(result.num_meshes > 0, "Expected at least one mesh");
    assert!(result.total_triangles > 0, "Expected at least one triangle");
}

#[test]
fn test_all_features_pipeline() {
    let path = project_root().join("sample_data/test_all_features.gdml");

    let result = run_pipeline(&path);

    // Verify expected counts from the file
    assert_eq!(
        result.num_constants, 2,
        "Expected 2 constants (HALF_PI, deg)"
    );
    assert_eq!(result.num_quantities, 1, "Expected 1 quantity (thickness)");
    assert_eq!(result.num_variables, 1, "Expected 1 variable (x_offset)");
    assert_eq!(
        result.num_elements, 4,
        "Expected 4 elements (H, O, N, Al_el)"
    );
    assert_eq!(
        result.num_materials, 3,
        "Expected 3 materials (Aluminium, Water, Air)"
    );
    assert_eq!(
        result.num_solids, 6,
        "Expected 6 solids (WorldBox, MyBox, MyTube, MyPartialTube, MyCone, MySphere)"
    );
    assert_eq!(result.num_volumes, 6, "Expected 6 volumes (World + 5 leaf)");
    assert_eq!(
        result.num_meshes, result.num_solids,
        "Every solid should be tessellated"
    );
    assert!(result.total_triangles > 0, "Expected triangles");
}

#[test]
fn test_round_trip_preserves_material_fidelity_and_escaping() {
    use gdml_studio_backend::gdml::materials::serialize_gdml;

    let src = r#"<?xml version="1.0"?>
<gdml>
  <define/>
  <materials>
    <isotope name="U235" N="235" Z="92"><atom value="235.0439"/></isotope>
    <isotope name="U238" N="238" Z="92"><atom value="238.0508"/></isotope>
    <element name="enriched_U">
      <fraction n="0.9" ref="U235"/>
      <fraction n="0.1" ref="U238"/>
    </element>
    <material name="Air &amp; Stuff" state="gas">
      <MEE value="85.7" unit="eV"/>
      <D value="0.0012" unit="g/cm3"/>
      <fraction n="1.0" ref="enriched_U"/>
    </material>
  </materials>
  <solids>
    <box name="WorldBox" x="100" y="100" z="100" lunit="mm"/>
    <opticalsurface name="mirror" model="glisur" finish="polished" type="dielectric_metal" value="1.0">
      <property name="REFLECTIVITY" ref="refl"/>
    </opticalsurface>
  </solids>
  <structure>
    <volume name="World"><materialref ref="Air &amp; Stuff"/><solidref ref="WorldBox"/></volume>
  </structure>
  <setup name="Default" version="1.0"><world ref="World"/></setup>
</gdml>"#;

    let assert_doc = |doc: &gdml_studio_backend::gdml::model::GdmlDocument, ctx: &str| {
        assert_eq!(doc.materials.isotopes.len(), 2, "{}: isotopes", ctx);
        let enriched = doc
            .materials
            .elements
            .iter()
            .find(|e| e.name == "enriched_U")
            .unwrap_or_else(|| panic!("{}: missing enriched_U element", ctx));
        assert_eq!(enriched.fractions.len(), 2, "{}: element fractions", ctx);
        let air = doc
            .materials
            .materials
            .iter()
            .find(|m| m.name == "Air & Stuff") // the "&amp;" must decode to a single "&"
            .unwrap_or_else(|| panic!("{}: missing 'Air & Stuff' material", ctx));
        assert_eq!(air.state.as_deref(), Some("gas"), "{}: state", ctx);
        assert!(air.mee.is_some(), "{}: MEE", ctx);
        assert!(
            doc.raw_unknown.iter().any(|r| r.tag == "opticalsurface"),
            "{}: <opticalsurface> not preserved",
            ctx
        );
    };

    let doc = parse_gdml_from_bytes(src.as_bytes(), "t.gdml".to_string()).unwrap();
    assert_doc(&doc, "first parse");

    // Serialize, then re-parse: every field must survive the round-trip.
    let xml = serialize_gdml(&doc).unwrap();
    assert!(
        !xml.contains("&amp;amp;"),
        "attribute double-escaping detected on save:\n{}",
        xml
    );
    let doc2 = parse_gdml_from_bytes(xml.as_bytes(), "t2.gdml".to_string()).unwrap();
    assert_doc(&doc2, "round-trip");
}

#[test]
fn test_segments_extremes_produce_finite_bounded_meshes() {
    // `segments` is request-controlled. 0 used to divide by zero (NaN geometry) in
    // several primitives, and a huge value would blow up memory. The central clamp
    // in tessellate_all_solids must make both cases finite and bounded.
    let path = project_root().join("sample_data/solids.gdml"); // widest variety of solids
    let doc = parse_gdml(&path).unwrap();
    let mut engine = EvalEngine::new();
    engine.evaluate_all(&doc.defines).unwrap();

    for &segments in &[0u32, 1, 2, u32::MAX] {
        let (meshes, _warnings) = tessellate_all_solids(&doc.solids, &engine, segments)
            .unwrap_or_else(|e| panic!("tessellation failed for segments={}: {}", segments, e));

        assert!(!meshes.is_empty(), "segments={}: expected meshes", segments);

        let mut total_tris = 0usize;
        for (name, mesh) in &meshes {
            for &v in mesh.positions.iter().chain(mesh.normals.iter()) {
                assert!(
                    v.is_finite(),
                    "segments={}: solid '{}' produced a non-finite value {}",
                    segments,
                    name,
                    v
                );
            }
            total_tris += mesh.triangle_count();
        }
        // With the clamp (max 512), even u32::MAX cannot explode the vertex count.
        assert!(
            total_tris < 10_000_000,
            "segments={}: triangle count {} not bounded — clamp failed",
            segments,
            total_tris
        );
    }
}

#[test]
fn test_mesh_geometry_validity() {
    // Verify that every mesh has valid geometry: positions divisible by 3,
    // normals divisible by 3, indices divisible by 3, and index values in range.
    let root = project_root();
    let files = [
        "sample_data/BgoDetModel_v2_00.gdml",
        "sample_data/fermi_simple_elements_satellite.gdml",
        "sample_data/NaiDetModelWithMLI_v2_00.gdml",
        "sample_data/test_all_features.gdml",
        "sample_data/pod_asm.gdml",
        "sample_data/pod_asm_tessellated.gdml",
        "sample_data/solids.gdml",
    ];

    for file in &files {
        let path = root.join(file);
        let doc = parse_gdml(&path).unwrap();
        let mut engine = EvalEngine::new();
        engine.evaluate_all(&doc.defines).unwrap();
        let (meshes, _warnings) =
            tessellate_all_solids(&doc.solids, &engine, DEFAULT_MESH_SEGMENTS).unwrap();

        for (solid_name, mesh) in &meshes {
            assert_eq!(
                mesh.positions.len() % 3,
                0,
                "{}: solid '{}' positions length {} not divisible by 3",
                file,
                solid_name,
                mesh.positions.len()
            );
            assert_eq!(
                mesh.normals.len() % 3,
                0,
                "{}: solid '{}' normals length {} not divisible by 3",
                file,
                solid_name,
                mesh.normals.len()
            );
            assert_eq!(
                mesh.indices.len() % 3,
                0,
                "{}: solid '{}' indices length {} not divisible by 3",
                file,
                solid_name,
                mesh.indices.len()
            );

            let vertex_count = mesh.positions.len() / 3;
            for &idx in &mesh.indices {
                assert!(
                    (idx as usize) < vertex_count,
                    "{}: solid '{}' has out-of-range index {} (vertex_count={})",
                    file,
                    solid_name,
                    idx,
                    vertex_count
                );
            }

            // Normals and positions should have the same length
            assert_eq!(
                mesh.positions.len(),
                mesh.normals.len(),
                "{}: solid '{}' positions/normals length mismatch ({} vs {})",
                file,
                solid_name,
                mesh.positions.len(),
                mesh.normals.len()
            );

            // No NaN or Inf in positions or normals
            for &v in &mesh.positions {
                assert!(
                    v.is_finite(),
                    "{}: solid '{}' has non-finite position value {}",
                    file,
                    solid_name,
                    v
                );
            }
            for &v in &mesh.normals {
                assert!(
                    v.is_finite(),
                    "{}: solid '{}' has non-finite normal value {}",
                    file,
                    solid_name,
                    v
                );
            }
        }
    }
}

// ─── Raw-element preservation and sectioned export ───────────────────────────

#[test]
fn raw_elements_round_trip_into_correct_sections() {
    use gdml_studio_backend::gdml::materials::serialize_gdml;

    let gdml = r#"<?xml version="1.0" encoding="UTF-8"?>
<gdml>
  <define>
    <constant name="c1" value="1"/>
    <matrix name="m1" coldim="2" values="1 2 3 4"/>
  </define>
  <materials>
    <material name="Vac" Z="1"><D value="1e-25" unit="g/cm3"/><atom value="1"/></material>
  </materials>
  <solids>
    <box name="world_box" x="100" y="100" z="100"/>
    <opticalsurface name="os1" model="glisur" finish="polished" type="dielectric_dielectric" value="1"/>
  </solids>
  <structure>
    <assembly name="asm1">
      <physvol><volumeref ref="WorldLV"/></physvol>
    </assembly>
    <volume name="WorldLV">
      <materialref ref="Vac"/>
      <solidref ref="world_box"/>
    </volume>
    <skinsurface name="ss1" surfaceproperty="os1"><volumeref ref="WorldLV"/></skinsurface>
  </structure>
  <setup name="Default" version="1.0"><world ref="WorldLV"/></setup>
</gdml>"#;

    let doc = parse_gdml_from_bytes(gdml.as_bytes(), "t.gdml".to_string()).unwrap();
    let tags: Vec<&str> = doc.raw_unknown.iter().map(|r| r.tag.as_str()).collect();
    for expected in ["matrix", "opticalsurface", "assembly", "skinsurface"] {
        assert!(
            tags.contains(&expected),
            "expected '{}' preserved, got {:?}",
            expected,
            tags
        );
    }

    let xml = serialize_gdml(&doc).unwrap();

    // Each preserved element must be re-emitted INSIDE its proper section so
    // the exported file stays schema-valid.
    let in_section = |needle: &str, open: &str, close: &str| {
        let s = xml.find(open).unwrap_or_else(|| panic!("missing {open}"));
        let e = xml.find(close).unwrap_or_else(|| panic!("missing {close}"));
        match xml.find(needle) {
            Some(p) => p > s && p < e,
            None => false,
        }
    };
    assert!(
        in_section("<matrix", "<define>", "</define>"),
        "matrix not inside <define>:\n{xml}"
    );
    assert!(
        in_section("<opticalsurface", "<solids>", "</solids>"),
        "opticalsurface not inside <solids>:\n{xml}"
    );
    assert!(
        in_section("<assembly", "<structure>", "</structure>"),
        "assembly not inside <structure>:\n{xml}"
    );
    assert!(
        in_section("<skinsurface", "<structure>", "</structure>"),
        "skinsurface not inside <structure>:\n{xml}"
    );

    // Re-parsing the export preserves the same elements again (stable round-trip).
    let doc2 = parse_gdml_from_bytes(xml.as_bytes(), "t2.gdml".to_string()).unwrap();
    assert_eq!(doc2.raw_unknown.len(), doc.raw_unknown.len());
}

#[test]
fn unsupported_volume_constructs_produce_warnings() {
    let gdml = r#"<?xml version="1.0" encoding="UTF-8"?>
<gdml>
  <materials>
    <material name="Vac" Z="1"><D value="1e-25" unit="g/cm3"/><atom value="1"/></material>
  </materials>
  <solids>
    <box name="world_box" x="100" y="100" z="100"/>
    <box name="slice_box" x="10" y="100" z="100"/>
  </solids>
  <structure>
    <volume name="SliceLV">
      <materialref ref="Vac"/>
      <solidref ref="slice_box"/>
    </volume>
    <volume name="WorldLV">
      <materialref ref="Vac"/>
      <solidref ref="world_box"/>
      <divisionvol axis="kXAxis" number="10" width="10" unit="mm">
        <volumeref ref="SliceLV"/>
      </divisionvol>
      <physvol>
        <volumeref ref="SliceLV"/>
        <scale name="mirror" x="-1" y="1" z="1"/>
      </physvol>
    </volume>
  </structure>
  <setup name="Default" version="1.0"><world ref="WorldLV"/></setup>
</gdml>"#;

    let doc = parse_gdml_from_bytes(gdml.as_bytes(), "t.gdml".to_string()).unwrap();
    assert!(
        doc.skipped_unsupported
            .iter()
            .any(|w| w.contains("divisionvol")),
        "expected divisionvol warning, got {:?}",
        doc.skipped_unsupported
    );
    assert!(
        doc.skipped_unsupported.iter().any(|w| w.contains("scale")),
        "expected physvol scale warning, got {:?}",
        doc.skipped_unsupported
    );
    // The divisionvol's volumeref must not leak into the volume as a solidref
    // or physvol entry.
    let world = doc
        .structure
        .volumes
        .iter()
        .find(|v| v.name == "WorldLV")
        .unwrap();
    assert_eq!(world.physvols.len(), 1);
    assert_eq!(world.solid_ref, "world_box");
}

/// The named mesh of a `<solids>` fragment.
fn mesh_from_solids(solids: &str, name: &str) -> TriangleMesh {
    let (mut meshes, _) = common::tessellate(solids, 128);
    meshes
        .remove(name)
        .unwrap_or_else(|| panic!("no mesh for '{name}'"))
}

#[test]
fn oversized_deltaphi_sweeps_a_full_circle() {
    // G4Tubs takes a deltaphi of a full turn or more as the full turn; one
    // that is not positive it refuses (parameters_geant4_rejects_are_reported).
    let expected = std::f64::consts::PI * 100.0 * 20.0;
    for delta in ["7.0", "6.283185307179586"] {
        let xml = format!(
            r#"    <tube name="T" rmin="0" rmax="10" z="20" deltaphi="{delta}" aunit="rad" lunit="mm"/>"#
        );
        let mesh = mesh_from_solids(&xml, "T");
        let vol = signed_volume_about(&mesh, [0.0, 0.0, 0.0]);
        let rel = (vol - expected).abs() / expected.abs();
        assert!(
            rel < 0.02,
            "deltaphi={delta}: volume {vol:.3}, expected {expected:.3} (rel {rel:.4})"
        );
    }
}

#[test]
fn twistedtubs_mid_radius_form_is_supported() {
    // The zlen == 0 branch: midinnerrad/midouterrad with explicit, and
    // deliberately asymmetric, z bounds. This form used to be rejected
    // outright, so the solid vanished from the scene.
    let xml = r#"    <twistedtubs name="T" twistedangle="0" midinnerrad="0" midouterrad="10"
                negativeEndz="-5" positiveEndz="15" phi="6.283185307179586"
                aunit="rad" lunit="mm"/>"#;
    let mesh = mesh_from_solids(xml, "T");
    assert!(
        mesh.triangle_count() > 0,
        "mid-radius form produced no mesh"
    );

    let expected = std::f64::consts::PI * 100.0 * 20.0;
    let vol = signed_volume_about(&mesh, [0.0, 0.0, 0.0]);
    let rel = (vol - expected).abs() / expected;
    assert!(
        rel < 0.02,
        "volume {vol:.2}, expected {expected:.2} (rel {rel:.4})"
    );

    // Bounds must be honoured as given, not recentred on the origin.
    let zs: Vec<f64> = (0..mesh.positions.len() / 3)
        .map(|i| mesh.positions[i * 3 + 2] as f64)
        .collect();
    let zmin = zs.iter().cloned().fold(f64::INFINITY, f64::min);
    let zmax = zs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    assert!((zmin + 5.0).abs() < 0.01, "zmin {zmin:.3}, expected -5");
    assert!((zmax - 15.0).abs() < 0.01, "zmax {zmax:.3}, expected 15");
}

#[test]
fn twistedtubs_nseg_and_totphi_set_the_sweep() {
    // fDPhi = totphi / nseg (G4TwistedTubs.cc:129).
    let xml = r#"    <twistedtubs name="T" twistedangle="0" endinnerrad="0" endouterrad="10"
                zlen="10" nseg="4" totphi="6.283185307179586" aunit="rad" lunit="mm"/>"#;
    let mesh = mesh_from_solids(xml, "T");
    // A quarter of the full cylinder; zlen is the full length.
    let expected = std::f64::consts::PI * 100.0 * 10.0 / 4.0;
    let vol = signed_volume_about(&mesh, [0.0, 0.0, 0.0]);
    let rel = (vol - expected).abs() / expected;
    assert!(
        rel < 0.02,
        "volume {vol:.2}, expected {expected:.2} (rel {rel:.4})"
    );
}
