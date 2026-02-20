use axum::extract::State;
use axum::response::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

use super::errors::ApiError;
use crate::config;
use crate::eval::engine::EvalEngine;
use crate::gdml::model::*;
use crate::gdml::parser;
use crate::mesh::tessellator;
use crate::state::app_state::{LoadedDocument, SharedState};

#[derive(Deserialize)]
pub struct UploadFileRequest {
    pub filename: String,
    pub content: String,
    pub segments: Option<u32>,
}

#[derive(Deserialize)]
pub struct UploadFilesRequest {
    pub files: HashMap<String, String>,
    pub main_file: String,
    pub segments: Option<u32>,
}

/// Merge a child GdmlDocument into the main document, resolving file_ref physvols.
fn merge_child_into_main(
    main_doc: &mut GdmlDocument,
    child_doc: &GdmlDocument,
    file_ref_name: &str,
    volname: &Option<String>,
    warnings: &mut Vec<String>,
) {
    // Determine the child's target volume (volname or its world_ref)
    let child_world = volname
        .as_deref()
        .unwrap_or(&child_doc.setup.world_ref)
        .to_string();

    if child_world.is_empty() {
        warnings.push(format!(
            "Child file '{}' has no world reference and no volname specified",
            file_ref_name
        ));
        return;
    }

    // Collect existing names to detect duplicates
    let existing_solids: HashSet<String> = main_doc
        .solids
        .solids
        .iter()
        .map(|s| s.name().to_string())
        .collect();
    let existing_materials: HashSet<String> = main_doc
        .materials
        .materials
        .iter()
        .map(|m| m.name.clone())
        .collect();
    let existing_elements: HashSet<String> = main_doc
        .materials
        .elements
        .iter()
        .map(|e| e.name.clone())
        .collect();
    let existing_volumes: HashSet<String> = main_doc
        .structure
        .volumes
        .iter()
        .map(|v| v.name.clone())
        .collect();

    // Merge defines (constants, quantities, variables, expressions, positions, rotations)
    main_doc.defines.constants.extend(child_doc.defines.constants.clone());
    main_doc.defines.quantities.extend(child_doc.defines.quantities.clone());
    main_doc.defines.variables.extend(child_doc.defines.variables.clone());
    main_doc.defines.expressions.extend(child_doc.defines.expressions.clone());
    main_doc.defines.positions.extend(child_doc.defines.positions.clone());
    main_doc.defines.rotations.extend(child_doc.defines.rotations.clone());

    // Merge elements (skip duplicates)
    for elem in &child_doc.materials.elements {
        if !existing_elements.contains(&elem.name) {
            main_doc.materials.elements.push(elem.clone());
        }
    }

    // Merge materials (skip duplicates)
    for mat in &child_doc.materials.materials {
        if !existing_materials.contains(&mat.name) {
            main_doc.materials.materials.push(mat.clone());
        }
    }

    // Merge solids (skip duplicates)
    for solid in &child_doc.solids.solids {
        if !existing_solids.contains(solid.name()) {
            main_doc.solids.solids.push(solid.clone());
        }
    }

    // Merge volumes (skip duplicates)
    for vol in &child_doc.structure.volumes {
        if !existing_volumes.contains(&vol.name) {
            main_doc.structure.volumes.push(vol.clone());
        }
    }

    // Now resolve file_ref physvols: replace file_ref with volume_ref pointing to child_world
    for vol in &mut main_doc.structure.volumes {
        for pv in &mut vol.physvols {
            if let Some(ref fref) = pv.file_ref {
                if fref.name == file_ref_name {
                    pv.volume_ref = child_world.clone();
                    pv.file_ref = None;
                }
            }
        }
    }
}

/// Collect all file references from a parsed document.
fn collect_file_refs(doc: &GdmlDocument) -> Vec<(String, Option<String>)> {
    let mut refs = Vec::new();
    for vol in &doc.structure.volumes {
        for pv in &vol.physvols {
            if let Some(ref fref) = pv.file_ref {
                refs.push((fref.name.clone(), fref.volname.clone()));
            }
        }
    }
    refs
}

pub async fn upload_file(
    State(state): State<SharedState>,
    Json(req): Json<UploadFileRequest>,
) -> Result<Json<Value>, ApiError> {
    if !req.filename.ends_with(".gdml") {
        return Err(ApiError::bad_request("Only .gdml files are supported"));
    }

    // Parse GDML from uploaded content
    let doc = parser::parse_gdml_from_bytes(req.content.as_bytes(), req.filename.clone())
        .map_err(|e| ApiError::bad_request(&format!("Parse error: {}", e)))?;

    // Check for unresolved file references
    let file_refs = collect_file_refs(&doc);
    let mut extra_warnings = Vec::new();
    if !file_refs.is_empty() {
        let names: Vec<_> = file_refs.iter().map(|(n, _)| n.as_str()).collect();
        extra_warnings.push(format!(
            "File contains references to external files that were not provided: {}. \
             Select all GDML files together to resolve these references.",
            names.join(", ")
        ));
    }

    // Evaluate expressions
    let mut engine = EvalEngine::new();
    engine
        .evaluate_all(&doc.defines)
        .map_err(|e| ApiError::internal(&format!("Expression evaluation error: {}", e)))?;

    // Tessellate solids
    let segments = req.segments.unwrap_or_else(config::mesh_segments);
    let (meshes, mut warnings) = tessellator::tessellate_all_solids(&doc.solids, &engine, segments)
        .map_err(|e| ApiError::internal(&format!("Tessellation error: {}", e)))?;
    warnings.extend(extra_warnings);

    let summary = json!({
        "filename": doc.filename,
        "defines_count": doc.defines.constants.len() + doc.defines.quantities.len()
            + doc.defines.variables.len() + doc.defines.expressions.len(),
        "positions_count": doc.defines.positions.len(),
        "rotations_count": doc.defines.rotations.len(),
        "materials_count": doc.materials.materials.len(),
        "elements_count": doc.materials.elements.len(),
        "solids_count": doc.solids.solids.len(),
        "volumes_count": doc.structure.volumes.len(),
        "meshes_count": meshes.len(),
        "world_ref": doc.setup.world_ref,
        "warnings": warnings,
    });

    let mut state_w = state.write().await;
    state_w.loaded = Some(LoadedDocument {
        document: doc,
        engine,
        meshes,
        warnings,
        file_path: req.filename,
    });

    Ok(Json(summary))
}

pub async fn upload_files(
    State(state): State<SharedState>,
    Json(req): Json<UploadFilesRequest>,
) -> Result<Json<Value>, ApiError> {
    if !req.main_file.ends_with(".gdml") {
        return Err(ApiError::bad_request("Only .gdml files are supported"));
    }

    let main_content = req
        .files
        .get(&req.main_file)
        .ok_or_else(|| ApiError::bad_request("Main file not found in uploaded files"))?;

    // Parse the main file
    let mut main_doc =
        parser::parse_gdml_from_bytes(main_content.as_bytes(), req.main_file.clone())
            .map_err(|e| ApiError::bad_request(&format!("Parse error in {}: {}", req.main_file, e)))?;

    // Parse all other files into a lookup map
    let mut child_docs: HashMap<String, GdmlDocument> = HashMap::new();
    for (name, content) in &req.files {
        if name != &req.main_file {
            match parser::parse_gdml_from_bytes(content.as_bytes(), name.clone()) {
                Ok(doc) => {
                    child_docs.insert(name.clone(), doc);
                }
                Err(e) => {
                    return Err(ApiError::bad_request(&format!(
                        "Parse error in {}: {}",
                        name, e
                    )));
                }
            }
        }
    }

    // Resolve file references: merge child documents into main
    let mut merge_warnings = Vec::new();
    let file_refs = collect_file_refs(&main_doc);
    for (ref_name, volname) in &file_refs {
        if let Some(child_doc) = child_docs.get(ref_name) {
            merge_child_into_main(&mut main_doc, child_doc, ref_name, volname, &mut merge_warnings);
        } else {
            merge_warnings.push(format!(
                "Referenced file '{}' was not provided in the upload",
                ref_name
            ));
        }
    }

    // Evaluate expressions on the merged document
    let mut engine = EvalEngine::new();
    engine
        .evaluate_all(&main_doc.defines)
        .map_err(|e| ApiError::internal(&format!("Expression evaluation error: {}", e)))?;

    // Tessellate solids
    let segments = req.segments.unwrap_or_else(config::mesh_segments);
    let (meshes, mut warnings) =
        tessellator::tessellate_all_solids(&main_doc.solids, &engine, segments)
            .map_err(|e| ApiError::internal(&format!("Tessellation error: {}", e)))?;
    warnings.extend(merge_warnings);

    let summary = json!({
        "filename": main_doc.filename,
        "defines_count": main_doc.defines.constants.len() + main_doc.defines.quantities.len()
            + main_doc.defines.variables.len() + main_doc.defines.expressions.len(),
        "positions_count": main_doc.defines.positions.len(),
        "rotations_count": main_doc.defines.rotations.len(),
        "materials_count": main_doc.materials.materials.len(),
        "elements_count": main_doc.materials.elements.len(),
        "solids_count": main_doc.solids.solids.len(),
        "volumes_count": main_doc.structure.volumes.len(),
        "meshes_count": meshes.len(),
        "world_ref": main_doc.setup.world_ref,
        "warnings": warnings,
    });

    let mut state_w = state.write().await;
    state_w.loaded = Some(LoadedDocument {
        document: main_doc,
        engine,
        meshes,
        warnings,
        file_path: req.main_file,
    });

    Ok(Json(summary))
}

pub async fn get_summary(
    State(state): State<SharedState>,
) -> Result<Json<Value>, ApiError> {
    let state_r = state.read().await;
    let loaded = state_r
        .loaded
        .as_ref()
        .ok_or_else(|| ApiError::not_found("No document loaded"))?;

    let doc = &loaded.document;
    Ok(Json(json!({
        "filename": doc.filename,
        "defines_count": doc.defines.constants.len() + doc.defines.quantities.len()
            + doc.defines.variables.len() + doc.defines.expressions.len(),
        "positions_count": doc.defines.positions.len(),
        "rotations_count": doc.defines.rotations.len(),
        "materials_count": doc.materials.materials.len(),
        "elements_count": doc.materials.elements.len(),
        "solids_count": doc.solids.solids.len(),
        "volumes_count": doc.structure.volumes.len(),
        "meshes_count": loaded.meshes.len(),
        "world_ref": doc.setup.world_ref,
        "warnings": loaded.warnings,
    })))
}

pub async fn get_meshes(
    State(state): State<SharedState>,
) -> Result<Json<Value>, ApiError> {
    let state_r = state.read().await;
    let loaded = state_r
        .loaded
        .as_ref()
        .ok_or_else(|| ApiError::not_found("No document loaded"))?;

    let doc = &loaded.document;
    let engine = &loaded.engine;

    // Build scene graph from the world volume
    let scene_graph = build_scene_graph(doc, engine);

    // Serialize meshes
    let meshes: HashMap<String, MeshData> = loaded
        .meshes
        .iter()
        .map(|(name, mesh)| {
            (
                name.clone(),
                MeshData {
                    positions: mesh.positions.clone(),
                    normals: mesh.normals.clone(),
                    indices: mesh.indices.clone(),
                },
            )
        })
        .collect();

    Ok(Json(json!({
        "meshes": meshes,
        "scene_graph": scene_graph,
    })))
}

pub async fn get_defines(
    State(state): State<SharedState>,
) -> Result<Json<Value>, ApiError> {
    let state_r = state.read().await;
    let loaded = state_r
        .loaded
        .as_ref()
        .ok_or_else(|| ApiError::not_found("No document loaded"))?;

    let doc = &loaded.document;
    let engine = &loaded.engine;

    #[derive(Serialize)]
    struct DefineValue {
        name: String,
        expression: String,
        evaluated: Option<f64>,
        unit: Option<String>,
        kind: String,
    }

    let mut defines: Vec<DefineValue> = Vec::new();

    for c in &doc.defines.constants {
        defines.push(DefineValue {
            name: c.name.clone(),
            expression: c.value.clone(),
            evaluated: engine.context.get(&c.name),
            unit: None,
            kind: "constant".to_string(),
        });
    }
    for q in &doc.defines.quantities {
        defines.push(DefineValue {
            name: q.name.clone(),
            expression: q.value.clone(),
            evaluated: engine.context.get(&q.name),
            unit: q.unit.clone(),
            kind: "quantity".to_string(),
        });
    }
    for v in &doc.defines.variables {
        defines.push(DefineValue {
            name: v.name.clone(),
            expression: v.value.clone(),
            evaluated: engine.context.get(&v.name),
            unit: None,
            kind: "variable".to_string(),
        });
    }
    for e in &doc.defines.expressions {
        defines.push(DefineValue {
            name: e.name.clone(),
            expression: e.value.clone(),
            evaluated: engine.context.get(&e.name),
            unit: None,
            kind: "expression".to_string(),
        });
    }

    Ok(Json(json!({ "defines": defines })))
}

pub async fn get_materials(
    State(state): State<SharedState>,
) -> Result<Json<Value>, ApiError> {
    let state_r = state.read().await;
    let loaded = state_r
        .loaded
        .as_ref()
        .ok_or_else(|| ApiError::not_found("No document loaded"))?;

    Ok(Json(json!({
        "elements": loaded.document.materials.elements,
        "materials": loaded.document.materials.materials,
    })))
}

pub async fn get_solids(
    State(state): State<SharedState>,
) -> Result<Json<Value>, ApiError> {
    let state_r = state.read().await;
    let loaded = state_r
        .loaded
        .as_ref()
        .ok_or_else(|| ApiError::not_found("No document loaded"))?;

    Ok(Json(json!({
        "solids": loaded.document.solids.solids,
    })))
}

pub async fn get_structure(
    State(state): State<SharedState>,
) -> Result<Json<Value>, ApiError> {
    let state_r = state.read().await;
    let loaded = state_r
        .loaded
        .as_ref()
        .ok_or_else(|| ApiError::not_found("No document loaded"))?;

    Ok(Json(json!({
        "volumes": loaded.document.structure.volumes,
        "world_ref": loaded.document.setup.world_ref,
    })))
}

// ─── Scene graph builder ─────────────────────────────────────────────────────

fn build_scene_graph(doc: &GdmlDocument, engine: &EvalEngine) -> SceneNode {
    let world_ref = &doc.setup.world_ref;
    let vol_map: HashMap<&str, &Volume> = doc
        .structure
        .volumes
        .iter()
        .map(|v| (v.name.as_str(), v))
        .collect();

    // Build material name → density (g/cm³) lookup
    let density_map: HashMap<&str, f64> = doc
        .materials
        .materials
        .iter()
        .filter_map(|m| {
            let d = m.density.as_ref()?;
            let val = d.value.parse::<f64>().ok()?;
            // Convert to g/cm³ if unit is specified
            let density = match d.unit.as_deref() {
                Some("kg/m3") | Some("kg/m³") => val / 1000.0,
                Some("mg/cm3") | Some("mg/cm³") => val / 1000.0,
                _ => val, // default g/cm³
            };
            Some((m.name.as_str(), density))
        })
        .collect();

    let mut visited = HashSet::new();

    if let Some(world_vol) = vol_map.get(world_ref.as_str()) {
        build_volume_node(world_vol, &vol_map, &density_map, engine, [0.0; 3], [0.0; 3], true, &mut visited)
    } else {
        SceneNode {
            name: "World".to_string(),
            volume_name: world_ref.clone(),
            solid_name: String::new(),
            material_name: String::new(),
            color: None,
            density: None,
            position: [0.0; 3],
            rotation: [0.0; 3],
            is_world: true,
            children: Vec::new(),
        }
    }
}

fn build_volume_node(
    vol: &Volume,
    vol_map: &HashMap<&str, &Volume>,
    density_map: &HashMap<&str, f64>,
    engine: &EvalEngine,
    position: [f64; 3],
    rotation: [f64; 3],
    is_world: bool,
    visited: &mut HashSet<String>,
) -> SceneNode {
    visited.insert(vol.name.clone());

    let color = vol
        .auxiliaries
        .iter()
        .find(|a| a.auxtype == "color")
        .map(|a| a.auxvalue.clone());

    let density = density_map.get(vol.material_ref.as_str()).copied();

    let children: Vec<SceneNode> = vol
        .physvols
        .iter()
        .filter_map(|pv| {
            if visited.contains(&pv.volume_ref) {
                tracing::warn!(
                    "Cycle detected in scene graph: volume '{}' references already-visited '{}'",
                    vol.name,
                    pv.volume_ref
                );
                return None;
            }

            let child_vol = vol_map.get(pv.volume_ref.as_str())?;

            let pos = resolve_placement_pos(&pv.position, engine);
            let rot = resolve_placement_rot(&pv.rotation, engine);

            Some(build_volume_node(child_vol, vol_map, density_map, engine, pos, rot, false, visited))
        })
        .collect();

    visited.remove(&vol.name);

    SceneNode {
        name: vol.name.clone(),
        volume_name: vol.name.clone(),
        solid_name: vol.solid_ref.clone(),
        material_name: vol.material_ref.clone(),
        color,
        density,
        position,
        rotation,
        is_world,
        children,
    }
}

fn resolve_placement_pos(pos: &Option<PlacementPos>, engine: &EvalEngine) -> [f64; 3] {
    match pos {
        Some(PlacementPos::Inline(p)) => {
            let x = p
                .x
                .as_ref()
                .map(|v| engine.resolve_value(v))
                .unwrap_or(0.0);
            let y = p
                .y
                .as_ref()
                .map(|v| engine.resolve_value(v))
                .unwrap_or(0.0);
            let z = p
                .z
                .as_ref()
                .map(|v| engine.resolve_value(v))
                .unwrap_or(0.0);
            let unit = p.unit.as_deref().unwrap_or("mm");
            [
                crate::gdml::units::length_to_mm(x, unit),
                crate::gdml::units::length_to_mm(y, unit),
                crate::gdml::units::length_to_mm(z, unit),
            ]
        }
        Some(PlacementPos::Ref(name)) => engine
            .position_values
            .get(name)
            .copied()
            .unwrap_or([0.0; 3]),
        None => [0.0; 3],
    }
}

fn resolve_placement_rot(rot: &Option<PlacementRot>, engine: &EvalEngine) -> [f64; 3] {
    match rot {
        Some(PlacementRot::Inline(r)) => {
            let x = r
                .x
                .as_ref()
                .map(|v| engine.resolve_value(v))
                .unwrap_or(0.0);
            let y = r
                .y
                .as_ref()
                .map(|v| engine.resolve_value(v))
                .unwrap_or(0.0);
            let z = r
                .z
                .as_ref()
                .map(|v| engine.resolve_value(v))
                .unwrap_or(0.0);
            let unit = r.unit.as_deref().unwrap_or("rad");
            [
                crate::gdml::units::angle_to_rad(x, unit),
                crate::gdml::units::angle_to_rad(y, unit),
                crate::gdml::units::angle_to_rad(z, unit),
            ]
        }
        Some(PlacementRot::Ref(name)) => engine
            .rotation_values
            .get(name)
            .copied()
            .unwrap_or([0.0; 3]),
        None => [0.0; 3],
    }
}
