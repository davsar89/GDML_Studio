use crate::mesh::types::TriangleMesh;
use std::f64::consts::PI;

/// Tessellate a torus (ring/donut shape).
///
/// - `rmin`: inner radius of tube cross-section (0 = solid torus)
/// - `rmax`: outer radius of tube cross-section
/// - `rtor`: distance from torus center to tube center
/// - `startphi`: starting angle around the ring axis (radians)
/// - `deltaphi`: angular sweep around the ring axis (radians)
/// - `segments`: number of subdivisions for both ring and tube
pub fn tessellate_torus(
    rmin: f64,
    rmax: f64,
    rtor: f64,
    startphi: f64,
    deltaphi: f64,
    segments: u32,
) -> TriangleMesh {
    let mut positions: Vec<f32> = Vec::new();
    let mut normals: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let ring_segs = segments;
    let tube_segs = segments;
    let full_ring = (deltaphi - 2.0 * PI).abs() < 1e-6;

    // Generate a surface of revolution for a given tube radius
    let gen_surface = |r: f64, outward: bool, positions: &mut Vec<f32>, normals: &mut Vec<f32>, indices: &mut Vec<u32>| {
        let base = (positions.len() / 3) as u32;

        // Generate vertices
        for i in 0..=ring_segs {
            let phi = startphi + deltaphi * (i as f64) / (ring_segs as f64);
            let cos_phi = phi.cos();
            let sin_phi = phi.sin();

            for j in 0..=tube_segs {
                let theta = 2.0 * PI * (j as f64) / (tube_segs as f64);
                let cos_theta = theta.cos();
                let sin_theta = theta.sin();

                let x = (rtor + r * cos_theta) * cos_phi;
                let y = (rtor + r * cos_theta) * sin_phi;
                let z = r * sin_theta;

                positions.push(x as f32);
                positions.push(y as f32);
                positions.push(z as f32);

                // Normal points radially outward from tube center
                let nx = cos_theta * cos_phi;
                let ny = cos_theta * sin_phi;
                let nz = sin_theta;
                if outward {
                    normals.push(nx as f32);
                    normals.push(ny as f32);
                    normals.push(nz as f32);
                } else {
                    normals.push(-nx as f32);
                    normals.push(-ny as f32);
                    normals.push(-nz as f32);
                }
            }
        }

        // Generate triangles
        let stride = tube_segs + 1;
        for i in 0..ring_segs {
            for j in 0..tube_segs {
                let a = base + i * stride + j;
                let b = base + i * stride + j + 1;
                let c = base + (i + 1) * stride + j + 1;
                let d = base + (i + 1) * stride + j;

                if outward {
                    indices.push(a); indices.push(b); indices.push(c);
                    indices.push(a); indices.push(c); indices.push(d);
                } else {
                    indices.push(a); indices.push(c); indices.push(b);
                    indices.push(a); indices.push(d); indices.push(c);
                }
            }
        }
    };

    // Outer surface
    gen_surface(rmax, true, &mut positions, &mut normals, &mut indices);

    // Inner surface (if hollow)
    if rmin > 1e-12 {
        gen_surface(rmin, false, &mut positions, &mut normals, &mut indices);
    }

    // End caps (if not a full ring)
    if !full_ring {
        let gen_cap = |phi: f64, flip: bool, positions: &mut Vec<f32>, normals: &mut Vec<f32>, indices: &mut Vec<u32>| {
            let cos_phi = phi.cos();
            let sin_phi = phi.sin();
            // Normal is tangent to ring direction
            let nx = if flip { sin_phi } else { -sin_phi };
            let ny = if flip { -cos_phi } else { cos_phi };
            let nz = 0.0;

            let r_inner = if rmin > 1e-12 { rmin } else { 0.0 };
            let cap_segs = tube_segs;
            let base = (positions.len() / 3) as u32;

            // Generate annular cap vertices
            for i in 0..=cap_segs {
                let theta = 2.0 * PI * (i as f64) / (cap_segs as f64);
                let cos_theta = theta.cos();
                let sin_theta = theta.sin();

                // Outer edge
                let xo = (rtor + rmax * cos_theta) * cos_phi;
                let yo = (rtor + rmax * cos_theta) * sin_phi;
                let zo = rmax * sin_theta;
                positions.push(xo as f32);
                positions.push(yo as f32);
                positions.push(zo as f32);
                normals.push(nx as f32);
                normals.push(ny as f32);
                normals.push(nz as f32);

                // Inner edge
                let xi = (rtor + r_inner * cos_theta) * cos_phi;
                let yi = (rtor + r_inner * cos_theta) * sin_phi;
                let zi = r_inner * sin_theta;
                positions.push(xi as f32);
                positions.push(yi as f32);
                positions.push(zi as f32);
                normals.push(nx as f32);
                normals.push(ny as f32);
                normals.push(nz as f32);
            }

            // Triangulate the annular ring
            for i in 0..cap_segs {
                let o0 = base + i * 2;
                let i0 = base + i * 2 + 1;
                let o1 = base + (i + 1) * 2;
                let i1 = base + (i + 1) * 2 + 1;

                if flip {
                    indices.push(o0); indices.push(i0); indices.push(o1);
                    indices.push(o1); indices.push(i0); indices.push(i1);
                } else {
                    indices.push(o0); indices.push(o1); indices.push(i0);
                    indices.push(o1); indices.push(i1); indices.push(i0);
                }
            }
        };

        // Start cap (at startphi)
        gen_cap(startphi, false, &mut positions, &mut normals, &mut indices);
        // End cap (at startphi + deltaphi)
        gen_cap(startphi + deltaphi, true, &mut positions, &mut normals, &mut indices);
    }

    TriangleMesh {
        positions,
        normals,
        indices,
    }
}
