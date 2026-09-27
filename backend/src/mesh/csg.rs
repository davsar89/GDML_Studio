use super::types::TriangleMesh;
use crate::gdml::model::BooleanOp;
use tracing::debug;

/// Coplanarity tolerance as a fraction of the largest coordinate magnitude in
/// the operands.
///
/// This must scale with the geometry, not be a fixed absolute distance.
/// `TriangleMesh` stores positions as `f32`, so a vertex that is exactly on its
/// own face plane in exact arithmetic lands up to ~1e-7 * |coordinate| away from
/// it once quantised — 3.8e-5 mm for a face 1 m from the origin. Against a fixed
/// 1e-5 mm tolerance those faces classify as SPANNING instead of COPLANAR, which
/// shatters them into slivers: a coplanar subtract of two rotated 100 mm cubes
/// (exact answer 500000 mm^3, 16 triangles) gave 622610 mm^3 and 81 triangles at
/// 1 m from the origin, and an inverted, non-manifold mesh at 1000 km. Scaling
/// the tolerance restores all of those to under 0.01%.
///
/// 1e-6 sits about an order of magnitude above the observed f32 drift, which is
/// tight enough to keep genuinely distinct faces apart. Axis-aligned geometry
/// never showed the problem at all — integers below 2^24 are exact in f32 — so
/// it takes a rotated face, i.e. anything meshed from trigonometry.
const EPSILON_SCALE: f64 = 1e-6;

/// Floor for the scaled tolerance, for degenerate or zero-extent input.
const MIN_EPSILON: f64 = 1e-9;

/// How many candidate planes [`pick_splitter`] evaluates per node.
const SPLITTER_CANDIDATES: usize = 8;

/// How many polygons each candidate is scored against.
const SPLITTER_SAMPLE: usize = 64;

#[derive(Clone)]
struct Plane {
    normal: [f64; 3],
    w: f64,
}

impl Plane {
    fn from_points(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> Option<Self> {
        if !is_finite3(a) || !is_finite3(b) || !is_finite3(c) {
            return None;
        }
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [
            ab[1] * ac[2] - ab[2] * ac[1],
            ab[2] * ac[0] - ab[0] * ac[2],
            ab[0] * ac[1] - ab[1] * ac[0],
        ];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if !len.is_finite() || len < 1e-12 {
            return None;
        }
        let normal = [n[0] / len, n[1] / len, n[2] / len];
        let w = dot(normal, a);
        if !w.is_finite() {
            return None;
        }
        Some(Plane { normal, w })
    }

    fn flip(&self) -> Plane {
        Plane {
            normal: [-self.normal[0], -self.normal[1], -self.normal[2]],
            w: -self.w,
        }
    }
}

#[derive(Clone)]
struct Vertex {
    pos: [f64; 3],
    normal: [f64; 3],
}

impl Vertex {
    fn flip(&self) -> Self {
        Vertex {
            pos: self.pos,
            normal: [-self.normal[0], -self.normal[1], -self.normal[2]],
        }
    }

    fn interpolate(&self, other: &Vertex, t: f64) -> Vertex {
        Vertex {
            pos: lerp3(self.pos, other.pos, t),
            normal: lerp3_normalize(self.normal, other.normal, t),
        }
    }
}

#[derive(Clone)]
struct CsgPolygon {
    vertices: Vec<Vertex>,
    plane: Plane,
}

impl CsgPolygon {
    fn from_vertices(vertices: Vec<Vertex>) -> Option<Self> {
        if vertices.len() < 3 {
            return None;
        }
        let plane = Plane::from_points(vertices[0].pos, vertices[1].pos, vertices[2].pos)?;
        Some(CsgPolygon { vertices, plane })
    }

    fn flip(&self) -> Self {
        let mut verts: Vec<Vertex> = self.vertices.iter().map(|v| v.flip()).collect();
        verts.reverse();
        CsgPolygon {
            vertices: verts,
            plane: self.plane.flip(),
        }
    }
}

const COPLANAR: u8 = 0;
const FRONT: u8 = 1;
const BACK: u8 = 2;
const SPANNING: u8 = 3;

/// A BSP node. Its plane is fixed when the node is created.
struct Node {
    plane: Plane,
    front: Option<usize>,
    back: Option<usize>,
}

/// BSP tree of one operand, kept as an arena and walked with explicit stacks.
///
/// A convex operand builds a chain one node per face (see [`pick_splitter`]):
/// up to 960 deep for an orb at the default 32 segments, 16k at 128.
/// Recursion, or the recursive `Drop` of boxed children, overflows a 2 MiB
/// worker stack at that depth -- an uncatchable abort. The depth cap that used
/// to prevent it left plane-less leaves that kept everything below them, so
/// box & orb came out larger than the orb.
///
/// The tree holds planes only: it classifies space, and the polygons being
/// clipped are passed in separately.
///
/// Ponytail: the chain makes a build O(N^2) and a clip O(N) per polygon for an
/// N-face convex operand (N counted inside the overlap box). A box & orb takes
/// 3 ms at 32 segments, 80 ms at 128 and 1.5 s at 256 (release). No face plane
/// of a convex mesh splits the others, so a better splitter cannot help; the
/// upgrade path is a robust mesh-boolean library.
struct Bsp {
    nodes: Vec<Node>,
}

impl Bsp {
    fn build(polygons: Vec<CsgPolygon>, eps: f64) -> Bsp {
        let mut nodes: Vec<Node> = Vec::new();
        // (polygons for a new node, (parent, is its front child))
        let mut stack = vec![(polygons, None::<(usize, bool)>)];
        while let Some((polygons, parent)) = stack.pop() {
            if polygons.is_empty() {
                continue;
            }
            let plane = pick_splitter(&polygons, eps);
            let (mut on_front, mut on_back) = (Vec::new(), Vec::new());
            let (mut front, mut back) = (Vec::new(), Vec::new());
            for poly in polygons {
                split_polygon(
                    &plane,
                    poly,
                    eps,
                    &mut on_front,
                    &mut on_back,
                    &mut front,
                    &mut back,
                );
            }
            // Polygons on the plane need nothing more: the node stands for them.
            let id = nodes.len();
            nodes.push(Node {
                plane,
                front: None,
                back: None,
            });
            match parent {
                Some((p, true)) => nodes[p].front = Some(id),
                Some((p, false)) => nodes[p].back = Some(id),
                None => {}
            }
            stack.push((front, Some((id, true))));
            stack.push((back, Some((id, false))));
        }
        Bsp { nodes }
    }

    /// The parts of `polygons` outside this tree's solid. A polygon lying on
    /// one of its faces counts as outside if it faces the same way.
    fn clip(&self, polygons: Vec<CsgPolygon>, eps: f64) -> Vec<CsgPolygon> {
        if self.nodes.is_empty() {
            return polygons;
        }
        let mut kept = Vec::new();
        let mut stack = vec![(0, polygons)];
        while let Some((id, polygons)) = stack.pop() {
            if polygons.is_empty() {
                continue;
            }
            let node = &self.nodes[id];
            let (mut on_front, mut on_back) = (Vec::new(), Vec::new());
            let (mut front, mut back) = (Vec::new(), Vec::new());
            for poly in polygons {
                split_polygon(
                    &node.plane,
                    poly,
                    eps,
                    &mut on_front,
                    &mut on_back,
                    &mut front,
                    &mut back,
                );
            }
            front.append(&mut on_front);
            back.append(&mut on_back);
            match node.front {
                Some(f) => stack.push((f, front)),
                None => kept.append(&mut front),
            }
            // Behind a leaf is inside the solid: dropped.
            if let Some(b) = node.back {
                stack.push((b, back));
            }
        }
        kept
    }

    /// Swap inside and outside.
    fn invert(&mut self) {
        for node in &mut self.nodes {
            node.plane = node.plane.flip();
            std::mem::swap(&mut node.front, &mut node.back);
        }
    }
}

/// Choose a partitioning plane that keeps the tree shallow.
///
/// Sampling a handful of candidates and scoring them for balance keeps a
/// non-convex operand near log2(N) deep. It cannot help a convex one: every
/// other face lies behind any face plane, all candidates score alike, and the
/// tree is a chain one node per face -- which is why [`Bsp`] never recurses.
///
/// Candidates are drawn by striding through the list rather than at random, so
/// the result stays deterministic — CSG output must not vary between runs.
fn pick_splitter(polygons: &[CsgPolygon], eps: f64) -> Plane {
    if polygons.len() <= 2 {
        return polygons[0].plane.clone();
    }

    let candidate_stride = (polygons.len() / SPLITTER_CANDIDATES).max(1);
    let sample_stride = (polygons.len() / SPLITTER_SAMPLE).max(1);

    let mut best_index = 0;
    let mut best_score = f64::MAX;

    for candidate in polygons.iter().step_by(candidate_stride).enumerate() {
        let (nth, poly) = candidate;
        let plane = &poly.plane;
        let (mut front, mut back, mut spanning) = (0i64, 0i64, 0i64);

        for other in polygons.iter().step_by(sample_stride) {
            let mut has_front = false;
            let mut has_back = false;
            for v in &other.vertices {
                let t = dot(plane.normal, v.pos) - plane.w;
                if t > eps {
                    has_front = true;
                } else if t < -eps {
                    has_back = true;
                }
            }
            match (has_front, has_back) {
                (true, true) => spanning += 1,
                (true, false) => front += 1,
                (false, true) => back += 1,
                (false, false) => {}
            }
        }

        // Imbalance dominates; splits are weighted because each one adds a
        // polygon to the working set.
        let score = (front - back).abs() as f64 + spanning as f64 * 1.5;
        if score < best_score {
            best_score = score;
            best_index = nth * candidate_stride;
        }
    }

    polygons[best_index].plane.clone()
}

fn split_polygon(
    plane: &Plane,
    polygon: CsgPolygon,
    eps: f64,
    coplanar_front: &mut Vec<CsgPolygon>,
    coplanar_back: &mut Vec<CsgPolygon>,
    front: &mut Vec<CsgPolygon>,
    back: &mut Vec<CsgPolygon>,
) {
    // Recomputed rather than stored: this runs O(N^2) times on a BSP chain,
    // and an allocation per call dominated the build.
    let dist = |v: &Vertex| dot(plane.normal, v.pos) - plane.w;
    let side = |t: f64| {
        if t < -eps {
            BACK
        } else if t > eps {
            FRONT
        } else {
            COPLANAR
        }
    };
    let mut polygon_type = 0u8;
    for v in &polygon.vertices {
        let t = dist(v);
        if !t.is_finite() {
            return;
        }
        polygon_type |= side(t);
    }

    match polygon_type {
        COPLANAR => {
            if dot(plane.normal, polygon.plane.normal) > 0.0 {
                coplanar_front.push(polygon);
            } else {
                coplanar_back.push(polygon);
            }
        }
        FRONT => front.push(polygon),
        BACK => back.push(polygon),
        _ => {
            // SPANNING
            let mut f = Vec::new();
            let mut b = Vec::new();
            let n = polygon.vertices.len();

            for i in 0..n {
                let j = (i + 1) % n;
                let vi = &polygon.vertices[i];
                let vj = &polygon.vertices[j];
                let (ti, tj) = (side(dist(vi)), side(dist(vj)));

                if ti != BACK {
                    f.push(vi.clone());
                }
                if ti != FRONT {
                    b.push(vi.clone());
                }
                if (ti | tj) == SPANNING {
                    let denom = dot(
                        plane.normal,
                        [
                            vj.pos[0] - vi.pos[0],
                            vj.pos[1] - vi.pos[1],
                            vj.pos[2] - vi.pos[2],
                        ],
                    );
                    if !denom.is_finite() || denom.abs() <= eps {
                        continue;
                    }
                    let t = (plane.w - dot(plane.normal, vi.pos)) / denom;
                    if !t.is_finite() {
                        continue;
                    }
                    let t = t.clamp(0.0, 1.0);
                    let v = vi.interpolate(vj, t);
                    if !is_finite3(v.pos) || !is_finite3(v.normal) {
                        continue;
                    }
                    f.push(v.clone());
                    b.push(v);
                }
            }

            if f.len() >= 3 {
                if let Some(p) = CsgPolygon::from_vertices(f) {
                    front.push(p);
                }
            }
            if b.len() >= 3 {
                if let Some(p) = CsgPolygon::from_vertices(b) {
                    back.push(p);
                }
            }
        }
    }
}

// ─── Public API ──────────────────────────────────────────────────────────────

fn mesh_to_polygons(mesh: &TriangleMesh) -> Vec<CsgPolygon> {
    let mut polygons = Vec::with_capacity(mesh.triangle_count());

    for tri in 0..mesh.triangle_count() {
        let i0 = mesh.indices[tri * 3] as usize;
        let i1 = mesh.indices[tri * 3 + 1] as usize;
        let i2 = mesh.indices[tri * 3 + 2] as usize;

        let p0 = [
            mesh.positions[i0 * 3] as f64,
            mesh.positions[i0 * 3 + 1] as f64,
            mesh.positions[i0 * 3 + 2] as f64,
        ];
        let p1 = [
            mesh.positions[i1 * 3] as f64,
            mesh.positions[i1 * 3 + 1] as f64,
            mesh.positions[i1 * 3 + 2] as f64,
        ];
        let p2 = [
            mesh.positions[i2 * 3] as f64,
            mesh.positions[i2 * 3 + 1] as f64,
            mesh.positions[i2 * 3 + 2] as f64,
        ];
        let n0 = [
            mesh.normals[i0 * 3] as f64,
            mesh.normals[i0 * 3 + 1] as f64,
            mesh.normals[i0 * 3 + 2] as f64,
        ];
        let n1 = [
            mesh.normals[i1 * 3] as f64,
            mesh.normals[i1 * 3 + 1] as f64,
            mesh.normals[i1 * 3 + 2] as f64,
        ];
        let n2 = [
            mesh.normals[i2 * 3] as f64,
            mesh.normals[i2 * 3 + 1] as f64,
            mesh.normals[i2 * 3 + 2] as f64,
        ];
        if !is_finite3(p0)
            || !is_finite3(p1)
            || !is_finite3(p2)
            || !is_finite3(n0)
            || !is_finite3(n1)
            || !is_finite3(n2)
        {
            continue;
        }

        let vertices = vec![
            Vertex {
                pos: p0,
                normal: n0,
            },
            Vertex {
                pos: p1,
                normal: n1,
            },
            Vertex {
                pos: p2,
                normal: n2,
            },
        ];

        if let Some(poly) = CsgPolygon::from_vertices(vertices) {
            polygons.push(poly);
        }
    }

    polygons
}

fn polygons_to_mesh(polygons: &[CsgPolygon]) -> TriangleMesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    for poly in polygons {
        if poly
            .vertices
            .iter()
            .any(|v| !is_finite3(v.pos) || !is_finite3(v.normal))
        {
            continue;
        }
        let base = (positions.len() / 3) as u32;
        for v in &poly.vertices {
            positions.push(v.pos[0] as f32);
            positions.push(v.pos[1] as f32);
            positions.push(v.pos[2] as f32);
            normals.push(v.normal[0] as f32);
            normals.push(v.normal[1] as f32);
            normals.push(v.normal[2] as f32);
        }
        // Fan triangulation for polygons with > 3 vertices
        for i in 2..poly.vertices.len() as u32 {
            indices.push(base);
            indices.push(base + i - 1);
            indices.push(base + i);
        }
    }

    TriangleMesh {
        positions,
        normals,
        indices,
    }
}

/// Largest absolute coordinate in a mesh, used to scale the coplanarity
/// tolerance to the geometry.
fn mesh_extent(mesh: &TriangleMesh) -> f64 {
    mesh.positions.iter().fold(0.0f64, |acc, &v| {
        let a = (v as f64).abs();
        if a.is_finite() && a > acc {
            a
        } else {
            acc
        }
    })
}

/// Coplanarity tolerance for a boolean between two meshes.
fn epsilon_for(a: &TriangleMesh, b: &TriangleMesh) -> f64 {
    (mesh_extent(a).max(mesh_extent(b)) * EPSILON_SCALE).max(MIN_EPSILON)
}

/// Axis-aligned bounds of `polygons`; inverted (lo > hi) when there are none.
fn bounds(polygons: &[CsgPolygon]) -> ([f64; 3], [f64; 3]) {
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for v in polygons.iter().flat_map(|p| &p.vertices) {
        for k in 0..3 {
            lo[k] = lo[k].min(v.pos[k]);
            hi[k] = hi[k].max(v.pos[k]);
        }
    }
    (lo, hi)
}

/// Split `polygons` by the faces of the box `lo..hi` into (inside or on the
/// box, outside it).
fn split_by_box(
    polygons: Vec<CsgPolygon>,
    lo: [f64; 3],
    hi: [f64; 3],
    eps: f64,
) -> (Vec<CsgPolygon>, Vec<CsgPolygon>) {
    let (mut inside, mut outside) = (polygons, Vec::new());
    for k in 0..3 {
        for (sign, w) in [(1.0, hi[k]), (-1.0, -lo[k])] {
            let mut normal = [0.0; 3];
            normal[k] = sign;
            let plane = Plane { normal, w };
            let (mut on_front, mut on_back, mut back) = (Vec::new(), Vec::new(), Vec::new());
            for poly in inside {
                split_polygon(
                    &plane,
                    poly,
                    eps,
                    &mut on_front,
                    &mut on_back,
                    &mut outside,
                    &mut back,
                );
            }
            back.append(&mut on_front);
            back.append(&mut on_back);
            inside = back;
        }
    }
    (inside, outside)
}

fn flip_all(polygons: Vec<CsgPolygon>) -> Vec<CsgPolygon> {
    polygons.iter().map(CsgPolygon::flip).collect()
}

/// `a op b` for closed, outward-wound meshes, by csg.js's BSP algorithm:
/// a | b keeps a outside b and b outside a, a - b = ~(~a | b) and
/// a & b = ~(~a | ~b).
pub fn boolean(a: &TriangleMesh, b: &TriangleMesh, op: &BooleanOp) -> TriangleMesh {
    let (pa, pb) = (mesh_to_polygons(a), mesh_to_polygons(b));
    let eps = epsilon_for(a, b);
    let ((a_lo, a_hi), (b_lo, b_hi)) = (bounds(&pa), bounds(&pb));
    let lo: [f64; 3] = std::array::from_fn(|k| a_lo[k].max(b_lo[k]));
    let hi: [f64; 3] = std::array::from_fn(|k| a_hi[k].min(b_hi[k]));
    // Disjoint or empty operands: nothing to clip.
    if (0..3).any(|k| lo[k] > hi[k] + eps) {
        return match op {
            BooleanOp::Subtraction => a.clone(),
            BooleanOp::Union => polygons_to_mesh(&[pa, pb].concat()),
            BooleanOp::Intersection => TriangleMesh::default(),
        };
    }

    // Only the parts of each operand inside the overlap box can meet the other
    // solid; the rest is outside it and skips the trees. Clipping whole faces
    // let every plane of b shred faces of a far from b, and chained booleans
    // multiplied those slivers.
    let (a_in, a_out) = split_by_box(pa, lo, hi, eps);
    let (b_in, b_out) = split_by_box(pb, lo, hi, eps);

    // Within the box, the part of a closed surface inside it classifies space
    // as the whole surface does: every BSP cell there borders one of its faces.
    // A surface that misses the box leaves one cell with no face to label it,
    // so then the whole surface is used.
    let tree = |inside: &[CsgPolygon], outside: &[CsgPolygon]| {
        Bsp::build(
            if inside.is_empty() { outside } else { inside }.to_vec(),
            eps,
        )
    };
    let (union, intersection) = (
        matches!(op, BooleanOp::Union),
        matches!(op, BooleanOp::Intersection),
    );
    let mut tree_a = tree(&a_in, &a_out);
    let mut tree_b = tree(&b_in, &b_out);
    if !union {
        tree_a.invert();
    }
    if intersection {
        tree_b.invert();
    }

    let mut result = if union {
        tree_b.clip(a_in, eps)
    } else {
        flip_all(tree_b.clip(flip_all(a_in), eps))
    };
    // The second pass drops faces of b lying on a face of a, which a keeps.
    let b_kept = tree_a.clip(flip_all(tree_a.clip(b_in, eps)), eps);
    if matches!(op, BooleanOp::Subtraction) {
        result.extend(b_kept);
    } else {
        result.extend(flip_all(b_kept));
    }
    if !intersection {
        result.extend(a_out);
    }
    if union {
        result.extend(b_out);
    }
    debug!("CSG {:?}: eps={:.3e}, {} polygons", op, eps, result.len());
    polygons_to_mesh(&result)
}

/// Transform a mesh by applying a translation and rotation (Euler angles in radians, XYZ order).
pub fn transform_mesh(mesh: &TriangleMesh, position: [f64; 3], rotation: [f64; 3]) -> TriangleMesh {
    let has_rotation =
        rotation[0].abs() > 1e-12 || rotation[1].abs() > 1e-12 || rotation[2].abs() > 1e-12;
    let has_translation =
        position[0].abs() > 1e-12 || position[1].abs() > 1e-12 || position[2].abs() > 1e-12;

    if !has_rotation && !has_translation {
        return mesh.clone();
    }

    // Build rotation matrix from Euler angles (ZYX convention, matching Geant4)
    let (sx, cx) = rotation[0].sin_cos();
    let (sy, cy) = rotation[1].sin_cos();
    let (sz, cz) = rotation[2].sin_cos();

    // Rotation matrix R = Rz * Ry * Rx
    let r = [
        [cy * cz, sx * sy * cz - cx * sz, cx * sy * cz + sx * sz],
        [cy * sz, sx * sy * sz + cx * cz, cx * sy * sz - sx * cz],
        [-sy, sx * cy, cx * cy],
    ];

    let mut positions = mesh.positions.clone();
    let mut normals = mesh.normals.clone();

    let n_verts = positions.len() / 3;
    for i in 0..n_verts {
        let px = positions[i * 3] as f64;
        let py = positions[i * 3 + 1] as f64;
        let pz = positions[i * 3 + 2] as f64;

        if has_rotation {
            let rx = r[0][0] * px + r[0][1] * py + r[0][2] * pz;
            let ry = r[1][0] * px + r[1][1] * py + r[1][2] * pz;
            let rz = r[2][0] * px + r[2][1] * py + r[2][2] * pz;
            positions[i * 3] = (rx + position[0]) as f32;
            positions[i * 3 + 1] = (ry + position[1]) as f32;
            positions[i * 3 + 2] = (rz + position[2]) as f32;

            let nx = normals[i * 3] as f64;
            let ny = normals[i * 3 + 1] as f64;
            let nz = normals[i * 3 + 2] as f64;
            normals[i * 3] = (r[0][0] * nx + r[0][1] * ny + r[0][2] * nz) as f32;
            normals[i * 3 + 1] = (r[1][0] * nx + r[1][1] * ny + r[1][2] * nz) as f32;
            normals[i * 3 + 2] = (r[2][0] * nx + r[2][1] * ny + r[2][2] * nz) as f32;
        } else {
            positions[i * 3] = (px + position[0]) as f32;
            positions[i * 3 + 1] = (py + position[1]) as f32;
            positions[i * 3 + 2] = (pz + position[2]) as f32;
        }
    }

    TriangleMesh {
        positions,
        normals,
        indices: mesh.indices.clone(),
    }
}

// ─── Math helpers ────────────────────────────────────────────────────────────

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn is_finite3(v: [f64; 3]) -> bool {
    v[0].is_finite() && v[1].is_finite() && v[2].is_finite()
}

fn lerp3(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn lerp3_normalize(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    let v = lerp3(a, b, t);
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-12 {
        return v;
    }
    [v[0] / len, v[1] / len, v[2] / len]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::primitives::{arb8_mesh::tessellate_box, sphere_mesh};

    fn mesh_is_finite(mesh: &TriangleMesh) -> bool {
        mesh.positions.iter().all(|v| v.is_finite()) && mesh.normals.iter().all(|v| v.is_finite())
    }

    /// Signed volume via the divergence theorem. Correct for any closed,
    /// consistently wound mesh; a negative result means inverted winding.
    ///
    /// Tetrahedra are summed about the bounding-box centre rather than the
    /// world origin. The result is mathematically independent of that choice,
    /// but numerically it is not: for a 100 mm box sitting 100 km away the
    /// per-triangle terms are ~1e15 while the total is ~5e5, and the ten orders
    /// of cancellation swamp the answer. Centring first keeps the terms the size
    /// of the object.
    fn signed_volume(mesh: &TriangleMesh) -> f64 {
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for v in mesh.positions.chunks_exact(3) {
            for k in 0..3 {
                lo[k] = lo[k].min(v[k] as f64);
                hi[k] = hi[k].max(v[k] as f64);
            }
        }
        let c = [
            (lo[0] + hi[0]) * 0.5,
            (lo[1] + hi[1]) * 0.5,
            (lo[2] + hi[2]) * 0.5,
        ];

        let p = |i: usize| -> [f64; 3] {
            [
                mesh.positions[i * 3] as f64 - c[0],
                mesh.positions[i * 3 + 1] as f64 - c[1],
                mesh.positions[i * 3 + 2] as f64 - c[2],
            ]
        };
        let mut total = 0.0;
        for tri in 0..mesh.triangle_count() {
            let a = p(mesh.indices[tri * 3] as usize);
            let b = p(mesh.indices[tri * 3 + 1] as usize);
            let c = p(mesh.indices[tri * 3 + 2] as usize);
            total += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0;
        }
        total
    }

    fn assert_volume(mesh: &TriangleMesh, expected: f64, rel_tol: f64, what: &str) {
        let got = signed_volume(mesh);
        let err = (got - expected).abs() / expected.abs().max(1e-12);
        assert!(
            err <= rel_tol,
            "{}: volume {:.4}, expected {:.4} (rel err {:.4} > {:.4})",
            what,
            got,
            expected,
            err,
            rel_tol
        );
    }

    #[test]
    fn coplanar_identical_boxes_remain_finite() {
        let a = tessellate_box(10.0, 10.0, 10.0);
        let b = tessellate_box(10.0, 10.0, 10.0);

        let i = boolean(&a, &b, &BooleanOp::Intersection);
        let s = boolean(&a, &b, &BooleanOp::Subtraction);

        assert!(mesh_is_finite(&i));
        assert!(mesh_is_finite(&s));
    }

    #[test]
    fn touching_coplanar_faces_remain_finite() {
        let a = tessellate_box(10.0, 10.0, 10.0);
        let b = transform_mesh(&a, [10.0, 0.0, 0.0], [0.0, 0.0, 0.0]);

        let i = boolean(&a, &b, &BooleanOp::Intersection);
        let s = boolean(&a, &b, &BooleanOp::Subtraction);

        assert!(mesh_is_finite(&i));
        assert!(mesh_is_finite(&s));
    }

    // The tests above only assert finiteness, which a wildly wrong boolean still
    // satisfies. These assert the geometry.

    #[test]
    fn subtract_half_overlap_has_correct_volume() {
        let a = tessellate_box(100.0, 100.0, 100.0);
        let b = transform_mesh(&a, [50.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        assert_volume(
            &boolean(&a, &b, &BooleanOp::Subtraction),
            500_000.0,
            0.01,
            "box - half-overlap box",
        );
    }

    #[test]
    fn intersect_half_overlap_has_correct_volume() {
        let a = tessellate_box(100.0, 100.0, 100.0);
        let b = transform_mesh(&a, [50.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        assert_volume(
            &boolean(&a, &b, &BooleanOp::Intersection),
            500_000.0,
            0.01,
            "box & half-overlap box",
        );
    }

    #[test]
    fn union_of_disjoint_boxes_sums_volumes() {
        // `union` had no assertion anywhere in the suite; it ran only as a
        // crash test via sample_data/solids.gdml.
        let a = tessellate_box(10.0, 10.0, 10.0);
        let b = transform_mesh(&a, [40.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        assert_volume(
            &boolean(&a, &b, &BooleanOp::Union),
            2000.0,
            0.01,
            "disjoint union",
        );
    }

    #[test]
    fn booleans_satisfy_inclusion_exclusion() {
        // |a| + |b| = |a | b| + |a & b| and |a| = |a - b| + |a & b| for any
        // pair, so these pin all three ops on curved, rotated operands that
        // have no closed-form overlap volume.
        let pi = std::f64::consts::PI;
        let a = sphere_mesh::tessellate_sphere(0.0, 40.0, 0.0, 2.0 * pi, 0.0, pi, 24);
        let b = transform_mesh(
            &tessellate_box(50.0, 60.0, 70.0),
            [30.0, 10.0, -5.0],
            [0.3, 0.4, 0.5],
        );
        let v = |op| signed_volume(&boolean(&a, &b, &op));
        let (or, and, minus) = (
            v(BooleanOp::Union),
            v(BooleanOp::Intersection),
            v(BooleanOp::Subtraction),
        );
        let (va, vb) = (signed_volume(&a), signed_volume(&b));
        assert!(and > 0.1 * vb, "operands must overlap, got {and}");
        assert!(
            (or + and - va - vb).abs() < 1e-6 * (va + vb),
            "{or} + {and}"
        );
        assert!(
            (minus + and - va).abs() < 1e-6 * (va + vb),
            "{minus} + {and}"
        );
    }

    #[test]
    fn subtract_fully_enclosed_cavity() {
        // The inner surface must come out wound inward, so the signed volumes
        // subtract. An inverted cavity would read as 1000 + 125 instead.
        let outer = tessellate_box(10.0, 10.0, 10.0);
        let inner = tessellate_box(5.0, 5.0, 5.0);
        assert_volume(
            &boolean(&outer, &inner, &BooleanOp::Subtraction),
            875.0,
            0.01,
            "hollow box",
        );
    }

    #[test]
    fn nested_booleans_stay_correct() {
        let a = tessellate_box(100.0, 100.0, 100.0);
        let b = transform_mesh(&a, [50.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        let c = transform_mesh(&a, [0.0, 50.0, 0.0], [0.0, 0.0, 0.0]);
        // (a - b) - c: keeps x in [-50,0] and y in [-50,0] => 50*50*100.
        let step = boolean(&a, &b, &BooleanOp::Subtraction);
        assert_volume(
            &boolean(&step, &c, &BooleanOp::Subtraction),
            250_000.0,
            0.02,
            "nested subtract",
        );
    }

    #[test]
    fn coplanar_subtract_stays_exact_far_from_origin() {
        // The regression the scaled epsilon exists for. With a fixed 1e-5 mm
        // tolerance this returned +24.5% at 1 m and an inverted, non-manifold
        // mesh at 1000 km. The rotation matters: axis-aligned boxes are exact in
        // f32 at any offset, so an axis-aligned version of this test passes even
        // on the broken code.
        for offset in [0.0, 1_000.0, 100_000.0, 1_000_000.0] {
            let rot = [0.3, 0.4, 0.5];
            let unit = tessellate_box(100.0, 100.0, 100.0);
            let a = transform_mesh(&unit, [offset, offset, offset], rot);
            let shifted = transform_mesh(&unit, [50.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
            let b = transform_mesh(&shifted, [offset, offset, offset], rot);

            let s = boolean(&a, &b, &BooleanOp::Subtraction);
            assert!(mesh_is_finite(&s), "offset {}: non-finite mesh", offset);
            assert_volume(
                &s,
                500_000.0,
                0.02,
                &format!("coplanar subtract @ {}", offset),
            );
        }
    }

    #[test]
    fn deep_boolean_does_not_exhaust_the_stack() {
        // The part of the sphere inside the cutter's box builds a BSP chain
        // ~2.4k nodes deep. A recursive build aborted the process on a 2 MiB
        // stack at segments >= 104 -- a stack overflow, not a catchable panic.
        // 256 KiB leaves ~100 bytes per level, less than any recursive frame.
        let handle = std::thread::Builder::new()
            .stack_size(256 << 10)
            .spawn(|| {
                let sphere = sphere_mesh::tessellate_sphere(
                    0.0,
                    50.0,
                    0.0,
                    2.0 * std::f64::consts::PI,
                    0.0,
                    std::f64::consts::PI,
                    128,
                );
                let cutter = tessellate_box(40.0, 40.0, 400.0);
                let result = boolean(&sphere, &cutter, &BooleanOp::Subtraction);
                assert!(mesh_is_finite(&result));
                assert!(result.triangle_count() > 0);
            })
            .expect("spawn");

        handle.join().expect("CSG overflowed a 256 KiB stack");
    }
}
