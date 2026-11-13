//! Validation and repair of triangle meshes (algorithm B in
//! `doc/cad-import/PLAN.md`): the one pipeline every `<tessellated>` solid of
//! an import goes through.
//!
//! A triangle soup is welded, cleaned of repeated and degenerate facets and
//! split into closed shells with consistent winding ([`shells`]). Shells are
//! then checked for crossing facets and nested into bodies: an outer shell and
//! the cavities directly inside it ([`bodies`]). Whatever cannot be repaired
//! without changing the shape beyond the tolerances below is rejected with the
//! reason, never dropped silently.

use serde::Serialize;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use super::intersect::{self, cross, norm, sub, Aabb, Bvh, Crossing, P};
use super::transform::Affine;

/// Shortest edge and lowest altitude a facet may have, in mm: 100 times
/// Geant4's kCarTolerance (1e-9 mm), at or below which G4TriangularFacet
/// declares a facet undefined and G4TessellatedSolid::AddFacet drops it.
pub const DEGENERATE_EPS_MM: f64 = 1e-7;

/// Rounds of degenerate repair before what is left counts as unrepairable.
const REPAIR_PASSES: usize = 20;

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub positions: Vec<P>,
    pub triangles: Vec<[u32; 3]>,
}

impl Mesh {
    pub fn corners(&self, t: [u32; 3]) -> [P; 3] {
        t.map(|v| self.positions[v as usize])
    }

    pub fn bounds(&self) -> Aabb {
        Aabb::of(&self.positions)
    }

    /// Enclosed volume in mm3: positive when the facets wind counterclockwise
    /// seen from outside. Summed about the centre of the bounds, which keeps
    /// far-off coordinates from cancelling.
    pub fn volume(&self) -> f64 {
        let b = self.bounds();
        let o = [0, 1, 2].map(|k| 0.5 * (b.lo[k] + b.hi[k]));
        let sum: f64 = self
            .triangles
            .iter()
            .map(|&t| {
                let [a, b, c] = self.corners(t).map(|p| sub(p, o));
                let n = cross(b, c);
                a[0] * n[0] + a[1] * n[1] + a[2] * n[2]
            })
            .sum();
        sum / 6.0
    }

    pub fn area(&self) -> f64 {
        self.triangles
            .iter()
            .map(|&t| {
                let [a, b, c] = self.corners(t);
                0.5 * norm(cross(sub(b, a), sub(c, a)))
            })
            .sum()
    }
}

/// What the pipeline changed in one source mesh.
#[derive(Clone, Debug, Default, Serialize)]
pub struct RepairLog {
    /// Open-boundary vertices moved onto a neighbour within the weld tolerance.
    pub welded_vertices: usize,
    /// The largest such move, in mm.
    pub weld_max_mm: f64,
    /// Facets dropped: collapsed by welding, repeated, or back to back with a
    /// reversed copy of themselves (a wall of zero thickness).
    pub dropped_facets: usize,
    /// Edges at most `DEGENERATE_EPS_MM` long, collapsed to their midpoint.
    pub collapsed_edges: usize,
    /// Edges flipped to remove facets at most `DEGENERATE_EPS_MM` wide.
    pub flipped_edges: usize,
}

/// A closed, consistently wound, edge-connected surface.
pub struct Shell {
    pub mesh: Mesh,
    /// Which source mesh it came from (an STL `solid` block).
    pub source: usize,
    /// Facets whose winding now differs from the source.
    reversed: usize,
}

#[derive(Default)]
pub struct Shelled {
    pub shells: Vec<Shell>,
    /// Why each component that is not a shell was set aside.
    pub rejected: Vec<String>,
    pub log: RepairLog,
}

/// A length for a message, to four significant digits.
pub fn mm(x: f64) -> String {
    if x == 0.0 || !x.is_finite() {
        return format!("{x} mm");
    }
    let decimals = (3 - x.abs().log10().floor() as i32).clamp(0, 15) as usize;
    format!("{x:.decimals$} mm")
}

/// Weld, repair and split a triangle soup into closed shells with consistent
/// winding. `weld` also merges open-boundary vertices that lie within
/// `max(1e-9 mm, 1e-7 D)` of each other (D the diagonal of the bounds), the
/// cracks an STL exporter leaves; topologically stitched meshes skip it.
pub fn shells(soup: &[[P; 3]], source: usize, weld: bool) -> Shelled {
    let mut out = Shelled::default();
    if soup.iter().flatten().flatten().any(|c| !c.is_finite()) {
        out.rejected
            .push("a vertex has a coordinate that is not a finite number".to_string());
        return out;
    }
    let (mut pos, mut tris) = merge_identical(soup);
    if weld {
        weld_boundary(&pos, &mut tris, &mut out.log);
    }
    drop_repeats(&mut tris, &mut out.log);

    // Wind each component consistently; set aside those no repair here can
    // close. `origin` remembers each kept facet's component across repair.
    let first = orient(&pos, &mut tris);
    let mut origin = vec![usize::MAX; tris.len()];
    for (c, comp) in first.iter().enumerate() {
        match &comp.problem {
            Some(problem) => out.rejected.push(problem.clone()),
            None => comp.tris.iter().for_each(|&t| origin[t] = c),
        }
    }
    let (mut tris, mut origin): (Vec<[u32; 3]>, Vec<usize>) = tris
        .into_iter()
        .zip(origin)
        .filter(|&(_, c)| c != usize::MAX)
        .unzip();

    repair_degenerates(&mut pos, &mut tris, &mut origin, &mut out.log);

    // The repairs keep each component closed and its winding consistent;
    // checking again costs little and catches what they could not fix.
    for comp in orient(&pos, &mut tris) {
        if let Some(problem) = comp.problem {
            out.rejected.push(problem);
            continue;
        }
        let narrow = comp
            .tris
            .iter()
            .filter(|&&t| degeneracy(&pos, tris[t]).is_some())
            .count();
        if narrow > 0 {
            out.rejected.push(format!(
                "{narrow} facets shorter or narrower than {} could not be repaired",
                mm(DEGENERATE_EPS_MM)
            ));
            continue;
        }
        out.shells.push(Shell {
            mesh: compact(&pos, comp.tris.iter().map(|&t| tris[t])),
            source,
            reversed: first[origin[comp.tris[0]]].reversed,
        });
    }
    out
}

/// One vertex per distinct coordinate triple (-0 and 0 are one point).
fn merge_identical(soup: &[[P; 3]]) -> (Vec<P>, Vec<[u32; 3]>) {
    let mut index: HashMap<[u64; 3], u32> = HashMap::with_capacity(soup.len());
    let mut pos = Vec::with_capacity(soup.len() / 2 + 3);
    let tris = soup
        .iter()
        .map(|t| {
            t.map(|p| {
                let p = p.map(|c| c + 0.0);
                *index.entry(p.map(f64::to_bits)).or_insert_with(|| {
                    pos.push(p);
                    (pos.len() - 1) as u32
                })
            })
        })
        .collect();
    (pos, tris)
}

fn edge_key(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

fn has_edge(t: [u32; 3], a: u32, b: u32) -> bool {
    (0..3).any(|k| t[k] == a && t[(k + 1) % 3] == b)
}

/// Merge open-boundary vertices within the weld tolerance: each, in index
/// order, moves onto the nearest earlier representative within reach or
/// becomes one, so no chain of merges moves a vertex further than that.
fn weld_boundary(pos: &[P], tris: &mut [[u32; 3]], log: &mut RepairLog) {
    let mut uses: HashMap<(u32, u32), u32> = HashMap::with_capacity(tris.len() * 3 / 2);
    for t in tris.iter() {
        for k in 0..3 {
            *uses.entry(edge_key(t[k], t[(k + 1) % 3])).or_default() += 1;
        }
    }
    let boundary: BTreeSet<u32> = uses
        .iter()
        .filter(|&(_, &n)| n == 1)
        .flat_map(|(&(a, b), _)| [a, b])
        .collect();
    if boundary.is_empty() {
        return;
    }
    let tol = (1e-7 * Aabb::of(pos).diagonal()).max(1e-9);
    let cell = |p: P| p.map(|c| (c / tol).floor() as i64);
    let mut representatives: HashMap<[i64; 3], Vec<u32>> = HashMap::new();
    let mut moved: HashMap<u32, u32> = HashMap::new();
    for &v in &boundary {
        let p = pos[v as usize];
        let c = cell(p);
        let mut best: Option<(f64, u32)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let key = [c[0] + dx, c[1] + dy, c[2] + dz];
                    for &r in representatives.get(&key).into_iter().flatten() {
                        let d = norm(sub(pos[r as usize], p));
                        if d <= tol && best.is_none_or(|(b, _)| d < b) {
                            best = Some((d, r));
                        }
                    }
                }
            }
        }
        match best {
            Some((d, r)) => {
                moved.insert(v, r);
                log.welded_vertices += 1;
                log.weld_max_mm = log.weld_max_mm.max(d);
            }
            None => representatives.entry(c).or_default().push(v),
        }
    }
    for v in tris.iter_mut().flatten() {
        if let Some(&r) = moved.get(v) {
            *v = r;
        }
    }
}

/// Drop facets with a repeated vertex, and repeats of a facet: one copy
/// stays, wound as most copies are; a facet with as many copies each way is
/// a zero-thickness wall, and all of them go.
fn drop_repeats(tris: &mut Vec<[u32; 3]>, log: &mut RepairLog) {
    let before = tris.len();
    tris.retain(|t| t[0] != t[1] && t[1] != t[2] && t[2] != t[0]);
    // The vertex set, and whether the winding is the set's ascending one.
    let canon = |t: &[u32; 3]| {
        let k = (0..3).min_by_key(|&k| t[k]).unwrap_or(0);
        let (b, c) = (t[(k + 1) % 3], t[(k + 2) % 3]);
        ([t[k], b.min(c), b.max(c)], b < c)
    };
    let mut net: HashMap<[u32; 3], i64> = HashMap::with_capacity(tris.len());
    for t in tris.iter() {
        let (key, ascending) = canon(t);
        *net.entry(key).or_default() += if ascending { 1 } else { -1 };
    }
    let mut kept = HashSet::new();
    tris.retain(|t| {
        let (key, ascending) = canon(t);
        let n = net[&key];
        n != 0 && (n > 0) == ascending && kept.insert(key)
    });
    log.dropped_facets += before - tris.len();
}

/// The facets of one edge-connected component.
struct Component {
    tris: Vec<usize>,
    /// Why it is not a closed, orientable surface, if it is not.
    problem: Option<String>,
    /// Facets reversed to agree with the first one.
    reversed: usize,
}

/// Split into edge-connected components and wind each consistently, flipping
/// facets in place: neighbours must traverse their shared edge in opposite
/// directions. A component is closed when every edge has exactly two facets.
fn orient(pos: &[P], tris: &mut [[u32; 3]]) -> Vec<Component> {
    let mut edges: HashMap<(u32, u32), Vec<usize>> = HashMap::with_capacity(tris.len() * 3 / 2);
    for (i, t) in tris.iter().enumerate() {
        for k in 0..3 {
            edges
                .entry(edge_key(t[k], t[(k + 1) % 3]))
                .or_default()
                .push(i);
        }
    }
    let mut component = vec![usize::MAX; tris.len()];
    let mut out = Vec::new();
    for seed in 0..tris.len() {
        if component[seed] != usize::MAX {
            continue;
        }
        let id = out.len();
        let mut comp = Component {
            tris: Vec::new(),
            problem: None,
            reversed: 0,
        };
        let (mut open, mut open_longest, mut shared) = (0usize, 0.0f64, 0usize);
        let mut twisted = false;
        let mut counted = HashSet::new();
        component[seed] = id;
        let mut queue = VecDeque::from([seed]);
        while let Some(t) = queue.pop_front() {
            comp.tris.push(t);
            for k in 0..3 {
                let (a, b) = (tris[t][k], tris[t][(k + 1) % 3]);
                let users = &edges[&edge_key(a, b)];
                if users.len() != 2 {
                    if counted.insert(edge_key(a, b)) {
                        if users.len() == 1 {
                            open += 1;
                            open_longest =
                                open_longest.max(norm(sub(pos[a as usize], pos[b as usize])));
                        } else {
                            shared += 1;
                        }
                    }
                    for &s in users {
                        if component[s] == usize::MAX {
                            component[s] = id;
                            queue.push_back(s);
                        }
                    }
                    continue;
                }
                let s = if users[0] == t { users[1] } else { users[0] };
                let agrees = has_edge(tris[s], b, a);
                if component[s] == usize::MAX {
                    component[s] = id;
                    if !agrees {
                        tris[s].swap(1, 2);
                        comp.reversed += 1;
                    }
                    queue.push_back(s);
                } else if !agrees {
                    twisted = true;
                }
            }
        }
        comp.problem = if open > 0 {
            Some(format!(
                "open surface: {open} edges have a facet on one side only (longest {})",
                mm(open_longest)
            ))
        } else if shared > 0 {
            Some(format!("{shared} edges are shared by more than two facets"))
        } else if twisted {
            Some("its facets cannot be wound consistently".to_string())
        } else {
            None
        };
        out.push(comp);
    }
    out
}

enum Degenerate {
    /// An edge (a, b) at most `DEGENERATE_EPS_MM` long.
    Needle(u32, u32),
    /// Its altitude onto its longest edge, edge `k` (from vertex k to k+1),
    /// is at most `DEGENERATE_EPS_MM`.
    Cap(usize),
}

fn degeneracy(pos: &[P], t: [u32; 3]) -> Option<Degenerate> {
    let p = t.map(|v| pos[v as usize]);
    let len = [0, 1, 2].map(|k| norm(sub(p[(k + 1) % 3], p[k])));
    let shortest = (0..3).min_by(|&i, &j| len[i].total_cmp(&len[j]))?;
    if len[shortest] <= DEGENERATE_EPS_MM {
        return Some(Degenerate::Needle(t[shortest], t[(shortest + 1) % 3]));
    }
    let longest = (0..3).max_by(|&i, &j| len[i].total_cmp(&len[j]))?;
    let altitude = norm(cross(sub(p[1], p[0]), sub(p[2], p[0]))) / len[longest];
    (altitude <= DEGENERATE_EPS_MM).then_some(Degenerate::Cap(longest))
}

/// Remove degenerate facets without opening the surface: collapse needles
/// and flip the long edge of caps. Deleting a cap instead, with three
/// distinct vertices, would leave a sliver hole. Each pass fixes facets
/// whose neighbourhoods no earlier fix of the pass touched, then the
/// adjacency is rebuilt.
fn repair_degenerates(
    pos: &mut [P],
    tris: &mut Vec<[u32; 3]>,
    origin: &mut Vec<usize>,
    log: &mut RepairLog,
) {
    for _ in 0..REPAIR_PASSES {
        let bad: Vec<usize> = (0..tris.len())
            .filter(|&t| degeneracy(pos, tris[t]).is_some())
            .collect();
        if bad.is_empty() {
            return;
        }
        let mut around = vec![Vec::new(); pos.len()];
        for (i, t) in tris.iter().enumerate() {
            for &v in t {
                around[v as usize].push(i);
            }
        }
        let mut touched = vec![false; pos.len()];
        let mut dead = vec![false; tris.len()];
        let mut changed = false;
        for t in bad {
            if dead[t] || tris[t].iter().any(|&v| touched[v as usize]) {
                continue;
            }
            let fixed = match degeneracy(pos, tris[t]) {
                Some(Degenerate::Needle(a, b)) => {
                    collapse(pos, tris, &around, &mut dead, &mut touched, a, b)
                        .then_some(&mut log.collapsed_edges)
                }
                Some(Degenerate::Cap(k)) => {
                    flip(tris, &around, &dead, &mut touched, t, k).then_some(&mut log.flipped_edges)
                }
                None => None,
            };
            if let Some(count) = fixed {
                *count += 1;
                changed = true;
            }
        }
        if dead.contains(&true) {
            let mut keep = dead.iter().map(|d| !d);
            let mut keep2 = keep.clone();
            tris.retain(|_| keep.next().unwrap_or(false));
            origin.retain(|_| keep2.next().unwrap_or(false));
        }
        if !changed {
            return;
        }
    }
}

/// Collapse edge (a, b) to its midpoint, unless that would fuse two parts of
/// the surface. The link condition: the vertices adjacent to both a and b are
/// exactly the apexes of the facets on the edge.
fn collapse(
    pos: &mut [P],
    tris: &mut [[u32; 3]],
    around: &[Vec<usize>],
    dead: &mut [bool],
    touched: &mut [bool],
    a: u32,
    b: u32,
) -> bool {
    let live = |v: u32| {
        around[v as usize]
            .iter()
            .copied()
            .filter(|&t| !dead[t])
            .collect::<Vec<_>>()
    };
    let link = |v: u32| -> BTreeSet<u32> {
        live(v)
            .into_iter()
            .flat_map(|t| tris[t])
            .filter(|&w| w != v)
            .collect()
    };
    let common: BTreeSet<u32> = link(a).intersection(&link(b)).copied().collect();
    let apexes: BTreeSet<u32> = live(a)
        .into_iter()
        .filter(|&t| tris[t].contains(&b))
        .flat_map(|t| tris[t])
        .filter(|&w| w != a && w != b)
        .collect();
    if common != apexes {
        return false;
    }
    let (pa, pb) = (pos[a as usize], pos[b as usize]);
    pos[a as usize] = [0, 1, 2].map(|k| 0.5 * (pa[k] + pb[k]));
    for &t in &around[b as usize] {
        if dead[t] {
            continue;
        }
        if tris[t].contains(&a) {
            dead[t] = true;
        } else {
            for v in tris[t].iter_mut().filter(|v| **v == b) {
                *v = a;
            }
        }
    }
    for &t in around[a as usize].iter().chain(&around[b as usize]) {
        for &v in &tris[t] {
            touched[v as usize] = true;
        }
    }
    touched[b as usize] = true;
    true
}

/// Flip the longest edge (u, v) of cap `t` = (u, v, w): with its neighbour
/// (v, u, x), the pair becomes (u, x, w) and (x, v, w). As w lies on uv
/// within `DEGENERATE_EPS_MM`, the surface keeps its shape. Refused unless
/// exactly those two facets share uv, consistently wound, and edge (w, x)
/// does not exist yet.
fn flip(
    tris: &mut [[u32; 3]],
    around: &[Vec<usize>],
    dead: &[bool],
    touched: &mut [bool],
    t: usize,
    k: usize,
) -> bool {
    let [u, v, w] = [tris[t][k], tris[t][(k + 1) % 3], tris[t][(k + 2) % 3]];
    let on_uv: Vec<usize> = around[u as usize]
        .iter()
        .copied()
        .filter(|&s| !dead[s] && tris[s].contains(&v))
        .collect();
    if on_uv.len() != 2 {
        return false;
    }
    let s = if on_uv[0] == t { on_uv[1] } else { on_uv[0] };
    let Some(&x) = tris[s].iter().find(|&&y| y != u && y != v) else {
        return false;
    };
    if !has_edge(tris[s], v, u)
        || x == w
        || around[w as usize]
            .iter()
            .any(|&r| !dead[r] && tris[r].contains(&x))
    {
        return false;
    }
    tris[t] = [u, x, w];
    tris[s] = [x, v, w];
    for y in [u, v, w, x] {
        touched[y as usize] = true;
    }
    true
}

/// The facets `tris` with only the vertices they use, numbered in order of
/// first use.
fn compact(pos: &[P], tris: impl IntoIterator<Item = [u32; 3]>) -> Mesh {
    let mut index: HashMap<u32, u32> = HashMap::new();
    let mut mesh = Mesh::default();
    for t in tris {
        let t = t.map(|v| {
            *index.entry(v).or_insert_with(|| {
                mesh.positions.push(pos[v as usize]);
                (mesh.positions.len() - 1) as u32
            })
        });
        mesh.triangles.push(t);
    }
    mesh
}

/// A solid: an outer shell and the cavities directly inside it, wound so
/// that every facet faces away from the material. One `<tessellated>` solid.
pub struct Body {
    pub mesh: Mesh,
    /// The source mesh of its outer shell.
    pub source: usize,
    pub shells: usize,
    /// Facets whose winding differs from the source.
    pub reversed: usize,
}

pub struct Bodies {
    pub bodies: Vec<Body>,
    /// Source mesh and reason for each body that was not kept.
    pub rejected: Vec<(usize, String)>,
}

/// Directions for parity rays, with no simple ratios between components, so
/// that grazing an edge or vertex is rare; a graze retries with the next.
const RAY_DIRECTIONS: [P; 6] = [
    [0.7385122, 0.4246963, 0.5236413],
    [-0.3216543, 0.8839156, 0.3393217],
    [0.2849173, -0.4136571, 0.8647325],
    [-0.6754819, -0.5233497, -0.5193218],
    [0.5527121, -0.7729385, -0.3115662],
    [-0.1417332, 0.2381947, -0.9608193],
];

/// Nest shells into bodies. A shell inside an even number of others is an
/// outer boundary; inside an odd number, a cavity of the shell immediately
/// around it. Facets are checked for crossings first, with exact predicates:
/// a body whose own facets cross is rejected, as Geant4 cannot tell its
/// inside from its outside; bodies that cross each other are kept, and
/// [`crossings`] reports them.
pub fn bodies(mut shells: Vec<Shell>) -> Bodies {
    let n = shells.len();
    let mut owner = Vec::new();
    let mut boxes = Vec::new();
    for (s, shell) in shells.iter().enumerate() {
        for (i, &t) in shell.mesh.triangles.iter().enumerate() {
            owner.push((s, i));
            boxes.push(Aabb::of(&shell.mesh.corners(t)));
        }
    }
    let corners = |shells: &[Shell], f: usize| {
        let (s, i) = owner[f];
        shells[s].mesh.corners(shells[s].mesh.triangles[i])
    };
    let bvh = Bvh::new(boxes);
    let mut self_crossings = vec![0usize; n];
    let mut crossed: BTreeSet<(usize, usize)> = BTreeSet::new();
    bvh.self_pairs(|f, g| {
        let (s, t) = (owner[f].0, owner[g].0);
        let pair = (s.min(t), s.max(t));
        if (s == t || !crossed.contains(&pair))
            && intersect::triangles_cross(&corners(&shells, f), &corners(&shells, g))
        {
            if s == t {
                self_crossings[s] += 1;
            } else {
                crossed.insert(pair);
            }
        }
    });

    // Nesting by ray parity from a point of each shell. Shells that cross
    // have no nesting relation; a shell can only be inside one whose bounds
    // contain its own.
    let bounds: Vec<Aabb> = shells.iter().map(|s| s.mesh.bounds()).collect();
    let mut all = Aabb::of(&[]);
    bounds.iter().for_each(|b| all.add(b));
    let size = all.diagonal()
        + all
            .lo
            .iter()
            .chain(&all.hi)
            .fold(0.0f64, |m, c| m.max(c.abs()));
    let (reach, margin) = (2.0 * all.diagonal() + 1.0, 1e-7 * size);
    let mut containers: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut undecided = vec![false; n];
    for s in 0..n {
        let candidate: Vec<bool> = (0..n)
            .map(|t| {
                t != s && !crossed.contains(&(s.min(t), s.max(t))) && bounds[t].contains(&bounds[s])
            })
            .collect();
        if !candidate.contains(&true) {
            continue;
        }
        let p = start_point(&shells[s].mesh);
        undecided[s] = true;
        for d in RAY_DIRECTIONS {
            let q = [0, 1, 2].map(|k| p[k] + reach * d[k]);
            let mut parity = vec![false; n];
            let mut graze = false;
            bvh.along_segment(p, q, margin, |f| {
                let t = owner[f].0;
                if graze || !candidate[t] {
                    return;
                }
                match intersect::crossing(p, q, &corners(&shells, f)) {
                    Crossing::Through => parity[t] ^= true,
                    Crossing::Graze => graze = true,
                    Crossing::Miss => {}
                }
            });
            if !graze {
                containers[s] = (0..n).filter(|&t| parity[t]).collect();
                undecided[s] = false;
                break;
            }
        }
    }
    let depth: Vec<usize> = containers.iter().map(Vec::len).collect();
    let parent: Vec<Option<usize>> = containers
        .iter()
        .map(|c| c.iter().copied().max_by_key(|&t| depth[t]))
        .collect();

    // Roles, parents first: a cavity has odd depth and an outer parent.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&s| depth[s]);
    let mut cavity = vec![false; n];
    for &s in &order {
        if let Some(p) = parent[s] {
            cavity[s] = depth[s] % 2 == 1 && !cavity[p];
        }
    }
    for (s, shell) in shells.iter_mut().enumerate() {
        if (shell.mesh.volume() > 0.0) == cavity[s] {
            shell.mesh.triangles.iter_mut().for_each(|t| t.swap(1, 2));
            shell.reversed = shell.mesh.triangles.len() - shell.reversed;
        }
    }
    let mut members: Vec<Vec<usize>> = Vec::new();
    let mut body_of = vec![usize::MAX; n];
    for s in (0..n).filter(|&s| !cavity[s]) {
        body_of[s] = members.len();
        members.push(vec![s]);
    }
    for s in (0..n).filter(|&s| cavity[s]) {
        if let Some(p) = parent[s] {
            body_of[s] = body_of[p];
            members[body_of[p]].push(s);
        }
    }

    // Keep the bodies that are valid solids.
    let mut rejected = Vec::new();
    let mut result: Vec<Body> = Vec::new();
    for (b, shell_ids) in members.iter().enumerate() {
        let source = shells[shell_ids[0]].source;
        let crossings: usize = shell_ids.iter().map(|&s| self_crossings[s]).sum();
        let inner = crossed
            .iter()
            .any(|&(s, t)| body_of[s] == b && body_of[t] == b);
        let mut mesh = Mesh::default();
        for &s in shell_ids {
            let base = mesh.positions.len() as u32;
            mesh.positions.extend(&shells[s].mesh.positions);
            mesh.triangles
                .extend(shells[s].mesh.triangles.iter().map(|t| t.map(|v| v + base)));
        }
        let problem = if crossings > 0 {
            Some(format!("{crossings} pairs of its facets cross each other"))
        } else if inner {
            Some("its outer surface and a cavity cross each other".to_string())
        } else if shell_ids.iter().any(|&s| undecided[s]) {
            Some("whether it lies inside another surface could not be decided".to_string())
        } else if mesh.volume() <= 0.0 {
            Some("it encloses no volume".to_string())
        } else {
            None
        };
        if let Some(problem) = problem {
            rejected.push((source, problem));
            continue;
        }
        result.push(Body {
            mesh,
            source,
            shells: shell_ids.len(),
            reversed: shell_ids.iter().map(|&s| shells[s].reversed).sum(),
        });
    }
    Bodies {
        bodies: result,
        rejected,
    }
}

/// Pairs of placed meshes that pass through each other by more than
/// `DEGENERATE_EPS_MM`, which Geant4 reports as an overlap; meshes that only
/// touch do not count, nor do overlaps bounded by faces that lie flush with
/// the other mesh's (the worker's exact check finds those in STEP files). `placed`
/// holds each mesh with its world transform, and the pairs index it, i < j.
/// `progress` hears how many of the meshes have been compared with the rest,
/// and stops the comparison by returning false.
pub fn crossings(
    placed: &[(&Mesh, Affine)],
    mut progress: impl FnMut(usize, usize) -> bool,
) -> Vec<(usize, usize)> {
    let bounds: Vec<Aabb> = placed
        .iter()
        .map(|(mesh, at)| Aabb::of(&mesh.bounds().corners().map(|c| at.apply(c))))
        .collect();
    let mut world: Vec<Option<Vec<P>>> = vec![None; placed.len()];
    let mut out = Vec::new();
    for i in 0..placed.len() {
        if !progress(i, placed.len()) {
            break;
        }
        for j in i + 1..placed.len() {
            if !bounds[i].overlaps(&bounds[j]) {
                continue;
            }
            let common = bounds[i].common(&bounds[j]);
            // The facets of each in the region where their bounds overlap.
            let mut near = |k: usize| -> Vec<[P; 3]> {
                let (mesh, at) = placed[k];
                let positions = world[k]
                    .get_or_insert_with(|| mesh.positions.iter().map(|&p| at.apply(p)).collect());
                (mesh.triangles.iter())
                    .map(|t| t.map(|v| positions[v as usize]))
                    .filter(|t| Aabb::of(t).overlaps(&common))
                    .collect()
            };
            let (a, b) = (near(i), near(j));
            let mut crossed = false;
            Bvh::new(a.iter().chain(&b).map(|t| Aabb::of(t)).collect()).self_pairs(|f, g| {
                let (f, g) = (f.min(g), f.max(g));
                if !crossed && f < a.len() && g >= a.len() {
                    crossed =
                        intersect::triangles_pierce(&a[f], &b[g - a.len()], DEGENERATE_EPS_MM);
                }
            });
            if crossed {
                out.push((i, j));
            }
        }
    }
    out
}

/// A point on the shell: the centroid of its largest facet.
fn start_point(mesh: &Mesh) -> P {
    let area = |t: [u32; 3]| {
        let [a, b, c] = mesh.corners(t);
        norm(cross(sub(b, a), sub(c, a)))
    };
    let t = mesh
        .triangles
        .iter()
        .copied()
        .max_by(|&s, &t| area(s).total_cmp(&area(t)))
        .unwrap_or([0, 0, 0]);
    let [a, b, c] = mesh.corners(t);
    [0, 1, 2].map(|k| (a[k] + b[k] + c[k]) / 3.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 8 corners of a box as indices 0..8, bit k set meaning `hi` on axis k.
    fn corner(lo: P, hi: P, i: usize) -> P {
        [0, 1, 2].map(|k| if i >> k & 1 == 1 { hi[k] } else { lo[k] })
    }

    /// Faces as corner quadruples, counterclockwise seen from outside.
    const FACES: [[usize; 4]; 6] = [
        [0, 4, 6, 2], // -x
        [1, 3, 7, 5], // +x
        [0, 1, 5, 4], // -y
        [2, 6, 7, 3], // +y
        [0, 2, 3, 1], // -z
        [4, 5, 7, 6], // +z
    ];

    /// A box as 12 outward-wound triangles, `skip` faces left out.
    fn box_soup_without(lo: P, hi: P, skip: &[usize]) -> Vec<[P; 3]> {
        let c = |i| corner(lo, hi, i);
        FACES
            .iter()
            .enumerate()
            .filter(|(f, _)| !skip.contains(f))
            .flat_map(|(_, q)| [[c(q[0]), c(q[1]), c(q[2])], [c(q[0]), c(q[2]), c(q[3])]])
            .collect()
    }

    fn box_soup(lo: P, hi: P) -> Vec<[P; 3]> {
        box_soup_without(lo, hi, &[])
    }

    fn flipped(t: [P; 3]) -> [P; 3] {
        [t[0], t[2], t[1]]
    }

    fn import(soup: &[[P; 3]]) -> (Shelled, Bodies) {
        let mut shelled = shells(soup, 0, true);
        let found = bodies(std::mem::take(&mut shelled.shells));
        (shelled, found)
    }

    const LO: P = [-5.0, -10.0, -15.0];
    const HI: P = [5.0, 10.0, 15.0];

    fn assert_close(a: f64, b: f64, rel: f64) {
        assert!((a - b).abs() <= rel * b.abs(), "{a} vs {b}");
    }

    #[test]
    fn a_box_is_one_body_with_its_volume() {
        let (shelled, found) = import(&box_soup(LO, HI));
        assert!(shelled.rejected.is_empty() && found.rejected.is_empty());
        let [body] = &found.bodies[..] else {
            panic!("expected one body")
        };
        assert_eq!(
            (body.mesh.positions.len(), body.mesh.triangles.len()),
            (8, 12)
        );
        assert_eq!(body.reversed, 0);
        assert_close(body.mesh.volume(), 6000.0, 1e-12);
        assert_close(body.mesh.area(), 2.0 * (200.0 + 300.0 + 600.0), 1e-12);
    }

    #[test]
    fn reversed_facets_are_rewound() {
        let all: Vec<_> = box_soup(LO, HI).into_iter().map(flipped).collect();
        let (_, found) = import(&all);
        assert_eq!(found.bodies[0].reversed, 12);
        assert_close(found.bodies[0].mesh.volume(), 6000.0, 1e-12);
        let mut one = box_soup(LO, HI);
        one[5] = flipped(one[5]);
        let (_, found) = import(&one);
        assert_eq!(found.bodies[0].reversed, 1);
        assert_close(found.bodies[0].mesh.volume(), 6000.0, 1e-12);
    }

    #[test]
    fn open_and_non_manifold_surfaces_are_rejected() {
        let (shelled, _) = import(&box_soup_without(LO, HI, &[5]));
        assert!(shelled.shells.is_empty());
        assert!(
            shelled.rejected[0].starts_with("open surface: 4 edges"),
            "{:?}",
            shelled.rejected
        );
        // Two boxes sharing the edge x = 5, y = 10: four facets meet there.
        let mut soup = box_soup(LO, HI);
        soup.extend(box_soup([5.0, 10.0, -15.0], [15.0, 20.0, 15.0]));
        let (shelled, _) = import(&soup);
        assert!(shelled.shells.is_empty());
        assert!(shelled.rejected[0].contains("shared by more than two facets"));
    }

    #[test]
    fn non_finite_coordinates_are_rejected() {
        let mut soup = box_soup(LO, HI);
        soup[0][0][1] = f64::NAN;
        let (shelled, _) = import(&soup);
        assert!(shelled.shells.is_empty() && shelled.rejected[0].contains("finite"));
    }

    #[test]
    fn cracks_within_tolerance_are_welded() {
        // D = 37.4 mm, so the tolerance is 3.7e-6 mm.
        let mut soup = box_soup(LO, HI);
        soup[0][0][0] += 2e-6;
        let (shelled, found) = import(&soup);
        assert_eq!(shelled.log.welded_vertices, 1);
        assert_close(shelled.log.weld_max_mm, 2e-6, 1e-6);
        assert_eq!(found.bodies.len(), 1);
        // Beyond the tolerance the crack stays open.
        soup[0][0][0] += 1e-4;
        assert!(import(&soup).1.bodies.is_empty());
    }

    #[test]
    fn repeated_and_back_to_back_facets_are_dropped() {
        let mut soup = box_soup(LO, HI);
        soup.push(soup[0]);
        let wall = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        soup.extend([wall, flipped(wall)]);
        soup.push([LO, LO, HI]);
        let (shelled, found) = import(&soup);
        assert_eq!(shelled.log.dropped_facets, 4);
        assert_eq!(found.bodies.len(), 1);
    }

    /// The +z face of the box, A B C D counterclockwise from above.
    fn top() -> [P; 4] {
        [4, 5, 7, 6].map(|i| corner(LO, HI, i))
    }

    #[test]
    fn a_needle_is_collapsed_and_the_body_stays_closed() {
        // E sits 1e-8 mm from A inside facet ABC, split into ABE, BCE, CAE:
        // two facets with the short edge AE.
        let [a, b, c, d] = top();
        let e = [a[0] + 1e-8, a[1] + 1e-8, a[2]];
        let mut soup = box_soup_without(LO, HI, &[5]);
        soup.extend([[a, b, e], [b, c, e], [c, a, e], [a, c, d]]);
        let (shelled, found) = import(&soup);
        assert_eq!(shelled.log.collapsed_edges, 1);
        assert_eq!(found.bodies[0].mesh.triangles.len(), 12);
        assert_close(found.bodies[0].mesh.volume(), 6000.0, 1e-9);
    }

    #[test]
    fn a_cap_is_flipped_away_and_the_body_stays_closed() {
        // E is the midpoint of the diagonal AC. ABC is split at E, so ACD's
        // edge AC meets AE and EC; the zero-area cap AEC closes the gap.
        let [a, b, c, d] = top();
        let e = [0, 1, 2].map(|k| 0.5 * (a[k] + c[k]));
        let mut soup = box_soup_without(LO, HI, &[5]);
        soup.extend([[a, b, e], [e, b, c], [a, c, d], [a, e, c]]);
        let (shelled, found) = import(&soup);
        assert_eq!(shelled.log.flipped_edges, 1);
        let [body] = &found.bodies[..] else {
            panic!("expected one body: {:?}", found.rejected)
        };
        assert_eq!(body.mesh.triangles.len(), 14);
        assert_close(body.mesh.volume(), 6000.0, 1e-12);
    }

    #[test]
    fn a_surface_crossing_itself_is_rejected() {
        // Pull one top corner through the bottom face.
        let mut soup = box_soup(LO, HI);
        let corner7 = corner(LO, HI, 7);
        for p in soup.iter_mut().flatten() {
            if *p == corner7 {
                *p = [HI[0], HI[1], LO[2] - 10.0];
            }
        }
        let (_, found) = import(&soup);
        assert!(found.bodies.is_empty());
        assert!(
            found.rejected[0].1.contains("cross each other"),
            "{:?}",
            found.rejected
        );
    }

    #[test]
    fn cavities_and_islands_nest_by_depth() {
        // Outer box, a cavity, and an island inside the cavity: all wound
        // outward in the source; the cavity must be turned inside out.
        let mut soup = box_soup([-10.0; 3], [10.0; 3]);
        soup.extend(box_soup([-6.0; 3], [6.0; 3]));
        soup.extend(box_soup([-2.0; 3], [2.0; 3]));
        let (_, found) = import(&soup);
        let volumes: Vec<f64> = found.bodies.iter().map(|b| b.mesh.volume()).collect();
        assert_eq!(
            found.bodies.iter().map(|b| b.shells).collect::<Vec<_>>(),
            [2, 1]
        );
        assert_close(volumes[0], 8000.0 - 1728.0, 1e-12);
        assert_close(volumes[1], 64.0, 1e-12);
        assert_eq!(found.bodies[0].reversed, 12);
    }

    #[test]
    fn bodies_that_cross_are_kept_and_reported() {
        let mut soup = box_soup(LO, HI);
        soup.extend(box_soup([0.0, 0.0, 0.0], [20.0, 20.0, 20.0]));
        let (_, found) = import(&soup);
        assert_eq!(found.bodies.len(), 2);
        let placed: Vec<(&Mesh, Affine)> = found
            .bodies
            .iter()
            .map(|b| (&b.mesh, Affine::IDENTITY))
            .collect();
        assert_eq!(crossings(&placed, |_, _| true), [(0, 1)]);
    }

    #[test]
    fn placed_meshes_cross_only_where_they_interpenetrate() {
        let (_, found) = import(&box_soup([0.0; 3], [1.0; 3]));
        let cube = &found.bodies[0].mesh;
        let shift = |t: P| Affine {
            t,
            ..Affine::IDENTITY
        };
        let mirror = Affine {
            r: [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            t: [0.0; 3],
        };
        // A rotation about (1, 1, 1) by 0.7 rad, whose matrix is inexact.
        let (s, c) = 0.7f64.sin_cos();
        let (a, b) = ((1.0 + 2.0 * c) / 3.0, (1.0 - c) / 3.0);
        let q = s / 3f64.sqrt();
        let turn = Affine {
            r: [[a, b - q, b + q], [b + q, a, b - q], [b - q, b + q, a]],
            t: [0.0; 3],
        };
        let placed = [
            (cube, Affine::IDENTITY),
            // Crosses 0 and 2.
            (cube, shift([0.5, 0.5, 0.5])),
            // On top of 0, face to face.
            (cube, shift([0.0, 0.0, 1.0])),
            // Reflected: beside 0, face to face at x = 0.
            (cube, mirror),
            // Reflected into 0, 2 and 3; face to face with 1.
            (cube, mirror.after(&shift([-0.5, 0.25, 0.25]))),
            // Face to face after an inexact rotation, far from the rest.
            (cube, shift([10.0; 3]).after(&turn)),
            (
                cube,
                shift([10.0; 3]).after(&turn).after(&shift([0.0, 0.0, 1.0])),
            ),
        ];
        assert_eq!(
            crossings(&placed, |_, _| true),
            [(0, 1), (0, 4), (1, 2), (2, 4), (3, 4)]
        );
    }
}
