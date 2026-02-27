use super::types::TriangleMesh;
use tracing::debug;

const EPSILON: f64 = 1e-5;

#[derive(Clone)]
struct Plane {
    normal: [f64; 3],
    w: f64,
}

impl Plane {
    fn from_points(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> Option<Self> {
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [
            ab[1] * ac[2] - ab[2] * ac[1],
            ab[2] * ac[0] - ab[0] * ac[2],
            ab[0] * ac[1] - ab[1] * ac[0],
        ];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len < 1e-12 {
            return None;
        }
        let normal = [n[0] / len, n[1] / len, n[2] / len];
        let w = dot(normal, a);
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

    fn build(polygons: Vec<CsgPolygon>) -> Self {
        let mut node = BspNode::new();
        if polygons.is_empty() {
            return node;
        }
        node.add_polygons(polygons);
        node
    }

    fn add_polygons(&mut self, polygons: Vec<CsgPolygon>) {
        if polygons.is_empty() {
            return;
        }

        if self.plane.is_none() {
            self.plane = Some(polygons[0].plane.clone());
        }

        let plane = self.plane.as_ref().unwrap();
        let mut coplanar_front = Vec::new();
        let mut coplanar_back = Vec::new();
        let mut front_list = Vec::new();
        let mut back_list = Vec::new();

        for poly in polygons {
            split_polygon(plane, &poly, &mut coplanar_front, &mut coplanar_back, &mut front_list, &mut back_list);
        }

        self.polygons.extend(coplanar_front);
        self.polygons.extend(coplanar_back);

        if !front_list.is_empty() {
            if self.front.is_none() {
                self.front = Some(Box::new(BspNode::new()));
            }
            self.front.as_mut().unwrap().add_polygons(front_list);
        }

        if !back_list.is_empty() {
            if self.back.is_none() {
                self.back = Some(Box::new(BspNode::new()));
            }
            self.back.as_mut().unwrap().add_polygons(back_list);
        }
    }

    fn all_polygons(&self) -> Vec<CsgPolygon> {
        let mut result = self.polygons.clone();
        if let Some(ref front) = self.front {
            result.extend(front.all_polygons());
        }
        if let Some(ref back) = self.back {
            result.extend(back.all_polygons());
        }
        result
    }

    fn clip_polygons(&self, polygons: &[CsgPolygon]) -> Vec<CsgPolygon> {
        let plane = match &self.plane {
            Some(p) => p,
            None => return polygons.to_vec(),
        };

        let mut front_list = Vec::new();
        let mut back_list = Vec::new();
        let mut coplanar_front = Vec::new();
        let mut coplanar_back = Vec::new();

        for poly in polygons {
            split_polygon(plane, poly, &mut coplanar_front, &mut coplanar_back, &mut front_list, &mut back_list);
        }

        // Coplanar polygons: front goes to front, back goes to back
        front_list.extend(coplanar_front);
        back_list.extend(coplanar_back);

        front_list = if let Some(ref front) = self.front {
            front.clip_polygons(&front_list)
        } else {
            front_list
        };

        back_list = if let Some(ref back) = self.back {
            back.clip_polygons(&back_list)
        } else {
            Vec::new() // discard if no back node
        };

        front_list.extend(back_list);
        front_list
    }

    fn clip_to(&mut self, other: &BspNode) {
        self.polygons = other.clip_polygons(&self.polygons);
        if let Some(ref mut front) = self.front {
            front.clip_to(other);
        }
        if let Some(ref mut back) = self.back {
            back.clip_to(other);
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

fn split_polygon(
    plane: &Plane,
    polygon: &CsgPolygon,
    coplanar_front: &mut Vec<CsgPolygon>,
    coplanar_back: &mut Vec<CsgPolygon>,
    front: &mut Vec<CsgPolygon>,
    back: &mut Vec<CsgPolygon>,
) {
    let mut polygon_type = 0u8;
    let mut types = Vec::with_capacity(polygon.vertices.len());

    for v in &polygon.vertices {
        let t = dot(plane.normal, v.pos) - plane.w;
        let typ = if t < -EPSILON {
            BACK
        } else if t > EPSILON {
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
                    let t = (plane.w - dot(plane.normal, vi.pos))
                        / dot(plane.normal, [
                            vj.pos[0] - vi.pos[0],
                            vj.pos[1] - vi.pos[1],
                            vj.pos[2] - vi.pos[2],
                        ]);
                    let t = t.clamp(0.0, 1.0);
                    let v = vi.interpolate(vj, t);
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

        let vertices = vec![
            Vertex {
                pos: [
                    mesh.positions[i0 * 3] as f64,
                    mesh.positions[i0 * 3 + 1] as f64,
                    mesh.positions[i0 * 3 + 2] as f64,
                ],
                normal: [
                    mesh.normals[i0 * 3] as f64,
                    mesh.normals[i0 * 3 + 1] as f64,
                    mesh.normals[i0 * 3 + 2] as f64,
                ],
            },
            Vertex {
                pos: [
                    mesh.positions[i1 * 3] as f64,
                    mesh.positions[i1 * 3 + 1] as f64,
                    mesh.positions[i1 * 3 + 2] as f64,
                ],
                normal: [
                    mesh.normals[i1 * 3] as f64,
                    mesh.normals[i1 * 3 + 1] as f64,
                    mesh.normals[i1 * 3 + 2] as f64,
                ],
            },
            Vertex {
                pos: [
                    mesh.positions[i2 * 3] as f64,
                    mesh.positions[i2 * 3 + 1] as f64,
                    mesh.positions[i2 * 3 + 2] as f64,
                ],
                normal: [
                    mesh.normals[i2 * 3] as f64,
                    mesh.normals[i2 * 3 + 1] as f64,
                    mesh.normals[i2 * 3 + 2] as f64,
                ],
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

pub fn subtract(a: &TriangleMesh, b: &TriangleMesh) -> TriangleMesh {
    let polys_a = mesh_to_polygons(a);
    let polys_b = mesh_to_polygons(b);

    if polys_a.is_empty() || polys_b.is_empty() {
        return a.clone();
    }

    let mut bsp_a = BspNode::build(polys_a);
    let mut bsp_b = BspNode::build(polys_b);

    debug!("CSG subtract: A has {} polygons, B has {} polygons",
        bsp_a.all_polygons().len(), bsp_b.all_polygons().len());

    // A - B = ~(~A | B)
    bsp_a.invert();
    bsp_a.clip_to(&bsp_b);
    bsp_b.clip_to(&bsp_a);
    bsp_b.invert();
    bsp_b.clip_to(&bsp_a);
    bsp_b.invert();

    debug!("CSG subtract: after clipping, A has {} polygons, B has {} polygons",
        bsp_a.all_polygons().len(), bsp_b.all_polygons().len());

    bsp_a.add_polygons(bsp_b.all_polygons());
    bsp_a.invert();

    let result = bsp_a.all_polygons();
    debug!("CSG subtract: result has {} polygons", result.len());

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

    let mut bsp_a = BspNode::build(polys_a);
    let mut bsp_b = BspNode::build(polys_b);

    bsp_a.clip_to(&bsp_b);
    bsp_b.clip_to(&bsp_a);
    bsp_b.invert();
    bsp_b.clip_to(&bsp_a);
    bsp_b.invert();
    bsp_a.add_polygons(bsp_b.all_polygons());

    polygons_to_mesh(&bsp_a.all_polygons())
}

pub fn intersect(a: &TriangleMesh, b: &TriangleMesh) -> TriangleMesh {
    let polys_a = mesh_to_polygons(a);
    let polys_b = mesh_to_polygons(b);

    if polys_a.is_empty() || polys_b.is_empty() {
        return TriangleMesh::new();
    }

    let mut bsp_a = BspNode::build(polys_a);
    let mut bsp_b = BspNode::build(polys_b);

    bsp_a.invert();
    bsp_b.clip_to(&bsp_a);
    bsp_b.invert();
    bsp_a.clip_to(&bsp_b);
    bsp_b.clip_to(&bsp_a);
    bsp_a.add_polygons(bsp_b.all_polygons());
    bsp_a.invert();

    polygons_to_mesh(&bsp_a.all_polygons())
}

/// Transform a mesh by applying a translation and rotation (Euler angles in radians, XYZ order).
pub fn transform_mesh(mesh: &TriangleMesh, position: [f64; 3], rotation: [f64; 3]) -> TriangleMesh {
    let has_rotation = rotation[0].abs() > 1e-12 || rotation[1].abs() > 1e-12 || rotation[2].abs() > 1e-12;
    let has_translation = position[0].abs() > 1e-12 || position[1].abs() > 1e-12 || position[2].abs() > 1e-12;

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
