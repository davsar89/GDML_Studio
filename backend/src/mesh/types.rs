use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TriangleMesh {
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub indices: Vec<u32>,
}

impl TriangleMesh {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}
