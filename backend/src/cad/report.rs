//! The import report: what each part became (its exact solid, or why it has
//! none) and what was repaired, rejected, excluded or found to overlap, for
//! the Import panel.

use serde_json::{json, Value};

use super::{Representation, Session, PLACEHOLDER_MATERIAL};

/// Approximate size of saved GDML per vertex (a `<position>` with
/// coordinates of up to 17 significant digits), per facet, and per part (its
/// solids, volume and placement, and its share of the rest).
const GDML_BYTES_PER_VERTEX: usize = 105;
const GDML_BYTES_PER_FACET: usize = 70;
const GDML_BYTES_PER_PART: usize = 1_000;

pub fn report(session: &Session) -> Value {
    let ir = &session.ir;
    let mut instances = vec![0usize; ir.definitions.len()];
    for (d, _) in ir.instances() {
        instances[d] += 1;
    }
    let shown_exact = |d: &super::ir::Definition| {
        session.options.representation(&d.id) == Representation::Auto && d.analytic.is_ok()
    };
    let definitions: Vec<Value> = ir
        .definitions
        .iter()
        .zip(&instances)
        .map(|(d, instances)| {
            let b = d.mesh.bounds();
            json!({
                "id": d.id,
                "label": d.label,
                "volume": session.lv_of_def.get(&d.id),
                "excluded": session.options.representation(&d.id) == Representation::Exclude,
                "representation": if shown_exact(d) { "exact" } else { "tessellated" },
                "analytic": d.analytic.as_ref().ok().map(|a| a.summary()),
                "analytic_reason": d.analytic.as_ref().err(),
                "epsilon_mm": d.analytic.as_ref().ok().map(|a| a.epsilon_mm),
                "instances": instances,
                "exact": d.exact,
                "issues": d.issues,
                "facets": d.mesh.triangles.len(),
                "vertices": d.mesh.positions.len(),
                "shells": d.shells,
                "volume_mm3": d.mesh.volume(),
                "area_mm2": d.mesh.area(),
                "bounds_mm": [b.lo, b.hi],
                "repairs": d.repairs,
                "reversed_facets": d.reversed_facets,
            })
        })
        .collect();
    let id = |d: usize| &ir.definitions[d].id;
    let interferences: Vec<Value> = (ir.interferences.iter())
        .map(|i| {
            json!({
                "paths": i.paths,
                "definitions": i.definitions.map(|d| d.map(id)),
                "volume_mm3": i.volume_mm3,
            })
        })
        .collect();
    let crossings: Vec<[&String; 2]> = (ir.crossings.iter())
        .map(|&(a, b)| [id(a), id(b)])
        .collect();
    // An exact solid touching a mesh: the mesh can cut into it by its chord.
    let included = |d: usize| session.lv_of_def.contains_key(id(d));
    let exact = |d: usize| shown_exact(&ir.definitions[d]);
    let mixed_contacts: Vec<[&String; 2]> = (ir.contacts.iter())
        .filter(|&&(a, b)| included(a) && included(b) && exact(a) != exact(b))
        .map(|&(a, b)| [id(a), id(b)])
        .collect();
    let parts = (ir.definitions.iter()).filter(|d| session.lv_of_def.contains_key(&d.id));
    let tessellated = parts.clone().filter(|d| !shown_exact(d));
    let (vertices, facets) = tessellated.fold((0, 0), |(v, f), d| {
        (v + d.mesh.positions.len(), f + d.mesh.triangles.len())
    });
    json!({
        "filename": session.filename,
        "format": ir.format,
        "bytes": session.bytes.len(),
        "options": session.options,
        "placeholder_material": PLACEHOLDER_MATERIAL,
        "definitions": definitions,
        "rejected": ir.rejected,
        "interferences": interferences,
        "unchecked": ir.unchecked,
        "crossings": crossings,
        "mixed_contacts": mixed_contacts,
        "facets": facets,
        "gdml_bytes_estimate": parts.count() * GDML_BYTES_PER_PART
            + vertices * GDML_BYTES_PER_VERTEX
            + facets * GDML_BYTES_PER_FACET,
    })
}
