use crate::mesh::types::TriangleMesh;
use std::f64::consts::PI;

/// Tessellate a cut tube (G4CutTubs) - a tube with angled end caps.
///
/// Same as a regular tube but the top/bottom faces are cut by planes
/// defined by normal vectors.
///
/// - `rmin`, `rmax`: inner/outer radii
/// - `z`: full height (halved internally)
/// - `startphi`, `deltaphi`: angular extent
/// - `low_norm`: [nx, ny, nz] outward normal of bottom cut plane
/// - `high_norm`: [nx, ny, nz] outward normal of top cut plane
/// - `segments`: number of phi subdivisions
pub fn tessellate_cut_tube(
    rmin: f64,
    rmax: f64,
    z: f64,
    startphi: f64,
    deltaphi: f64,
    low_norm: [f64; 3],
    high_norm: [f64; 3],
    segments: u32,
) -> TriangleMesh {
    let mut positions: Vec<f32> = Vec::new();
    let mut normals: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let hz = z / 2.0;
    let full_circle = (deltaphi - 2.0 * PI).abs() < 1e-6;

    // Compute z-offset for a point (x, y) on a cut plane.
    // Plane passes through (0, 0, z0) with outward normal n.
    // n.x*(x-0) + n.y*(y-0) + n.z*(z-z0) = 0
    // z = z0 - (n.x*x + n.y*y) / n.z
    let cut_z = |x: f64, y: f64, z0: f64, norm: [f64; 3]| -> f64 {
        if norm[2].abs() > 1e-12 {
            z0 - (norm[0] * x + norm[1] * y) / norm[2]
        } else {
            z0
        }
    };

    // Generate a cylindrical surface at radius r
    let gen_surface = |r: f64, outward: bool, positions: &mut Vec<f32>, normals: &mut Vec<f32>, indices: &mut Vec<u32>| {
        if r < 1e-12 { return; }
        let base = (positions.len() / 3) as u32;
        let segs = segments;

        // Generate vertices: 2 rows (bottom, top) x (segs+1) columns
        for i in 0..=segs {
            let phi = startphi + deltaphi * (i as f64) / (segs as f64);
            let cos_phi = phi.cos();
            let sin_phi = phi.sin();
            let x = r * cos_phi;
            let y = r * sin_phi;

            // Bottom vertex
            let zb = cut_z(x, y, -hz, low_norm);
            positions.push(x as f32);
            positions.push(y as f32);
            positions.push(zb as f32);

            // Top vertex
            let zt = cut_z(x, y, hz, high_norm);
            positions.push(x as f32);
            positions.push(y as f32);
            positions.push(zt as f32);

            let (nx, ny) = if outward { (cos_phi, sin_phi) } else { (-cos_phi, -sin_phi) };
            for _ in 0..2 {
                normals.push(nx as f32);
                normals.push(ny as f32);
                normals.push(0.0);
            }
        }

        // Triangles
        for i in 0..segs {
            let b0 = base + i * 2;      // bottom-left
            let t0 = base + i * 2 + 1;  // top-left
            let b1 = base + (i + 1) * 2;     // bottom-right
            let t1 = base + (i + 1) * 2 + 1; // top-right

            if outward {
                indices.push(b0); indices.push(b1); indices.push(t1);
                indices.push(b0); indices.push(t1); indices.push(t0);
            } else {
                indices.push(b0); indices.push(t1); indices.push(b1);
                indices.push(b0); indices.push(t0); indices.push(t1);
            }
        }
    };

    // Outer surface
    gen_surface(rmax, true, &mut positions, &mut normals, &mut indices);

    // Inner surface
    if rmin > 1e-12 {
        gen_surface(rmin, false, &mut positions, &mut normals, &mut indices);
    }

    // End caps (annular rings on the cut planes)
    let gen_cap = |is_top: bool, positions: &mut Vec<f32>, normals: &mut Vec<f32>, indices: &mut Vec<u32>| {
        let z0 = if is_top { hz } else { -hz };
        let norm = if is_top { high_norm } else { low_norm };
        let r_inner = if rmin > 1e-12 { rmin } else { 0.0 };

        // Normalize the plane normal for face normal
        let len = (norm[0] * norm[0] + norm[1] * norm[1] + norm[2] * norm[2]).sqrt();
        let (fnx, fny, fnz) = if len > 1e-12 {
            (norm[0] / len, norm[1] / len, norm[2] / len)
        } else {
            (0.0, 0.0, if is_top { 1.0 } else { -1.0 })
        };

        let base = (positions.len() / 3) as u32;
        let segs = segments;

        for i in 0..=segs {
            let phi = startphi + deltaphi * (i as f64) / (segs as f64);
            let cos_phi = phi.cos();
            let sin_phi = phi.sin();

            // Outer edge
            let xo = rmax * cos_phi;
            let yo = rmax * sin_phi;
            let zo = cut_z(xo, yo, z0, norm);
            positions.push(xo as f32);
            positions.push(yo as f32);
            positions.push(zo as f32);
            normals.push(fnx as f32);
            normals.push(fny as f32);
            normals.push(fnz as f32);

            // Inner edge
            let xi = r_inner * cos_phi;
            let yi = r_inner * sin_phi;
            let zi = cut_z(xi, yi, z0, norm);
            positions.push(xi as f32);
            positions.push(yi as f32);
            positions.push(zi as f32);
            normals.push(fnx as f32);
            normals.push(fny as f32);
            normals.push(fnz as f32);
        }

        for i in 0..segs {
            let o0 = base + i * 2;
            let i0 = base + i * 2 + 1;
            let o1 = base + (i + 1) * 2;
            let i1 = base + (i + 1) * 2 + 1;

            if is_top {
                indices.push(o0); indices.push(o1); indices.push(i0);
                indices.push(o1); indices.push(i1); indices.push(i0);
            } else {
                indices.push(o0); indices.push(i0); indices.push(o1);
                indices.push(o1); indices.push(i0); indices.push(i1);
            }
        }
    };

    gen_cap(false, &mut positions, &mut normals, &mut indices); // bottom
    gen_cap(true, &mut positions, &mut normals, &mut indices);  // top

    // Phi-cut wedge faces
    if !full_circle {
        let gen_wedge = |phi: f64, flip: bool, positions: &mut Vec<f32>, normals: &mut Vec<f32>, indices: &mut Vec<u32>| {
            let cos_phi = phi.cos();
            let sin_phi = phi.sin();
            let nx = if flip { sin_phi } else { -sin_phi };
            let ny = if flip { -cos_phi } else { cos_phi };

            let r_inner = if rmin > 1e-12 { rmin } else { 0.0 };
            let base = (positions.len() / 3) as u32;

            // 4 vertices: outer-bottom, outer-top, inner-top, inner-bottom
            let pts = [
                (rmax * cos_phi, rmax * sin_phi, -hz, low_norm),
                (rmax * cos_phi, rmax * sin_phi, hz, high_norm),
                (r_inner * cos_phi, r_inner * sin_phi, hz, high_norm),
                (r_inner * cos_phi, r_inner * sin_phi, -hz, low_norm),
            ];

            for &(x, y, z0, norm) in &pts {
                let z = cut_z(x, y, z0, norm);
                positions.push(x as f32);
                positions.push(y as f32);
                positions.push(z as f32);
                normals.push(nx as f32);
                normals.push(ny as f32);
                normals.push(0.0 as f32);
            }

            if flip {
                indices.push(base); indices.push(base + 1); indices.push(base + 2);
                indices.push(base); indices.push(base + 2); indices.push(base + 3);
            } else {
                indices.push(base); indices.push(base + 2); indices.push(base + 1);
                indices.push(base); indices.push(base + 3); indices.push(base + 2);
            }
        };

        gen_wedge(startphi, false, &mut positions, &mut normals, &mut indices);
        gen_wedge(startphi + deltaphi, true, &mut positions, &mut normals, &mut indices);
    }

    TriangleMesh {
        positions,
        normals,
        indices,
    }
}
