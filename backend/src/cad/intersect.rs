//! Exact intersection tests between triangles and segments, and a bounding
//! volume hierarchy over triangles. They back the self-intersection check and
//! the shell nesting of the mesh pipeline (algorithm B in
//! `doc/cad-import/PLAN.md`).
//!
//! Every decision is the sign of Shewchuk's adaptive orientation predicates
//! (the `robust` crate), which is exact for `f64` input, so touching, coplanar
//! and nearly parallel triangles are never misjudged.

use robust::{orient2d, orient3d, Coord, Coord3D};

pub type P = [f64; 3];

pub fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn cross(a: P, b: P) -> P {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn norm(a: P) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

/// The sign of `x` as -1, 0 or 1 (`f64::signum` calls 0 positive).
fn sign(x: f64) -> i8 {
    if x > 0.0 {
        1
    } else if x < 0.0 {
        -1
    } else {
        0
    }
}

/// Side of the plane through `a`, `b`, `c` on which `d` lies.
fn o3(a: P, b: P, c: P, d: P) -> i8 {
    let c3 = |p: P| Coord3D {
        x: p[0],
        y: p[1],
        z: p[2],
    };
    sign(orient3d(c3(a), c3(b), c3(c), c3(d)))
}

/// The two coordinates that a projection of the plane of `t` keeps: those
/// across the largest component of its normal. Dropping a coordinate is exact,
/// and the dropped normal component is far from zero, so incidence within the
/// plane survives the projection.
fn projection(t: &[P; 3]) -> (usize, usize) {
    let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
    let k = (0..3)
        .max_by(|&i, &j| n[i].abs().total_cmp(&n[j].abs()))
        .unwrap_or(2);
    [(1, 2), (2, 0), (0, 1)][k]
}

/// Side of the line through `a`, `b` on which `c` lies, in projection `ax`.
fn o2(ax: (usize, usize), a: P, b: P, c: P) -> i8 {
    let c2 = |p: P| Coord {
        x: p[ax.0],
        y: p[ax.1],
    };
    sign(orient2d(c2(a), c2(b), c2(c)))
}

/// For `r` on the line through `p`, `q`: whether it lies on the segment.
fn within(ax: (usize, usize), p: P, q: P, r: P) -> bool {
    [ax.0, ax.1]
        .iter()
        .all(|&i| r[i] >= p[i].min(q[i]) && r[i] <= p[i].max(q[i]))
}

/// Closed segments `pq` and `rs` of one plane meet.
fn segments_meet_2d(ax: (usize, usize), p: P, q: P, r: P, s: P) -> bool {
    let (d1, d2) = (o2(ax, r, s, p), o2(ax, r, s, q));
    let (d3, d4) = (o2(ax, p, q, r), o2(ax, p, q, s));
    if d1 * d2 < 0 && d3 * d4 < 0 {
        return true;
    }
    (d1 == 0 && within(ax, r, s, p))
        || (d2 == 0 && within(ax, r, s, q))
        || (d3 == 0 && within(ax, p, q, r))
        || (d4 == 0 && within(ax, p, q, s))
}

/// Point `p` of the plane of `t` lies in the closed triangle.
fn point_in_triangle_2d(ax: (usize, usize), t: &[P; 3], p: P) -> bool {
    let s = [
        o2(ax, t[0], t[1], p),
        o2(ax, t[1], t[2], p),
        o2(ax, t[2], t[0], p),
    ];
    s.iter().all(|&v| v >= 0) || s.iter().all(|&v| v <= 0)
}

/// Segment `pq`, lying in the plane of `t`, meets the closed triangle.
fn segment_meets_triangle_2d(ax: (usize, usize), p: P, q: P, t: &[P; 3]) -> bool {
    point_in_triangle_2d(ax, t, p)
        || point_in_triangle_2d(ax, t, q)
        || (0..3).any(|k| segments_meet_2d(ax, p, q, t[k], t[(k + 1) % 3]))
}

/// The closed segment `pq` meets the closed triangle `t`.
pub fn segment_meets_triangle(p: P, q: P, t: &[P; 3]) -> bool {
    let (sp, sq) = (o3(t[0], t[1], t[2], p), o3(t[0], t[1], t[2], q));
    if sp * sq > 0 {
        return false;
    }
    if sp == 0 && sq == 0 {
        return segment_meets_triangle_2d(projection(t), p, q, t);
    }
    // The segment crosses the plane at one point; it is in the triangle when
    // the line through p and q passes each edge with the same orientation.
    let s = [
        o3(p, q, t[0], t[1]),
        o3(p, q, t[1], t[2]),
        o3(p, q, t[2], t[0]),
    ];
    s.iter().all(|&v| v >= 0) || s.iter().all(|&v| v <= 0)
}

/// Two closed triangles with no vertex in common meet.
fn triangles_meet(t1: &[P; 3], t2: &[P; 3]) -> bool {
    let s2 = t2.map(|p| o3(t1[0], t1[1], t1[2], p));
    if s2.iter().all(|&v| v > 0) || s2.iter().all(|&v| v < 0) {
        return false;
    }
    let s1 = t1.map(|p| o3(t2[0], t2[1], t2[2], p));
    if s1.iter().all(|&v| v > 0) || s1.iter().all(|&v| v < 0) {
        return false;
    }
    if s2 == [0; 3] {
        let ax = projection(t1);
        return (0..3).any(|i| {
            (0..3).any(|j| segments_meet_2d(ax, t1[i], t1[(i + 1) % 3], t2[j], t2[(j + 1) % 3]))
        }) || point_in_triangle_2d(ax, t1, t2[0])
            || point_in_triangle_2d(ax, t2, t1[0]);
    }
    // Where two convex sets meet, the extreme points of the intersection lie
    // on the boundary of one of them, so an edge of one meets the other.
    (0..3).any(|k| segment_meets_triangle(t1[k], t1[(k + 1) % 3], t2))
        || (0..3).any(|k| segment_meets_triangle(t2[k], t2[(k + 1) % 3], t1))
}

/// The edge from vertex `t[0]` to `x` runs into triangle `t` beyond that
/// vertex: it lies in the plane of `t` and points into its angle at `t[0]`.
fn edge_enters(x: P, t: &[P; 3]) -> bool {
    if o3(t[0], t[1], t[2], x) != 0 {
        return false;
    }
    let ax = projection(t);
    let (v, c, d) = (t[0], t[1], t[2]);
    let toward = |side: i8, of: i8| side == 0 || side == of;
    toward(o2(ax, v, c, x), o2(ax, v, c, d)) && toward(o2(ax, v, d, x), o2(ax, v, d, c))
}

/// Whether two non-degenerate triangles meet anywhere except at the vertices
/// and the edge they share. Shared means equal coordinates, so two shells that
/// touch at one point do not count as crossing there.
pub fn triangles_cross(t1: &[P; 3], t2: &[P; 3]) -> bool {
    let shared: Vec<(usize, usize)> = (0..3)
        .flat_map(|i| (0..3).map(move |j| (i, j)))
        .filter(|&(i, j)| t1[i] == t2[j])
        .collect();
    match shared[..] {
        [] => triangles_meet(t1, t2),
        [(i, j)] => {
            // T1 = (v, a, b) and T2 = (v, c, d): what they have in common
            // beyond v reaches an edge, either one opposite v or one from v
            // that lies in the other triangle's plane.
            let r1 = [t1[i], t1[(i + 1) % 3], t1[(i + 2) % 3]];
            let r2 = [t2[j], t2[(j + 1) % 3], t2[(j + 2) % 3]];
            segment_meets_triangle(r1[1], r1[2], &r2)
                || segment_meets_triangle(r2[1], r2[2], &r1)
                || edge_enters(r1[1], &r2)
                || edge_enters(r1[2], &r2)
                || edge_enters(r2[1], &r1)
                || edge_enters(r2[2], &r1)
        }
        [(i1, j1), (i2, j2)] => {
            // A shared edge: off one plane, the triangles meet only along it;
            // in one plane, they overlap when both lie on the same side of it.
            let (s1, s2) = (t1[i1], t1[i2]);
            let a = t1[3 - i1 - i2];
            let b = t2[3 - j1 - j2];
            if o3(s1, s2, a, b) != 0 {
                return false;
            }
            let ax = projection(&[s1, s2, a]);
            o2(ax, s1, s2, a) * o2(ax, s1, s2, b) >= 0
        }
        _ => true,
    }
}

/// Whether two triangles pass through each other by more than `eps`: each
/// has corners farther than `eps` from the other's plane on both sides, and
/// where they cross the line common to both planes, their segments overlap
/// by more than `eps` (the two triangles meet in that overlap and nowhere
/// else). Surfaces that touch, in a common plane, along an edge or at a
/// point, do not, nor do surfaces that cross by less than `eps`, as rounding
/// leaves surfaces that touch.
pub fn triangles_pierce(t1: &[P; 3], t2: &[P; 3], eps: f64) -> bool {
    let dot = |a: P, b: P| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let unit_normal = |t: &[P; 3]| {
        let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
        n.map(|c| c / norm(n))
    };
    let (n1, n2) = (unit_normal(t1), unit_normal(t2));
    // Signed distances of the corners of `t` from the plane of `by`.
    let distances = |t: &[P; 3], by: &[P; 3], n: P| t.map(|p| dot(sub(p, by[0]), n));
    let (d1, d2) = (distances(t1, t2, n2), distances(t2, t1, n1));
    let straddles = |d: [f64; 3]| d.iter().any(|&x| x > eps) && d.iter().any(|&x| x < -eps);
    if !(straddles(d1) && straddles(d2)) {
        return false;
    }
    // Where the edges of `t` cross the other plane, as positions along the
    // common line.
    let line = cross(n1, n2);
    let along = |t: &[P; 3], d: [f64; 3]| {
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            if d[a].min(d[b]) <= 0.0 && d[a].max(d[b]) >= 0.0 && d[a] != d[b] {
                let f = d[a] / (d[a] - d[b]);
                let x = dot([0, 1, 2].map(|k| t[a][k] + f * (t[b][k] - t[a][k])), line);
                (lo, hi) = (lo.min(x), hi.max(x));
            }
        }
        (lo, hi)
    };
    let ((lo1, hi1), (lo2, hi2)) = (along(t1, d1), along(t2, d2));
    hi1.min(hi2) - lo1.max(lo2) > eps * norm(line)
}

/// How a segment from `p` to `q` passes a triangle, for ray parity.
#[derive(Debug, PartialEq, Eq)]
pub enum Crossing {
    Miss,
    Through,
    /// It grazes an edge or vertex, or ends on the triangle: parity is
    /// undefined and the caller tries another direction.
    Graze,
}

pub fn crossing(p: P, q: P, t: &[P; 3]) -> Crossing {
    let (sp, sq) = (o3(t[0], t[1], t[2], p), o3(t[0], t[1], t[2], q));
    for (s, end) in [(sp, p), (sq, q)] {
        if s == 0 {
            return if point_in_triangle_2d(projection(t), t, end) {
                Crossing::Graze
            } else {
                Crossing::Miss
            };
        }
    }
    if sp == sq {
        return Crossing::Miss;
    }
    let s = [
        o3(p, q, t[0], t[1]),
        o3(p, q, t[1], t[2]),
        o3(p, q, t[2], t[0]),
    ];
    if s.contains(&1) && s.contains(&-1) {
        Crossing::Miss
    } else if s.contains(&0) {
        Crossing::Graze
    } else {
        Crossing::Through
    }
}

/// Axis-aligned box.
#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub lo: P,
    pub hi: P,
}

impl Aabb {
    pub fn of(points: &[P]) -> Self {
        let mut b = Aabb {
            lo: [f64::INFINITY; 3],
            hi: [f64::NEG_INFINITY; 3],
        };
        for p in points {
            b.add(&Aabb { lo: *p, hi: *p });
        }
        b
    }

    pub fn add(&mut self, other: &Aabb) {
        for k in 0..3 {
            self.lo[k] = self.lo[k].min(other.lo[k]);
            self.hi[k] = self.hi[k].max(other.hi[k]);
        }
    }

    /// Closed boxes overlap (touching counts).
    pub fn overlaps(&self, other: &Aabb) -> bool {
        (0..3).all(|k| self.lo[k] <= other.hi[k] && other.lo[k] <= self.hi[k])
    }

    pub fn contains(&self, other: &Aabb) -> bool {
        (0..3).all(|k| self.lo[k] <= other.lo[k] && other.hi[k] <= self.hi[k])
    }

    pub fn diagonal(&self) -> f64 {
        norm(sub(self.hi, self.lo))
    }

    /// Corner `c` has `hi` on axis k where bit k of `c` is set.
    pub fn corners(&self) -> [P; 8] {
        std::array::from_fn(|c| {
            [0, 1, 2].map(|k| {
                if c >> k & 1 == 1 {
                    self.hi[k]
                } else {
                    self.lo[k]
                }
            })
        })
    }

    /// Where two boxes overlap (empty, with `lo > hi`, where they do not).
    pub fn common(&self, other: &Aabb) -> Aabb {
        Aabb {
            lo: [0, 1, 2].map(|k| self.lo[k].max(other.lo[k])),
            hi: [0, 1, 2].map(|k| self.hi[k].min(other.hi[k])),
        }
    }

    /// Segment `pq` passes through the box grown by `margin` on every side.
    fn meets_segment(&self, p: P, q: P, margin: f64) -> bool {
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        for k in 0..3 {
            let (lo, hi) = (self.lo[k] - margin, self.hi[k] + margin);
            let d = q[k] - p[k];
            if d == 0.0 {
                if p[k] < lo || p[k] > hi {
                    return false;
                }
                continue;
            }
            let (a, b) = ((lo - p[k]) / d, (hi - p[k]) / d);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
            if t0 > t1 {
                return false;
            }
        }
        true
    }
}

enum Node {
    Leaf { start: usize, end: usize },
    Inner { left: usize, right: usize },
}

/// Median-split bounding volume hierarchy over boxes (one per triangle).
pub struct Bvh {
    boxes: Vec<Aabb>,
    bounds: Vec<Aabb>,
    nodes: Vec<Node>,
    items: Vec<u32>,
}

const LEAF_SIZE: usize = 4;

impl Bvh {
    pub fn new(boxes: Vec<Aabb>) -> Self {
        let mut bvh = Bvh {
            items: (0..boxes.len() as u32).collect(),
            boxes,
            bounds: Vec::new(),
            nodes: Vec::new(),
        };
        if !bvh.boxes.is_empty() {
            bvh.build(0, bvh.items.len());
        }
        bvh
    }

    fn build(&mut self, start: usize, end: usize) -> usize {
        let mut bounds = self.boxes[self.items[start] as usize];
        for &i in &self.items[start + 1..end] {
            bounds.add(&self.boxes[i as usize]);
        }
        let id = self.nodes.len();
        self.nodes.push(Node::Leaf { start, end });
        self.bounds.push(bounds);
        if end - start <= LEAF_SIZE {
            return id;
        }
        let centre = |b: &Aabb, k: usize| b.lo[k] + b.hi[k];
        let mut spread = Aabb {
            lo: [f64::INFINITY; 3],
            hi: [f64::NEG_INFINITY; 3],
        };
        for &i in &self.items[start..end] {
            let b = &self.boxes[i as usize];
            let c = [centre(b, 0), centre(b, 1), centre(b, 2)];
            spread.add(&Aabb { lo: c, hi: c });
        }
        let extent = sub(spread.hi, spread.lo);
        let axis = (0..3)
            .max_by(|&i, &j| extent[i].total_cmp(&extent[j]))
            .unwrap_or(0);
        let mid = (start + end) / 2;
        let boxes = &self.boxes;
        self.items[start..end].select_nth_unstable_by(mid - start, |&a, &b| {
            centre(&boxes[a as usize], axis).total_cmp(&centre(&boxes[b as usize], axis))
        });
        let left = self.build(start, mid);
        let right = self.build(mid, end);
        self.nodes[id] = Node::Inner { left, right };
        id
    }

    /// Calls `f(i, j)` once for every pair of items whose boxes overlap.
    pub fn self_pairs(&self, mut f: impl FnMut(usize, usize)) {
        if self.nodes.is_empty() {
            return;
        }
        let mut stack = vec![(0usize, 0usize)];
        while let Some((a, b)) = stack.pop() {
            if a != b && !self.bounds[a].overlaps(&self.bounds[b]) {
                continue;
            }
            match (&self.nodes[a], &self.nodes[b]) {
                (&Node::Leaf { start: sa, end: ea }, &Node::Leaf { start: sb, end: eb }) => {
                    for x in sa..ea {
                        let from = if a == b { x + 1 } else { sb };
                        for y in from..eb {
                            let (i, j) = (self.items[x] as usize, self.items[y] as usize);
                            if self.boxes[i].overlaps(&self.boxes[j]) {
                                f(i, j);
                            }
                        }
                    }
                }
                (&Node::Inner { left, right }, _) if a == b => {
                    stack.extend([(left, left), (right, right), (left, right)]);
                }
                (&Node::Inner { left, right }, &Node::Leaf { .. }) => {
                    stack.extend([(left, b), (right, b)]);
                }
                (_, &Node::Inner { left, right }) => {
                    stack.extend([(a, left), (a, right)]);
                }
            }
        }
    }

    /// Calls `f(i)` for every item whose box, grown by `margin`, the segment
    /// `pq` passes through.
    pub fn along_segment(&self, p: P, q: P, margin: f64, mut f: impl FnMut(usize)) {
        if self.nodes.is_empty() {
            return;
        }
        let mut stack = vec![0usize];
        while let Some(n) = stack.pop() {
            if !self.bounds[n].meets_segment(p, q, margin) {
                continue;
            }
            match self.nodes[n] {
                Node::Leaf { start, end } => {
                    for &i in &self.items[start..end] {
                        if self.boxes[i as usize].meets_segment(p, q, margin) {
                            f(i as usize);
                        }
                    }
                }
                Node::Inner { left, right } => stack.extend([left, right]),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: [P; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];

    #[test]
    fn disjoint_touching_and_crossing_triangles() {
        // Far above: disjoint.
        let above = [[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]];
        assert!(!triangles_cross(&T, &above));
        // Piercing through the interior.
        let pierce = [[0.2, 0.2, -1.0], [0.3, 0.2, 1.0], [0.2, 0.3, 1.0]];
        assert!(triangles_cross(&T, &pierce));
        // A vertex exactly on the interior counts: distinct shells may not touch.
        let touch = [[0.25, 0.25, 0.0], [0.3, 0.2, 1.0], [0.2, 0.3, 1.0]];
        assert!(triangles_cross(&T, &touch));
        // Coplanar and overlapping, and coplanar but apart.
        let over = [[0.5, 0.5, 0.0], [-0.5, 0.2, 0.0], [0.2, -0.5, 0.0]];
        assert!(triangles_cross(&T, &over));
        let apart = [[2.0, 2.0, 0.0], [3.0, 2.0, 0.0], [2.0, 3.0, 0.0]];
        assert!(!triangles_cross(&T, &apart));
    }

    #[test]
    fn shared_edges_cross_only_when_folded_flat() {
        // Neighbours across the edge (0,0,0)-(1,0,0): flat, bent, folded back.
        let flat = [[1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.5, -1.0, 0.0]];
        let bent = [[1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.5, 0.5, 1.0]];
        let folded = [[1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.5, 0.25, 0.0]];
        assert!(!triangles_cross(&T, &flat));
        assert!(!triangles_cross(&T, &bent));
        assert!(triangles_cross(&T, &folded));
    }

    #[test]
    fn shared_vertex_crossings() {
        // A fan neighbour bent away from the plane only shares the vertex.
        let away = [[0.0, 0.0, 0.0], [-1.0, 0.0, 1.0], [-1.0, -1.0, 1.0]];
        assert!(!triangles_cross(&T, &away));
        // Its opposite edge passes through the interior of T.
        let through = [[0.0, 0.0, 0.0], [0.5, 0.2, -1.0], [0.2, 0.5, 1.0]];
        assert!(triangles_cross(&T, &through));
        // An edge from the shared vertex lies in T's plane, inside its angle.
        let inside = [[0.0, 0.0, 0.0], [0.3, 0.3, 0.0], [0.3, 0.3, 1.0]];
        assert!(triangles_cross(&T, &inside));
        // The same edge outside T's angle only touches at the vertex.
        let outside = [[0.0, 0.0, 0.0], [-0.3, -0.3, 0.0], [-0.3, -0.3, 1.0]];
        assert!(!triangles_cross(&T, &outside));
    }

    #[test]
    fn piercing_needs_a_transverse_crossing() {
        let pierce = [[0.2, 0.2, -1.0], [0.3, 0.2, 1.0], [0.2, 0.3, 1.0]];
        let through = [[0.0, 0.0, 0.0], [0.5, 0.2, -1.0], [0.2, 0.5, 1.0]];
        assert!(triangles_pierce(&T, &pierce, 1e-9) && triangles_pierce(&T, &through, 1e-9));
        // Touching at a point, in a common plane, or only at a shared corner
        // (the mirror image of `through` straddles too, but away from T).
        let touch = [[0.25, 0.25, 0.0], [0.3, 0.2, 1.0], [0.2, 0.3, 1.0]];
        let over = [[0.5, 0.5, 0.0], [-0.5, 0.2, 0.0], [0.2, -0.5, 0.0]];
        let opposite = [[0.0, 0.0, 0.0], [-0.5, -0.2, -1.0], [-0.2, -0.5, 1.0]];
        // Passing through by less than eps.
        let shallow = [[0.2, 0.2, -1e-10], [0.3, 0.2, 1.0], [0.2, 0.3, 1.0]];
        for t in [touch, over, opposite, shallow] {
            assert!(!triangles_pierce(&T, &t, 1e-9), "{t:?}");
        }
    }

    #[test]
    fn crossing_classifies_through_miss_and_graze() {
        let p = [0.2, 0.2, -1.0];
        assert_eq!(crossing(p, [0.2, 0.2, 1.0], &T), Crossing::Through);
        assert_eq!(crossing(p, [2.0, 2.0, 1.0], &T), Crossing::Miss);
        // Through the edge x = 0.
        assert_eq!(
            crossing([0.0, 0.5, -1.0], [0.0, 0.5, 1.0], &T),
            Crossing::Graze
        );
        // Starting in T's plane outside it is a miss, inside it a graze.
        assert_eq!(
            crossing([2.0, 2.0, 0.0], [2.0, 2.0, 1.0], &T),
            Crossing::Miss
        );
        assert_eq!(
            crossing([0.2, 0.2, 0.0], [0.2, 0.2, 1.0], &T),
            Crossing::Graze
        );
    }

    #[test]
    fn bvh_finds_exactly_the_overlapping_pairs() {
        let boxes: Vec<Aabb> = (0..100)
            .map(|i| {
                let x = i as f64;
                Aabb {
                    lo: [x, 0.0, 0.0],
                    hi: [x + 1.5, 1.0, 1.0],
                }
            })
            .collect();
        let mut pairs = Vec::new();
        Bvh::new(boxes.clone()).self_pairs(|i, j| pairs.push((i.min(j), i.max(j))));
        pairs.sort();
        let mut brute = Vec::new();
        for i in 0..boxes.len() {
            for j in i + 1..boxes.len() {
                if boxes[i].overlaps(&boxes[j]) {
                    brute.push((i, j));
                }
            }
        }
        assert_eq!(pairs, brute);
        let mut hit = Vec::new();
        Bvh::new(boxes).along_segment([10.25, 0.5, 0.5], [10.25, 0.5, 5.0], 0.0, |i| hit.push(i));
        hit.sort();
        assert_eq!(hit, vec![9, 10]);
    }
}
