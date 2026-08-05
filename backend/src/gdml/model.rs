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
    /// Elements the parser recognizes by name but does not interpret
    /// (e.g. `<opticalsurface>`, `<skinsurface>`, `<userinfo>`, `<loop>`).
    /// Stored verbatim so they survive a load → save round-trip instead of
    /// being silently dropped.
    #[serde(default)]
    pub raw_unknown: Vec<RawElement>,
    /// Every `<setup>` in the source, in document order.
    ///
    /// `setup` above is the *selected* one — the block named "Default" if there
    /// is one, else the first — matching `G4GDMLParser::Read`. Keeping the rest
    /// means an alternative setup is not deleted on save.
    #[serde(default)]
    pub setups: Vec<SetupSection>,
    /// Comment placement, so a load → save round trip does not delete the user's
    /// annotations. See [`DocumentOrder`].
    #[serde(default)]
    pub order: DocumentOrder,
    /// Attributes on the `<gdml>` root, captured verbatim. The writer used to
    /// hardcode two xsi attributes, so anything else — most commonly
    /// `xmlns:gdml`, present in 5 of the 10 shipped samples — was dropped.
    #[serde(default)]
    pub root_attributes: Vec<(String, String)>,
    /// Names of the defines that appeared inside `<materials><define>` rather
    /// than the top-level `<define>`.
    ///
    /// GDML allows a `<define>` block nested in `<materials>`; both large
    /// samples use one to carry `universe_mean_density`. Defines are global
    /// whichever block they sit in, so everything is parsed into
    /// [`Self::defines`] as before and this only records where each item came
    /// from, letting the writer put it back. `None` means the source had no
    /// nested block; `Some(vec![])` means it had an empty one.
    #[serde(default)]
    pub materials_define: Option<Vec<String>>,
    /// Human-readable notes about unsupported constructs the parser had to
    /// skip entirely (e.g. `<divisionvol>` inside a volume). Surfaced as load
    /// warnings: these are NOT preserved and will be missing from a save.
    #[serde(default)]
    pub skipped_unsupported: Vec<String>,
}

/// One run of XML comments, anchored to the element that followed it.
///
/// Comments are stored by *anchor* rather than by absolute index because the
/// document is a set of typed collections that the writer emits in a fixed
/// order — there is no single position a comment could be pinned to. Anchoring
/// to the name of the next element keeps a comment attached to the thing it
/// describes even though the surrounding items may be re-ordered on write.
///
/// A comment anchored to an element that is later deleted goes with it, which is
/// the desired behaviour: a comment about a material the user removed should not
/// outlive it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentAnchor {
    /// `"root"`, `"define"`, `"materials"`, `"solids"` or `"structure"`.
    pub section: String,
    /// `name` of the element this run precedes, or `None` for comments at the
    /// end of a section with nothing after them.
    pub before: Option<String>,
    pub text: String,
}

/// Where the source document's comments were, so they can be put back.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentOrder {
    /// Comments before the `<gdml>` element.
    pub prolog: Vec<String>,
    pub anchors: Vec<CommentAnchor>,
    /// Comments after `</gdml>`.
    pub epilog: Vec<String>,
    /// The `<!DOCTYPE ...>` declaration, verbatim and including any internal
    /// subset, so it is not deleted on save.
    ///
    /// quick-xml counts `<`/`>` within the internal subset, so
    /// `<!DOCTYPE gdml [<!ENTITY size "10">]>` arrives intact. Entity
    /// *references* are not expanded — see the note in the parser.
    #[serde(default)]
    pub doctype: Option<String>,
    /// The `<define>` children in the order the source declared them.
    ///
    /// The writer otherwise emits one typed collection at a time — every
    /// constant, then every quantity, and so on — which can move a define ahead
    /// of one it references. `G4GDMLReadDefine::DefineRead` is a single forward
    /// pass over the children and evaluates each `value` inline as it reads it
    /// (`G4GDMLReadDefine.cc:601` and `:203`), so a forward reference is fatal
    /// there even though this project's own evaluator sorts topologically and
    /// never notices.
    ///
    /// Empty for programmatically built documents, which fall back to the
    /// grouped order.
    #[serde(default)]
    pub define_slots: Vec<DefineSlot>,
}

/// One `<define>` child, identifying which collection it lives in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefineSlot {
    pub kind: DefineKind,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DefineKind {
    Constant,
    Quantity,
    Variable,
    Expression,
    Position,
    Rotation,
    Scale,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawElement {
    /// The section this element was captured from, so it can be re-emitted
    /// there.
    ///
    /// The writer used to infer the section from the tag name alone. That is
    /// right for every tag it handles except `<loop>`, which is legal in
    /// `<define>`, `<solids>` *and* `<structure>` — so a loop in solids or
    /// structure was re-emitted nested inside `<define>`, producing a file
    /// Geant4 will not load. Recording the section is right by construction and
    /// removes the whole class of bug.
    #[serde(default)]
    pub section: Option<String>,
    pub tag: String,
    pub xml: String,
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
    /// Named `<scale>` elements. Previously swallowed into `raw_unknown`, which
    /// preserved them on save but left nothing for `<scaleref>` to resolve
    /// against.
    #[serde(default)]
    pub scales: Vec<Scale>,
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

/// A named `<scale>` in `<define>`, referenced by `<scaleref>`.
///
/// Structurally identical to [`Position`], but kept separate because it is
/// dimensionless — a scale factor has no `lunit`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scale {
    pub name: String,
    pub x: Option<String>,
    pub y: Option<String>,
    pub z: Option<String>,
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
    #[serde(default)]
    pub isotopes: Vec<Isotope>,
    pub elements: Vec<Element>,
    pub materials: Vec<Material>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Isotope {
    pub name: String,
    pub n: Option<String>,
    pub z: Option<String>,
    pub atom_value: Option<String>,
    /// `<atom unit="g/mole"/>` — read but previously discarded.
    #[serde(default)]
    pub atom_unit: Option<String>,
    /// `<atom type="A"/>`.
    #[serde(default)]
    pub atom_type: Option<String>,
}

/// `<property name="RINDEX" ref="rindexMatrix"/>` — how an optical material
/// binds a named `<matrix>`. Dropping these silently removes every optical
/// property from the file while leaving the matrices they referenced behind.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialProperty {
    pub name: String,
    #[serde(default)]
    pub ref_name: Option<String>,
    #[serde(default)]
    pub values: Option<String>,
}

/// A `<fraction n=".." ref=".."/>` entry used in isotope-defined elements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fraction {
    pub n: String,
    pub ref_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    pub name: String,
    pub formula: Option<String>,
    pub z: Option<String>,
    pub atom_value: Option<String>,
    #[serde(default)]
    pub atom_unit: Option<String>,
    #[serde(default)]
    pub atom_type: Option<String>,
    /// Isotope composition for elements defined via `<fraction>` children.
    #[serde(default)]
    pub fractions: Vec<Fraction>,
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
    /// Physical state: "solid" | "liquid" | "gas" (GDML `state` attribute).
    #[serde(default)]
    pub state: Option<String>,
    pub density: Option<Density>,
    pub density_ref: Option<String>,
    pub temperature: Option<PropertyValue>,
    pub pressure: Option<PropertyValue>,
    /// Mean excitation energy (`<MEE value=".." unit=".."/>`).
    #[serde(default)]
    pub mee: Option<PropertyValue>,
    pub atom_value: Option<String>,
    #[serde(default)]
    pub atom_unit: Option<String>,
    #[serde(default)]
    pub atom_type: Option<String>,
    /// `<property>` bindings — the optical property table.
    #[serde(default)]
    pub properties: Vec<MaterialProperty>,
    /// `<RL value=".." unit=".."/>` radiation length.
    #[serde(default)]
    pub rl: Option<PropertyValue>,
    /// `<AL value=".." unit=".."/>` absorption length.
    #[serde(default)]
    pub al: Option<PropertyValue>,
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
    Torus(TorusSolid),
    Trap(TrapSolid),
    Para(ParaSolid),
    CutTube(CutTubeSolid),
    Polyhedra(PolyhedraSolid),
    Tessellated(TessellatedSolid),
    Ellipsoid(EllipsoidSolid),
    Eltube(EltubeSolid),
    Tet(TetSolid),
    GenericPolycone(GenericPolyconeSolid),
    Hype(HypeSolid),
    Elcone(ElconeSolid),
    Paraboloid(ParaboloidSolid),
    GenericPolyhedra(GenericPolyhedraSolid),
    Arb8(Arb8Solid),
    TwistedTubs(TwistedTubsSolid),
    TwistedBox(TwistedBoxSolid),
    TwistedTrap(TwistedTrapSolid),
    TwistedTrd(TwistedTrdSolid),
    Scaled(ScaledSolidDef),
    Reflected(ReflectedSolidDef),
    MultiUnion(MultiUnionSolid),
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
            Solid::Torus(s) => &s.name,
            Solid::Trap(s) => &s.name,
            Solid::Para(s) => &s.name,
            Solid::CutTube(s) => &s.name,
            Solid::Polyhedra(s) => &s.name,
            Solid::Tessellated(s) => &s.name,
            Solid::Ellipsoid(s) => &s.name,
            Solid::Eltube(s) => &s.name,
            Solid::Tet(s) => &s.name,
            Solid::GenericPolycone(s) => &s.name,
            Solid::Hype(s) => &s.name,
            Solid::Elcone(s) => &s.name,
            Solid::Paraboloid(s) => &s.name,
            Solid::GenericPolyhedra(s) => &s.name,
            Solid::Arb8(s) => &s.name,
            Solid::TwistedTubs(s) => &s.name,
            Solid::TwistedBox(s) => &s.name,
            Solid::TwistedTrap(s) => &s.name,
            Solid::TwistedTrd(s) => &s.name,
            Solid::Scaled(s) => &s.name,
            Solid::Reflected(s) => &s.name,
            Solid::MultiUnion(s) => &s.name,
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
pub struct PolyhedraSolid {
    pub name: String,
    pub startphi: Option<String>,
    pub deltaphi: Option<String>,
    pub numsides: String,
    pub aunit: Option<String>,
    pub lunit: Option<String>,
    pub zplanes: Vec<ZPlane>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CutTubeSolid {
    pub name: String,
    pub rmin: Option<String>,
    pub rmax: String,
    pub z: String,
    pub startphi: Option<String>,
    pub deltaphi: Option<String>,
    pub low_x: Option<String>,
    pub low_y: Option<String>,
    pub low_z: Option<String>,
    pub high_x: Option<String>,
    pub high_y: Option<String>,
    pub high_z: Option<String>,
    pub aunit: Option<String>,
    pub lunit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParaSolid {
    pub name: String,
    pub x: String,
    pub y: String,
    pub z: String,
    pub alpha: Option<String>,
    pub theta: Option<String>,
    pub phi: Option<String>,
    pub aunit: Option<String>,
    pub lunit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrapSolid {
    pub name: String,
    pub z: String,
    pub theta: Option<String>,
    pub phi: Option<String>,
    pub y1: String,
    pub x1: String,
    pub x2: String,
    pub alpha1: Option<String>,
    pub y2: String,
    pub x3: String,
    pub x4: String,
    pub alpha2: Option<String>,
    pub aunit: Option<String>,
    pub lunit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorusSolid {
    pub name: String,
    pub rmin: Option<String>,
    pub rmax: String,
    pub rtor: String,
    pub startphi: Option<String>,
    pub deltaphi: Option<String>,
    pub aunit: Option<String>,
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
    /// `lunit`/`aunit` are inert for tessellated solids — vertices are resolved
    /// through named `<position>` defines, which already carry their own units —
    /// but they are written in real files and were dropped on export.
    #[serde(default)]
    pub lunit: Option<String>,
    #[serde(default)]
    pub aunit: Option<String>,
    pub facets: Vec<TessellatedFacet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EllipsoidSolid {
    pub name: String,
    pub ax: String,
    pub by: String,
    pub cz: String,
    pub zcut1: Option<String>,
    pub zcut2: Option<String>,
    pub lunit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EltubeSolid {
    pub name: String,
    pub dx: String,
    pub dy: String,
    pub dz: String,
    pub lunit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TetSolid {
    pub name: String,
    pub vertex1: String,
    pub vertex2: String,
    pub vertex3: String,
    pub vertex4: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RZPoint {
    pub r: String,
    pub z: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericPolyconeSolid {
    pub name: String,
    pub startphi: Option<String>,
    pub deltaphi: Option<String>,
    pub aunit: Option<String>,
    pub lunit: Option<String>,
    pub rzpoints: Vec<RZPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypeSolid {
    pub name: String,
    pub rmin: Option<String>,
    pub rmax: String,
    pub inst: Option<String>,
    pub outst: Option<String>,
    pub z: String,
    pub aunit: Option<String>,
    pub lunit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElconeSolid {
    pub name: String,
    pub dx: String,
    pub dy: String,
    pub zmax: String,
    pub zcut: String,
    pub lunit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParaboloidSolid {
    pub name: String,
    pub rlo: String,
    pub rhi: String,
    pub dz: String,
    pub lunit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericPolyhedraSolid {
    pub name: String,
    pub startphi: Option<String>,
    pub deltaphi: Option<String>,
    pub numsides: String,
    pub aunit: Option<String>,
    pub lunit: Option<String>,
    pub rzpoints: Vec<RZPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Arb8Solid {
    pub name: String,
    pub dz: String,
    pub v1x: String,
    pub v1y: String,
    pub v2x: String,
    pub v2y: String,
    pub v3x: String,
    pub v3y: String,
    pub v4x: String,
    pub v4y: String,
    pub v5x: String,
    pub v5y: String,
    pub v6x: String,
    pub v6y: String,
    pub v7x: String,
    pub v7y: String,
    pub v8x: String,
    pub v8y: String,
    pub lunit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// `<twistedtubs>`.
///
/// Two mutually exclusive parameterisations, selected by `zlen`
/// (`G4GDMLReadSolids.cc:3330`): non-zero picks the end-radius form, zero picks
/// the mid-radius form with explicit z bounds. Note `zlen` is a HALF-length --
/// it is passed to the constructor's `halfzlen` parameter, which then calls
/// `SetFields(..., -halfzlen, halfzlen)`.
///
/// The sweep is `totphi / nseg` when `nseg` is given, else `phi`.
#[derive(Default)]
pub struct TwistedTubsSolid {
    pub name: String,
    pub twistedangle: String,
    pub endinnerrad: Option<String>,
    pub endouterrad: Option<String>,
    /// Half-length, not full length. Absent or zero selects the mid-radius form.
    pub zlen: Option<String>,
    pub phi: Option<String>,
    #[serde(default)]
    pub midinnerrad: Option<String>,
    #[serde(default)]
    pub midouterrad: Option<String>,
    #[serde(default)]
    pub negative_endz: Option<String>,
    #[serde(default)]
    pub positive_endz: Option<String>,
    #[serde(default)]
    pub nseg: Option<String>,
    #[serde(default)]
    pub totphi: Option<String>,
    pub aunit: Option<String>,
    pub lunit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwistedBoxSolid {
    pub name: String,
    pub phi_twist: String,
    pub x: String,
    pub y: String,
    pub z: String,
    pub aunit: Option<String>,
    pub lunit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwistedTrapSolid {
    pub name: String,
    pub phi_twist: String,
    pub z: String,
    pub theta: String,
    pub phi: String,
    pub y1: String,
    pub x1: String,
    pub x2: String,
    pub y2: String,
    pub x3: String,
    pub x4: String,
    pub alph: String,
    pub aunit: Option<String>,
    pub lunit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwistedTrdSolid {
    pub name: String,
    pub phi_twist: String,
    pub x1: String,
    pub x2: String,
    pub y1: String,
    pub y2: String,
    pub z: String,
    pub aunit: Option<String>,
    pub lunit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaledSolidDef {
    pub name: String,
    pub solid_ref: String,
    pub scale_x: String,
    pub scale_y: String,
    pub scale_z: String,
    /// Set when the source wrote `<scaleref ref=".."/>` instead of an inline
    /// `<scale>`. Geant4 accepts both; only the inline form was handled, so a
    /// scaleref silently fell back to (1,1,1) — an unscaled render — and was
    /// rewritten as a fabricated inline scale on export.
    #[serde(default)]
    pub scale_ref: Option<String>,
    /// `name` of the inline `<scale>`, so it round-trips instead of being
    /// regenerated as "{solid}_scale".
    #[serde(default)]
    pub scale_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectedSolidDef {
    pub name: String,
    pub solid_ref: String,
    pub sx: String,
    pub sy: String,
    pub sz: String,
    pub rx: String,
    pub ry: String,
    pub rz: String,
    pub dx: String,
    pub dy: String,
    pub dz: String,
    pub aunit: Option<String>,
    pub lunit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiUnionNode {
    /// `<multiUnionNode name="..">`. Present in sample_data/solids.gdml and
    /// dropped on export before this was modelled.
    #[serde(default)]
    pub name: Option<String>,
    pub solid_ref: String,
    pub position: Option<PlacementPos>,
    pub rotation: Option<PlacementRot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiUnionSolid {
    pub name: String,
    pub nodes: Vec<MultiUnionNode>,
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
    /// Comments found inside this volume's body.
    ///
    /// Re-emitted together at the top of the volume rather than at their exact
    /// original positions: a physvol usually has no `name`, so there is nothing
    /// stable to anchor an individual comment to. Content is preserved; position
    /// within the volume is not. In practice these are commented-out physvols
    /// and section banners, for which that is adequate.
    #[serde(default)]
    pub body_comments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaVol {
    pub volume_ref: String,
    pub number: String,
    pub direction: [Option<String>; 3],
    /// `"rho"` or `"phi"` when the source replicated along a curvilinear axis.
    ///
    /// Geant4's `AxisRead` accepts these alongside x/y/z, but only Cartesian
    /// replication is implemented here — and the axis selector falls through to
    /// z, so such a replica used to render as a z-stack with nothing said.
    #[serde(default)]
    pub curvilinear_axis: Option<String>,
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
    /// `<physvol copynumber="..">`. Geant4 reads it and hands it to
    /// G4PVPlacement, where it is the copy number used for sensitive-detector
    /// and readout identity — so dropping it on export corrupts the detector,
    /// not just its description.
    #[serde(default)]
    pub copynumber: Option<String>,
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
    #[serde(default)]
    pub auxunit: Option<String>,
    /// Nested `<auxiliary>` children — e.g. production cuts inside a Region.
    /// Only the self-closing form was handled before, so a nested cut was
    /// silently re-parented onto the volume as a sibling of its own Region.
    #[serde(default)]
    pub children: Vec<Auxiliary>,
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
