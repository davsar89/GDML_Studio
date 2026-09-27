//! Helpers shared by the test crates; each uses a different subset.
#![allow(dead_code)]

use std::collections::HashMap;

use gdml_studio_backend::eval::engine::EvalEngine;
use gdml_studio_backend::gdml::parser::parse_gdml_from_bytes;
use gdml_studio_backend::mesh::tessellator::tessellate_all_solids;
use gdml_studio_backend::mesh::types::TriangleMesh;

/// Signed volume of the mesh about reference point `o` (tetrahedron sum).
pub fn signed_volume_about(mesh: &TriangleMesh, o: [f64; 3]) -> f64 {
    let v = |i: usize| -> [f64; 3] {
        [
            mesh.positions[i * 3] as f64 - o[0],
            mesh.positions[i * 3 + 1] as f64 - o[1],
            mesh.positions[i * 3 + 2] as f64 - o[2],
        ]
    };
    let mut vol = 0.0;
    for t in 0..mesh.triangle_count() {
        let a = v(mesh.indices[t * 3] as usize);
        let b = v(mesh.indices[t * 3 + 1] as usize);
        let c = v(mesh.indices[t * 3 + 2] as usize);
        // (a x b) . c / 6
        vol += (a[1] * b[2] - a[2] * b[1]) * c[0]
            + (a[2] * b[0] - a[0] * b[2]) * c[1]
            + (a[0] * b[1] - a[1] * b[0]) * c[2];
    }
    vol / 6.0
}

/// Tessellate a `<solids>` fragment through the real path: parser, evaluator
/// and tessellator; and the warnings.
pub fn tessellate(solids: &str, segments: u32) -> (HashMap<String, TriangleMesh>, Vec<String>) {
    let gdml = format!("<gdml><solids>{solids}</solids></gdml>");
    let doc = parse_gdml_from_bytes(gdml.as_bytes(), "t.gdml".into()).unwrap();
    let mut engine = EvalEngine::new();
    engine.evaluate_all(&doc.defines).unwrap();
    tessellate_all_solids(&doc.solids, &engine, segments).unwrap()
}
