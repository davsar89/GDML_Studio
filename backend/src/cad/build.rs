//! The GdmlDocument of an import (algorithm H in `doc/cad-import/PLAN.md`).
//!
//! Each included definition becomes a `<tessellated>` solid and a logical
//! volume. The source's assemblies become `<assembly>` volumes placed as the
//! source places them, or, flattened, every part is placed in the world at
//! its composed transform; Geant4 places the same solids either way. The
//! world is a box around the source origin. Coordinates stay those of the
//! source, in mm, written with Rust's shortest round-trip formatting so that
//! Geant4 reads back exactly the `f64` values computed here.

use std::collections::BTreeMap;

use super::intersect::Aabb;
use super::ir::{CadImport, Child, Definition, Target};
use super::mesh::Mesh;
use super::names::Names;
use super::transform::Affine;
use super::{Hierarchy, Options, Representation, PLACEHOLDER_MATERIAL};
use crate::gdml::model::*;

pub const WORLD_VOLUME: &str = "World";
pub const WORLD_SOLID: &str = "World_box";
/// Geant4 builds `G4_*` materials from its NIST table when a document names
/// one it does not define (G4GDMLReadMaterials::GetMaterial), so the world
/// needs no definition.
pub const WORLD_MATERIAL: &str = "G4_Galactic";

pub struct Built {
    pub document: GdmlDocument,
    /// The logical volume of each included definition.
    pub lv_of_def: BTreeMap<String, String>,
}

/// What a volume holds: (what it places, where) in source order.
type Contents = Vec<(Target, Affine)>;

/// The document for the definitions `options` includes. `material_of` names
/// each one's material; `materials` defines those that need defining, except
/// the placeholder, which is added when some volume uses it.
pub fn document(
    filename: &str,
    ir: &CadImport,
    options: &Options,
    mut materials: MaterialSection,
    material_of: impl Fn(&Definition) -> String,
) -> Result<Built, String> {
    let included: Vec<bool> = (ir.definitions.iter())
        .map(|d| options.representation(&d.id) != Representation::Exclude)
        .collect();
    // The contents of the world and of each assembly kept.
    let mut assemblies = Vec::new();
    let world = match options.hierarchy {
        Hierarchy::Keep => keep(
            ir,
            &ir.roots,
            &included,
            &mut vec![None; ir.assemblies.len()],
            &mut assemblies,
        ),
        Hierarchy::Flatten => (ir.instances().into_iter())
            .filter(|&(d, _)| included[d])
            .map(|(d, at)| (Target::Definition(d), at))
            .collect(),
    };
    if world.is_empty() {
        return Err("Every part is excluded; include at least one to import.".to_string());
    }
    let mut count: BTreeMap<Target, usize> = BTreeMap::new();
    for (target, _) in assemblies.iter().flat_map(|(_, c)| c).chain(&world) {
        *count.entry(*target).or_default() += 1;
    }

    // Names, definitions first, then assemblies. Placements are `_pv`, or
    // `_pv0`, `_pv1`, ... with those copy numbers; the position, rotation
    // and scale of each are named after it, as Geant4's writer does, since
    // the schema requires their names.
    let mut names = Names::default();
    names.reserve(WORLD_VOLUME);
    names.reserve(WORLD_SOLID);
    let pv = |n: usize, i: usize| match n {
        1 => "_pv".to_string(),
        _ => format!("_pv{i}"),
    };
    let mut base: BTreeMap<Target, String> = BTreeMap::new();
    for (&target, &n) in &count {
        let (label, own): (&str, &[&str]) = match target {
            Target::Definition(d) => (&ir.definitions[d].label, &["", "_lv"]),
            Target::Assembly(a) => (&ir.assemblies[a].label, &["_asm"]),
        };
        let mut suffixes: Vec<String> = own.iter().map(|s| s.to_string()).collect();
        for i in 0..n {
            suffixes.extend(["", "_pos", "_rot", "_scl"].map(|s| format!("{}{s}", pv(n, i))));
        }
        base.insert(target, names.claim(label, &suffixes));
    }
    let volume_name = |target: &Target| match target {
        Target::Definition(_) => format!("{}_lv", base[target]),
        Target::Assembly(_) => format!("{}_asm", base[target]),
    };
    let mut placed: BTreeMap<Target, usize> = BTreeMap::new();
    let mut physvols = |contents: &Contents| -> Vec<PhysVol> {
        (contents.iter())
            .map(|(target, at)| {
                let (n, i) = (count[target], placed.entry(*target).or_default());
                *i += 1;
                physvol(
                    &format!("{}{}", base[target], pv(n, *i - 1)),
                    &volume_name(target),
                    (n > 1).then_some(*i - 1),
                    at,
                )
            })
            .collect()
    };

    let mut defines = DefineSection::default();
    let mut solids = Vec::new();
    let mut volumes = Vec::new();
    let mut lv_of_def = BTreeMap::new();
    for (k, target) in count.keys().enumerate() {
        let Target::Definition(d) = *target else {
            continue;
        };
        let def = &ir.definitions[d];
        let name = &base[target];
        solids.push(Solid::Tessellated(tessellated(
            name,
            k,
            &def.mesh,
            &mut defines.positions,
        )));
        volumes.push(volume(
            &volume_name(target),
            Some((material_of(def), name)),
            Vec::new(),
        ));
        lv_of_def.insert(def.id.clone(), volume_name(target));
    }
    for (a, contents) in &assemblies {
        let target = Target::Assembly(*a);
        volumes.push(volume(&volume_name(&target), None, physvols(contents)));
    }

    // The world is centred on the source origin, so its half extents reach
    // the farther side of the bounds on each axis, plus a margin.
    let mut bounds = Aabb::of(&[]);
    for (d, at) in ir.instances() {
        if included[d] {
            let corners = ir.definitions[d].mesh.bounds().corners();
            bounds.add(&Aabb::of(&corners.map(|c| at.apply(c))));
        }
    }
    let margin = (0.01 * bounds.diagonal()).max(10.0);
    let full = |k: usize| num(2.0 * (bounds.lo[k].abs().max(bounds.hi[k].abs()) + margin));
    solids.push(Solid::Box(BoxSolid {
        name: WORLD_SOLID.to_string(),
        x: full(0),
        y: full(1),
        z: full(2),
        lunit: Some("mm".to_string()),
    }));
    let world = physvols(&world);
    volumes.push(volume(
        WORLD_VOLUME,
        Some((WORLD_MATERIAL.to_string(), WORLD_SOLID)),
        world,
    ));
    if volumes
        .iter()
        .any(|v| v.material_ref == PLACEHOLDER_MATERIAL)
        && !materials
            .materials
            .iter()
            .any(|m| m.name == PLACEHOLDER_MATERIAL)
    {
        materials.materials.push(placeholder_material());
    }

    let stem = filename.rsplit_once('.').map_or(filename, |(stem, _)| stem);
    Ok(Built {
        document: GdmlDocument {
            filename: format!("{stem}.gdml"),
            defines,
            materials,
            solids: SolidSection { solids },
            structure: StructureSection { volumes },
            setup: SetupSection {
                name: "Default".to_string(),
                version: "1.0".to_string(),
                world_ref: WORLD_VOLUME.to_string(),
            },
            raw_unknown: Vec::new(),
            setups: Vec::new(),
            order: DocumentOrder::default(),
            root_attributes: Vec::new(),
            materials_define: None,
            skipped_unsupported: Vec::new(),
        },
        lv_of_def,
    })
}

/// What of `children` is kept: the included definitions, and the
/// assemblies with something kept under them, which go to `kept` with their
/// contents. Each assembly is worked out once, however often it is placed
/// (`seen`: whether it is kept).
fn keep(
    ir: &CadImport,
    children: &[Child],
    included: &[bool],
    seen: &mut [Option<bool>],
    kept: &mut Vec<(usize, Contents)>,
) -> Contents {
    let mut out = Vec::new();
    for child in children {
        let keep = match child.target {
            Target::Definition(d) => included[d],
            Target::Assembly(a) => match seen[a] {
                Some(kept) => kept,
                None => {
                    let contents = keep(ir, &ir.assemblies[a].children, included, seen, kept);
                    let some = !contents.is_empty();
                    if some {
                        kept.push((a, contents));
                    }
                    seen[a] = Some(some);
                    some
                }
            },
        };
        if keep {
            out.push((child.target, child.at));
        }
    }
    out
}

/// A coordinate or length as GDML text: the shortest decimal that reads back
/// as the same `f64` (`-0` written as `0`).
fn num(x: f64) -> String {
    format!("{:?}", x + 0.0)
}

/// Placement `name` of `lv` at `at`, written only as far as it differs from
/// the identity.
fn physvol(name: &str, lv: &str, copy: Option<usize>, at: &Affine) -> PhysVol {
    let p = at.placement();
    let [x, y, z] = p.position.map(|c| Some(num(c)));
    let [ax, ay, az] = p.angles_deg.map(|a| Some(num(a)));
    PhysVol {
        name: Some(name.to_string()),
        volume_ref: lv.to_string(),
        copynumber: copy.map(|i| i.to_string()),
        file_ref: None,
        position: (p.position != [0.0; 3]).then(|| {
            PlacementPos::Inline(Position {
                name: format!("{name}_pos"),
                x,
                y,
                z,
                unit: Some("mm".to_string()),
            })
        }),
        rotation: (p.angles_deg != [0.0; 3]).then(|| {
            PlacementRot::Inline(Rotation {
                name: format!("{name}_rot"),
                x: ax,
                y: ay,
                z: az,
                unit: Some("deg".to_string()),
            })
        }),
        scale: p.reflected.then(|| {
            PlacementScale::Inline(Scale {
                name: format!("{name}_scl"),
                x: Some("-1".to_string()),
                y: Some("1".to_string()),
                z: Some("1".to_string()),
            })
        }),
    }
}

/// `mesh` as solid `name`, its vertices as positions `t<k>_<i>` in mm.
/// Facets carry no `type` (ABSOLUTE) and no `lunit`: Geant4 multiplies the
/// already scaled positions by a facet's `lunit` (TriangularRead).
fn tessellated(
    name: &str,
    k: usize,
    mesh: &Mesh,
    positions: &mut Vec<Position>,
) -> TessellatedSolid {
    let vertex = |i: u32| format!("t{k}_{i}");
    positions.extend(mesh.positions.iter().zip(0..).map(|(p, i)| Position {
        name: vertex(i),
        x: Some(num(p[0])),
        y: Some(num(p[1])),
        z: Some(num(p[2])),
        unit: Some("mm".to_string()),
    }));
    TessellatedSolid {
        name: name.to_string(),
        lunit: None,
        aunit: None,
        facets: mesh
            .triangles
            .iter()
            .map(|t| TessellatedFacet::Triangular {
                vertex1: vertex(t[0]),
                vertex2: vertex(t[1]),
                vertex3: vertex(t[2]),
                r#type: None,
                lunit: None,
            })
            .collect(),
    }
}

/// A volume of `(material, solid)`, or an assembly without them.
fn volume(name: &str, made_of: Option<(String, &str)>, physvols: Vec<PhysVol>) -> Volume {
    let (material, solid) = made_of.clone().unzip();
    Volume {
        name: name.to_string(),
        assembly: made_of.is_none(),
        material_ref: material.unwrap_or_default(),
        solid_ref: solid.unwrap_or_default().to_string(),
        physvols,
        auxiliaries: Vec::new(),
        replica: None,
        body_comments: Vec::new(),
        preserved: Vec::new(),
    }
}

/// The material of a part nobody has given one yet: G4_Galactic in
/// substance (Z = 1, A = 1.00794 g/mole, 1e-25 g/cm3, gas) under a name that
/// says it still needs a real one.
pub fn placeholder_material() -> Material {
    Material {
        name: PLACEHOLDER_MATERIAL.to_string(),
        formula: None,
        z: Some("1".to_string()),
        state: Some("gas".to_string()),
        density: Some(Density {
            value: "1e-25".to_string(),
            unit: Some("g/cm3".to_string()),
        }),
        density_ref: None,
        temperature: None,
        temperature_ref: None,
        pressure: None,
        pressure_ref: None,
        mee: None,
        mee_ref: None,
        atom_value: Some("1.00794".to_string()),
        atom_unit: Some("g/mole".to_string()),
        atom_type: None,
        properties: Vec::new(),
        rl: None,
        al: None,
        components: Vec::new(),
    }
}
