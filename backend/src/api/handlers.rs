use axum::extract::{Query, State};
use axum::response::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};

use super::errors::ApiError;
use crate::config;
use crate::eval::engine::EvalEngine;
use crate::gdml::materials as nist;
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

fn definitions_equivalent<T: Serialize>(existing: &T, incoming: &T) -> Result<bool, ApiError> {
    let existing = serde_json::to_value(existing)
        .map_err(|e| ApiError::internal(&format!("Failed to compare merged definitions: {}", e)))?;
    let incoming = serde_json::to_value(incoming)
        .map_err(|e| ApiError::internal(&format!("Failed to compare merged definitions: {}", e)))?;
    Ok(existing == incoming)
}

fn merge_named_items<T, F>(
    target: &mut Vec<T>,
    incoming: &[T],
    kind: &str,
    source_name: &str,
    get_name: F,
) -> Result<(), ApiError>
where
    T: Clone + Serialize,
    F: Fn(&T) -> &str + Copy,
{
    for item in incoming {
        let name = get_name(item);
        if let Some(existing) = target.iter().find(|existing| get_name(existing) == name) {
            if !definitions_equivalent(existing, item)? {
                return Err(ApiError::bad_request(&format!(
                    "Conflicting {} '{}' found while merging '{}'",
                    kind, name, source_name
                )));
            }
            continue;
        }
        target.push(item.clone());
    }
    Ok(())
}

/// Merge a child GdmlDocument into the main document, resolving file_ref physvols.
fn merge_child_into_main(
    main_doc: &mut GdmlDocument,
    child_doc: &GdmlDocument,
    file_ref_name: &str,
    volname: &Option<String>,
    warnings: &mut Vec<String>,
) -> Result<(), ApiError> {
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
        return Ok(());
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
    merge_named_items(
        &mut main_doc.defines.constants,
        &child_doc.defines.constants,
        "constant",
        file_ref_name,
        |item| item.name.as_str(),
    )?;
    merge_named_items(
        &mut main_doc.defines.quantities,
        &child_doc.defines.quantities,
        "quantity",
        file_ref_name,
        |item| item.name.as_str(),
    )?;
    merge_named_items(
        &mut main_doc.defines.variables,
        &child_doc.defines.variables,
        "variable",
        file_ref_name,
        |item| item.name.as_str(),
    )?;
    merge_named_items(
        &mut main_doc.defines.expressions,
        &child_doc.defines.expressions,
        "expression",
        file_ref_name,
        |item| item.name.as_str(),
    )?;
    merge_named_items(
        &mut main_doc.defines.positions,
        &child_doc.defines.positions,
        "position",
        file_ref_name,
        |item| item.name.as_str(),
    )?;
    merge_named_items(
        &mut main_doc.defines.rotations,
        &child_doc.defines.rotations,
        "rotation",
        file_ref_name,
        |item| item.name.as_str(),
    )?;

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
                if fref.name == file_ref_name && &fref.volname == volname {
                    pv.volume_ref = child_world.clone();
                    pv.file_ref = None;
                }
            }
        }
    }
    Ok(())
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

/// Resolve all file references recursively in breadth-first order.
/// Newly discovered file_ref nodes from merged children are also processed.
fn resolve_all_file_refs(
    main_doc: &mut GdmlDocument,
    child_docs: &HashMap<String, GdmlDocument>,
) -> Result<Vec<String>, ApiError> {
    let mut warnings = Vec::new();
    let mut pending = VecDeque::new();
    let mut queued: HashSet<(String, Option<String>)> = HashSet::new();
    let mut processed: HashSet<(String, Option<String>)> = HashSet::new();

    for fref in collect_file_refs(main_doc) {
        if queued.insert(fref.clone()) {
            pending.push_back(fref);
        }
    }

    while let Some((ref_name, volname)) = pending.pop_front() {
        let key = (ref_name.clone(), volname.clone());
        queued.remove(&key);
        if !processed.insert(key.clone()) {
            continue;
        }

        if let Some(child_doc) = child_docs.get(&ref_name) {
            merge_child_into_main(main_doc, child_doc, &ref_name, &volname, &mut warnings)?;
        } else {
            warnings.push(format!(
                "Referenced file '{}' was not provided in the upload",
                ref_name
            ));
        }

        // A merge may introduce more unresolved file_ref nodes.
        for discovered in collect_file_refs(main_doc) {
            if processed.contains(&discovered) {
                continue;
            }
            if queued.insert(discovered.clone()) {
                pending.push_back(discovered);
            }
        }
    }

    Ok(warnings)
}

fn ensure_material_ref_exists(doc: &GdmlDocument, candidate: &str) -> Result<(), ApiError> {
    let exists = doc.materials.materials.iter().any(|m| m.name == candidate);
    if !exists {
        return Err(ApiError::bad_request(&format!(
            "Material '{}' does not exist",
            candidate
        )));
    }
    Ok(())
}

fn validate_material_components(
    doc: &GdmlDocument,
    material: &Material,
    excluding_material: Option<&str>,
) -> Result<(), ApiError> {
    for component in &material.components {
        let ref_name = match component {
            MaterialComponent::Fraction { ref_name, .. }
            | MaterialComponent::Composite { ref_name, .. } => ref_name,
        };

        if ref_name == &material.name {
            return Err(ApiError::bad_request(&format!(
                "Material '{}' cannot reference itself in components",
                material.name
            )));
        }

        let element_exists = doc.materials.elements.iter().any(|e| e.name == *ref_name);
        let material_exists = doc
            .materials
            .materials
            .iter()
            .any(|m| m.name == *ref_name && Some(m.name.as_str()) != excluding_material);

        if !element_exists && !material_exists {
            return Err(ApiError::bad_request(&format!(
                "Material '{}' references unknown component '{}'",
                material.name, ref_name
            )));
        }
    }

    Ok(())
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
        parser::parse_gdml_from_bytes(main_content.as_bytes(), req.main_file.clone()).map_err(
            |e| ApiError::bad_request(&format!("Parse error in {}: {}", req.main_file, e)),
        )?;

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

    // Resolve file references: merge child documents into main (including nested refs)
    let merge_warnings = resolve_all_file_refs(&mut main_doc, &child_docs)?;

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

pub async fn get_summary(State(state): State<SharedState>) -> Result<Json<Value>, ApiError> {
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

pub async fn get_meshes(State(state): State<SharedState>) -> Result<Json<Value>, ApiError> {
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

pub async fn get_defines(State(state): State<SharedState>) -> Result<Json<Value>, ApiError> {
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

pub async fn get_materials(State(state): State<SharedState>) -> Result<Json<Value>, ApiError> {
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

pub async fn get_solids(State(state): State<SharedState>) -> Result<Json<Value>, ApiError> {
    let state_r = state.read().await;
    let loaded = state_r
        .loaded
        .as_ref()
        .ok_or_else(|| ApiError::not_found("No document loaded"))?;

    Ok(Json(json!({
        "solids": loaded.document.solids.solids,
    })))
}

pub async fn get_structure(State(state): State<SharedState>) -> Result<Json<Value>, ApiError> {
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
        build_volume_node(
            world_vol,
            &vol_map,
            &density_map,
            engine,
            [0.0; 3],
            [0.0; 3],
            true,
            &mut visited,
            format!("/{}", world_vol.name),
        )
    } else {
        SceneNode {
            name: "World".to_string(),
            instance_id: "/World".to_string(),
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
    instance_id: String,
) -> SceneNode {
    visited.insert(vol.name.clone());

    let color = vol
        .auxiliaries
        .iter()
        .find(|a| a.auxtype == "color")
        .map(|a| a.auxvalue.clone());

    let density = density_map.get(vol.material_ref.as_str()).copied();

    let mut children: Vec<SceneNode> = vol
        .physvols
        .iter()
        .enumerate()
        .filter_map(|(idx, pv)| {
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
            let child_instance_id = match pv.name.as_deref() {
                Some(name) if !name.is_empty() => {
                    format!(
                        "{}/physvol[{}]({}):{}",
                        instance_id, idx, name, pv.volume_ref
                    )
                }
                _ => format!("{}/physvol[{}]:{}", instance_id, idx, pv.volume_ref),
            };

            Some(build_volume_node(
                child_vol,
                vol_map,
                density_map,
                engine,
                pos,
                rot,
                false,
                visited,
                child_instance_id,
            ))
        })
        .collect();

    // Expand replicavol into child nodes
    if let Some(ref replica) = vol.replica {
        if let Some(child_vol) = vol_map.get(replica.volume_ref.as_str()) {
            let number = engine.resolve_value(&replica.number) as usize;
            let width_val = engine.resolve_value(&replica.width);
            let width_unit = replica.width_unit.as_deref().unwrap_or("mm");
            let width_mm = crate::gdml::units::length_to_mm(width_val, width_unit);

            let offset_val = engine.resolve_value(&replica.offset);
            let offset_unit = replica.offset_unit.as_deref().unwrap_or("mm");
            let offset_mm = crate::gdml::units::length_to_mm(offset_val, offset_unit);

            // Determine axis index: x=0, y=1, z=2
            let axis = if replica.direction[0]
                .as_deref()
                .map(|v| engine.resolve_value(v))
                .unwrap_or(0.0)
                .abs()
                > 0.5
            {
                0
            } else if replica.direction[1]
                .as_deref()
                .map(|v| engine.resolve_value(v))
                .unwrap_or(0.0)
                .abs()
                > 0.5
            {
                1
            } else {
                2
            };

            for n in 0..number {
                let mut pos = [0.0_f64; 3];
                pos[axis] = offset_mm + (n as f64) * width_mm;
                let replica_instance_id =
                    format!("{}/replica[{}]:{}", instance_id, n, replica.volume_ref);
                let child_node = build_volume_node(
                    child_vol,
                    vol_map,
                    density_map,
                    engine,
                    pos,
                    [0.0; 3],
                    false,
                    visited,
                    replica_instance_id,
                );
                children.push(child_node);
            }
        }
    }

    visited.remove(&vol.name);

    SceneNode {
        name: vol.name.clone(),
        instance_id,
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
            let x = p.x.as_ref().map(|v| engine.resolve_value(v)).unwrap_or(0.0);
            let y = p.y.as_ref().map(|v| engine.resolve_value(v)).unwrap_or(0.0);
            let z = p.z.as_ref().map(|v| engine.resolve_value(v)).unwrap_or(0.0);
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
            let x = r.x.as_ref().map(|v| engine.resolve_value(v)).unwrap_or(0.0);
            let y = r.y.as_ref().map(|v| engine.resolve_value(v)).unwrap_or(0.0);
            let z = r.z.as_ref().map(|v| engine.resolve_value(v)).unwrap_or(0.0);
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

// ─── NIST Materials ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct NistSearchQuery {
    pub search: Option<String>,
    pub category: Option<String>,
}

pub async fn get_nist_materials(Query(query): Query<NistSearchQuery>) -> Json<Value> {
    let results = nist::search_nist_materials(
        query.search.as_deref().unwrap_or(""),
        query.category.as_deref(),
    );
    Json(json!({ "materials": results }))
}

#[derive(Deserialize)]
pub struct NistMaterialQuery {
    pub name: String,
}

pub async fn get_nist_material(
    Query(query): Query<NistMaterialQuery>,
) -> Result<Json<Value>, ApiError> {
    let mat = nist::find_nist_material(&query.name)
        .ok_or_else(|| ApiError::not_found(&format!("NIST material '{}' not found", query.name)))?;
    Ok(Json(json!({ "material": mat })))
}

// ─── Material CRUD ──────────────────────────────────────────────────────────

fn ensure_material_name_available(
    doc: &GdmlDocument,
    candidate: &str,
    excluding_material: Option<&str>,
) -> Result<(), ApiError> {
    let material_conflict = doc
        .materials
        .materials
        .iter()
        .any(|m| m.name == candidate && Some(m.name.as_str()) != excluding_material);
    if material_conflict {
        return Err(ApiError::bad_request(&format!(
            "Material '{}' already exists",
            candidate
        )));
    }

    let element_conflict = doc.materials.elements.iter().any(|e| e.name == candidate);
    if element_conflict {
        return Err(ApiError::bad_request(&format!(
            "Name '{}' is already used by an element",
            candidate
        )));
    }

    Ok(())
}

fn ensure_element_name_available(
    doc: &GdmlDocument,
    candidate: &str,
    excluding_element: Option<&str>,
) -> Result<(), ApiError> {
    let element_conflict = doc
        .materials
        .elements
        .iter()
        .any(|e| e.name == candidate && Some(e.name.as_str()) != excluding_element);
    if element_conflict {
        return Err(ApiError::bad_request(&format!(
            "Element '{}' already exists",
            candidate
        )));
    }

    let material_conflict = doc.materials.materials.iter().any(|m| m.name == candidate);
    if material_conflict {
        return Err(ApiError::bad_request(&format!(
            "Name '{}' is already used by a material",
            candidate
        )));
    }

    Ok(())
}

fn cascade_component_ref_rename(materials: &mut [Material], old_name: &str, new_name: &str) {
    if old_name == new_name {
        return;
    }
    for mat in materials {
        for component in &mut mat.components {
            match component {
                MaterialComponent::Fraction { ref_name, .. }
                | MaterialComponent::Composite { ref_name, .. } => {
                    if ref_name == old_name {
                        *ref_name = new_name.to_string();
                    }
                }
            }
        }
    }
}

fn is_name_referenced_in_material_components(materials: &[Material], name: &str) -> bool {
    materials.iter().any(|m| {
        m.components.iter().any(|c| match c {
            MaterialComponent::Fraction { ref_name, .. }
            | MaterialComponent::Composite { ref_name, .. } => ref_name == name,
        })
    })
}

#[derive(Deserialize)]
pub struct UpdateMaterialRequest {
    pub name: String,
    pub material: Material,
}

pub async fn update_material(
    State(state): State<SharedState>,
    Json(req): Json<UpdateMaterialRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut state_w = state.write().await;
    let loaded = state_w
        .loaded
        .as_mut()
        .ok_or_else(|| ApiError::not_found("No document loaded"))?;

    let old_name = req.name.clone();
    let new_name = req.material.name.clone();
    ensure_material_name_available(&loaded.document, &new_name, Some(old_name.as_str()))?;
    validate_material_components(&loaded.document, &req.material, Some(old_name.as_str()))?;

    let mat_idx = loaded
        .document
        .materials
        .materials
        .iter()
        .position(|m| m.name == old_name)
        .ok_or_else(|| ApiError::not_found(&format!("Material '{}' not found", req.name)))?;

    loaded.document.materials.materials[mat_idx] = req.material;

    if old_name != new_name {
        // Cascade rename to volumes
        for vol in &mut loaded.document.structure.volumes {
            if vol.material_ref == old_name {
                vol.material_ref = new_name.clone();
            }
        }

        // Cascade rename to material component references.
        cascade_component_ref_rename(
            &mut loaded.document.materials.materials,
            old_name.as_str(),
            new_name.as_str(),
        );
    }

    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct AddMaterialRequest {
    pub material: Material,
}

pub async fn add_material(
    State(state): State<SharedState>,
    Json(req): Json<AddMaterialRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut state_w = state.write().await;
    let loaded = state_w
        .loaded
        .as_mut()
        .ok_or_else(|| ApiError::not_found("No document loaded"))?;

    ensure_material_name_available(&loaded.document, &req.material.name, None)?;
    validate_material_components(&loaded.document, &req.material, None)?;

    loaded.document.materials.materials.push(req.material);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct DeleteMaterialRequest {
    pub name: String,
}

pub async fn delete_material(
    State(state): State<SharedState>,
    Json(req): Json<DeleteMaterialRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut state_w = state.write().await;
    let loaded = state_w
        .loaded
        .as_mut()
        .ok_or_else(|| ApiError::not_found("No document loaded"))?;

    // Check if material is in use by any volume.
    let in_use_by_volume = loaded
        .document
        .structure
        .volumes
        .iter()
        .any(|v| v.material_ref == req.name);

    // Check if material is in use by any material component.
    let in_use_by_component =
        is_name_referenced_in_material_components(&loaded.document.materials.materials, &req.name);

    if in_use_by_volume || in_use_by_component {
        let mut contexts = Vec::new();
        if in_use_by_volume {
            contexts.push("one or more volumes");
        }
        if in_use_by_component {
            contexts.push("one or more material components");
        }
        return Err(ApiError::bad_request(&format!(
            "Material '{}' is still referenced by {}",
            req.name,
            contexts.join(" and ")
        )));
    }

    let before = loaded.document.materials.materials.len();
    loaded
        .document
        .materials
        .materials
        .retain(|m| m.name != req.name);
    if loaded.document.materials.materials.len() == before {
        return Err(ApiError::not_found(&format!(
            "Material '{}' not found",
            req.name
        )));
    }

    Ok(Json(json!({ "ok": true })))
}

// ─── Element CRUD ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UpdateElementRequest {
    pub name: String,
    pub element: Element,
}

pub async fn update_element(
    State(state): State<SharedState>,
    Json(req): Json<UpdateElementRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut state_w = state.write().await;
    let loaded = state_w
        .loaded
        .as_mut()
        .ok_or_else(|| ApiError::not_found("No document loaded"))?;

    let old_name = req.name.clone();
    let new_name = req.element.name.clone();
    ensure_element_name_available(&loaded.document, &new_name, Some(old_name.as_str()))?;

    let el_idx = loaded
        .document
        .materials
        .elements
        .iter()
        .position(|e| e.name == old_name)
        .ok_or_else(|| ApiError::not_found(&format!("Element '{}' not found", req.name)))?;

    loaded.document.materials.elements[el_idx] = req.element;

    if old_name != new_name {
        cascade_component_ref_rename(
            &mut loaded.document.materials.materials,
            old_name.as_str(),
            new_name.as_str(),
        );
    }

    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct AddElementRequest {
    pub element: Element,
}

pub async fn add_element(
    State(state): State<SharedState>,
    Json(req): Json<AddElementRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut state_w = state.write().await;
    let loaded = state_w
        .loaded
        .as_mut()
        .ok_or_else(|| ApiError::not_found("No document loaded"))?;

    ensure_element_name_available(&loaded.document, &req.element.name, None)?;

    loaded.document.materials.elements.push(req.element);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct DeleteElementRequest {
    pub name: String,
}

pub async fn delete_element(
    State(state): State<SharedState>,
    Json(req): Json<DeleteElementRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut state_w = state.write().await;
    let loaded = state_w
        .loaded
        .as_mut()
        .ok_or_else(|| ApiError::not_found("No document loaded"))?;

    // Check if element is referenced by any material component
    let in_use = loaded.document.materials.materials.iter().any(|m| {
        m.components.iter().any(|c| match c {
            MaterialComponent::Fraction { ref_name, .. } => ref_name == &req.name,
            MaterialComponent::Composite { ref_name, .. } => ref_name == &req.name,
        })
    });
    if in_use {
        return Err(ApiError::bad_request(&format!(
            "Element '{}' is still referenced by one or more materials",
            req.name
        )));
    }

    let before = loaded.document.materials.elements.len();
    loaded
        .document
        .materials
        .elements
        .retain(|e| e.name != req.name);
    if loaded.document.materials.elements.len() == before {
        return Err(ApiError::not_found(&format!(
            "Element '{}' not found",
            req.name
        )));
    }

    Ok(Json(json!({ "ok": true })))
}

// ─── Volume material ref ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UpdateMaterialRefRequest {
    pub volume_name: String,
    pub material_ref: String,
}

pub async fn update_volume_material_ref(
    State(state): State<SharedState>,
    Json(req): Json<UpdateMaterialRefRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut state_w = state.write().await;
    let loaded = state_w
        .loaded
        .as_mut()
        .ok_or_else(|| ApiError::not_found("No document loaded"))?;

    ensure_material_ref_exists(&loaded.document, &req.material_ref)?;

    let vol = loaded
        .document
        .structure
        .volumes
        .iter_mut()
        .find(|v| v.name == req.volume_name)
        .ok_or_else(|| ApiError::not_found(&format!("Volume '{}' not found", req.volume_name)))?;

    vol.material_ref = req.material_ref;
    Ok(Json(json!({ "ok": true })))
}

// ─── Export ─────────────────────────────────────────────────────────────────

pub async fn export_gdml(State(state): State<SharedState>) -> Result<Json<Value>, ApiError> {
    let state_r = state.read().await;
    let loaded = state_r
        .loaded
        .as_ref()
        .ok_or_else(|| ApiError::not_found("No document loaded"))?;

    let xml = nist::serialize_gdml(&loaded.document)
        .map_err(|e| ApiError::internal(&format!("Serialization error: {}", e)))?;

    Ok(Json(json!({
        "gdml": xml,
        "filename": loaded.document.filename,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use axum::http::StatusCode;
    use std::collections::HashMap;

    fn base_doc(filename: &str, world_ref: &str) -> GdmlDocument {
        GdmlDocument {
            filename: filename.to_string(),
            defines: DefineSection::default(),
            materials: MaterialSection::default(),
            solids: SolidSection::default(),
            structure: StructureSection::default(),
            setup: SetupSection {
                name: "Default".to_string(),
                version: "1.0".to_string(),
                world_ref: world_ref.to_string(),
            },
        }
    }

    fn material(name: &str) -> Material {
        Material {
            name: name.to_string(),
            formula: None,
            z: None,
            density: None,
            density_ref: None,
            temperature: None,
            pressure: None,
            atom_value: None,
            components: Vec::new(),
        }
    }

    fn volume(name: &str, material_ref: &str) -> Volume {
        Volume {
            name: name.to_string(),
            material_ref: material_ref.to_string(),
            solid_ref: "Solid".to_string(),
            physvols: Vec::new(),
            auxiliaries: Vec::new(),
            replica: None,
        }
    }

    fn file_ref_physvol(file: &str, volname: Option<&str>) -> PhysVol {
        PhysVol {
            name: None,
            volume_ref: String::new(),
            file_ref: Some(FileRef {
                name: file.to_string(),
                volname: volname.map(|s| s.to_string()),
            }),
            position: None,
            rotation: None,
        }
    }

    #[test]
    fn resolves_nested_file_refs_recursively() {
        let mut main = base_doc("main.gdml", "MainWorld");
        let mut main_world = volume("MainWorld", "Vacuum");
        main_world
            .physvols
            .push(file_ref_physvol("child.gdml", None));
        main.structure.volumes.push(main_world);

        let mut child = base_doc("child.gdml", "ChildWorld");
        let mut child_world = volume("ChildWorld", "Vacuum");
        child_world
            .physvols
            .push(file_ref_physvol("grand.gdml", None));
        child.structure.volumes.push(child_world);

        let mut grand = base_doc("grand.gdml", "GrandWorld");
        grand.structure.volumes.push(volume("GrandWorld", "Vacuum"));

        let mut child_docs = HashMap::new();
        child_docs.insert("child.gdml".to_string(), child);
        child_docs.insert("grand.gdml".to_string(), grand);

        let warnings = match resolve_all_file_refs(&mut main, &child_docs) {
            Ok(warnings) => warnings,
            Err(err) => panic!("resolve_all_file_refs should succeed: {}", err.message),
        };
        assert!(warnings.is_empty());
        assert!(collect_file_refs(&main).is_empty());

        let main_world = main
            .structure
            .volumes
            .iter()
            .find(|v| v.name == "MainWorld")
            .unwrap();
        assert_eq!(main_world.physvols[0].volume_ref, "ChildWorld");
        assert!(main_world.physvols[0].file_ref.is_none());

        let child_world = main
            .structure
            .volumes
            .iter()
            .find(|v| v.name == "ChildWorld")
            .unwrap();
        assert_eq!(child_world.physvols[0].volume_ref, "GrandWorld");
        assert!(child_world.physvols[0].file_ref.is_none());
    }

    #[tokio::test]
    async fn material_rename_cascades_to_components_and_volumes() {
        let state = crate::state::app_state::create_shared_state();
        let mut doc = base_doc("test.gdml", "World");
        let mut world = volume("World", "Steel");
        world.physvols.push(PhysVol {
            name: Some("pv_leaf".to_string()),
            volume_ref: "Leaf".to_string(),
            file_ref: None,
            position: None,
            rotation: None,
        });
        doc.structure.volumes.push(world);
        doc.structure.volumes.push(volume("Leaf", "Steel"));

        let steel = material("Steel");
        let mut alloy = material("Alloy");
        alloy.components.push(MaterialComponent::Composite {
            n: "1".to_string(),
            ref_name: "Steel".to_string(),
        });
        doc.materials.materials.push(steel);
        doc.materials.materials.push(alloy);

        {
            let mut w = state.write().await;
            w.loaded = Some(LoadedDocument {
                document: doc,
                engine: EvalEngine::new(),
                meshes: HashMap::new(),
                warnings: Vec::new(),
                file_path: "test.gdml".to_string(),
            });
        }

        let req = UpdateMaterialRequest {
            name: "Steel".to_string(),
            material: material("SteelRenamed"),
        };
        let res = update_material(State(state.clone()), Json(req)).await;
        assert!(res.is_ok(), "update_material should succeed");

        let r = state.read().await;
        let loaded = r.loaded.as_ref().unwrap();
        assert!(loaded
            .document
            .structure
            .volumes
            .iter()
            .all(|v| v.material_ref != "Steel"));
        assert!(loaded
            .document
            .structure
            .volumes
            .iter()
            .any(|v| v.material_ref == "SteelRenamed"));
        assert!(loaded.document.materials.materials.iter().any(|m| {
            m.components.iter().any(|c| match c {
                MaterialComponent::Fraction { ref_name, .. }
                | MaterialComponent::Composite { ref_name, .. } => ref_name == "SteelRenamed",
            })
        }));
    }

    #[tokio::test]
    async fn material_rename_rejects_name_collisions() {
        let state = crate::state::app_state::create_shared_state();
        let mut doc = base_doc("test.gdml", "World");
        doc.structure.volumes.push(volume("World", "A"));
        doc.materials.materials.push(material("A"));
        doc.materials.materials.push(material("B"));

        {
            let mut w = state.write().await;
            w.loaded = Some(LoadedDocument {
                document: doc,
                engine: EvalEngine::new(),
                meshes: HashMap::new(),
                warnings: Vec::new(),
                file_path: "test.gdml".to_string(),
            });
        }

        let req = UpdateMaterialRequest {
            name: "A".to_string(),
            material: material("B"),
        };
        let err = update_material(State(state.clone()), Json(req))
            .await
            .unwrap_err();
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn deleting_material_referenced_by_components_is_blocked() {
        let state = crate::state::app_state::create_shared_state();
        let mut doc = base_doc("test.gdml", "World");
        doc.structure.volumes.push(volume("World", "Alloy"));
        doc.materials.materials.push(material("Steel"));
        let mut alloy = material("Alloy");
        alloy.components.push(MaterialComponent::Fraction {
            n: "1.0".to_string(),
            ref_name: "Steel".to_string(),
        });
        doc.materials.materials.push(alloy);

        {
            let mut w = state.write().await;
            w.loaded = Some(LoadedDocument {
                document: doc,
                engine: EvalEngine::new(),
                meshes: HashMap::new(),
                warnings: Vec::new(),
                file_path: "test.gdml".to_string(),
            });
        }

        let err = delete_material(
            State(state.clone()),
            Json(DeleteMaterialRequest {
                name: "Steel".to_string(),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn add_material_rejects_unknown_component_reference() {
        let state = crate::state::app_state::create_shared_state();
        let mut doc = base_doc("test.gdml", "World");
        doc.structure.volumes.push(volume("World", "Air"));
        doc.materials.materials.push(material("Air"));

        {
            let mut w = state.write().await;
            w.loaded = Some(LoadedDocument {
                document: doc,
                engine: EvalEngine::new(),
                meshes: HashMap::new(),
                warnings: Vec::new(),
                file_path: "test.gdml".to_string(),
            });
        }

        let mut invalid = material("Mixture");
        invalid.components.push(MaterialComponent::Fraction {
            n: "1.0".to_string(),
            ref_name: "MissingRef".to_string(),
        });

        let err = add_material(
            State(state.clone()),
            Json(AddMaterialRequest { material: invalid }),
        )
        .await
        .unwrap_err();
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn update_material_rejects_direct_self_reference() {
        let state = crate::state::app_state::create_shared_state();
        let mut doc = base_doc("test.gdml", "World");
        doc.structure.volumes.push(volume("World", "Steel"));
        doc.materials.materials.push(material("Steel"));

        {
            let mut w = state.write().await;
            w.loaded = Some(LoadedDocument {
                document: doc,
                engine: EvalEngine::new(),
                meshes: HashMap::new(),
                warnings: Vec::new(),
                file_path: "test.gdml".to_string(),
            });
        }

        let mut invalid = material("Steel");
        invalid.components.push(MaterialComponent::Composite {
            n: "1".to_string(),
            ref_name: "Steel".to_string(),
        });

        let err = update_material(
            State(state.clone()),
            Json(UpdateMaterialRequest {
                name: "Steel".to_string(),
                material: invalid,
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn update_volume_material_ref_rejects_unknown_material() {
        let state = crate::state::app_state::create_shared_state();
        let mut doc = base_doc("test.gdml", "World");
        doc.structure.volumes.push(volume("World", "Steel"));
        doc.materials.materials.push(material("Steel"));

        {
            let mut w = state.write().await;
            w.loaded = Some(LoadedDocument {
                document: doc,
                engine: EvalEngine::new(),
                meshes: HashMap::new(),
                warnings: Vec::new(),
                file_path: "test.gdml".to_string(),
            });
        }

        let err = update_volume_material_ref(
            State(state.clone()),
            Json(UpdateMaterialRefRequest {
                volume_name: "World".to_string(),
                material_ref: "Missing".to_string(),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn resolve_all_file_refs_deduplicates_identical_define_names() {
        let mut main = base_doc("main.gdml", "MainWorld");
        let mut main_world = volume("MainWorld", "Vacuum");
        main_world
            .physvols
            .push(file_ref_physvol("child.gdml", None));
        main.structure.volumes.push(main_world);
        main.defines.constants.push(Constant {
            name: "A".to_string(),
            value: "1".to_string(),
        });

        let mut child = base_doc("child.gdml", "ChildWorld");
        child.structure.volumes.push(volume("ChildWorld", "Vacuum"));
        child.defines.constants.push(Constant {
            name: "A".to_string(),
            value: "1".to_string(),
        });

        let mut child_docs = HashMap::new();
        child_docs.insert("child.gdml".to_string(), child);

        let warnings = match resolve_all_file_refs(&mut main, &child_docs) {
            Ok(warnings) => warnings,
            Err(err) => panic!("resolve_all_file_refs should succeed: {}", err.message),
        };
        assert!(warnings.is_empty());
        assert_eq!(main.defines.constants.len(), 1);
    }

    #[test]
    fn resolve_all_file_refs_rejects_conflicting_define_names() {
        let mut main = base_doc("main.gdml", "MainWorld");
        let mut main_world = volume("MainWorld", "Vacuum");
        main_world
            .physvols
            .push(file_ref_physvol("child.gdml", None));
        main.structure.volumes.push(main_world);
        main.defines.constants.push(Constant {
            name: "A".to_string(),
            value: "1".to_string(),
        });

        let mut child = base_doc("child.gdml", "ChildWorld");
        child.structure.volumes.push(volume("ChildWorld", "Vacuum"));
        child.defines.constants.push(Constant {
            name: "A".to_string(),
            value: "2".to_string(),
        });

        let mut child_docs = HashMap::new();
        child_docs.insert("child.gdml".to_string(), child);

        let err = match resolve_all_file_refs(&mut main, &child_docs) {
            Ok(_) => panic!("resolve_all_file_refs should reject conflicting defines"),
            Err(err) => err,
        };
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn repeated_logical_volume_instances_have_unique_instance_ids() {
        let mut doc = base_doc("test.gdml", "World");
        let mut world = volume("World", "Vacuum");
        world.physvols.push(PhysVol {
            name: Some("pv_0".to_string()),
            volume_ref: "Leaf".to_string(),
            file_ref: None,
            position: None,
            rotation: None,
        });
        world.physvols.push(PhysVol {
            name: Some("pv_1".to_string()),
            volume_ref: "Leaf".to_string(),
            file_ref: None,
            position: None,
            rotation: None,
        });
        doc.structure.volumes.push(world);
        doc.structure.volumes.push(volume("Leaf", "Vacuum"));

        let engine = EvalEngine::new();
        let graph = build_scene_graph(&doc, &engine);
        assert_eq!(graph.children.len(), 2);
        assert_eq!(graph.children[0].volume_name, graph.children[1].volume_name);
        assert_ne!(graph.children[0].instance_id, graph.children[1].instance_id);
    }
}
