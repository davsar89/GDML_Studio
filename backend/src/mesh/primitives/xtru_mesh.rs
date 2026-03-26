use crate::mesh::types::TriangleMesh;

/// Generate an extruded solid mesh from a 2D polygon profile and z-sections.
/// Each section specifies (z_position, x_offset, y_offset, scaling_factor).
pub fn tessellate_xtru(
    vertices: &[(f64, f64)],
    sections: &[(f64, f64, f64, f64)], // (z, x_offset, y_offset, scale)
) -> TriangleMesh {
    let mut positions: Vec<f32> = Vec::new();
    let mut normals: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let nv = vertices.len();
    let ns = sections.len();
    if nv < 3 || ns < 2 {
        return TriangleMesh {
            positions,
            normals,
            indices,
        };
    }

    // Determine polygon winding: positive signed area = CCW
    let signed_area = polygon_signed_area(vertices);
    let ccw = signed_area > 0.0;

    // Compute 2D outward edge normals for the polygon
    // For CCW winding, outward normal of edge (i -> i+1) is (dy, -dx) normalized
    let edge_normals: Vec<(f32, f32)> = (0..nv)
        .map(|i| {
            let j = (i + 1) % nv;
            let dx = vertices[j].0 - vertices[i].0;
            let dy = vertices[j].1 - vertices[i].1;
            let (nx, ny) = if ccw { (dy, -dx) } else { (-dy, dx) };
            let len = (nx * nx + ny * ny).sqrt();
            if len > 1e-10 {
                ((nx / len) as f32, (ny / len) as f32)
            } else {
                (0.0f32, 0.0f32)
            }
        })
        .collect();

    // Side walls: for each pair of adjacent sections, connect polygon edges with quads
    for s in 0..ns - 1 {
        let (z1, xo1, yo1, sc1) = sections[s];
        let (z2, xo2, yo2, sc2) = sections[s + 1];
        let z1f = z1 as f32;
        let z2f = z2 as f32;

        for i in 0..nv {
            let j = (i + 1) % nv;
            let (enx, eny) = edge_normals[i];

            // Transform vertices at each section
            let (x1i, y1i) = transform_vertex(vertices[i], xo1, yo1, sc1);
            let (x1j, y1j) = transform_vertex(vertices[j], xo1, yo1, sc1);
            let (x2i, y2i) = transform_vertex(vertices[i], xo2, yo2, sc2);
            let (x2j, y2j) = transform_vertex(vertices[j], xo2, yo2, sc2);

            let base = positions.len() as u32 / 3;

            // Quad: bottom-i, bottom-j, top-j, top-i
            positions.extend_from_slice(&[x1i, y1i, z1f]);
            normals.extend_from_slice(&[enx, eny, 0.0]);
            positions.extend_from_slice(&[x1j, y1j, z1f]);
            normals.extend_from_slice(&[enx, eny, 0.0]);
            positions.extend_from_slice(&[x2j, y2j, z2f]);
            normals.extend_from_slice(&[enx, eny, 0.0]);
            positions.extend_from_slice(&[x2i, y2i, z2f]);
            normals.extend_from_slice(&[enx, eny, 0.0]);

            // CCW polygon edges produce CCW-wound quads; CW needs reversed winding
            if ccw {
                indices.extend_from_slice(&[base, base + 1, base + 2]);
                indices.extend_from_slice(&[base, base + 2, base + 3]);
            } else {
                indices.extend_from_slice(&[base, base + 2, base + 1]);
                indices.extend_from_slice(&[base, base + 3, base + 2]);
            }
        }
    }

    // Triangulate the 2D polygon for caps
    let tri_indices = ear_clip_triangulate(vertices, ccw);

    // Bottom cap (first section, normal pointing -Z)
    {
        let (z, xo, yo, sc) = sections[0];
        let zf = z as f32;
        let cap_base = positions.len() as u32 / 3;

        for &(vx, vy) in vertices {
            let (tx, ty) = transform_vertex((vx, vy), xo, yo, sc);
            positions.extend_from_slice(&[tx, ty, zf]);
            normals.extend_from_slice(&[0.0, 0.0, -1.0]);
        }
        for tri in &tri_indices {
            // Reverse winding for bottom cap
            indices.extend_from_slice(&[
                cap_base + tri[0] as u32,
                cap_base + tri[2] as u32,
                cap_base + tri[1] as u32,
            ]);
        }
    }

    // Top cap (last section, normal pointing +Z)
    {
        let (z, xo, yo, sc) = sections[ns - 1];
        let zf = z as f32;
        let cap_base = positions.len() as u32 / 3;

        for &(vx, vy) in vertices {
            let (tx, ty) = transform_vertex((vx, vy), xo, yo, sc);
            positions.extend_from_slice(&[tx, ty, zf]);
            normals.extend_from_slice(&[0.0, 0.0, 1.0]);
        }
        for tri in &tri_indices {
            indices.extend_from_slice(&[
                cap_base + tri[0] as u32,
                cap_base + tri[1] as u32,
                cap_base + tri[2] as u32,
            ]);
        }
    }

    TriangleMesh {
        positions,
        normals,
        indices,
    }
}

fn transform_vertex(v: (f64, f64), x_offset: f64, y_offset: f64, scale: f64) -> (f32, f32) {
    ((v.0 * scale + x_offset) as f32, (v.1 * scale + y_offset) as f32)
}

fn polygon_signed_area(vertices: &[(f64, f64)]) -> f64 {
    let n = vertices.len();
    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += vertices[i].0 * vertices[j].1;
        area -= vertices[j].0 * vertices[i].1;
    }
    area * 0.5
}

/// Ear-clipping triangulation for a simple polygon.
/// Returns a list of triangles as [i, j, k] index triples into the original vertex array.
/// The `ccw` flag indicates whether vertices are counter-clockwise.
fn ear_clip_triangulate(vertices: &[(f64, f64)], ccw: bool) -> Vec<[usize; 3]> {
    let n = vertices.len();
    if n < 3 {
        return Vec::new();
    }
    if n == 3 {
        return if ccw {
            vec![[0, 1, 2]]
        } else {
            vec![[0, 2, 1]]
        };
    }

    // Work with indices into the original vertex array
    let mut remaining: Vec<usize> = if ccw {
        (0..n).collect()
    } else {
        (0..n).rev().collect()
    };

    let mut triangles = Vec::new();

    let mut safety = remaining.len() * remaining.len();
    while remaining.len() > 3 && safety > 0 {
        let m = remaining.len();
        let mut ear_found = false;

        for i in 0..m {
            let prev = if i == 0 { m - 1 } else { i - 1 };
            let next = (i + 1) % m;

            let a = vertices[remaining[prev]];
            let b = vertices[remaining[i]];
            let c = vertices[remaining[next]];

            // Check if this vertex is convex (cross product > 0 for CCW)
            let cross = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
            if cross <= 0.0 {
                safety -= 1;
                continue;
            }

            // Check no other vertex lies inside triangle abc
            let mut ear = true;
            for j in 0..m {
                if j == prev || j == i || j == next {
                    continue;
                }
                let p = vertices[remaining[j]];
                if point_in_triangle(p, a, b, c) {
                    ear = false;
                    break;
                }
            }

            if ear {
                triangles.push([remaining[prev], remaining[i], remaining[next]]);
                remaining.remove(i);
                ear_found = true;
                break;
            }
            safety -= 1;
        }

        if !ear_found {
            break;
        }
    }

    // Add the last remaining triangle
    if remaining.len() == 3 {
        triangles.push([remaining[0], remaining[1], remaining[2]]);
    }

    triangles
}

fn point_in_triangle(p: (f64, f64), a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> bool {
    let d1 = sign(p, a, b);
    let d2 = sign(p, b, c);
    let d3 = sign(p, c, a);

    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);

    !(has_neg && has_pos)
}

fn sign(p1: (f64, f64), p2: (f64, f64), p3: (f64, f64)) -> f64 {
    (p1.0 - p3.0) * (p2.1 - p3.1) - (p2.0 - p3.0) * (p1.1 - p3.1)
}
