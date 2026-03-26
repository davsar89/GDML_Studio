use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GdmlDocument {
    pub filename: String,
    pub defines: DefineSection,
    pub materials: MaterialSection,
    pub solids: SolidSection,
    pub structure: StructureSection,
    pub setup: SetupSection,
}

// ─── Define Section ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DefineSection {
    pub constants: Vec<Constant>,
    pub quantities: Vec<Quantity>,
    pub variables: Vec<Variable>,
    pub expressions: Vec<Expression>,
    pub positions: Vec<Position>,
    pub rotations: Vec<Rotation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constant {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quantity {
    pub name: String,
    pub r#type: Option<String>,
    pub value: String,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expression {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub name: String,
    pub x: Option<String>,
    pub y: Option<String>,
    pub z: Option<String>,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rotation {
    pub name: String,
    pub x: Option<String>,
    pub y: Option<String>,
    pub z: Option<String>,
    pub unit: Option<String>,
}

// ─── Materials Section ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MaterialSection {
    pub elements: Vec<Element>,
    pub materials: Vec<Material>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    pub name: String,
    pub formula: Option<String>,
    pub z: Option<String>,
    pub atom_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyValue {
    pub value: String,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    pub name: String,
    pub formula: Option<String>,
    pub z: Option<String>,
    pub density: Option<Density>,
    pub density_ref: Option<String>,
    pub temperature: Option<PropertyValue>,
    pub pressure: Option<PropertyValue>,
    pub atom_value: Option<String>,
    pub components: Vec<MaterialComponent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Density {
    pub value: String,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MaterialComponent {
    Fraction { n: String, ref_name: String },
    Composite { n: String, ref_name: String },
}

// ─── Solids Section ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SolidSection {
    pub solids: Vec<Solid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Solid {
    Box(BoxSolid),
    Tube(TubeSolid),
    Cone(ConeSolid),
    Sphere(SphereSolid),
    Trd(TrdSolid),
    Polycone(PolyconeSolid),
    Xtru(XtruSolid),
    Orb(OrbSolid),
    Tessellated(TessellatedSolid),
    Boolean(BooleanSolid),
}

impl Solid {
    pub fn name(&self) -> &str {
        match self {
            Solid::Box(s) => &s.name,
            Solid::Tube(s) => &s.name,
            Solid::Cone(s) => &s.name,
            Solid::Sphere(s) => &s.name,
            Solid::Trd(s) => &s.name,
            Solid::Polycone(s) => &s.name,
            Solid::Xtru(s) => &s.name,
            Solid::Orb(s) => &s.name,
            Solid::Tessellated(s) => &s.name,
            Solid::Boolean(s) => &s.name,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BooleanOp {
    Subtraction,
    Union,
    Intersection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BooleanSolid {
    pub name: String,
    pub operation: BooleanOp,
    pub first_ref: String,
    pub second_ref: String,
    pub position: Option<PlacementPos>,
    pub rotation: Option<PlacementRot>,
    pub first_position: Option<PlacementPos>,
    pub first_rotation: Option<PlacementRot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxSolid {
    pub name: String,
    pub x: String,
    pub y: String,
    pub z: String,
    pub lunit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TubeSolid {
    pub name: String,
    pub rmin: Option<String>,
    pub rmax: String,
    pub z: String,
    pub startphi: Option<String>,
    pub deltaphi: Option<String>,
    pub aunit: Option<String>,
    pub lunit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConeSolid {
    pub name: String,
    pub rmin1: Option<String>,
    pub rmax1: String,
    pub rmin2: Option<String>,
    pub rmax2: String,
    pub z: String,
    pub startphi: Option<String>,
    pub deltaphi: Option<String>,
    pub aunit: Option<String>,
    pub lunit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SphereSolid {
    pub name: String,
    pub rmin: Option<String>,
    pub rmax: String,
    pub startphi: Option<String>,
    pub deltaphi: Option<String>,
    pub starttheta: Option<String>,
    pub deltatheta: Option<String>,
    pub aunit: Option<String>,
    pub lunit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrdSolid {
    pub name: String,
    pub x1: String,
    pub y1: String,
    pub x2: String,
    pub y2: String,
    pub z: String,
    pub lunit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZPlane {
    pub rmin: Option<String>,
    pub rmax: String,
    pub z: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolyconeSolid {
    pub name: String,
    pub startphi: Option<String>,
    pub deltaphi: Option<String>,
    pub aunit: Option<String>,
    pub lunit: Option<String>,
    pub zplanes: Vec<ZPlane>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwoDimVertex {
    pub x: String,
    pub y: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XtruSection {
    pub z_order: String,
    pub z_position: String,
    pub x_offset: String,
    pub y_offset: String,
    pub scaling_factor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XtruSolid {
    pub name: String,
    pub lunit: Option<String>,
    pub vertices: Vec<TwoDimVertex>,
    pub sections: Vec<XtruSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrbSolid {
    pub name: String,
    pub r: String,
    pub lunit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TessellatedFacet {
    Triangular {
        vertex1: String,
        vertex2: String,
        vertex3: String,
        r#type: Option<String>,
    },
    Quadrangular {
        vertex1: String,
        vertex2: String,
        vertex3: String,
        vertex4: String,
        r#type: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TessellatedSolid {
    pub name: String,
    pub facets: Vec<TessellatedFacet>,
}

// ─── Structure Section ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StructureSection {
    pub volumes: Vec<Volume>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    pub name: String,
    pub material_ref: String,
    pub solid_ref: String,
    pub physvols: Vec<PhysVol>,
    pub auxiliaries: Vec<Auxiliary>,
    pub replica: Option<ReplicaVol>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaVol {
    pub volume_ref: String,
    pub number: String,
    pub direction: [Option<String>; 3],
    pub width: String,
    pub width_unit: Option<String>,
    pub offset: String,
    pub offset_unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRef {
    pub name: String,
    pub volname: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysVol {
    pub name: Option<String>,
    pub volume_ref: String,
    pub file_ref: Option<FileRef>,
    pub position: Option<PlacementPos>,
    pub rotation: Option<PlacementRot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlacementPos {
    Inline(Position),
    Ref(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlacementRot {
    Inline(Rotation),
    Ref(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Auxiliary {
    pub auxtype: String,
    pub auxvalue: String,
}

// ─── Setup Section ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupSection {
    pub name: String,
    pub version: String,
    pub world_ref: String,
}

// ─── Scene/Mesh data for API responses ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneData {
    pub meshes: HashMap<String, MeshData>,
    pub scene_graph: SceneNode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshData {
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneNode {
    pub name: String,
    pub instance_id: String,
    pub volume_name: String,
    pub solid_name: String,
    pub material_name: String,
    pub color: Option<String>,
    pub density: Option<f64>,
    pub position: [f64; 3],
    pub rotation: [f64; 3],
    pub is_world: bool,
    pub children: Vec<SceneNode>,
}
