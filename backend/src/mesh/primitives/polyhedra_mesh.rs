use crate::mesh::types::TriangleMesh;
use std::f64::consts::PI;

/// Tessellate a polyhedra (G4Polyhedra) - like polycone but with N flat sides.
///
/// - `planes`: slice of (z, rmin, rmax) tuples. rmin/rmax are the radius to
///   the midpoint of each polygon side (apothem), same as Geant4 convention.
/// - `startphi`: starting angle (radians)
/// - `deltaphi`: angular sweep (radians)
/// - `numsides`: number of polygon sides
pub fn tessellate_polyhedra(
    planes: &[(f64, f64, f64)],
    startphi: f64,
    deltaphi: f64,
    numsides: u32,
) -> TriangleMesh {
    let mut positions: Vec<f32> = Vec::new();
    let mut normals: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    if planes.len() < 2 || numsides < 3 {
        return TriangleMesh { positions, normals, indices };
    }

    let full_circle = (deltaphi - 2.0 * PI).abs() < 1e-6;
    let n = numsides;

    // Convert apothem (rmin/rmax) to vertex radius: r_vertex = r_apothem / cos(pi/n)
    let cos_half = (PI / n as f64).cos();

    // Generate polygon vertices at each z-plane for a given radius type
    let polygon_verts = |r_apothem: f64, z: f64| -> Vec<[f64; 3]> {
        let r = if r_apothem > 1e-12 { r_apothem / cos_half } else { 0.0 };
        (0..=n)
            .map(|i| {
                let phi = startphi + deltaphi * (i as f64) / (n as f64);
                [r * phi.cos(), r * phi.sin(), z]
            })
            .collect()
    };

    // Generate side walls between adjacent z-planes for outer or inner surface
    let gen_walls = |is_outer: bool, positions: &mut Vec<f32>, normals: &mut Vec<f32>, indices: &mut Vec<u32>| {
        for p in 0..planes.len() - 1 {
            let (z0, rmin0, rmax0) = planes[p];
            let (z1, rmin1, rmax1) = planes[p + 1];
            let r0 = if is_outer { rmax0 } else { rmin0 };
            let r1 = if is_outer { rmax1 } else { rmin1 };

            let verts0 = polygon_verts(r0, z0);
            let verts1 = polygon_verts(r1, z1);

            for i in 0..n as usize {
                let base = (positions.len() / 3) as u32;

                let v00 = verts0[i];
                let v01 = verts0[i + 1];
                let v10 = verts1[i];
                let v11 = verts1[i + 1];

                // Compute face normal
                let u = [v01[0] - v00[0], v01[1] - v00[1], v01[2] - v00[2]];
                let v = [v10[0] - v00[0], v10[1] - v00[1], v10[2] - v00[2]];
                let mut nx = u[1] * v[2] - u[2] * v[1];
                let mut ny = u[2] * v[0] - u[0] * v[2];
                let mut nz = u[0] * v[1] - u[1] * v[0];
                let len = (nx * nx + ny * ny + nz * nz).sqrt();
                if len > 1e-12 {
                    nx /= len; ny /= len; nz /= len;
                }
                if !is_outer {
                    nx = -nx; ny = -ny; nz = -nz;
                }

                for vtx in &[v00, v01, v11, v10] {
                    positions.push(vtx[0] as f32);
                    positions.push(vtx[1] as f32);
                    positions.push(vtx[2] as f32);
                    normals.push(nx as f32);
                    normals.push(ny as f32);
                    normals.push(nz as f32);
                }

                if is_outer {
                    indices.push(base); indices.push(base + 1); indices.push(base + 2);
                    indices.push(base); indices.push(base + 2); indices.push(base + 3);
                } else {
                    indices.push(base); indices.push(base + 2); indices.push(base + 1);
                    indices.push(base); indices.push(base + 3); indices.push(base + 2);
                }
            }
        }
    };

    // Outer walls
    gen_walls(true, &mut positions, &mut normals, &mut indices);

    // Inner walls (if any rmin > 0)
    let has_inner = planes.iter().any(|(_, rmin, _)| *rmin > 1e-12);
    if has_inner {
        gen_walls(false, &mut positions, &mut normals, &mut indices);
    }

    // End caps (bottom and top annular polygons)
    let gen_cap = |plane_idx: usize, is_top: bool, positions: &mut Vec<f32>, normals: &mut Vec<f32>, indices: &mut Vec<u32>| {
        let (z, rmin, rmax) = planes[plane_idx];
        let outer_verts = polygon_verts(rmax, z);
        let inner_verts = polygon_verts(rmin, z);
        let nz: f32 = if is_top { 1.0 } else { -1.0 };

        for i in 0..n as usize {
            let base = (positions.len() / 3) as u32;

            let o0 = outer_verts[i];
            let o1 = outer_verts[i + 1];
            let i0 = inner_verts[i];
            let i1 = inner_verts[i + 1];

            for vtx in &[o0, o1, i1, i0] {
                positions.push(vtx[0] as f32);
                positions.push(vtx[1] as f32);
                positions.push(vtx[2] as f32);
                normals.push(0.0);
                normals.push(0.0);
                normals.push(nz);
            }

            if is_top {
                indices.push(base); indices.push(base + 1); indices.push(base + 2);
                indices.push(base); indices.push(base + 2); indices.push(base + 3);
            } else {
                indices.push(base); indices.push(base + 2); indices.push(base + 1);
                indices.push(base); indices.push(base + 3); indices.push(base + 2);
            }
        }
    };

    gen_cap(0, false, &mut positions, &mut normals, &mut indices);
    gen_cap(planes.len() - 1, true, &mut positions, &mut normals, &mut indices);

    // Phi-cut faces (if not full circle)
    if !full_circle {
        let gen_phi_face = |phi: f64, flip: bool, positions: &mut Vec<f32>, normals: &mut Vec<f32>, indices: &mut Vec<u32>| {
            let cos_phi = phi.cos();
            let sin_phi = phi.sin();
            let nx = if flip { sin_phi } else { -sin_phi };
            let ny = if flip { -cos_phi } else { cos_phi };

            for p in 0..planes.len() - 1 {
                let (z0, rmin0, rmax0) = planes[p];
                let (z1, rmin1, rmax1) = planes[p + 1];

                let r_out0 = if rmax0 > 1e-12 { rmax0 / cos_half } else { 0.0 };
                let r_out1 = if rmax1 > 1e-12 { rmax1 / cos_half } else { 0.0 };
                let r_in0 = if rmin0 > 1e-12 { rmin0 / cos_half } else { 0.0 };
                let r_in1 = if rmin1 > 1e-12 { rmin1 / cos_half } else { 0.0 };

                let base = (positions.len() / 3) as u32;
                let pts = [
                    [r_out0 * cos_phi, r_out0 * sin_phi, z0],
                    [r_out1 * cos_phi, r_out1 * sin_phi, z1],
                    [r_in1 * cos_phi, r_in1 * sin_phi, z1],
                    [r_in0 * cos_phi, r_in0 * sin_phi, z0],
                ];

                for vtx in &pts {
                    positions.push(vtx[0] as f32);
                    positions.push(vtx[1] as f32);
                    positions.push(vtx[2] as f32);
                    normals.push(nx as f32);
                    normals.push(ny as f32);
                    normals.push(0.0);
                }

                if flip {
                    indices.push(base); indices.push(base + 1); indices.push(base + 2);
                    indices.push(base); indices.push(base + 2); indices.push(base + 3);
                } else {
                    indices.push(base); indices.push(base + 2); indices.push(base + 1);
                    indices.push(base); indices.push(base + 3); indices.push(base + 2);
                }
            }
        };

        gen_phi_face(startphi, false, &mut positions, &mut normals, &mut indices);
        gen_phi_face(startphi + deltaphi, true, &mut positions, &mut normals, &mut indices);
    }

    TriangleMesh {
        positions,
        normals,
        indices,
    }
}
