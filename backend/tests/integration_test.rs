//! Integration tests for the GDML Studio backend pipeline.
//!
//! Exercises the full parse -> evaluate -> tessellate pipeline on each of the
//! three sample GDML files shipped in the project root.

use std::path::Path;

use gdml_studio_backend::config::DEFAULT_MESH_SEGMENTS;
use gdml_studio_backend::eval::engine::EvalEngine;
use gdml_studio_backend::gdml::parser::parse_gdml;
use gdml_studio_backend::mesh::tessellator::tessellate_all_solids;

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
    num_positions: usize,
    num_rotations: usize,
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

    // 2. Evaluate all defines / expressions
    let mut engine = EvalEngine::new();
    engine
        .evaluate_all(&doc.defines)
        .unwrap_or_else(|e| panic!("Failed to evaluate defines for {}: {}", gdml_path.display(), e));

    // 3. Tessellate all solids
    let (meshes, _warnings) = tessellate_all_solids(&doc.solids, &engine, DEFAULT_MESH_SEGMENTS)
        .unwrap_or_else(|e| panic!("Failed to tessellate solids for {}: {}", gdml_path.display(), e));

    let total_triangles: usize = meshes.values().map(|m| m.triangle_count()).sum();

    PipelineResult {
        num_constants: doc.defines.constants.len(),
        num_quantities: doc.defines.quantities.len(),
        num_variables: doc.defines.variables.len(),
        num_positions: doc.defines.positions.len(),
        num_rotations: doc.defines.rotations.len(),
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
fn test_bgo_det_model_pipeline() {
    let path = project_root().join("sample_data/BgoDetModel_v2_00.gdml");
    assert!(path.exists(), "GDML file not found: {}", path.display());

    let result = run_pipeline(&path);

    // The file should parse something non-trivial
    println!("BgoDetModel_v2_00.gdml:");
    println!("  constants:  {}", result.num_constants);
    println!("  quantities: {}", result.num_quantities);
    println!("  variables:  {}", result.num_variables);
    println!("  positions:  {}", result.num_positions);
    println!("  rotations:  {}", result.num_rotations);
    println!("  elements:   {}", result.num_elements);
    println!("  materials:  {}", result.num_materials);
    println!("  solids:     {}", result.num_solids);
    println!("  volumes:    {}", result.num_volumes);
    println!("  meshes:     {}", result.num_meshes);
    println!("  triangles:  {}", result.total_triangles);

    assert!(result.num_solids > 0, "Expected at least one solid");
    assert!(result.num_volumes > 0, "Expected at least one volume");
    assert!(result.num_meshes > 0, "Expected at least one mesh");
    assert!(result.total_triangles > 0, "Expected at least one triangle");
    // Every solid should have produced a mesh
    assert_eq!(
        result.num_meshes, result.num_solids,
        "Every solid should be tessellated into a mesh"
    );
}

#[test]
fn test_fermi_satellite_pipeline() {
    let path = project_root().join("sample_data/fermi_simple_elements_satellite.gdml");
    assert!(path.exists(), "GDML file not found: {}", path.display());

    let result = run_pipeline(&path);

    println!("fermi_simple_elements_satellite.gdml:");
    println!("  constants:  {}", result.num_constants);
    println!("  quantities: {}", result.num_quantities);
    println!("  variables:  {}", result.num_variables);
    println!("  positions:  {}", result.num_positions);
    println!("  rotations:  {}", result.num_rotations);
    println!("  elements:   {}", result.num_elements);
    println!("  materials:  {}", result.num_materials);
    println!("  solids:     {}", result.num_solids);
    println!("  volumes:    {}", result.num_volumes);
    println!("  meshes:     {}", result.num_meshes);
    println!("  triangles:  {}", result.total_triangles);

    assert!(result.num_solids > 0, "Expected at least one solid");
    assert!(result.num_volumes > 0, "Expected at least one volume");
    assert!(result.num_meshes > 0, "Expected at least one mesh");
    assert!(result.total_triangles > 0, "Expected at least one triangle");
    assert_eq!(
        result.num_meshes, result.num_solids,
        "Every solid should be tessellated into a mesh"
    );
}

#[test]
fn test_nai_det_model_pipeline() {
    let path = project_root().join("sample_data/NaiDetModelWithMLI_v2_00.gdml");
    assert!(path.exists(), "GDML file not found: {}", path.display());

    let result = run_pipeline(&path);

    println!("NaiDetModelWithMLI_v2_00.gdml:");
    println!("  constants:  {}", result.num_constants);
    println!("  quantities: {}", result.num_quantities);
    println!("  variables:  {}", result.num_variables);
    println!("  positions:  {}", result.num_positions);
    println!("  rotations:  {}", result.num_rotations);
    println!("  elements:   {}", result.num_elements);
    println!("  materials:  {}", result.num_materials);
    println!("  solids:     {}", result.num_solids);
    println!("  volumes:    {}", result.num_volumes);
    println!("  meshes:     {}", result.num_meshes);
    println!("  triangles:  {}", result.total_triangles);

    assert!(result.num_solids > 0, "Expected at least one solid");
    assert!(result.num_volumes > 0, "Expected at least one volume");
    assert!(result.num_meshes > 0, "Expected at least one mesh");
    assert!(result.total_triangles > 0, "Expected at least one triangle");
    assert_eq!(
        result.num_meshes, result.num_solids,
        "Every solid should be tessellated into a mesh"
    );
}

#[test]
fn test_all_features_pipeline() {
    let path = project_root().join("sample_data/test_all_features.gdml");
    assert!(path.exists(), "GDML file not found: {}", path.display());

    let result = run_pipeline(&path);

    println!("test_all_features.gdml:");
    println!("  constants:  {}", result.num_constants);
    println!("  quantities: {}", result.num_quantities);
    println!("  variables:  {}", result.num_variables);
    println!("  positions:  {}", result.num_positions);
    println!("  rotations:  {}", result.num_rotations);
    println!("  elements:   {}", result.num_elements);
    println!("  materials:  {}", result.num_materials);
    println!("  solids:     {}", result.num_solids);
    println!("  volumes:    {}", result.num_volumes);
    println!("  meshes:     {}", result.num_meshes);
    println!("  triangles:  {}", result.total_triangles);

    // Verify expected counts from the file
    assert_eq!(result.num_constants, 2, "Expected 2 constants (HALF_PI, deg)");
    assert_eq!(result.num_quantities, 1, "Expected 1 quantity (thickness)");
    assert_eq!(result.num_variables, 1, "Expected 1 variable (x_offset)");
    assert_eq!(result.num_elements, 4, "Expected 4 elements (H, O, N, Al_el)");
    assert_eq!(result.num_materials, 3, "Expected 3 materials (Aluminium, Water, Air)");
    assert_eq!(result.num_solids, 6, "Expected 6 solids (WorldBox, MyBox, MyTube, MyPartialTube, MyCone, MySphere)");
    assert_eq!(result.num_volumes, 6, "Expected 6 volumes (World + 5 leaf)");
    assert_eq!(result.num_meshes, result.num_solids, "Every solid should be tessellated");
    assert!(result.total_triangles > 0, "Expected triangles");
}

#[test]
fn test_all_files_have_setup_section() {
    let root = project_root();
    let files = [
        "sample_data/BgoDetModel_v2_00.gdml",
        "sample_data/fermi_simple_elements_satellite.gdml",
        "sample_data/NaiDetModelWithMLI_v2_00.gdml",
        "sample_data/test_all_features.gdml",
    ];

    for file in &files {
        let path = root.join(file);
        let doc = parse_gdml(&path)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", file, e));

        assert!(
            !doc.setup.world_ref.is_empty(),
            "{} should have a non-empty world_ref in its setup section",
            file
        );
    }
}

#[test]
fn test_all_files_materials_non_empty() {
    let root = project_root();
    let files = [
        "sample_data/BgoDetModel_v2_00.gdml",
        "sample_data/fermi_simple_elements_satellite.gdml",
        "sample_data/NaiDetModelWithMLI_v2_00.gdml",
        "sample_data/test_all_features.gdml",
    ];

    for file in &files {
        let path = root.join(file);
        let doc = parse_gdml(&path)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", file, e));

        let total = doc.materials.elements.len() + doc.materials.materials.len();
        assert!(
            total > 0,
            "{} should have at least one element or material defined",
            file
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
    ];

    for file in &files {
        let path = root.join(file);
        let doc = parse_gdml(&path).unwrap();
        let mut engine = EvalEngine::new();
        engine.evaluate_all(&doc.defines).unwrap();
        let (meshes, _warnings) = tessellate_all_solids(&doc.solids, &engine, DEFAULT_MESH_SEGMENTS).unwrap();

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
