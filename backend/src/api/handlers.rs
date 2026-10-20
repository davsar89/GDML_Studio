use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json, Response};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

use super::errors::ApiError;
use crate::config;
use crate::eval::engine::EvalEngine;
use crate::gdml::materials as nist;
use crate::gdml::model::*;
use crate::gdml::parser;
use crate::mesh::{tessellator, types::TriangleMesh};
use crate::state::app_state::{LoadedDocument, SharedState};

#[derive(Deserialize)]
pub struct UploadFilesRequest {
    pub files: HashMap<String, String>,
    pub main_file: String,
    pub segments: Option<u32>,
}

/// Whether two definitions are field-for-field the same.
fn same_definition<T: Serialize>(a: &T, b: &T) -> bool {
    matches!((serde_json::to_value(a), serde_json::to_value(b)), (Ok(a), Ok(b)) if a == b)
}

/// Append child items whose names are not already taken, warning when a name is
/// taken by a *different* definition.
///
/// An identical duplicate is normal — two modules including the same materials
/// file — and passes without comment. A genuine conflict means the child's
/// version is being discarded, which changes the rendered geometry, so it is
/// reported rather than swallowed.
fn merge_by_name<T, F>(
    target: &mut Vec<T>,
    incoming: &[T],
    name_of: F,
    kind: &str,
    file_ref_name: &str,
    warnings: &mut Vec<String>,
) where
    T: Clone + Serialize,
    F: Fn(&T) -> &str,
{
    for item in incoming {
        let name = name_of(item);
        let Some(existing) = target.iter().find(|existing| name_of(existing) == name) else {
            target.push(item.clone());
            continue;
        };
        if !same_definition(existing, item) {
            warnings.push(format!(
                "File \"{file_ref_name}\" defines {kind} \"{name}\" differently from the \
                 main document. The main document's version is used; this file's is \
                 IGNORED, so its geometry may be wrong."
            ));
        }
    }
}

/// Merge a child module's definitions into the main document. Runs once per
/// module, however many physvols place it.
fn merge_child_into_main(
    main_doc: &mut GdmlDocument,
    child_doc: &GdmlDocument,
    file_ref_name: &str,
    warnings: &mut Vec<String>,
) {
    // Known gap: Geant4 gives each file its own evaluator, so modules may define
    // one name differently; a single namespace keeps the main file's value.
    merge_by_name(
        &mut main_doc.defines.constants,
        &child_doc.defines.constants,
        |d| d.name.as_str(),
        "constant",
        file_ref_name,
        warnings,
    );
    merge_by_name(
        &mut main_doc.defines.quantities,
        &child_doc.defines.quantities,
        |d| d.name.as_str(),
        "quantity",
        file_ref_name,
        warnings,
    );
    merge_by_name(
        &mut main_doc.defines.variables,
        &child_doc.defines.variables,
        |d| d.name.as_str(),
        "variable",
        file_ref_name,
        warnings,
    );
    merge_by_name(
        &mut main_doc.defines.expressions,
        &child_doc.defines.expressions,
        |d| d.name.as_str(),
        "expression",
        file_ref_name,
        warnings,
    );
    merge_by_name(
        &mut main_doc.defines.positions,
        &child_doc.defines.positions,
        |d| d.name.as_str(),
        "position",
        file_ref_name,
        warnings,
    );
    merge_by_name(
        &mut main_doc.defines.rotations,
        &child_doc.defines.rotations,
        |d| d.name.as_str(),
        "rotation",
        file_ref_name,
        warnings,
    );
    merge_by_name(
        &mut main_doc.defines.scales,
        &child_doc.defines.scales,
        |d| d.name.as_str(),
        "scale",
        file_ref_name,
        warnings,
    );
    merge_by_name(
        &mut main_doc.defines.matrices,
        &child_doc.defines.matrices,
        |d| d.name.as_str(),
        "matrix",
        file_ref_name,
        warnings,
    );
    // Append only new slots, in the child's declaration order. Collection order
    // alone is insufficient: a constant can depend on an earlier quantity.
    for slot in &child_doc.order.define_slots {
        if !main_doc.order.define_slots.contains(slot) {
            main_doc.order.define_slots.push(slot.clone());
            if child_doc
                .materials_define
                .as_ref()
                .is_some_and(|names| names.contains(&slot.name))
            {
                main_doc
                    .materials_define
                    .get_or_insert_with(Vec::new)
                    .push(slot.name.clone());
            }
        }
    }
    main_doc
        .order
        .anchors
        .extend(child_doc.order.anchors.iter().cloned());
    merge_by_name(
        &mut main_doc.materials.isotopes,
        &child_doc.materials.isotopes,
        |item| item.name.as_str(),
        "isotope",
        file_ref_name,
        warnings,
    );

    // Merge elements, materials, solids and volumes by name.
    //
    // Geant4 does not flatten like this: G4GDMLReadStructure::FileRead builds a
    // separate reader for each module and reads it with `isModule = true`, so two
    // modules may legitimately each define a `Box` with different dimensions.
    // Flattening into one namespace means the first definition wins -- and doing
    // that silently rendered the second module with the first module's geometry.
    // Until modules are properly namespaced, at least say so.
    merge_by_name(
        &mut main_doc.materials.elements,
        &child_doc.materials.elements,
        |e| e.name.as_str(),
        "element",
        file_ref_name,
        warnings,
    );
    merge_by_name(
        &mut main_doc.materials.materials,
        &child_doc.materials.materials,
        |m| m.name.as_str(),
        "material",
        file_ref_name,
        warnings,
    );
    merge_by_name(
        &mut main_doc.solids.solids,
        &child_doc.solids.solids,
        |s| s.name(),
        "solid",
        file_ref_name,
        warnings,
    );
    merge_by_name(
        &mut main_doc.structure.volumes,
        &child_doc.structure.volumes,
        |v| v.name.as_str(),
        "volume",
        file_ref_name,
        warnings,
    );

    // Preserved-verbatim elements and the child's own parse warnings were not
    // carried over at all, so a child's <opticalsurface> vanished on save and its
    // warnings never reached the user.
    main_doc
        .raw_unknown
        .extend(child_doc.raw_unknown.iter().cloned());
    main_doc
        .skipped_unsupported
        .extend(child_doc.skipped_unsupported.iter().cloned());
}

/// Point the physvols that place `name`/`volname` at the child's volume of
/// that name, or at its world. False when there is none, or nothing to place.
fn resolve_file_physvols(
    main_doc: &mut GdmlDocument,
    child_doc: &GdmlDocument,
    name: &str,
    volname: &Option<String>,
) -> bool {
    let target = volname.as_deref().unwrap_or(&child_doc.setup.world_ref);
    if target.is_empty() {
        return false;
    }
    let mut resolved = false;
    for vol in &mut main_doc.structure.volumes {
        for pv in &mut vol.physvols {
            if pv
                .file_ref
                .as_ref()
                .is_some_and(|f| f.name == name && &f.volname == volname)
            {
                pv.volume_ref = target.to_string();
                pv.file_ref = None;
                resolved = true;
            }
        }
    }
    resolved
}

/// The uploaded module a `<file name>` names: by that name, else by its
/// basename, since a browser uploads files without their directories.
fn find_module<'a>(
    child_docs: &'a HashMap<String, GdmlDocument>,
    name: &str,
) -> Option<(&'a String, &'a GdmlDocument)> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    child_docs
        .get_key_value(name)
        .or_else(|| child_docs.get_key_value(base))
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

/// Resolve every file reference, including those merged modules bring in.
fn resolve_all_file_refs(
    main_doc: &mut GdmlDocument,
    child_docs: &HashMap<String, GdmlDocument>,
) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut merged = HashSet::new();
    // A pass that resolves nothing ends the loop. Each module is merged once,
    // so the references merging adds run out.
    loop {
        let mut progress = false;
        for (name, volname) in collect_file_refs(main_doc) {
            let Some((file, child_doc)) = find_module(child_docs, &name) else {
                continue;
            };
            if merged.insert(file) {
                merge_child_into_main(main_doc, child_doc, file, &mut warnings);
            }
            progress |= resolve_file_physvols(main_doc, child_doc, &name, &volname);
        }
        if !progress {
            break;
        }
    }

    let mut unresolved = collect_file_refs(main_doc);
    unresolved.sort();
    unresolved.dedup();
    for (name, _) in unresolved {
        warnings.push(match find_module(child_docs, &name) {
            None => format!(
                "Referenced file '{name}' was not provided in the upload; select all GDML \
                 files together to resolve it."
            ),
            Some(_) => {
                format!("Child file '{name}' has no world reference and no volname specified")
            }
        });
    }
    warnings
}

/// A volume may name a document material or a NIST one: Geant4's GetMaterial
/// falls back to G4NistManager::FindOrBuildMaterial.
fn ensure_material_ref_exists(doc: &GdmlDocument, candidate: &str) -> Result<(), ApiError> {
    let exists = doc.materials.materials.iter().any(|m| m.name == candidate)
        || nist::find_nist_material(candidate).is_some();
    if !exists {
        return Err(ApiError::bad_request(&format!(
            "Material '{candidate}' does not exist"
        )));
    }
    Ok(())
}

fn validate_material_components(
    doc: &MaterialSection,
    material: &Material,
    excluding_material: Option<&str>,
) -> Result<(), ApiError> {
    // MaterialRead builds a material without components from Z and atom, and
    // G4Element refuses Z < 1.
    if material.components.is_empty() && (material.z.is_none() || material.atom_value.is_none()) {
        return Err(ApiError::bad_request(&format!(
            "Material '{}' needs atomic number and mass, or components",
            material.name
        )));
    }
    // G4Material takes its components all by mass fraction or all by atom
    // count; mixing the two is fatal (mat031).
    let atoms = |c: &MaterialComponent| matches!(c, MaterialComponent::Composite { .. });
    if material.components.iter().any(atoms) && !material.components.iter().all(atoms) {
        return Err(ApiError::bad_request(&format!(
            "Material '{}' mixes composite (atom count) and fraction (mass) components, which Geant4 refuses",
            material.name
        )));
    }
    for component in &material.components {
        let (ref_name, composite) = match component {
            MaterialComponent::Fraction { ref_name, .. } => (ref_name, false),
            MaterialComponent::Composite { ref_name, .. } => (ref_name, true),
        };

        // MixtureRead looks a name up as an element first, so naming an
        // element that shares the material's name is not a self-reference.
        let element_exists = doc.elements.iter().any(|e| e.name == *ref_name);
        if ref_name == &material.name && !element_exists {
            return Err(ApiError::bad_request(&format!(
                "Material '{}' cannot reference itself in components",
                material.name
            )));
        }
        // A <fraction> may name a material, but MixtureRead resolves a
        // <composite> with GetElement alone, and a miss there is fatal.
        if composite && !element_exists {
            return Err(ApiError::bad_request(&format!(
                "Material '{}': a composite must name an element, and '{}' is not one",
                material.name, ref_name
            )));
        }

        let material_exists = doc
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

/// Reject an edit after which no material order works: Geant4 resolves each
/// component while reading its material, so a component cycle cannot load.
fn ensure_material_order(materials: &MaterialSection) -> Result<(), ApiError> {
    nist::material_order(materials)
        .map(|_| ())
        .map_err(|e| ApiError::bad_request(&e.to_string()))
}

/// Warn about elements that are preserved verbatim on save but not interpreted
/// or rendered (e.g. optical surfaces, `<userinfo>`).
fn raw_unknown_warnings(doc: &GdmlDocument) -> Vec<String> {
    let mut tags: Vec<&str> = doc.raw_unknown.iter().map(|r| r.tag.as_str()).collect();
    tags.sort_unstable();
    tags.dedup();
    tags.into_iter()
        .map(|t| match t {
            // <loop> is a special case: unlike the other preserved tags it is
            // expanded and drawn. Expansion feeds a separate render document,
            // so the solid and volume COUNTS in the summary still come from
            // the source and report the loop as written rather than as drawn.
            "loop" => "<loop> is expanded for the 3D view and preserved exactly as written \
                       on save. The summary counts the loop once, not once per iteration."
                .to_string(),
            other => format!(
                "<{}> elements are preserved on save but are not interpreted or rendered.",
                other
            ),
        })
        .collect()
}

pub async fn upload_files(
    State(state): State<SharedState>,
    Json(req): Json<UploadFilesRequest>,
) -> Result<Json<Value>, ApiError> {
    // A large file takes seconds to parse and tessellate: do it off the async
    // workers and before taking the lock, so other requests are still served.
    let loaded = tokio::task::spawn_blocking(move || load_document(req))
        .await
        .map_err(|e| ApiError::internal(&format!("Upload failed: {e}")))??;
    let summary = summary_json(&loaded);
    state.write().await.loaded = Some(loaded);
    Ok(Json(summary))
}

/// Parse an upload, merge its modules, expand its loops and tessellate. A
/// single-file upload is an upload of one file.
fn load_document(req: UploadFilesRequest) -> Result<LoadedDocument, ApiError> {
    let UploadFilesRequest {
        files,
        main_file,
        segments,
    } = req;
    if !main_file.to_ascii_lowercase().ends_with(".gdml") {
        return Err(ApiError::bad_request("Only .gdml files are supported"));
    }
    let main_content = files
        .get(&main_file)
        .ok_or_else(|| ApiError::bad_request("Main file not found in uploaded files"))?;
    // The client's message names the upload; of several files, say which.
    let parse = |name: &String, content: &String| {
        parser::parse_gdml_from_bytes(content.as_bytes(), name.clone()).map_err(|e| {
            let which = match files.len() {
                1 => String::new(),
                _ => format!(" in {name}"),
            };
            ApiError::bad_request(&format!("Parse error{which}: {e}"))
        })
    };
    let mut doc = parse(&main_file, main_content)?;
    let child_docs = files
        .iter()
        .filter(|(name, _)| **name != main_file)
        .map(|(name, content)| Ok((name.clone(), parse(name, content)?)))
        .collect::<Result<HashMap<_, _>, ApiError>>()?;

    // Resolve file references: merge child documents into main (including nested refs)
    let merge_warnings = resolve_all_file_refs(&mut doc, &child_docs);

    let mut engine = EvalEngine::new();
    let mut loop_warnings = Vec::new();
    let render = if child_docs.is_empty() {
        // Bootstrap loop bounds without evaluating vectors that may use generated defines.
        if main_content.contains("<loop") {
            let bootstrap_defines = DefineSection {
                constants: doc.defines.constants.clone(),
                quantities: doc.defines.quantities.clone(),
                variables: doc.defines.variables.clone(),
                expressions: doc.defines.expressions.clone(),
                matrices: doc.defines.matrices.clone(),
                ..DefineSection::default()
            };
            engine
                .evaluate_all(&bootstrap_defines)
                .map_err(|e| ApiError::bad_request(&format!("Expression evaluation error: {e}")))?;
        }
        // Expand <loop> for the preview. The parsed `doc` keeps its loops
        // verbatim so the export stays faithful; geometry is built from the twin.
        build_render_document(main_content, &main_file, &engine, &mut loop_warnings)
    } else {
        // Loops are not expanded for a multi-file load. Each file is parsed
        // separately and then merged, so there is no single XML document to
        // expand, and a loop's bounds may reference defines that only exist
        // after the merge. Rather than expand each file against a partial
        // symbol table and be wrong in the cross-file case, say so and render
        // unexpanded.
        let loop_files: Vec<&str> = files
            .iter()
            .filter(|(_, c)| c.contains("<loop"))
            .map(|(n, _)| n.as_str())
            .collect();
        if !loop_files.is_empty() {
            loop_warnings.push(format!(
                "<loop> is not expanded for multi-file loads ({}); those placements are missing from the preview. The saved file is unaffected.",
                loop_files.join(", ")
            ));
        }
        None
    };
    let geometry = render.as_ref().unwrap_or(&doc);
    // Rebuild the final context from the document actually used for geometry.
    engine
        .evaluate_all(&geometry.defines)
        .map_err(|e| ApiError::bad_request(&format!("Expression evaluation error: {e}")))?;

    // Tessellate solids
    let segments = segments.unwrap_or_else(config::mesh_segments);
    let (meshes, mut warnings) =
        tessellator::tessellate_all_solids(&geometry.solids, &engine, segments)
            .map_err(|e| ApiError::internal(&format!("Tessellation error: {}", e)))?;
    warnings.append(&mut loop_warnings);
    // Surface non-fatal expression-evaluation failures (treated as 0) to the user.
    warnings.extend(engine.take_warnings());
    warnings.extend(raw_unknown_warnings(&doc));
    // The geometry document, so warnings from inside an expanded <loop> count.
    warnings.extend(geometry.skipped_unsupported.iter().cloned());
    if doc.setup.world_ref.is_empty() {
        warnings.push("No world volume reference found (<setup>/<world> missing or empty); the geometry may not display.".to_string());
    }
    warnings.extend(merge_warnings);

    Ok(LoadedDocument {
        document: doc,
        render,
        engine,
        meshes,
        warnings,
        unsaved: false,
    })
}

/// The counts the toolbar shows, and the load warnings.
fn summary_json(loaded: &LoadedDocument) -> Value {
    let doc = &loaded.document;
    json!({
        "filename": doc.filename,
        "defines_count": doc.defines.constants.len() + doc.defines.quantities.len()
            + doc.defines.variables.len() + doc.defines.expressions.len(),
        "positions_count": doc.defines.positions.len(),
        "rotations_count": doc.defines.rotations.len(),
        "materials_count": doc.materials.materials.len(),
        "elements_count": doc.materials.elements.len(),
        "solids_count": doc.solids.solids.len(),
        "volumes_count": doc.structure.volumes.len(),
        "meshes_count": placed_meshes(loaded).len(),
        "world_ref": doc.setup.world_ref,
        "warnings": loaded.warnings,
        "unsaved": loaded.unsaved,
    })
}

/// The meshes volumes draw. The client looks up `meshes[node.solid_name]`
/// only, so boolean operands and unplaced solids are neither sent nor counted.
fn placed_meshes(loaded: &LoadedDocument) -> HashMap<&str, &TriangleMesh> {
    loaded
        .geometry()
        .structure
        .volumes
        .iter()
        .filter_map(|v| loaded.meshes.get_key_value(&v.solid_ref))
        .map(|(name, mesh)| (name.as_str(), mesh))
        .collect()
}

pub async fn get_summary(State(state): State<SharedState>) -> Result<Json<Value>, ApiError> {
    let state_r = state.read().await;
    let loaded = loaded(state_r.loaded.as_ref())?;
    Ok(Json(summary_json(loaded)))
}

/// The loaded document, or 404 when there is none.
fn loaded<T>(doc: Option<T>) -> Result<T, ApiError> {
    doc.ok_or_else(|| ApiError::not_found("No document loaded"))
}

/// The reply to an edit that went through. The document now differs from any
/// export, which a reloaded page reads back from the summary.
fn edited(loaded: &mut LoadedDocument) -> Json<Value> {
    loaded.unsaved = true;
    Json(json!({ "ok": true }))
}

/// Run `f` on the loaded document on a blocking thread. Building a large scene
/// and serialising megabytes of vertices would otherwise stall an async worker.
async fn with_loaded<T: Send + 'static>(
    state: SharedState,
    f: impl FnOnce(&LoadedDocument) -> T + Send + 'static,
) -> Result<T, ApiError> {
    let state_r = state.read_owned().await;
    tokio::task::spawn_blocking(move || Ok(f(loaded(state_r.loaded.as_ref())?)))
        .await
        .map_err(|e| ApiError::internal(&format!("Request failed: {e}")))?
}

/// Scene graph without the mesh buffers.
///
/// Editing a material never re-tessellates — `tessellate_all_solids` runs only
/// on upload — but node colour is derived from density, so the tree still has to
/// be rebuilt after an edit. Serving it separately keeps a debounced keystroke
/// from re-downloading every vertex in the document (~16 MB on the largest
/// sample) and, on the client, from rebuilding every BufferGeometry and
/// re-uploading the whole scene to the GPU.
pub async fn get_scene(State(state): State<SharedState>) -> Result<Json<Value>, ApiError> {
    with_loaded(state, |loaded| {
        let (scene_graph, warnings) = scene(loaded);
        Json(json!({ "scene_graph": scene_graph, "warnings": warnings }))
    })
    .await
}

/// The loaded document's scene graph and its warnings.
fn scene(loaded: &LoadedDocument) -> (SceneNode, Vec<String>) {
    let mut warnings = Vec::new();
    let graph = build_scene_graph(
        loaded.geometry(),
        &loaded.document.materials,
        &loaded.engine,
        &mut warnings,
    );
    (graph, warnings)
}

pub async fn get_meshes(State(state): State<SharedState>) -> Result<Response, ApiError> {
    with_loaded(state, |loaded| {
        let (scene_graph, warnings) = scene(loaded);

        // The buffers are serialised by reference straight into the response:
        // a serde_json::Value tree cost one 32-byte Value per number and widened
        // every f32 to f64 (12.34f32 came out as 12.340000152587891). Peak on
        // the largest sample was ~50 MB for 3.7 MB of vertex data.
        #[derive(Serialize)]
        struct MeshesResponse<'a> {
            meshes: HashMap<&'a str, &'a TriangleMesh>,
            scene_graph: SceneNode,
            warnings: Vec<String>,
        }
        Json(MeshesResponse {
            meshes: placed_meshes(loaded),
            scene_graph,
            warnings,
        })
        .into_response()
    })
    .await
}

pub async fn get_defines(State(state): State<SharedState>) -> Result<Json<Value>, ApiError> {
    let state_r = state.read().await;
    let loaded = loaded(state_r.loaded.as_ref())?;

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

    let plain = |name: &String, value: &String, kind: &str| DefineValue {
        name: name.clone(),
        expression: value.clone(),
        evaluated: engine.get(name),
        unit: None,
        kind: kind.to_string(),
    };
    let d = &doc.defines;
    let defines: Vec<DefineValue> = d
        .constants
        .iter()
        .map(|c| plain(&c.name, &c.value, "constant"))
        .chain(d.quantities.iter().map(|q| {
            // Shown next to its unit, so in that unit rather than in mm or rad.
            let factor = q.unit.as_deref().map_or(Ok(1.0), |u| engine.eval_expr(u));
            DefineValue {
                evaluated: engine.get(&q.name).map(|v| v / factor.unwrap_or(1.0)),
                unit: q.unit.clone(),
                ..plain(&q.name, &q.value, "quantity")
            }
        }))
        .chain(
            d.variables
                .iter()
                .map(|v| plain(&v.name, &v.value, "variable")),
        )
        .chain(
            d.expressions
                .iter()
                .map(|e| plain(&e.name, &e.value, "expression")),
        )
        .collect();

    Ok(Json(json!({ "defines": defines })))
}

pub async fn get_materials(State(state): State<SharedState>) -> Result<Json<Value>, ApiError> {
    let state_r = state.read().await;
    let loaded = loaded(state_r.loaded.as_ref())?;

    Ok(Json(json!({
        "elements": loaded.document.materials.elements,
        "materials": loaded.document.materials.materials,
    })))
}

pub async fn get_structure(State(state): State<SharedState>) -> Result<Json<Value>, ApiError> {
    let state_r = state.read().await;
    let loaded = loaded(state_r.loaded.as_ref())?;

    Ok(Json(json!({
        "volumes": loaded.document.structure.volumes,
        "world_ref": loaded.document.setup.world_ref,
    })))
}

// ─── Scene graph builder ─────────────────────────────────────────────────────

/// Build the loop-expanded twin of a freshly parsed document.
///
/// Returns `None` when the source has no `<loop>`, so the common case costs one
/// substring search and nothing else. Expansion failures are reported as
/// warnings and fall back to the unexpanded document rather than refusing the
/// upload: a file that renders incompletely is more useful than one that will
/// not open.
fn build_render_document(
    source_xml: &str,
    filename: &str,
    engine: &EvalEngine,
    warnings: &mut Vec<String>,
) -> Option<GdmlDocument> {
    if !source_xml.contains("<loop") {
        return None;
    }
    match crate::gdml::loops::expand_loops(source_xml, engine) {
        Ok(expanded) => {
            match parser::parse_gdml_from_bytes(expanded.as_bytes(), filename.to_string()) {
                Ok(doc) => Some(doc),
                Err(e) => {
                    warnings.push(format!(
                        "<loop> expanded but the result did not parse ({e}); the preview shows the geometry unexpanded. The saved file is unaffected."
                    ));
                    None
                }
            }
        }
        Err(e) => {
            warnings.push(format!(
                "<loop> could not be expanded ({e}); its placements are missing from the preview. The saved file is unaffected."
            ));
            None
        }
    }
}

/// Build the scene from `doc`'s structure and `materials` for colouring.
///
/// The two are separate because `doc` may be the loop-expanded render
/// document while materials are only ever edited on the source. Passing them
/// apart makes that explicit rather than relying on the two being kept in sync.
fn build_scene_graph(
    doc: &GdmlDocument,
    materials: &crate::gdml::model::MaterialSection,
    engine: &EvalEngine,
    warnings: &mut Vec<String>,
) -> SceneNode {
    let world_ref = &doc.setup.world_ref;
    let vol_map: HashMap<&str, &Volume> = doc
        .structure
        .volumes
        .iter()
        .map(|v| (v.name.as_str(), v))
        .collect();

    // Material name → density in g/cm3. DRead evaluates `value` as an
    // expression and defaults the unit to g/cm3 (G4GDMLReadMaterials.cc:147-150).
    let density_map: HashMap<&str, f64> = materials
        .materials
        .iter()
        .filter_map(|m| {
            let d = m.density.as_ref()?;
            let unit = d.unit.as_deref().unwrap_or("g/cm3");
            let density = engine
                .eval_expr(&format!("({})*(({unit})/(g/cm3))", d.value))
                .ok()?;
            Some((m.name.as_str(), density))
        })
        .collect();

    let mut visited = HashSet::new();
    let mut budget = SceneBudget::default();

    let root = vol_map
        .get(world_ref.as_str())
        .and_then(|world_vol| {
            build_volume_node(
                world_vol,
                &vol_map,
                &density_map,
                engine,
                [0.0; 3],
                [0.0; 3],
                true,
                &mut visited,
                "",
                format!("/{}", world_vol.name),
                warnings,
                0,
                &mut budget,
            )
        })
        .unwrap_or_else(|| SceneNode {
            name: "World".to_string(),
            instance_id: "/World".to_string(),
            volume_name: world_ref.clone(),
            solid_name: String::new(),
            material_name: String::new(),
            color: None,
            density: None,
            position: [0.0; 3],
            rotation: [0.0; 3],
            scale: [1.0; 3],
            is_world: true,
            children: Vec::new(),
        });

    if budget.truncated {
        warnings.push(format!(
            "Scene graph truncated at {} nodes / depth {} / {} MiB of names. This \
             geometry expands multiplicatively (a volume placed in several mothers \
             is re-expanded under each), so parts of the tree are not shown.",
            MAX_SCENE_NODES,
            MAX_SCENE_DEPTH,
            MAX_SCENE_BYTES >> 20
        ));
    }

    // Replica counts, widths and inline placements are first evaluated here,
    // so their evaluation warnings are collected here too.
    warnings.extend(engine.take_warnings());
    dedupe_warnings(warnings);
    root
}

/// Collapse repeats and cap the list.
///
/// Warnings here are raised per *instance* — `build_volume_node` runs once per
/// placement — so a replicavol nested inside a 16x16 replica reports the same
/// problem 256 times. pinhole_lab.gdml produced 277 warnings of which 4 were
/// distinct, burying the panel and the two warnings that were actually about
/// different volumes. Repeats are counted rather than dropped, since "256
/// placements affected" is the useful part.
///
/// First-occurrence order is preserved so the output is deterministic.
/// `EvalEngine::record_warning` already does the equivalent for evaluation
/// warnings; this is the same treatment for scene-graph ones.
fn dedupe_warnings(warnings: &mut Vec<String>) {
    const MAX_WARNINGS: usize = 100;

    let mut counts: HashMap<&str, usize> = HashMap::new();
    let mut order: Vec<&str> = Vec::new();
    for w in warnings.iter() {
        if !counts.contains_key(w.as_str()) {
            order.push(w.as_str());
        }
        *counts.entry(w.as_str()).or_insert(0) += 1;
    }

    let mut out: Vec<String> = order
        .iter()
        .take(MAX_WARNINGS)
        .map(|w| match counts[w] {
            1 => (*w).to_string(),
            n => format!("{w} (x{n})"),
        })
        .collect();

    if order.len() > MAX_WARNINGS {
        out.push(format!(
            "... and {} more distinct warnings, not shown.",
            order.len() - MAX_WARNINGS
        ));
    }

    *warnings = out;
}

/// Total nodes the scene graph may emit before it stops expanding.
///
/// `visited` is path-scoped — a volume is inserted on entry and removed once its
/// children are built — which correctly detects cycles but means a volume
/// reachable by two different paths is re-expanded along each. That is the
/// intended behaviour (it is how repeated placements render), and real files
/// stay small: the largest shipped sample produces about 4,400 nodes. But it is
/// multiplicative and was unbounded, so 25 volumes each containing two physvols
/// pointing at the next produce 2^25 nodes, each holding several strings, on
/// every request. Hostile input rather than a real-file problem. 100k nodes is
/// ~21 MB of JSON, already more meshes than the viewer can draw interactively;
/// 500k was 105 MB and took 1.1 s to serialise.
const MAX_SCENE_NODES: usize = 100_000;

/// Maximum nesting depth, bounding recursion independently of the node count.
const MAX_SCENE_DEPTH: u32 = 256;

/// Bytes of instance id and names all nodes may hold. An id repeats its whole
/// path, so deep trees of long names outgrow memory well before the node cap.
const MAX_SCENE_BYTES: usize = 64 << 20;

#[derive(Default)]
struct SceneBudget {
    nodes: usize,
    bytes: usize,
    truncated: bool,
}

// Recursion carries both immutable lookup tables and mutable accumulators. The
// lookups belong in a context struct; that refactor is worth doing on its own
// rather than bundled into a bug fix.
#[allow(clippy::too_many_arguments)]
fn build_volume_node(
    vol: &Volume,
    vol_map: &HashMap<&str, &Volume>,
    density_map: &HashMap<&str, f64>,
    engine: &EvalEngine,
    position: [f64; 3],
    rotation: [f64; 3],
    is_world: bool,
    visited: &mut HashSet<String>,
    parent_id: &str,
    id_suffix: String,
    warnings: &mut Vec<String>,
    depth: u32,
    budget: &mut SceneBudget,
) -> Option<SceneNode> {
    // Every child kind comes through here, so these checks run once for
    // physvols and replicas alike. `None` means nothing was added; a replica
    // loop stops at the first one, since its copies would all fail alike.
    if visited.contains(&vol.name) {
        // G4PVPlacement refuses "a volume inside itself", and GDML's
        // define-before-use makes longer cycles unloadable too.
        warnings.push(format!(
            "volume '{}' is placed inside itself; Geant4 cannot load this, and \
             the placement is skipped.",
            vol.name
        ));
        return None;
    }
    if depth > MAX_SCENE_DEPTH || budget.nodes >= MAX_SCENE_NODES || budget.bytes >= MAX_SCENE_BYTES
    {
        budget.truncated = true;
        return None;
    }
    // Built only once admitted: a refused sibling would copy the whole path.
    let instance_id = format!("{parent_id}{id_suffix}");
    budget.nodes += 1;
    budget.bytes +=
        instance_id.len() + 2 * vol.name.len() + vol.solid_ref.len() + vol.material_ref.len();

    visited.insert(vol.name.clone());

    let color = vol
        .auxiliaries
        .iter()
        .find(|a| a.auxtype.eq_ignore_ascii_case("color"))
        .map(|a| a.auxvalue.clone());

    let density = density_map.get(vol.material_ref.as_str()).copied();

    let mut children: Vec<SceneNode> = vol
        .physvols
        .iter()
        .enumerate()
        .filter_map(|(idx, pv)| {
            let child_vol = match vol_map.get(pv.volume_ref.as_str()) {
                Some(v) => v,
                None => {
                    warnings.push(format!(
                        "volume '{}': physvol references undefined volume '{}'; skipping it.",
                        vol.name, pv.volume_ref
                    ));
                    return None;
                }
            };

            let placement = engine.resolve_position(&pv.position).and_then(|pos| {
                let rot = engine.resolve_rotation(&pv.rotation)?;
                Ok((pos, rot, engine.resolve_scale(&pv.scale)?))
            });
            let (pos, rot, scale) = match placement {
                Ok(transform) => transform,
                Err(e) => {
                    warnings.push(format!(
                        "volume '{}', placement {}: {}; skipping placement",
                        vol.name, idx, e
                    ));
                    return None;
                }
            };
            let id_suffix = match pv.name.as_deref() {
                Some(name) if !name.is_empty() => {
                    format!("/physvol[{}]({}):{}", idx, name, pv.volume_ref)
                }
                _ => format!("/physvol[{}]:{}", idx, pv.volume_ref),
            };

            // G4ReflectionFactory::Place keeps only a reflection: it drops a
            // scale that is not one and refuses a reflection that is not ±1.
            if scale.iter().any(|s| (s.abs() - 1.0).abs() > 1e-9) {
                warnings.push(format!(
                    "volume '{}', placement {}: scale {scale:?} is not a reflection, \
                     which Geant4 does not apply; only its signs are drawn.",
                    vol.name, idx
                ));
            }
            build_volume_node(
                child_vol,
                vol_map,
                density_map,
                engine,
                pos,
                rot,
                false,
                visited,
                &instance_id,
                id_suffix,
                warnings,
                depth + 1,
                budget,
            )
            .map(|node| SceneNode {
                scale: scale.map(f64::signum),
                ..node
            })
        })
        .collect();

    // Expand replicavol into child nodes
    if let Some(ref replica) = vol.replica {
        if let Some(child_vol) = vol_map.get(replica.volume_ref.as_str()) {
            // No cap of its own: the scene budget ends the loops below, and the
            // copies drawn keep the positions of the full count.
            let number = engine.resolve_value(&replica.number) as usize;
            // kPhi replicates in ANGLE, the Cartesian axes in LENGTH, so the
            // width/offset units cannot be converted until the axis is known.
            let is_phi = replica.curvilinear_axis.as_deref() == Some("phi");
            let is_rho = replica.curvilinear_axis.as_deref() == Some("rho");
            let width_val = engine.resolve_value(&replica.width);
            let offset_val = engine.resolve_value(&replica.offset);
            let convert = |v: f64, unit: Option<&str>| -> f64 {
                if is_phi {
                    crate::gdml::units::angle_to_rad(v, unit.unwrap_or("rad"))
                } else {
                    crate::gdml::units::length_to_mm(v, unit.unwrap_or("mm"))
                }
            };
            let width = convert(width_val, replica.width_unit.as_deref());
            let offset = convert(offset_val, replica.offset_unit.as_deref());

            if is_rho {
                // G4ReplicaNavigation::ComputeTransformation has "No setup
                // required for radial case" for kRho -- there is no placement
                // transform at all. Each slice is a different SOLID, its radial
                // extent computed during navigation, which needs per-replica
                // re-tessellation this viewer does not do. Emitting `number`
                // coincident copies would cost N times the geometry and show
                // exactly what one copy shows, so draw one and say so.
                warnings.push(format!(
                    "replicavol '{}': radial (kRho) replication subdivides the solid itself, which is not modelled; one un-subdivided copy is drawn in place of the {} slices.",
                    replica.volume_ref, number
                ));
                children.extend(build_volume_node(
                    child_vol,
                    vol_map,
                    density_map,
                    engine,
                    [0.0; 3],
                    [0.0; 3],
                    false,
                    visited,
                    &instance_id,
                    format!("/replica[rho]:{}", replica.volume_ref),
                    warnings,
                    depth + 1,
                    budget,
                ));
            } else {
                // A Cartesian replica is stacked along x, y or z; a phi one is
                // rotated instead.
                let along = |i: usize| {
                    let v = replica.direction[i].as_deref();
                    v.map_or(0.0, |v| engine.resolve_value(v)).abs() > 0.5
                };
                let axis = if is_phi {
                    None
                } else if along(0) {
                    Some(0)
                } else if along(1) {
                    Some(1)
                } else {
                    Some(2)
                };
                if axis.is_some() && offset.abs() > 1e-9 {
                    // Matches Geant4: ComputeTransformation's kXAxis/kYAxis/
                    // kZAxis cases compute the centred position from width and
                    // replicaNo alone and never read `offset`
                    // (G4ReplicaNavigation.cc:667-679). Said here because the
                    // number in the file has no effect on what is drawn.
                    warnings.push(format!(
                        "replicavol '{}': offset ({} mm) does not move a Cartesian stack -- Geant4 ignores it here too; the stack is centred in its mother.",
                        replica.volume_ref, offset
                    ));
                }

                for n in 0..number {
                    let (mut pos, mut rot) = ([0.0_f64; 3], [0.0_f64; 3]);
                    match axis {
                        // G4ReplicaNavigation.cc:667 --
                        //   val = -width*0.5*(nReplicas-1) + width*replicaNo
                        Some(axis) => {
                            pos[axis] = -width * 0.5 * (number.saturating_sub(1) as f64)
                                + (n as f64) * width
                        }
                        // G4ReplicaNavigation.cc:682 sets
                        //   val = -(offset + width*(replicaNo + 0.5))
                        // and applies rotateZ(val) to the physical volume. That
                        // stored matrix is the mother-from-daughter rotation, i.e.
                        // the inverse of the daughter's orientation, which is the
                        // same convention SceneNode::rotation uses (the viewer
                        // negates it) -- so `val` goes in directly.
                        None => rot[2] = -(offset + width * (n as f64 + 0.5)),
                    }
                    let Some(child_node) = build_volume_node(
                        child_vol,
                        vol_map,
                        density_map,
                        engine,
                        pos,
                        rot,
                        false,
                        visited,
                        &instance_id,
                        format!("/replica[{}]:{}", n, replica.volume_ref),
                        warnings,
                        depth + 1,
                        budget,
                    ) else {
                        break;
                    };
                    children.push(child_node);
                }
            }
        }
    }

    visited.remove(&vol.name);

    Some(SceneNode {
        name: vol.name.clone(),
        instance_id,
        volume_name: vol.name.clone(),
        solid_name: vol.solid_ref.clone(),
        material_name: vol.material_ref.clone(),
        color,
        density,
        position,
        rotation,
        scale: [1.0; 3],
        is_world,
        children,
    })
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
    doc: &MaterialSection,
    candidate: &str,
    excluding_material: Option<&str>,
) -> Result<(), ApiError> {
    if candidate.trim().is_empty() {
        return Err(ApiError::bad_request("A name is required"));
    }
    let material_conflict = doc
        .materials
        .iter()
        .any(|m| m.name == candidate && Some(m.name.as_str()) != excluding_material);
    if material_conflict {
        return Err(ApiError::bad_request(&format!(
            "Material '{candidate}' already exists"
        )));
    }

    // Elements and materials are separate tables in Geant4, so a material the
    // file already names like an element may keep that name.
    let element_conflict =
        Some(candidate) != excluding_material && doc.elements.iter().any(|e| e.name == candidate);
    if element_conflict {
        return Err(ApiError::bad_request(&format!(
            "Name '{candidate}' is already used by an element"
        )));
    }

    Ok(())
}

fn ensure_element_name_available(doc: &MaterialSection, candidate: &str) -> Result<(), ApiError> {
    if candidate.trim().is_empty() {
        return Err(ApiError::bad_request("A name is required"));
    }
    if doc.elements.iter().any(|e| e.name == candidate) {
        return Err(ApiError::bad_request(&format!(
            "Element '{candidate}' already exists"
        )));
    }

    if doc.materials.iter().any(|m| m.name == candidate) {
        return Err(ApiError::bad_request(&format!(
            "Name '{candidate}' is already used by a material"
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

/// Stage NIST dependencies without mutating the loaded document on failure.
fn with_staged_elements(
    materials: &MaterialSection,
    elements: &[Element],
    engine: &EvalEngine,
) -> Result<MaterialSection, ApiError> {
    let mut staged = materials.clone();
    for element in elements {
        if let Some(existing) = staged.elements.iter().find(|e| e.name == element.name) {
            if !same_definition(existing, element) {
                return Err(ApiError::bad_request(&format!(
                    "Element '{}' already has a different definition",
                    element.name
                )));
            }
            continue;
        }
        ensure_element_name_available(&staged, &element.name)?;
        validate_element(&staged, element, engine)?;
        staged.elements.push(element.clone());
    }
    Ok(staged)
}

/// Geant4's requirements for an element: Z and atomic mass, or isotope
/// fractions naming isotopes that exist.
fn validate_element(
    doc: &MaterialSection,
    element: &Element,
    engine: &EvalEngine,
) -> Result<(), ApiError> {
    if element.fractions.is_empty() && (element.z.is_none() || element.atom_value.is_none()) {
        return Err(ApiError::bad_request(&format!(
            "Element '{}' needs atomic number and mass, or isotope fractions",
            element.name
        )));
    }
    for (value, label, integral) in [
        (&element.z, "atomic number", true),
        (&element.atom_value, "atomic mass", false),
    ] {
        if let Some(value) = value {
            let value = engine.eval_expr(value).map_err(|e| {
                ApiError::bad_request(&format!("Element '{}': invalid {label}: {e}", element.name))
            })?;
            if value <= 0.0 || (integral && value.fract() != 0.0) {
                return Err(ApiError::bad_request(&format!(
                    "Element '{}': invalid {label}",
                    element.name
                )));
            }
        }
    }
    for fraction in &element.fractions {
        let weight = engine
            .eval_expr(&fraction.n)
            .map_err(|e| ApiError::bad_request(&format!("Invalid isotope fraction: {e}")))?;
        if !(0.0..=1.0).contains(&weight) {
            return Err(ApiError::bad_request(
                "Isotope fraction must be between 0 and 1",
            ));
        }
        if !doc.isotopes.iter().any(|i| i.name == fraction.ref_name) {
            return Err(ApiError::bad_request(&format!(
                "Element '{}' references unknown isotope '{}'",
                element.name, fraction.ref_name
            )));
        }
    }
    Ok(())
}

/// `before` is the material this one replaces, if any.
fn validate_material_values(
    material: &Material,
    before: Option<&Material>,
    engine: &EvalEngine,
) -> Result<(), ApiError> {
    let positive = |value: &str, field: &str| -> Result<f64, ApiError> {
        let v = engine
            .eval_expr(value)
            .map_err(|e| ApiError::bad_request(&format!("Invalid {field}: {e}")))?;
        if v <= 0.0 {
            return Err(ApiError::bad_request(&format!("{field} must be positive")));
        }
        Ok(v)
    };
    if let Some(density) = &material.density {
        positive(&density.value, "Density")?;
    }
    if let Some(z) = &material.z {
        if positive(z, "Atomic number")?.fract() != 0.0 {
            return Err(ApiError::bad_request("Atomic number must be an integer"));
        }
    }
    let mut mass = 0.0;
    for component in &material.components {
        match component {
            MaterialComponent::Fraction { n, .. } => {
                let fraction = positive(n, "Mass fraction")?;
                if fraction > 1.0 {
                    return Err(ApiError::bad_request("Mass fraction cannot exceed 1"));
                }
                mass += fraction;
            }
            MaterialComponent::Composite { n, .. } => {
                if positive(n, "Atom count")?.fract() != 0.0 {
                    return Err(ApiError::bad_request("Atom count must be an integer"));
                }
            }
        }
    }
    // G4Material renormalises mass fractions that miss 1 by over perThousand,
    // with only a warning. An edited composition must sum to 1; a loaded one
    // left as it is loads in Geant4, so it is not judged again.
    let edited = before.is_none_or(|b| !same_definition(&b.components, &material.components));
    if edited && mass > 0.0 && (mass - 1.0).abs() > 1e-3 {
        return Err(ApiError::bad_request(&format!(
            "Mass fractions of '{}' sum to {mass}, not 1",
            material.name
        )));
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct UpdateMaterialRequest {
    pub name: String,
    pub material: Material,
    #[serde(default)]
    pub elements: Vec<Element>,
}

pub async fn update_material(
    State(state): State<SharedState>,
    Json(req): Json<UpdateMaterialRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut state_w = state.write().await;
    let loaded = loaded(state_w.loaded.as_mut())?;

    let old_name = req.name.clone();
    let new_name = req.material.name.clone();
    let mut staged =
        with_staged_elements(&loaded.document.materials, &req.elements, &loaded.engine)?;
    ensure_material_name_available(&staged, &new_name, Some(&old_name))?;
    validate_material_components(&staged, &req.material, Some(&old_name))?;
    let index = staged
        .materials
        .iter()
        .position(|m| m.name == old_name)
        .ok_or_else(|| ApiError::not_found(&format!("Material '{}' not found", req.name)))?;
    validate_material_values(
        &req.material,
        Some(&staged.materials[index]),
        &loaded.engine,
    )?;
    staged.materials[index] = req.material;
    // A component naming an element of the old name meant that element.
    if !staged.elements.iter().any(|e| e.name == old_name) {
        cascade_component_ref_rename(&mut staged.materials, &old_name, &new_name);
    }
    ensure_material_order(&staged)?;
    if old_name != new_name {
        loaded
            .change_material_refs(None, &old_name, &new_name)
            .map_err(|e| ApiError::bad_request(&format!("Cannot rename material: {e}")))?;
    }
    loaded.document.materials = staged;

    Ok(edited(loaded))
}

#[derive(Deserialize)]
pub struct AddMaterialRequest {
    pub material: Material,
    #[serde(default)]
    pub elements: Vec<Element>,
}

pub async fn add_material(
    State(state): State<SharedState>,
    Json(req): Json<AddMaterialRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut state_w = state.write().await;
    let loaded = loaded(state_w.loaded.as_mut())?;

    let mut staged =
        with_staged_elements(&loaded.document.materials, &req.elements, &loaded.engine)?;
    ensure_material_name_available(&staged, &req.material.name, None)?;
    validate_material_components(&staged, &req.material, None)?;
    validate_material_values(&req.material, None, &loaded.engine)?;
    staged.materials.push(req.material);
    ensure_material_order(&staged)?;
    loaded.document.materials = staged;
    Ok(edited(loaded))
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
    let loaded = loaded(state_w.loaded.as_mut())?;

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

    // A volume inside a preserved <loop> counts too: export writes the loop
    // back verbatim, and its reference would dangle.
    let in_use_by_loop = loaded
        .preserved_xml_uses_material(&req.name)
        .map_err(|e| ApiError::internal(&format!("Cannot read preserved XML: {e}")))?;

    let contexts: Vec<&str> = [
        (in_use_by_volume, "one or more volumes"),
        (in_use_by_component, "one or more material components"),
        (in_use_by_loop, "a preserved <loop>"),
    ]
    .into_iter()
    .filter_map(|(used, what)| used.then_some(what))
    .collect();
    if !contexts.is_empty() {
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

    Ok(edited(loaded))
}

// ─── Element CRUD ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AddElementRequest {
    pub element: Element,
}

pub async fn add_element(
    State(state): State<SharedState>,
    Json(req): Json<AddElementRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut state_w = state.write().await;
    let loaded = loaded(state_w.loaded.as_mut())?;

    ensure_element_name_available(&loaded.document.materials, &req.element.name)?;
    validate_element(&loaded.document.materials, &req.element, &loaded.engine)?;

    loaded.document.materials.elements.push(req.element);
    Ok(edited(loaded))
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
    let loaded = loaded(state_w.loaded.as_mut())?;

    if is_name_referenced_in_material_components(&loaded.document.materials.materials, &req.name) {
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

    Ok(edited(loaded))
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
    let loaded = loaded(state_w.loaded.as_mut())?;

    ensure_material_ref_exists(&loaded.document, &req.material_ref)?;

    let vol = loaded
        .document
        .structure
        .volumes
        .iter()
        .find(|v| v.name == req.volume_name)
        .ok_or_else(|| ApiError::not_found(&format!("Volume '{}' not found", req.volume_name)))?;
    if vol.assembly {
        return Err(ApiError::bad_request(&format!(
            "'{}' is an assembly, which has no material",
            vol.name
        )));
    }
    let old = vol.material_ref.clone();
    loaded
        .change_material_refs(Some(&req.volume_name), &old, &req.material_ref)
        .map_err(|e| ApiError::bad_request(&format!("Cannot assign material: {e}")))?;
    Ok(edited(loaded))
}

// ─── Export ─────────────────────────────────────────────────────────────────

pub async fn export_gdml(State(state): State<SharedState>) -> Result<Json<Value>, ApiError> {
    let mut state_w = state.write().await;
    let loaded = loaded(state_w.loaded.as_mut())?;

    let xml = nist::serialize_gdml(&loaded.document)
        .map_err(|e| ApiError::internal(&format!("Serialization error: {}", e)))?;
    loaded.unsaved = false;

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
            root_attributes: Vec::new(),
            materials_define: None,
            order: Default::default(),
            defines: DefineSection::default(),
            materials: MaterialSection::default(),
            solids: SolidSection::default(),
            structure: StructureSection::default(),
            setup: SetupSection {
                name: "Default".to_string(),
                version: "1.0".to_string(),
                world_ref: world_ref.to_string(),
            },
            setups: Vec::new(),
            raw_unknown: Vec::new(),
            skipped_unsupported: Vec::new(),
        }
    }

    /// Shared state holding `document` as if it had just been uploaded.
    fn state_with(document: GdmlDocument) -> SharedState {
        let state = crate::state::app_state::create_shared_state();
        state.try_write().unwrap().loaded = Some(LoadedDocument {
            document,
            render: None,
            engine: EvalEngine::new(),
            meshes: HashMap::new(),
            warnings: Vec::new(),
            unsaved: false,
        });
        state
    }

    fn material(name: &str) -> Material {
        Material {
            name: name.to_string(),
            formula: None,
            z: Some("1".to_string()),
            state: None,
            density: None,
            density_ref: None,
            temperature: None,
            temperature_ref: None,
            pressure: None,
            pressure_ref: None,
            mee: None,
            mee_ref: None,
            rl: None,
            al: None,
            properties: Vec::new(),
            atom_value: Some("1".to_string()),
            atom_unit: None,
            atom_type: None,
            components: Vec::new(),
        }
    }

    fn volume(name: &str, material_ref: &str) -> Volume {
        Volume {
            name: name.to_string(),
            assembly: false,
            material_ref: material_ref.to_string(),
            solid_ref: "Solid".to_string(),
            physvols: Vec::new(),
            auxiliaries: Vec::new(),
            replica: None,
            body_comments: Vec::new(),
            preserved: Vec::new(),
        }
    }

    fn physvol(name: &str, volume_ref: &str) -> PhysVol {
        PhysVol {
            name: Some(name.to_string()),
            volume_ref: volume_ref.to_string(),
            copynumber: None,
            file_ref: None,
            position: None,
            rotation: None,
            scale: None,
        }
    }

    fn file_ref_physvol(file: &str, volname: Option<&str>) -> PhysVol {
        PhysVol {
            name: None,
            volume_ref: String::new(),
            copynumber: None,
            file_ref: Some(FileRef {
                name: file.to_string(),
                volname: volname.map(|s| s.to_string()),
            }),
            position: None,
            rotation: None,
            scale: None,
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

        let child_docs = HashMap::from([
            ("child.gdml".to_string(), child),
            ("grand.gdml".to_string(), grand),
        ]);

        let warnings = resolve_all_file_refs(&mut main, &child_docs);
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
        let mut doc = base_doc("test.gdml", "World");
        let mut world = volume("World", "Steel");
        world.physvols.push(physvol("pv_leaf", "Leaf"));
        doc.structure.volumes.push(world);
        doc.structure.volumes.push(volume("Leaf", "Steel"));

        let steel = material("Steel");
        let mut alloy = material("Alloy");
        // A <fraction> may name a material; a <composite> only an element.
        alloy.components.push(MaterialComponent::Fraction {
            n: "1.0".to_string(),
            ref_name: "Steel".to_string(),
        });
        doc.materials.materials.push(steel);
        doc.materials.materials.push(alloy);

        let state = state_with(doc);

        let req = UpdateMaterialRequest {
            elements: Vec::new(),
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
        let mut doc = base_doc("test.gdml", "World");
        doc.structure.volumes.push(volume("World", "A"));
        doc.materials.materials.push(material("A"));
        doc.materials.materials.push(material("B"));

        let state = state_with(doc);

        let req = UpdateMaterialRequest {
            elements: Vec::new(),
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
        let mut doc = base_doc("test.gdml", "World");
        doc.structure.volumes.push(volume("World", "Alloy"));
        doc.materials.materials.push(material("Steel"));
        let mut alloy = material("Alloy");
        alloy.components.push(MaterialComponent::Fraction {
            n: "1.0".to_string(),
            ref_name: "Steel".to_string(),
        });
        doc.materials.materials.push(alloy);

        let state = state_with(doc);

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
        let mut doc = base_doc("test.gdml", "World");
        doc.structure.volumes.push(volume("World", "Air"));
        doc.materials.materials.push(material("Air"));

        let state = state_with(doc);

        let mut invalid = material("Mixture");
        invalid.components.push(MaterialComponent::Fraction {
            n: "1.0".to_string(),
            ref_name: "MissingRef".to_string(),
        });

        let err = add_material(
            State(state.clone()),
            Json(AddMaterialRequest {
                elements: Vec::new(),
                material: invalid,
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn update_material_rejects_direct_self_reference() {
        let mut doc = base_doc("test.gdml", "World");
        doc.structure.volumes.push(volume("World", "Steel"));
        doc.materials.materials.push(material("Steel"));

        let state = state_with(doc);

        let mut invalid = material("Steel");
        invalid.components.push(MaterialComponent::Composite {
            n: "1".to_string(),
            ref_name: "Steel".to_string(),
        });

        let err = update_material(
            State(state.clone()),
            Json(UpdateMaterialRequest {
                elements: Vec::new(),
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
        let mut doc = base_doc("test.gdml", "World");
        doc.structure.volumes.push(volume("World", "Steel"));
        doc.materials.materials.push(material("Steel"));

        let state = state_with(doc);

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

    /// Merge a "child.gdml" into a "main.gdml" that defines A = 1, where the
    /// child defines A = `child_a`.
    fn merge_child_defining_a(child_a: &str) -> (GdmlDocument, Vec<String>) {
        let constant_a = |value: &str| Constant {
            name: "A".to_string(),
            value: value.to_string(),
        };
        let mut main = base_doc("main.gdml", "MainWorld");
        let mut main_world = volume("MainWorld", "Vacuum");
        main_world
            .physvols
            .push(file_ref_physvol("child.gdml", None));
        main.structure.volumes.push(main_world);
        main.defines.constants.push(constant_a("1"));

        let mut child = base_doc("child.gdml", "ChildWorld");
        child.structure.volumes.push(volume("ChildWorld", "Vacuum"));
        child.defines.constants.push(constant_a(child_a));

        let child_docs = HashMap::from([("child.gdml".to_string(), child)]);
        let warnings = resolve_all_file_refs(&mut main, &child_docs);
        (main, warnings)
    }

    #[test]
    fn resolve_all_file_refs_deduplicates_identical_define_names() {
        let (main, warnings) = merge_child_defining_a("1");
        assert!(warnings.is_empty());
        assert_eq!(main.defines.constants.len(), 1);
    }

    #[test]
    fn resolve_all_file_refs_warns_on_conflicting_define_names() {
        // Geant4 loads such a pair: each module file gets its own evaluator. One
        // namespace cannot hold both values, so the main file's is kept and the
        // clash reported, instead of refusing the whole upload.
        let (main, warnings) = merge_child_defining_a("2");
        assert!(
            warnings.iter().any(|w| w.contains("constant \"A\"")),
            "{warnings:?}"
        );
        assert_eq!(main.defines.constants.len(), 1);
        assert_eq!(main.defines.constants[0].value, "1");
        assert!(
            collect_file_refs(&main).is_empty(),
            "the child was not merged"
        );
    }

    #[test]
    fn repeated_logical_volume_instances_have_unique_instance_ids() {
        let mut doc = base_doc("test.gdml", "World");
        let mut world = volume("World", "Vacuum");
        world.physvols.push(physvol("pv_0", "Leaf"));
        world.physvols.push(physvol("pv_1", "Leaf"));
        doc.structure.volumes.push(world);
        doc.structure.volumes.push(volume("Leaf", "Vacuum"));

        let engine = EvalEngine::new();
        let mut warnings = Vec::new();
        let graph = build_scene_graph(&doc, &doc.materials, &engine, &mut warnings);
        assert_eq!(graph.children.len(), 2);
        assert_eq!(graph.children[0].volume_name, graph.children[1].volume_name);
        assert_ne!(graph.children[0].instance_id, graph.children[1].instance_id);
    }

    #[test]
    fn scene_warnings_are_deduped_with_counts() {
        let mut w = vec![
            "alpha".to_string(),
            "beta".to_string(),
            "alpha".to_string(),
            "alpha".to_string(),
        ];
        dedupe_warnings(&mut w);
        assert_eq!(w, vec!["alpha (x3)".to_string(), "beta".to_string()]);
    }

    #[test]
    fn scene_warnings_are_capped() {
        let mut w: Vec<String> = (0..150).map(|i| format!("w{i}")).collect();
        dedupe_warnings(&mut w);
        assert_eq!(w.len(), 101, "100 warnings plus the overflow note");
        assert!(w.last().unwrap().contains("50 more"));
    }

    #[test]
    fn replica_positions_are_centered_in_mother() {
        let (graph, warnings) = replica_scene(
            None,
            [Some("1".to_string()), None, None],
            "4",
            "10",
            Some("mm"),
            "0",
            None,
        );
        // Geant4 centers the stack: 4 slices of width 10 -> x = -15, -5, +5, +15
        let xs: Vec<f64> = graph.children.iter().map(|c| c.position[0]).collect();
        assert_eq!(xs, vec![-15.0, -5.0, 5.0, 15.0]);
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    }

    /// Build a scene whose world replicates "Slice" along `curvilinear`, or
    /// `direction` when that is `None`.
    fn replica_scene(
        curvilinear: Option<&str>,
        direction: [Option<String>; 3],
        number: &str,
        width: &str,
        width_unit: Option<&str>,
        offset: &str,
        offset_unit: Option<&str>,
    ) -> (SceneNode, Vec<String>) {
        let mut doc = base_doc("test.gdml", "World");
        let mut world = volume("World", "Vacuum");
        world.replica = Some(ReplicaVol {
            volume_ref: "Slice".to_string(),
            curvilinear_axis: curvilinear.map(|s| s.to_string()),
            number: number.to_string(),
            direction,
            width: width.to_string(),
            width_unit: width_unit.map(|s| s.to_string()),
            offset: offset.to_string(),
            offset_unit: offset_unit.map(|s| s.to_string()),
        });
        doc.structure.volumes.push(world);
        doc.structure.volumes.push(volume("Slice", "Vacuum"));

        let engine = EvalEngine::new();
        let mut warnings = Vec::new();
        let graph = build_scene_graph(&doc, &doc.materials, &engine, &mut warnings);
        (graph, warnings)
    }

    #[test]
    fn phi_replicas_are_rotated_not_stacked_along_z() {
        // G4ReplicaNavigation.cc:682 --
        //   val = -(offset + width*(replicaNo + 0.5)); rotateZ(val)
        // Four 90-degree slices with no offset sit at -45, -135, -225, -315
        // degrees in the stored (inverse) convention SceneNode::rotation uses.
        let (graph, warnings) = replica_scene(
            Some("phi"),
            [const { None }; 3],
            "4",
            "90",
            Some("deg"),
            "0",
            Some("deg"),
        );

        let d = std::f64::consts::PI / 180.0;
        let rz: Vec<f64> = graph.children.iter().map(|c| c.rotation[2]).collect();
        let want = [-45.0 * d, -135.0 * d, -225.0 * d, -315.0 * d];
        assert_eq!(rz.len(), 4);
        for (got, exp) in rz.iter().zip(want.iter()) {
            assert!((got - exp).abs() < 1e-9, "rotation {got}, expected {exp}");
        }
        // Nothing may be displaced -- a phi replica is a pure rotation.
        for c in &graph.children {
            assert_eq!(c.position, [0.0, 0.0, 0.0], "phi replica was translated");
        }
        assert!(
            warnings.is_empty(),
            "phi replication should no longer warn: {warnings:?}"
        );
    }

    #[test]
    fn phi_replica_offset_shifts_every_slice() {
        // Unlike the Cartesian axes, kPhi really does read `offset`.
        let (graph, _) = replica_scene(
            Some("phi"),
            [const { None }; 3],
            "2",
            "90",
            Some("deg"),
            "10",
            Some("deg"),
        );
        let d = std::f64::consts::PI / 180.0;
        let rz: Vec<f64> = graph.children.iter().map(|c| c.rotation[2]).collect();
        for (got, exp) in rz.iter().zip([-55.0 * d, -145.0 * d].iter()) {
            assert!((got - exp).abs() < 1e-9, "rotation {got}, expected {exp}");
        }
    }

    #[test]
    fn rho_replication_draws_one_copy_and_says_so() {
        // ComputeTransformation has "No setup required for radial case": there
        // is no placement transform, because each slice is a different solid.
        let (graph, warnings) = replica_scene(
            Some("rho"),
            [const { None }; 3],
            "5",
            "10",
            Some("mm"),
            "0",
            None,
        );
        assert_eq!(
            graph.children.len(),
            1,
            "five coincident copies show exactly what one shows"
        );
        assert_eq!(graph.children[0].position, [0.0, 0.0, 0.0]);
        assert!(
            warnings.iter().any(|w| w.contains("kRho")),
            "radial replication must be reported: {warnings:?}"
        );
    }

    #[test]
    fn cartesian_replica_offset_is_reported_as_inert() {
        // ComputeTransformation's kXAxis case never reads `offset`, so a file
        // carrying one gets a stack that ignores it -- in Geant4 too.
        let (_, warnings) = replica_scene(
            None,
            [Some("1".to_string()), None, None],
            "2",
            "10",
            Some("mm"),
            "5",
            Some("mm"),
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("Geant4 ignores it here too")),
            "expected the offset note: {warnings:?}"
        );
    }
}
