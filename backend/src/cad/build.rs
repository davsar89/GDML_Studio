//! The GdmlDocument of an import (algorithm H in `doc/cad-import/PLAN.md`).
//!
//! Each included definition becomes a logical volume of its exact solid
//! (Auto, when the worker proved one) or of a `<tessellated>` solid. The
//! source's assemblies become `<assembly>` volumes placed as the source
//! places them, or, flattened, every part is placed in the world at
//! its composed transform; Geant4 places the same solids either way. The
//! world is a box around the source origin. Coordinates stay those of the
//! source, in mm, written with Rust's shortest round-trip formatting so that
//! Geant4 reads back exactly the `f64` values computed here.

use std::collections::BTreeMap;

use super::intersect::Aabb;
use super::ir::{Analytic, CadImport, Child, Definition, Shape, Target};
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
    // The exact solid of each definition shown as one, in its stock's frame.
    let exact: BTreeMap<Target, &Analytic> = (ir.definitions.iter().enumerate())
        .filter(|(_, d)| options.representation(&d.id) == Representation::Auto)
        .filter_map(|(i, d)| Some((Target::Definition(i), d.analytic.as_ref().ok()?)))
        .collect();
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
        // Names for the exact solid's operands whether or not it is shown,
        // so that switching keeps every name.
        if let Target::Definition(d) = target {
            let tools = ir.definitions[d]
                .analytic
                .as_ref()
                .map_or(0, |a| a.tools.len());
            if tools > 0 {
                suffixes.extend(["_stock".to_string(), "_tools".to_string()]);
            }
            for k in 0..tools {
                suffixes.extend(["", "_pos", "_rot"].map(|s| format!("_tool{k}{s}")));
            }
        }
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
                let at = exact.get(target).map_or(*at, |a| at.after(&a.stock.at()));
                physvol(
                    &format!("{}{}", base[target], pv(n, *i - 1)),
                    &volume_name(target),
                    (n > 1).then_some(*i - 1),
                    &at,
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
        match exact.get(target) {
            Some(a) => analytic(name, a, &mut solids),
            None => solids.push(Solid::Tessellated(tessellated(
                name,
                k,
                &def.mesh,
                &mut defines.positions,
            ))),
        }
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
    PhysVol {
        name: Some(name.to_string()),
        volume_ref: lv.to_string(),
        copynumber: copy.map(|i| i.to_string()),
        file_ref: None,
        position: position(name, p.position),
        rotation: rotation(name, p.angles_deg),
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

/// `<position name="<name>_pos">` in mm, unless it is the origin.
fn position(name: &str, t: [f64; 3]) -> Option<PlacementPos> {
    let [x, y, z] = t.map(|c| Some(num(c)));
    (t != [0.0; 3]).then(|| {
        PlacementPos::Inline(Position {
            name: format!("{name}_pos"),
            x,
            y,
            z,
            unit: Some("mm".to_string()),
        })
    })
}

/// `<rotation name="<name>_rot">` in degrees, unless it is none.
fn rotation(name: &str, angles_deg: [f64; 3]) -> Option<PlacementRot> {
    let [x, y, z] = angles_deg.map(|a| Some(num(a)));
    (angles_deg != [0.0; 3]).then(|| {
        PlacementRot::Inline(Rotation {
            name: format!("{name}_rot"),
            x,
            y,
            z,
            unit: Some("deg".to_string()),
        })
    })
}

/// The solids of `a`, operands first: the stock as `name`, or the
/// subtraction `name` of `<name>_stock` and `<name>_tool0` (or of the
/// multiUnion `<name>_tools` of `<name>_tool<k>`), each tool placed in the
/// stock's frame.
fn analytic(name: &str, a: &Analytic, solids: &mut Vec<Solid>) {
    if a.tools.is_empty() {
        solids.push(primitive(name, &a.stock.shape));
        return;
    }
    let stock = format!("{name}_stock");
    solids.push(primitive(&stock, &a.stock.shape));
    let to_stock = a.stock.at().inverse();
    let mut nodes: Vec<MultiUnionNode> = Vec::new();
    for (k, tool) in a.tools.iter().enumerate() {
        let tool_name = format!("{name}_tool{k}");
        solids.push(primitive(&tool_name, &tool.shape));
        let (t, angles) = to_stock.after(&tool.at()).operand();
        nodes.push(MultiUnionNode {
            name: None,
            position: position(&tool_name, t),
            rotation: rotation(&tool_name, angles),
            solid_ref: tool_name,
        });
    }
    let (second_ref, position, rotation) = match nodes.len() {
        1 => {
            let node = nodes.remove(0);
            (node.solid_ref, node.position, node.rotation)
        }
        _ => {
            let tools = format!("{name}_tools");
            solids.push(Solid::MultiUnion(MultiUnionSolid {
                name: tools.clone(),
                nodes,
            }));
            (tools, None, None)
        }
    };
    solids.push(Solid::Boolean(BooleanSolid {
        name: name.to_string(),
        operation: BooleanOp::Subtraction,
        first_ref: stock,
        second_ref,
        position,
        rotation,
        first_position: None,
        first_rotation: None,
    }));
}

/// `shape` as GDML solid `name`, lengths in mm and angles in rad.
fn primitive(name: &str, shape: &Shape) -> Solid {
    let name = name.to_string();
    let n = |x: &f64| num(*x);
    let some = |x: &f64| Some(num(*x));
    let (mm, rad, zero) = (
        Some("mm".to_string()),
        Some("rad".to_string()),
        Some("0".to_string()),
    );
    match shape {
        Shape::Box { x, y, z } => Solid::Box(BoxSolid {
            name,
            x: n(x),
            y: n(y),
            z: n(z),
            lunit: mm,
        }),
        Shape::Xtru { points, z1, z2 } => Solid::Xtru(XtruSolid {
            name,
            lunit: mm,
            vertices: (points.iter())
                .map(|p| TwoDimVertex {
                    x: num(p[0]),
                    y: num(p[1]),
                })
                .collect(),
            sections: [z1, z2]
                .iter()
                .enumerate()
                .map(|(i, z)| XtruSection {
                    z_order: i.to_string(),
                    z_position: n(z),
                    x_offset: "0".to_string(),
                    y_offset: "0".to_string(),
                    scaling_factor: "1".to_string(),
                })
                .collect(),
        }),
        Shape::Tube {
            rmin,
            rmax,
            z,
            deltaphi,
        } => Solid::Tube(TubeSolid {
            name,
            rmin: some(rmin),
            rmax: n(rmax),
            z: n(z),
            startphi: zero,
            deltaphi: some(deltaphi),
            aunit: rad,
            lunit: mm,
        }),
        Shape::Cone {
            rmin1,
            rmax1,
            rmin2,
            rmax2,
            z,
            deltaphi,
        } => Solid::Cone(ConeSolid {
            name,
            rmin1: some(rmin1),
            rmax1: n(rmax1),
            rmin2: some(rmin2),
            rmax2: n(rmax2),
            z: n(z),
            startphi: zero,
            deltaphi: some(deltaphi),
            aunit: rad,
            lunit: mm,
        }),
        Shape::GenericPolycone { points, deltaphi } => {
            Solid::GenericPolycone(GenericPolyconeSolid {
                name,
                startphi: zero,
                deltaphi: some(deltaphi),
                aunit: rad,
                lunit: mm,
                rzpoints: (points.iter())
                    .map(|p| RZPoint {
                        r: num(p[0]),
                        z: num(p[1]),
                    })
                    .collect(),
            })
        }
        Shape::Sphere {
            rmin,
            rmax,
            deltaphi,
            starttheta,
            deltatheta,
        } => Solid::Sphere(SphereSolid {
            name,
            rmin: some(rmin),
            rmax: n(rmax),
            startphi: zero,
            deltaphi: some(deltaphi),
            starttheta: some(starttheta),
            deltatheta: some(deltatheta),
            aunit: rad,
            lunit: mm,
        }),
        Shape::Orb { r } => Solid::Orb(OrbSolid {
            name,
            r: n(r),
            lunit: mm,
        }),
        Shape::Torus {
            rmin,
            rmax,
            rtor,
            deltaphi,
        } => Solid::Torus(TorusSolid {
            name,
            rmin: some(rmin),
            rmax: n(rmax),
            rtor: n(rtor),
            startphi: zero,
            deltaphi: some(deltaphi),
            aunit: rad,
            lunit: mm,
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
