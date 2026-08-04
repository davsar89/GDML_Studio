use super::types::TriangleMesh;
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

/// Maximum BSP tree depth before a node stops subdividing and stores the
/// remainder as leaf polygons.
///
/// `add_polygons` recurses once per split. With the splitter chosen by
/// [`pick_splitter`] a closed mesh of N triangles gives a tree of depth around
/// log2(N), so this bound is never reached in practice — it exists because
/// exceeding it used to be fatal rather than merely inaccurate. Picking
/// `polygons[0]` as the splitter (as this did previously) degenerates a convex
/// mesh into a linked list of depth N/2, and a sphere-vs-box subtract then
/// overflowed the 2 MiB tokio worker stack at `segments` as low as 104 —
/// aborting the whole process, since a Rust stack overflow is not catchable.
/// Beyond the cap the tree stops partitioning space, so results degrade in
/// accuracy rather than crashing.
const MAX_BSP_DEPTH: u32 = 192;

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

struct BspNode {
    plane: Option<Plane>,
    front: Option<Box<BspNode>>,
    back: Option<Box<BspNode>>,
    polygons: Vec<CsgPolygon>,
}

impl BspNode {
    fn new() -> Self {
        BspNode {
            plane: None,
            front: None,
            back: None,
            polygons: Vec::new(),
        }
    }

    fn build(polygons: Vec<CsgPolygon>, eps: f64) -> Self {
        let mut node = BspNode::new();
        if polygons.is_empty() {
            return node;
        }
        node.add_polygons(polygons, eps, 0);
        node
    }

    fn add_polygons(&mut self, polygons: Vec<CsgPolygon>, eps: f64, depth: u32) {
        if polygons.is_empty() {
            return;
        }

        // Past the depth bound, keep the remainder as leaf polygons instead of
        // partitioning further. They are still collected and clipped; the tree
        // simply stops dividing space here.
        if depth >= MAX_BSP_DEPTH {
            debug!(
                "CSG: BSP depth cap {} reached, storing {} polygons unsplit",
                MAX_BSP_DEPTH,
                polygons.len()
            );
            self.polygons.extend(polygons);
            return;
        }

        if self.plane.is_none() {
            self.plane = Some(pick_splitter(&polygons, eps));
        }

        let plane = self.plane.as_ref().unwrap();
        let mut coplanar_front = Vec::new();
        let mut coplanar_back = Vec::new();
        let mut front_list = Vec::new();
        let mut back_list = Vec::new();

        for poly in polygons {
            split_polygon(
                plane,
                &poly,
                eps,
                &mut coplanar_front,
                &mut coplanar_back,
                &mut front_list,
                &mut back_list,
            );
        }

        self.polygons.extend(coplanar_front);
        self.polygons.extend(coplanar_back);

        if !front_list.is_empty() {
            if self.front.is_none() {
                self.front = Some(Box::new(BspNode::new()));
            }
            self.front
                .as_mut()
                .unwrap()
                .add_polygons(front_list, eps, depth + 1);
        }

        if !back_list.is_empty() {
            if self.back.is_none() {
                self.back = Some(Box::new(BspNode::new()));
            }
            self.back
                .as_mut()
                .unwrap()
                .add_polygons(back_list, eps, depth + 1);
        }
    }

    /// Collect every polygon in the tree.
    ///
    /// Iterative rather than recursive: this is called on trees built elsewhere
    /// (including operands merged by `add_polygons`), so it should not depend on
    /// the depth bound holding.
    fn all_polygons(&self) -> Vec<CsgPolygon> {
        let mut result = Vec::new();
        let mut stack: Vec<&BspNode> = vec![self];
        while let Some(node) = stack.pop() {
            result.extend(node.polygons.iter().cloned());
            if let Some(ref front) = node.front {
                stack.push(front);
            }
            if let Some(ref back) = node.back {
                stack.push(back);
            }
        }
        result
    }

    fn clip_polygons(&self, polygons: &[CsgPolygon], eps: f64) -> Vec<CsgPolygon> {
        let plane = match &self.plane {
            Some(p) => p,
            None => return polygons.to_vec(),
        };

        let mut front_list = Vec::new();
        let mut back_list = Vec::new();
        let mut coplanar_front = Vec::new();
        let mut coplanar_back = Vec::new();

        for poly in polygons {
            split_polygon(
                plane,
                poly,
                eps,
                &mut coplanar_front,
                &mut coplanar_back,
                &mut front_list,
                &mut back_list,
            );
        }

        // Coplanar polygons: front goes to front, back goes to back
        front_list.extend(coplanar_front);
        back_list.extend(coplanar_back);

        front_list = if let Some(ref front) = self.front {
            front.clip_polygons(&front_list, eps)
        } else {
            front_list
        };

        back_list = if let Some(ref back) = self.back {
            back.clip_polygons(&back_list, eps)
        } else {
            Vec::new() // discard if no back node
        };

        front_list.extend(back_list);
        front_list
    }

    fn clip_to(&mut self, other: &BspNode, eps: f64) {
        self.polygons = other.clip_polygons(&self.polygons, eps);
        if let Some(ref mut front) = self.front {
            front.clip_to(other, eps);
        }
        if let Some(ref mut back) = self.back {
            back.clip_to(other, eps);
        }
    }

    fn invert(&mut self) {
        for poly in &mut self.polygons {
            *poly = poly.flip();
        }
        if let Some(ref p) = self.plane {
            self.plane = Some(p.flip());
        }
        if let Some(ref mut front) = self.front {
            front.invert();
        }
        if let Some(ref mut back) = self.back {
            back.invert();
        }
        std::mem::swap(&mut self.front, &mut self.back);
    }
}

/// Choose a partitioning plane that keeps the tree shallow.
///
/// Taking `polygons[0].plane` unconditionally is what made this structure
/// dangerous: on a convex mesh every other polygon falls on one side of any face
/// plane, so the tree degenerates into a linked list one node deep per polygon.
/// Sampling a handful of candidates and scoring them for balance turns that into
/// roughly log2(N) depth, which fixes both the stack overflow and the quadratic
/// build time that came with it.
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
    polygon: &CsgPolygon,
    eps: f64,
    coplanar_front: &mut Vec<CsgPolygon>,
    coplanar_back: &mut Vec<CsgPolygon>,
    front: &mut Vec<CsgPolygon>,
    back: &mut Vec<CsgPolygon>,
) {
    let mut polygon_type = 0u8;
    let mut types = Vec::with_capacity(polygon.vertices.len());

    for v in &polygon.vertices {
        let t = dot(plane.normal, v.pos) - plane.w;
        if !t.is_finite() {
            return;
        }
        let typ = if t < -eps {
            BACK
        } else if t > eps {
            FRONT
        } else {
            COPLANAR
        };
        polygon_type |= typ;
        types.push((typ, t));
    }

    match polygon_type {
        COPLANAR => {
            if dot(plane.normal, polygon.plane.normal) > 0.0 {
                coplanar_front.push(polygon.clone());
            } else {
                coplanar_back.push(polygon.clone());
            }
        }
        FRONT => {
            front.push(polygon.clone());
        }
        BACK => {
            back.push(polygon.clone());
        }
        _ => {
            // SPANNING
            let mut f = Vec::new();
            let mut b = Vec::new();
            let n = polygon.vertices.len();

            for i in 0..n {
                let j = (i + 1) % n;
                let (ti, _) = types[i];
                let (tj, _) = types[j];
                let vi = &polygon.vertices[i];
                let vj = &polygon.vertices[j];

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

pub fn subtract(a: &TriangleMesh, b: &TriangleMesh) -> TriangleMesh {
    let polys_a = mesh_to_polygons(a);
    let polys_b = mesh_to_polygons(b);

    if polys_a.is_empty() || polys_b.is_empty() {
        return a.clone();
    }

    let eps = epsilon_for(a, b);
    let mut bsp_a = BspNode::build(polys_a, eps);
    let mut bsp_b = BspNode::build(polys_b, eps);

    // A - B = ~(~A | B)
    bsp_a.invert();
    bsp_a.clip_to(&bsp_b, eps);
    bsp_b.clip_to(&bsp_a, eps);
    bsp_b.invert();
    bsp_b.clip_to(&bsp_a, eps);
    bsp_b.invert();

    bsp_a.add_polygons(bsp_b.all_polygons(), eps, 0);
    bsp_a.invert();

    let result = bsp_a.all_polygons();
    debug!(
        "CSG subtract: eps={:.3e}, result has {} polygons",
        eps,
        result.len()
    );

    polygons_to_mesh(&result)
}

pub fn union(a: &TriangleMesh, b: &TriangleMesh) -> TriangleMesh {
    let polys_a = mesh_to_polygons(a);
    let polys_b = mesh_to_polygons(b);

    if polys_a.is_empty() {
        return b.clone();
    }
    if polys_b.is_empty() {
        return a.clone();
    }

    let eps = epsilon_for(a, b);
    let mut bsp_a = BspNode::build(polys_a, eps);
    let mut bsp_b = BspNode::build(polys_b, eps);

    bsp_a.clip_to(&bsp_b, eps);
    bsp_b.clip_to(&bsp_a, eps);
    bsp_b.invert();
    bsp_b.clip_to(&bsp_a, eps);
    bsp_b.invert();
    bsp_a.add_polygons(bsp_b.all_polygons(), eps, 0);

    polygons_to_mesh(&bsp_a.all_polygons())
}

pub fn intersect(a: &TriangleMesh, b: &TriangleMesh) -> TriangleMesh {
    let polys_a = mesh_to_polygons(a);
    let polys_b = mesh_to_polygons(b);

    if polys_a.is_empty() || polys_b.is_empty() {
        return TriangleMesh::new();
    }

    let eps = epsilon_for(a, b);
    let mut bsp_a = BspNode::build(polys_a, eps);
    let mut bsp_b = BspNode::build(polys_b, eps);

    bsp_a.invert();
    bsp_b.clip_to(&bsp_a, eps);
    bsp_b.invert();
    bsp_a.clip_to(&bsp_b, eps);
    bsp_b.clip_to(&bsp_a, eps);
    bsp_a.add_polygons(bsp_b.all_polygons(), eps, 0);
    bsp_a.invert();

    polygons_to_mesh(&bsp_a.all_polygons())
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
    use crate::mesh::primitives::{box_mesh, sphere_mesh};

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
            total += (a[0] * (b[1] * c[2] - b[2] * c[1])
                - a[1] * (b[0] * c[2] - b[2] * c[0])
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
        let a = box_mesh::tessellate_box(10.0, 10.0, 10.0);
        let b = box_mesh::tessellate_box(10.0, 10.0, 10.0);

        let i = intersect(&a, &b);
        let s = subtract(&a, &b);

        assert!(mesh_is_finite(&i));
        assert!(mesh_is_finite(&s));
    }

    #[test]
    fn touching_coplanar_faces_remain_finite() {
        let a = box_mesh::tessellate_box(10.0, 10.0, 10.0);
        let b = transform_mesh(&a, [10.0, 0.0, 0.0], [0.0, 0.0, 0.0]);

        let i = intersect(&a, &b);
        let s = subtract(&a, &b);

        assert!(mesh_is_finite(&i));
        assert!(mesh_is_finite(&s));
    }

    // The tests above only assert finiteness, which a wildly wrong boolean still
    // satisfies. These assert the geometry.

    #[test]
    fn subtract_half_overlap_has_correct_volume() {
        let a = box_mesh::tessellate_box(100.0, 100.0, 100.0);
        let b = transform_mesh(&a, [50.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        assert_volume(&subtract(&a, &b), 500_000.0, 0.01, "box - half-overlap box");
    }

    #[test]
    fn intersect_half_overlap_has_correct_volume() {
        let a = box_mesh::tessellate_box(100.0, 100.0, 100.0);
        let b = transform_mesh(&a, [50.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        assert_volume(&intersect(&a, &b), 500_000.0, 0.01, "box & half-overlap box");
    }

    #[test]
    fn union_of_disjoint_boxes_sums_volumes() {
        // `union` had no assertion anywhere in the suite; it ran only as a
        // crash test via sample_data/solids.gdml.
        let a = box_mesh::tessellate_box(10.0, 10.0, 10.0);
        let b = transform_mesh(&a, [40.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        assert_volume(&union(&a, &b), 2000.0, 0.01, "disjoint union");
    }

    #[test]
    fn subtract_fully_enclosed_cavity() {
        // The inner surface must come out wound inward, so the signed volumes
        // subtract. An inverted cavity would read as 1000 + 125 instead.
        let outer = box_mesh::tessellate_box(10.0, 10.0, 10.0);
        let inner = box_mesh::tessellate_box(5.0, 5.0, 5.0);
        assert_volume(&subtract(&outer, &inner), 875.0, 0.01, "hollow box");
    }

    #[test]
    fn nested_booleans_stay_correct() {
        let a = box_mesh::tessellate_box(100.0, 100.0, 100.0);
        let b = transform_mesh(&a, [50.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        let c = transform_mesh(&a, [0.0, 50.0, 0.0], [0.0, 0.0, 0.0]);
        // (a - b) - c: keeps x in [-50,0] and y in [-50,0] => 50*50*100.
        let step = subtract(&a, &b);
        assert_volume(&subtract(&step, &c), 250_000.0, 0.02, "nested subtract");
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
            let unit = box_mesh::tessellate_box(100.0, 100.0, 100.0);
            let a = transform_mesh(&unit, [offset, offset, offset], rot);
            let shifted = transform_mesh(&unit, [50.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
            let b = transform_mesh(&shifted, [offset, offset, offset], rot);

            let s = subtract(&a, &b);
            assert!(mesh_is_finite(&s), "offset {}: non-finite mesh", offset);
            assert_volume(&s, 500_000.0, 0.02, &format!("coplanar subtract @ {}", offset));
        }
    }

    #[test]
    fn deep_boolean_does_not_exhaust_the_stack() {
        // `add_polygons` recurses once per split. Choosing polygons[0] as the
        // splitter degenerated a convex mesh into a chain of depth N/2, and this
        // exact call aborted the process on a 2 MiB stack at segments >= 104 --
        // a stack overflow, not a catchable panic. Run it on a deliberately
        // small stack so the invariant is pinned regardless of the harness
        // default.
        let handle = std::thread::Builder::new()
            .stack_size(1 << 20) // 1 MiB, half what a tokio worker gets
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
                let cutter = box_mesh::tessellate_box(40.0, 40.0, 400.0);
                let result = subtract(&sphere, &cutter);
                assert!(mesh_is_finite(&result));
                assert!(result.triangle_count() > 0);
            })
            .expect("spawn");

        handle.join().expect("CSG overflowed a 1 MiB stack");
    }
}
