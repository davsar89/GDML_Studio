//! CAD import (`doc/cad-import/PLAN.md`). A CAD file is read into an
//! intermediate representation of repaired and validated parts ([`ir`]), and
//! [`build`] makes from it the GdmlDocument that the application edits and
//! saves like any other. STL is read here, STEP by an external [`worker`].

pub mod build;
pub mod intersect;
pub mod ir;
pub mod mesh;
pub mod names;
pub mod report;
pub mod stl;
pub mod transform;
pub mod worker;

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::gdml::model::{Element, GdmlDocument, Material};
use intersect::P;
use ir::{Assembly, CadImport, Child, Definition, Exact, Format, Interference, Rejected, Target};
use transform::Affine;

/// The material of parts that have none assigned yet
/// ([`build::placeholder_material`]).
pub const PLACEHOLDER_MATERIAL: &str = "CAD_UNASSIGNED";

/// What the user chose for an import.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    /// The length of one STL unit in mm: STL files carry no unit.
    pub stl_unit_mm: f64,
    /// The relative error of each mesh volume that STEP meshing aims for.
    pub volume_target: f64,
    pub hierarchy: Hierarchy,
    /// A material for every part, with the elements it needs (the client
    /// prepares both from the NIST table).
    pub part_material: Option<PartMaterial>,
    /// Per-part choices, keyed by definition id.
    pub overrides: BTreeMap<String, Override>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            stl_unit_mm: 1.0,
            volume_target: 1e-3,
            hierarchy: Hierarchy::Keep,
            part_material: None,
            overrides: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Hierarchy {
    /// The source's assemblies become `<assembly>` volumes.
    #[default]
    Keep,
    /// Every part is placed in the world.
    Flatten,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartMaterial {
    pub material: Material,
    #[serde(default)]
    pub elements: Vec<Element>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Override {
    pub representation: Representation,
    /// The chordal deviation of the part's mesh in mm, in place of the one
    /// the volume target leads to (STEP).
    pub chord_mm: Option<f64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Representation {
    /// The most exact representation the import can verify; for now that is
    /// the mesh itself.
    #[default]
    Auto,
    Tessellated,
    Exclude,
}

impl Options {
    pub fn representation(&self, id: &str) -> Representation {
        self.overrides
            .get(id)
            .map_or(Representation::Auto, |o| o.representation)
    }

    pub fn validate(&self) -> Result<(), String> {
        let positive = |x: f64| x.is_finite() && x > 0.0;
        if !positive(self.stl_unit_mm) {
            return Err("The STL unit must be a positive length in mm.".to_string());
        }
        if !(1e-6..=0.1).contains(&self.volume_target) {
            return Err("The volume target must be between 1e-6 and 0.1.".to_string());
        }
        if self
            .overrides
            .values()
            .any(|o| o.chord_mm.is_some_and(|c| !positive(c)))
        {
            return Err("A part's chordal deviation must be a positive length in mm.".to_string());
        }
        Ok(())
    }

    /// The chordal deviation fixed for each STEP solid (ids
    /// "<label entry>.<solid>"): the finest asked for any of its bodies.
    fn chords(&self) -> BTreeMap<String, f64> {
        let mut out = BTreeMap::new();
        for (id, o) in &self.overrides {
            if let Some(c) = o.chord_mm {
                let solid = id.rsplit_once('.').map_or(id.as_str(), |(solid, _)| solid);
                let entry = out.entry(solid.to_string()).or_insert(c);
                *entry = c.min(*entry);
            }
        }
        out
    }

    /// Whether changing to `next` means reading the file again rather than
    /// rebuilding the document from the parts already read.
    pub fn rereads(&self, next: &Options, format: Format) -> bool {
        match format {
            Format::Stl => self.stl_unit_mm != next.stl_unit_mm,
            Format::Step => {
                self.volume_target != next.volume_target || self.chords() != next.chords()
            }
        }
    }

    pub fn part_material_name(&self) -> Option<&str> {
        self.part_material
            .as_ref()
            .map(|p| p.material.name.as_str())
    }
}

/// What an imported document keeps of its source, for the report and for
/// re-importing with other options.
pub struct Session {
    pub filename: String,
    pub bytes: Arc<[u8]>,
    pub options: Options,
    pub ir: Arc<CadImport>,
    /// The logical volume of each included definition.
    pub lv_of_def: BTreeMap<String, String>,
}

/// The import in progress, shared with the requests that ask how far it got
/// or cancel it. One import runs at a time.
#[derive(Default)]
pub struct Control {
    busy: AtomicBool,
    cancel: AtomicBool,
    progress: Mutex<Progress>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Progress {
    pub phase: String,
    pub done: usize,
    pub total: usize,
}

/// The claim on [`Control`] of the running import, released when dropped.
pub struct Running(Arc<Control>);

impl Control {
    /// Claim it for a new import; none while another runs.
    pub fn start(self: &Arc<Self>) -> Option<Running> {
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Running(self.clone()))
    }

    pub fn set(&self, phase: &str, done: usize, total: usize) {
        *self.progress.lock().unwrap_or_else(|e| e.into_inner()) = Progress {
            phase: phase.to_string(),
            done,
            total,
        };
    }

    /// How far the running import got; none when no import runs.
    pub fn progress(&self) -> Option<Progress> {
        self.busy.load(Ordering::Acquire).then(|| {
            self.progress
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
        })
    }

    /// Ask the running import to stop; false when none runs.
    pub fn cancel(&self) -> bool {
        let busy = self.busy.load(Ordering::Acquire);
        if busy {
            self.cancel.store(true, Ordering::Release);
        }
        busy
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Acquire)
    }
}

impl Running {
    pub fn control(&self) -> &Control {
        &self.0
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.0.set("", 0, 0);
        self.0.cancel.store(false, Ordering::Release);
        self.0.busy.store(false, Ordering::Release);
    }
}

pub fn format_of(filename: &str) -> Option<Format> {
    let extension = filename.rsplit_once('.')?.1.to_ascii_lowercase();
    match extension.as_str() {
        "stl" => Some(Format::Stl),
        "step" | "stp" => Some(Format::Step),
        _ => None,
    }
}

/// The file name without directories or extension.
fn stem(filename: &str) -> &str {
    let base = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    base.rsplit_once('.').map_or(base, |(stem, _)| stem)
}

const CANCELLED: &str = "the import was cancelled";

/// Read a CAD file into repaired, validated parts. Fails when no part at all
/// can be imported, saying why for each, and when `control` is cancelled.
pub fn read(
    bytes: &[u8],
    filename: &str,
    options: &Options,
    control: &Control,
) -> Result<CadImport, String> {
    let mut ir = match format_of(filename) {
        Some(Format::Stl) => read_stl(bytes, filename, options)?,
        Some(Format::Step) => {
            read_step(bytes, options, control).map_err(|e| format!("{filename}: {e}."))?
        }
        None => {
            return Err(format!(
                "{filename} is not a file type this version imports (.stl, .step, .stp)."
            ))
        }
    };
    if ir.definitions.is_empty() {
        let reasons: Vec<String> = ir
            .rejected
            .iter()
            .map(|r| format!("{}: {}", r.label, r.reason))
            .collect();
        return Err(format!(
            "No part of {filename} could be imported. {}.",
            reasons.join("; ")
        ));
    }
    ir.crossings = crossings(&ir, control);
    if control.cancelled() {
        return Err(CANCELLED.to_string());
    }
    Ok(ir)
}

/// Each `solid` block is repaired on its own, then all shells are nested
/// together: a cavity written as a separate block still finds its body.
/// Every body is placed once, in the world.
fn read_stl(bytes: &[u8], filename: &str, options: &Options) -> Result<CadImport, String> {
    let solids = stl::parse(bytes, options.stl_unit_mm, stem(filename))
        .map_err(|e| format!("{filename}: {e}."))?;
    let mut rejected = Vec::new();
    let mut shells = Vec::new();
    let mut logs = Vec::new();
    for (i, solid) in solids.iter().enumerate() {
        let shelled = mesh::shells(&solid.triangles, i, true);
        rejected.extend(shelled.rejected.into_iter().map(|reason| Rejected {
            label: solid.name.clone(),
            reason,
        }));
        shells.extend(shelled.shells);
        logs.push(shelled.log);
    }
    let found = mesh::bodies(shells);
    rejected.extend(found.rejected.into_iter().map(|(i, reason)| Rejected {
        label: solids[i].name.clone(),
        reason,
    }));
    // Ids count the bodies of each block: "block.body".
    let mut count = vec![0usize; solids.len()];
    let definitions: Vec<Definition> = found
        .bodies
        .into_iter()
        .map(|body| {
            count[body.source] += 1;
            Definition {
                id: format!("{}.{}", body.source, count[body.source] - 1),
                label: solids[body.source].name.clone(),
                exact: None,
                shells: body.shells,
                repairs: logs[body.source].clone(),
                reversed_facets: body.reversed,
                issues: Vec::new(),
                mesh: body.mesh,
            }
        })
        .collect();
    Ok(CadImport {
        format: Format::Stl,
        roots: (0..definitions.len())
            .map(|d| Child {
                target: Target::Definition(d),
                at: Affine::IDENTITY,
            })
            .collect(),
        definitions,
        assemblies: Vec::new(),
        rejected,
        interferences: Vec::new(),
        unchecked: 0,
        crossings: Vec::new(),
    })
}

/// A solid's mesh after the pipeline (algorithm B), in its part's frame.
struct Checked {
    bodies: Vec<mesh::Body>,
    log: mesh::RepairLog,
    /// What of its mesh was set aside, and why.
    problems: Vec<String>,
}

fn check(solid: &worker::Solid) -> Checked {
    let node = |v: u32| {
        let i = 3 * v as usize;
        let p = solid.mesh.positions.get(i..i + 3)?;
        Some([p[0], p[1], p[2]])
    };
    let soup: Option<Vec<[P; 3]>> = (solid.mesh.triangles.as_chunks::<3>().0.iter())
        .map(|t| Some([node(t[0])?, node(t[1])?, node(t[2])?]))
        .collect();
    let shelled = mesh::shells(&soup.unwrap_or_default(), 0, false);
    let found = mesh::bodies(shelled.shells);
    let mut problems = solid.mesh.issues.clone();
    problems.extend(shelled.rejected);
    problems.extend(found.rejected.into_iter().map(|(_, reason)| reason));
    if found.bodies.is_empty() && problems.is_empty() {
        problems.push("it has no mesh".to_string());
    }
    Checked {
        bodies: found.bodies,
        log: shelled.log,
        problems,
    }
}

/// Each solid's stitched mesh goes through the same repair and nesting as an
/// STL block, in its part's frame; a solid that does not come through
/// cleanly is meshed once more at half the chord, and left out, whole, if it
/// still does not. Definition ids are
/// "<label entry>.<solid>.<body>". The assembly tree is kept as it is,
/// with each part replaced by its definitions. The error completes
/// "<filename>: ...".
fn read_step(bytes: &[u8], options: &Options, control: &Control) -> Result<CadImport, String> {
    let chords = options.chords();
    let meshing = |chords, only| worker::Meshing {
        volume_target: options.volume_target,
        chords,
        only,
    };
    let mut out = worker::run(bytes, &meshing(&chords, false), control, None)?;
    let total: usize = out.labels.iter().map(|l| l.solids.len()).sum();
    let mut checked: HashMap<(usize, usize), Checked> = HashMap::new();
    let mut retry = BTreeMap::new();
    for (l, label) in out.labels.iter().enumerate() {
        for (s, solid) in label.solids.iter().enumerate() {
            control.set("validate", checked.len(), total);
            if control.cancelled() {
                return Err(CANCELLED.to_string());
            }
            let Some(solid) = solid else { continue };
            let c = check(solid);
            if !c.problems.is_empty() && solid.mesh.chord_mm > 0.0 {
                retry.insert(format!("{}.{s}", label.entry), solid.mesh.chord_mm / 2.0);
            }
            checked.insert((l, s), c);
        }
    }
    if !retry.is_empty() {
        let again = worker::run(bytes, &meshing(&retry, true), control, Some("remesh"));
        if control.cancelled() {
            return Err(CANCELLED.to_string());
        }
        let mut again = again.ok();
        for (l, label) in out.labels.iter_mut().enumerate() {
            for (s, solid) in label.solids.iter_mut().enumerate() {
                if !retry.contains_key(&format!("{}.{s}", label.entry)) {
                    continue;
                }
                let better = (again.as_mut())
                    .and_then(|a| a.labels.iter_mut().find(|f| f.entry == label.entry))
                    .and_then(|f| f.solids.get_mut(s)?.take())
                    .map(|finer| (check(&finer), finer))
                    .filter(|(c, _)| c.problems.is_empty());
                match better {
                    Some((c, finer)) => {
                        checked.insert((l, s), c);
                        *solid = Some(finer);
                    }
                    None => (checked.get_mut(&(l, s)).expect("checked"))
                        .problems
                        .push("meshing it at half the chord did not help".to_string()),
                }
            }
        }
    }

    let index: HashMap<&str, usize> = (out.labels.iter().enumerate())
        .map(|(i, l)| (l.entry.as_str(), i))
        .collect();
    let name = |l: usize| match out.labels[l].name.as_str() {
        "" => "unnamed",
        name => name,
    };
    let mut definitions = Vec::new();
    let mut assemblies = Vec::new();
    let top = (out.roots.first())
        .and_then(|root| index.get(root.as_str()))
        .map_or("unnamed", |&l| name(l));
    let mut rejected: Vec<Rejected> = (out.issues.iter())
        .map(|reason| Rejected {
            label: top.to_string(),
            reason: reason.clone(),
        })
        .collect();
    // What a placement of each label puts in its parent.
    let mut targets: Vec<Vec<Target>> = vec![Vec::new(); out.labels.len()];
    let mut of_solid: HashMap<(usize, usize), usize> = HashMap::new();
    for (l, label) in out.labels.iter().enumerate() {
        let reject = |reason: &str| Rejected {
            label: name(l).to_string(),
            reason: reason.to_string(),
        };
        rejected.extend(label.issues.iter().map(|r| reject(r)));
        if label.assembly {
            targets[l].push(Target::Assembly(assemblies.len()));
            assemblies.push(Assembly {
                label: name(l).to_string(),
                children: Vec::new(),
            });
            continue;
        }
        let bodies: usize = (0..label.solids.len())
            .filter_map(|s| checked.get(&(l, s)).filter(|c| c.problems.is_empty()))
            .map(|c| c.bodies.len())
            .sum();
        for (s, solid) in label.solids.iter().enumerate() {
            let (Some(solid), Some(c)) = (solid, checked.remove(&(l, s))) else {
                continue;
            };
            let mut issues = solid.issues.clone();
            if !solid.valid {
                issues.push("OpenCASCADE finds its B-rep invalid".to_string());
            }
            if !c.problems.is_empty() {
                let which = if label.solids.len() > 1 {
                    format!("solid {}: ", s + 1)
                } else {
                    String::new()
                };
                let reasons = [issues.as_slice(), &c.problems].concat().join("; ");
                rejected.push(reject(&format!("{which}{reasons}")));
                continue;
            }
            let single = c.bodies.len() == 1;
            for (b, body) in c.bodies.into_iter().enumerate() {
                of_solid.entry((l, s)).or_insert(definitions.len());
                targets[l].push(Target::Definition(definitions.len()));
                definitions.push(Definition {
                    id: format!("{}.{s}.{b}", label.entry),
                    label: match bodies {
                        1 => name(l).to_string(),
                        _ => format!("{} {}", name(l), targets[l].len()),
                    },
                    mesh: body.mesh,
                    exact: single.then_some(Exact {
                        volume_mm3: solid.volume_mm3,
                        area_mm2: solid.area_mm2,
                        bounds_mm: solid.bounds_mm,
                        tolerance_mm: solid.tolerance_mm,
                        chord_mm: solid.mesh.chord_mm,
                    }),
                    shells: body.shells,
                    repairs: c.log.clone(),
                    reversed_facets: body.reversed,
                    issues: issues.clone(),
                });
            }
        }
    }
    // Each assembly's children; a placement Geant4 cannot express is left out.
    let place = |l: usize, at: Affine| targets[l].iter().map(move |&target| Child { target, at });
    for (l, label) in out.labels.iter().enumerate() {
        let Some(&Target::Assembly(a)) = targets[l].first().filter(|_| label.assembly) else {
            continue;
        };
        for c in &label.components {
            let (Some(&t), at) = (index.get(c.target.as_str()), Affine::from_rows(&c.matrix))
            else {
                continue;
            };
            if at.is_rigid() {
                assemblies[a].children.extend(place(t, at));
            } else {
                rejected.push(Rejected {
                    label: name(t).to_string(),
                    reason: format!(
                        "its placement in '{}' scales or shears it, which a GDML placement \
                         cannot express",
                        name(l)
                    ),
                });
            }
        }
    }
    let roots = (out.roots.iter())
        .filter_map(|root| index.get(root.as_str()))
        .flat_map(|&l| place(l, Affine::IDENTITY))
        .collect();

    // A path is [root, component index at each level, solid index].
    let occurrence = |path: &[usize]| -> Option<(String, Option<usize>)> {
        let (&root, rest) = path.split_first()?;
        let (&solid, components) = rest.split_last()?;
        let mut l = *index.get(out.roots.get(root)?.as_str())?;
        let mut names = vec![name(l)];
        for &c in components {
            l = *index.get(out.labels[l].components.get(c)?.target.as_str())?;
            names.push(name(l));
        }
        Some((names.join(" / "), of_solid.get(&(l, solid)).copied()))
    };
    let interferences = (out.interferences.iter())
        .filter_map(|i| {
            let ((a, da), (b, db)) = (occurrence(&i.a)?, occurrence(&i.b)?);
            Some(Interference {
                paths: [a, b],
                definitions: [da, db],
                volume_mm3: i.volume_mm3,
            })
        })
        .collect();
    Ok(CadImport {
        format: Format::Step,
        definitions,
        assemblies,
        roots,
        rejected,
        interferences,
        unchecked: out.unchecked,
        crossings: Vec::new(),
    })
}

/// Pairs of definitions some placements of which have crossing meshes
/// ([`mesh::crossings`]).
fn crossings(ir: &CadImport, control: &Control) -> Vec<(usize, usize)> {
    let instances = ir.instances();
    let placed: Vec<_> = (instances.iter())
        .map(|&(d, at)| (&ir.definitions[d].mesh, at))
        .collect();
    let mut pairs: Vec<(usize, usize)> = mesh::crossings(&placed, |done, total| {
        control.set("check", done, total);
        !control.cancelled()
    })
    .into_iter()
    .map(|(i, j)| {
        let (a, b) = (instances[i].0, instances[j].0);
        (a.min(b), a.max(b))
    })
    .collect();
    pairs.sort_unstable();
    pairs.dedup();
    pairs
}

/// The material of each definition's volume in `doc`, by definition id.
pub fn assigned_materials(session: &Session, doc: &GdmlDocument) -> BTreeMap<String, String> {
    session
        .lv_of_def
        .iter()
        .filter_map(|(id, lv)| {
            let volume = doc.structure.volumes.iter().find(|v| &v.name == lv)?;
            Some((id.clone(), volume.material_ref.clone()))
        })
        .collect()
}

/// Load warnings for what an import could not do as asked: rejected parts,
/// and included volumes that overlap, as the source's interference check
/// found or, failing that, as their meshes cross.
pub fn warnings(ir: &CadImport, lv_of_def: &BTreeMap<String, String>) -> Vec<String> {
    let mut out: Vec<String> = ir
        .rejected
        .iter()
        .map(|r| format!("Part '{}' was not imported: {}.", r.label, r.reason))
        .collect();
    let lv = |d: Option<usize>| lv_of_def.get(&ir.definitions[d?].id).map(String::as_str);
    let mut overlaps: BTreeMap<(&str, &str), (usize, f64)> = BTreeMap::new();
    for i in &ir.interferences {
        if let (Some(a), Some(b)) = (lv(i.definitions[0]), lv(i.definitions[1])) {
            let (count, largest) = overlaps.entry((a.min(b), a.max(b))).or_default();
            *count += 1;
            *largest = largest.max(i.volume_mm3);
        }
    }
    for (&(a, b), &(count, largest)) in &overlaps {
        let mm3 = format!("{}3", mesh::mm(largest));
        out.push(match count {
            1 => format!(
                "Volumes '{a}' and '{b}' overlap by {mm3} in the CAD file; Geant4 reports an \
                 overlap there."
            ),
            n => format!(
                "Volumes '{a}' and '{b}' overlap in {n} places in the CAD file, by up to {mm3}; \
                 Geant4 reports an overlap at each."
            ),
        });
    }
    if ir.unchecked > 0 {
        out.push(format!(
            "{} pairs of neighbouring parts were not checked for overlaps in the CAD file, for \
             lack of time or because OpenCASCADE failed on them.",
            ir.unchecked
        ));
    }
    for &(a, b) in &ir.crossings {
        let (Some(a), Some(b)) = (lv(Some(a)), lv(Some(b))) else {
            continue;
        };
        if overlaps.contains_key(&(a.min(b), a.max(b))) {
            continue;
        }
        out.push(if a == b {
            format!("The meshes of two placements of volume '{a}' cross each other; Geant4 reports an overlap there.")
        } else {
            format!(
                "The meshes of volumes '{a}' and '{b}' cross each other; Geant4 reports an \
                 overlap there."
            )
        });
    }
    out
}
