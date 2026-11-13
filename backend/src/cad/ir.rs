//! The intermediate representation between reading a CAD file and building
//! its GDML document: what was found in the file, in mm, with what was
//! repaired and what was rejected.

use serde::{Deserialize, Serialize};

use super::mesh::{Mesh, RepairLog};
use super::transform::Affine;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Stl,
    Step,
}

#[derive(Clone, Debug)]
pub struct CadImport {
    pub format: Format,
    pub definitions: Vec<Definition>,
    /// The assemblies of the source, which the document can keep.
    pub assemblies: Vec<Assembly>,
    /// What the world holds, in source order.
    pub roots: Vec<Child>,
    pub rejected: Vec<Rejected>,
    /// Placed solids of the source whose interiors overlap (STEP).
    pub interferences: Vec<Interference>,
    /// Pairs of placed solids the interference check did not decide.
    pub unchecked: usize,
    /// Pairs of definitions some placements of which have crossing meshes.
    pub crossings: Vec<(usize, usize)>,
    /// Pairs of definitions some placements of which come closer than their
    /// meshes' chordal deviations (STEP), in order.
    pub contacts: Vec<(usize, usize)>,
}

/// One solid of the source: one `<tessellated>` solid, or an exact one, and
/// its volume.
#[derive(Clone, Debug)]
pub struct Definition {
    /// Stable across re-imports of the same file: overrides and material
    /// assignments are keyed on it.
    pub id: String,
    /// The name the source gives it.
    pub label: String,
    /// Closed and wound outward, in mm, in the part's own frame.
    pub mesh: Mesh,
    /// The CAD kernel's properties of the solid (STEP only).
    pub exact: Option<Exact>,
    /// The exact GDML solid proven equal to it, or why there is none.
    pub analytic: Result<Analytic, String>,
    pub shells: usize,
    pub repairs: RepairLog,
    /// Facets whose winding was reversed.
    pub reversed_facets: usize,
    /// What the source or its reading flagged without preventing the import.
    pub issues: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Exact {
    pub volume_mm3: f64,
    pub area_mm2: f64,
    /// Minimum x, y, z, then maximum, in the part's frame.
    pub bounds_mm: [f64; 6],
    /// The largest vertex, edge or face tolerance of the B-rep.
    pub tolerance_mm: f64,
    /// The chordal deviation of the mesh.
    pub chord_mm: f64,
}

/// A part as `stock` minus the union of `tools`, which the STEP worker
/// proved equal to its CAD solid within `epsilon_mm` (algorithm E).
#[derive(Clone, Debug, Deserialize)]
pub struct Analytic {
    pub epsilon_mm: f64,
    pub stock: Primitive,
    pub tools: Vec<Primitive>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Primitive {
    #[serde(flatten)]
    pub shape: Shape,
    /// Its frame in the part's frame: 3 x 4, row-major, translation in mm.
    pub frame: [f64; 12],
}

/// A GDML solid in its own frame, with GDML's meaning of its numbers: mm,
/// rad, `startphi` 0, box/tube/cone `z` a full length, points coordinates
/// (cad-worker/src/certify.cpp builds the same solid from them).
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "solid", rename_all = "camelCase")]
pub enum Shape {
    Box {
        x: f64,
        y: f64,
        z: f64,
    },
    /// A prism of the (x, y) polygon `points`, clockwise, from `z1` to `z2`.
    Xtru {
        points: Vec<[f64; 2]>,
        z1: f64,
        z2: f64,
    },
    Tube {
        rmin: f64,
        rmax: f64,
        z: f64,
        deltaphi: f64,
    },
    Cone {
        rmin1: f64,
        rmax1: f64,
        rmin2: f64,
        rmax2: f64,
        z: f64,
        deltaphi: f64,
    },
    /// Swept by the (r, z) polygon `points`.
    GenericPolycone {
        points: Vec<[f64; 2]>,
        deltaphi: f64,
    },
    Sphere {
        rmin: f64,
        rmax: f64,
        deltaphi: f64,
        starttheta: f64,
        deltatheta: f64,
    },
    Orb {
        r: f64,
    },
    Torus {
        rmin: f64,
        rmax: f64,
        rtor: f64,
        deltaphi: f64,
    },
}

impl Analytic {
    /// "tube", or "box - 2 cut-outs".
    pub fn summary(&self) -> String {
        let stock = self.stock.shape.name();
        match self.tools.len() {
            0 => stock.to_string(),
            1 => format!("{stock} - 1 cut-out"),
            n => format!("{stock} - {n} cut-outs"),
        }
    }
}

impl Primitive {
    /// Its frame: where its own coordinates sit in the part's frame.
    pub fn at(&self) -> Affine {
        Affine::from_rows(&self.frame)
    }
}

impl Shape {
    /// Its GDML element.
    pub fn name(&self) -> &'static str {
        match self {
            Shape::Box { .. } => "box",
            Shape::Xtru { .. } => "xtru",
            Shape::Tube { .. } => "tube",
            Shape::Cone { .. } => "cone",
            Shape::GenericPolycone { .. } => "genericPolycone",
            Shape::Sphere { .. } => "sphere",
            Shape::Orb { .. } => "orb",
            Shape::Torus { .. } => "torus",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Assembly {
    pub label: String,
    pub children: Vec<Child>,
}

#[derive(Clone, Copy, Debug)]
pub struct Child {
    pub target: Target,
    /// Where the child sits in its parent.
    pub at: Affine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Target {
    Definition(usize),
    Assembly(usize),
}

/// Two placed solids of the source that overlap by `volume_mm3`.
#[derive(Clone, Debug)]
pub struct Interference {
    /// Each solid's labels from the top of the assembly down, joined by " / ".
    pub paths: [String; 2],
    /// Their definitions, when they were imported.
    pub definitions: [Option<usize>; 2],
    pub volume_mm3: f64,
}

/// A part of the source that could not be imported.
#[derive(Clone, Debug, Serialize)]
pub struct Rejected {
    pub label: String,
    pub reason: String,
}

impl CadImport {
    /// Every placement of a definition in the world, depth first in source
    /// order: the definition and its world transform.
    pub fn instances(&self) -> Vec<(usize, Affine)> {
        let mut out = Vec::new();
        self.collect(&self.roots, Affine::IDENTITY, &mut out);
        out
    }

    fn collect(&self, children: &[Child], at: Affine, out: &mut Vec<(usize, Affine)>) {
        for child in children {
            let at = at.after(&child.at);
            match child.target {
                Target::Definition(d) => out.push((d, at)),
                Target::Assembly(a) => self.collect(&self.assemblies[a].children, at, out),
            }
        }
    }
}
