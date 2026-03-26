use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

use super::csg;
use super::primitives::{box_mesh, cone_mesh, polycone_mesh, sphere_mesh, torus_mesh, trd_mesh, tube_mesh, xtru_mesh};
use super::types::TriangleMesh;
use crate::eval::engine::EvalEngine;
use crate::gdml::model::*;
use crate::gdml::units;

pub fn tessellate_all_solids(
    solids: &SolidSection,
    engine: &EvalEngine,
    segments: u32,
) -> Result<(HashMap<String, TriangleMesh>, Vec<String>)> {
    let mut meshes = HashMap::new();
    let mut warnings = Vec::new();

    // Build a name -> Solid lookup for boolean solid resolution
    let solid_map: HashMap<&str, &Solid> = solids.solids.iter().map(|s| (s.name(), s)).collect();

    // Phase 1: Tessellate all primitive solids
    for solid in &solids.solids {
        let name = solid.name().to_string();
        match solid {
            Solid::Boolean(_) => {} // skip for now
            _ => match tessellate_solid(solid, engine, segments) {
                Ok(mesh) => {
                    meshes.insert(name, mesh);
                }
                Err(e) => {
                    let msg = format!("Failed to tessellate solid '{}': {}", name, e);
                    tracing::warn!("{}", msg);
                    warnings.push(msg);
                }
            },
        }
    }

    // Phase 2: Resolve boolean solids (may reference other booleans)
    for solid in &solids.solids {
        if let Solid::Boolean(bs) = solid {
            let mut resolving = HashSet::new();
            match tessellate_boolean_solid(
                bs,
                &solid_map,
                &mut meshes,
                engine,
                segments,
                &mut resolving,
            ) {
                Ok(mesh) => {
                    meshes.insert(bs.name.clone(), mesh);
                }
                Err(e) => {
                    let msg = format!("Failed to tessellate boolean solid '{}': {}", bs.name, e);
                    tracing::warn!("{}", msg);
                    warnings.push(msg);
                }
            }
        }
    }

    Ok((meshes, warnings))
}

fn tessellate_solid(solid: &Solid, engine: &EvalEngine, segments: u32) -> Result<TriangleMesh> {
    match solid {
        Solid::Box(s) => tessellate_box_solid(s, engine),
        Solid::Tube(s) => tessellate_tube_solid(s, engine, segments),
        Solid::Cone(s) => tessellate_cone_solid(s, engine, segments),
        Solid::Sphere(s) => tessellate_sphere_solid(s, engine, segments),
        Solid::Trd(s) => tessellate_trd_solid(s, engine),
        Solid::Polycone(s) => tessellate_polycone_solid(s, engine, segments),
        Solid::Xtru(s) => tessellate_xtru_solid(s, engine),
        Solid::Orb(s) => tessellate_orb_solid(s, engine, segments),
        Solid::Torus(s) => tessellate_torus_solid(s, engine, segments),
        Solid::Tessellated(s) => tessellate_tessellated_solid(s, engine),
        Solid::Boolean(_) => Err(anyhow::anyhow!("Boolean solids resolved in phase 2")),
    }
}

fn tessellate_boolean_solid(
    bs: &BooleanSolid,
    solid_map: &HashMap<&str, &Solid>,
    meshes: &mut HashMap<String, TriangleMesh>,
    engine: &EvalEngine,
    segments: u32,
    resolving: &mut HashSet<String>,
) -> Result<TriangleMesh> {
    if let Some(mesh) = meshes.get(&bs.name) {
        return Ok(mesh.clone());
    }

    if !resolving.insert(bs.name.clone()) {
        return Err(anyhow::anyhow!(
            "Cyclic boolean solid dependency detected at '{}'",
            bs.name
        ));
    }

    let result = (|| -> Result<TriangleMesh> {
        // Resolve first operand (may itself be a boolean)
        let first_mesh = resolve_operand(
            &bs.first_ref,
            solid_map,
            meshes,
            engine,
            segments,
            resolving,
        )?;

        // Resolve second operand
        let second_mesh = resolve_operand(
            &bs.second_ref,
            solid_map,
            meshes,
            engine,
            segments,
            resolving,
        )?;

        // Apply first solid transform if specified
        let first_mesh =
            apply_placement_transform(&first_mesh, &bs.first_position, &bs.first_rotation, engine);

        // Apply second solid transform (position/rotation of second relative to first)
        let second_mesh =
            apply_placement_transform(&second_mesh, &bs.position, &bs.rotation, engine);

        // Perform CSG operation
        let result = match bs.operation {
            BooleanOp::Subtraction => csg::subtract(&first_mesh, &second_mesh),
            BooleanOp::Union => csg::union(&first_mesh, &second_mesh),
            BooleanOp::Intersection => csg::intersect(&first_mesh, &second_mesh),
        };

        Ok(result)
    })();

    resolving.remove(&bs.name);
    result
}

fn resolve_operand(
    name: &str,
    solid_map: &HashMap<&str, &Solid>,
    meshes: &mut HashMap<String, TriangleMesh>,
    engine: &EvalEngine,
    segments: u32,
    resolving: &mut HashSet<String>,
) -> Result<TriangleMesh> {
    // Check if already tessellated
    if let Some(mesh) = meshes.get(name) {
        return Ok(mesh.clone());
    }

    // Look up the solid definition and tessellate it
    let solid = solid_map
        .get(name)
        .ok_or_else(|| anyhow::anyhow!("Boolean operand '{}' not found", name))?;

    match solid {
        Solid::Boolean(bs) => {
            let mesh =
                tessellate_boolean_solid(bs, solid_map, meshes, engine, segments, resolving)?;
            meshes.insert(name.to_string(), mesh.clone());
            Ok(mesh)
        }
        _ => {
            let mesh = tessellate_solid(solid, engine, segments)?;
            meshes.insert(name.to_string(), mesh.clone());
            Ok(mesh)
        }
    }
}

fn apply_placement_transform(
    mesh: &TriangleMesh,
    pos: &Option<PlacementPos>,
    rot: &Option<PlacementRot>,
    engine: &EvalEngine,
) -> TriangleMesh {
    let position = resolve_placement_pos(pos, engine);
    let rotation = resolve_placement_rot(rot, engine);

    csg::transform_mesh(mesh, position, rotation)
}

fn resolve_placement_pos(pos: &Option<PlacementPos>, engine: &EvalEngine) -> [f64; 3] {
    match pos {
        Some(PlacementPos::Inline(p)) => {
            let x = p.x.as_ref().map(|v| engine.resolve_value(v)).unwrap_or(0.0);
            let y = p.y.as_ref().map(|v| engine.resolve_value(v)).unwrap_or(0.0);
            let z = p.z.as_ref().map(|v| engine.resolve_value(v)).unwrap_or(0.0);
            let unit = p.unit.as_deref().unwrap_or("mm");
            [
                units::length_to_mm(x, unit),
                units::length_to_mm(y, unit),
                units::length_to_mm(z, unit),
            ]
        }
        Some(PlacementPos::Ref(name)) => engine
            .position_values
            .get(name)
            .copied()
            .unwrap_or([0.0; 3]),
        None => [0.0; 3],
    }
}

fn resolve_placement_rot(rot: &Option<PlacementRot>, engine: &EvalEngine) -> [f64; 3] {
    match rot {
        Some(PlacementRot::Inline(r)) => {
            let x = r.x.as_ref().map(|v| engine.resolve_value(v)).unwrap_or(0.0);
            let y = r.y.as_ref().map(|v| engine.resolve_value(v)).unwrap_or(0.0);
            let z = r.z.as_ref().map(|v| engine.resolve_value(v)).unwrap_or(0.0);
            let unit = r.unit.as_deref().unwrap_or("rad");
            [
                units::angle_to_rad(x, unit),
                units::angle_to_rad(y, unit),
                units::angle_to_rad(z, unit),
            ]
        }
        Some(PlacementRot::Ref(name)) => engine
            .rotation_values
            .get(name)
            .copied()
            .unwrap_or([0.0; 3]),
        None => [0.0; 3],
    }
}

fn resolve(engine: &EvalEngine, expr: &str) -> f64 {
    engine.resolve_value(expr)
}

fn resolve_opt(engine: &EvalEngine, expr: &Option<String>) -> f64 {
    match expr {
        Some(s) => engine.resolve_value(s),
        None => 0.0,
    }
}

/// Resolve a length expression, applying lunit conversion only for literal values.
/// If the expression references any symbols that are already length values in mm,
/// skip the lunit conversion to avoid double-converting.
fn resolve_with_lunit(engine: &EvalEngine, expr: &str, lunit: &str) -> f64 {
    let val = engine.resolve_value(expr);
    if engine.expression_uses_length_symbols(expr) {
        val
    } else {
        units::length_to_mm(val, lunit)
    }
}

fn resolve_opt_with_lunit(engine: &EvalEngine, expr: &Option<String>, lunit: &str) -> f64 {
    match expr {
        Some(s) => resolve_with_lunit(engine, s, lunit),
        None => 0.0,
    }
}

fn tessellate_box_solid(s: &BoxSolid, engine: &EvalEngine) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let x = resolve_with_lunit(engine, &s.x, lunit);
    let y = resolve_with_lunit(engine, &s.y, lunit);
    let z = resolve_with_lunit(engine, &s.z, lunit);
    Ok(box_mesh::tessellate_box(x, y, z))
}

fn tessellate_tube_solid(
    s: &TubeSolid,
    engine: &EvalEngine,
    segments: u32,
) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let aunit = s.aunit.as_deref().unwrap_or("rad");
    let rmin = resolve_opt_with_lunit(engine, &s.rmin, lunit);
    let rmax = resolve_with_lunit(engine, &s.rmax, lunit);
    let z = resolve_with_lunit(engine, &s.z, lunit);
    let startphi = units::angle_to_rad(resolve_opt(engine, &s.startphi), aunit);
    let deltaphi = match &s.deltaphi {
        Some(expr) => units::angle_to_rad(resolve(engine, expr), aunit),
        None => 2.0 * PI,
    };
    Ok(tube_mesh::tessellate_tube(
        rmin, rmax, z, startphi, deltaphi, segments,
    ))
}

fn tessellate_cone_solid(
    s: &ConeSolid,
    engine: &EvalEngine,
    segments: u32,
) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let aunit = s.aunit.as_deref().unwrap_or("rad");
    let rmin1 = resolve_opt_with_lunit(engine, &s.rmin1, lunit);
    let rmax1 = resolve_with_lunit(engine, &s.rmax1, lunit);
    let rmin2 = resolve_opt_with_lunit(engine, &s.rmin2, lunit);
    let rmax2 = resolve_with_lunit(engine, &s.rmax2, lunit);
    let z = resolve_with_lunit(engine, &s.z, lunit);
    let startphi = units::angle_to_rad(resolve_opt(engine, &s.startphi), aunit);
    let deltaphi = match &s.deltaphi {
        Some(expr) => units::angle_to_rad(resolve(engine, expr), aunit),
        None => 2.0 * PI,
    };
    Ok(cone_mesh::tessellate_cone(
        rmin1, rmax1, rmin2, rmax2, z, startphi, deltaphi, segments,
    ))
}

fn tessellate_sphere_solid(
    s: &SphereSolid,
    engine: &EvalEngine,
    segments: u32,
) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let aunit = s.aunit.as_deref().unwrap_or("rad");
    let rmin = resolve_opt_with_lunit(engine, &s.rmin, lunit);
    let rmax = resolve_with_lunit(engine, &s.rmax, lunit);
    let startphi = units::angle_to_rad(resolve_opt(engine, &s.startphi), aunit);
    let deltaphi = match &s.deltaphi {
        Some(expr) => units::angle_to_rad(resolve(engine, expr), aunit),
        None => 2.0 * PI,
    };
    let starttheta = units::angle_to_rad(resolve_opt(engine, &s.starttheta), aunit);
    let deltatheta = match &s.deltatheta {
        Some(expr) => units::angle_to_rad(resolve(engine, expr), aunit),
        None => PI,
    };
    Ok(sphere_mesh::tessellate_sphere(
        rmin, rmax, startphi, deltaphi, starttheta, deltatheta, segments,
    ))
}

fn tessellate_trd_solid(s: &TrdSolid, engine: &EvalEngine) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let x1 = resolve_with_lunit(engine, &s.x1, lunit);
    let y1 = resolve_with_lunit(engine, &s.y1, lunit);
    let x2 = resolve_with_lunit(engine, &s.x2, lunit);
    let y2 = resolve_with_lunit(engine, &s.y2, lunit);
    let z = resolve_with_lunit(engine, &s.z, lunit);
    Ok(trd_mesh::tessellate_trd(x1, y1, x2, y2, z))
}

fn tessellate_tessellated_solid(s: &TessellatedSolid, engine: &EvalEngine) -> Result<TriangleMesh> {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    let lookup = |name: &str| -> Result<[f64; 3]> {
        engine
            .position_values
            .get(name)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("Tessellated vertex '{}' not found in defines", name))
    };

    for facet in &s.facets {
        match facet {
            TessellatedFacet::Triangular {
                vertex1,
                vertex2,
                vertex3,
                ..
            } => {
                let v1 = lookup(vertex1)?;
                let v2 = lookup(vertex2)?;
                let v3 = lookup(vertex3)?;

                // Compute face normal via cross product
                let e1 = [v2[0] - v1[0], v2[1] - v1[1], v2[2] - v1[2]];
                let e2 = [v3[0] - v1[0], v3[1] - v1[1], v3[2] - v1[2]];
                let nx = e1[1] * e2[2] - e1[2] * e2[1];
                let ny = e1[2] * e2[0] - e1[0] * e2[2];
                let nz = e1[0] * e2[1] - e1[1] * e2[0];
                let len = (nx * nx + ny * ny + nz * nz).sqrt();
                let (nx, ny, nz) = if len > 1e-12 {
                    (nx / len, ny / len, nz / len)
                } else {
                    (0.0, 0.0, 1.0)
                };

                let base = (positions.len() / 3) as u32;
                for v in &[v1, v2, v3] {
                    positions.push(v[0] as f32);
                    positions.push(v[1] as f32);
                    positions.push(v[2] as f32);
                    normals.push(nx as f32);
                    normals.push(ny as f32);
                    normals.push(nz as f32);
                }
                indices.push(base);
                indices.push(base + 1);
                indices.push(base + 2);
            }
            TessellatedFacet::Quadrangular {
                vertex1,
                vertex2,
                vertex3,
                vertex4,
                ..
            } => {
                let v1 = lookup(vertex1)?;
                let v2 = lookup(vertex2)?;
                let v3 = lookup(vertex3)?;
                let v4 = lookup(vertex4)?;

                // Compute face normal from first triangle
                let e1 = [v2[0] - v1[0], v2[1] - v1[1], v2[2] - v1[2]];
                let e2 = [v3[0] - v1[0], v3[1] - v1[1], v3[2] - v1[2]];
                let nx = e1[1] * e2[2] - e1[2] * e2[1];
                let ny = e1[2] * e2[0] - e1[0] * e2[2];
                let nz = e1[0] * e2[1] - e1[1] * e2[0];
                let len = (nx * nx + ny * ny + nz * nz).sqrt();
                let (nx, ny, nz) = if len > 1e-12 {
                    (nx / len, ny / len, nz / len)
                } else {
                    (0.0, 0.0, 1.0)
                };

                let base = (positions.len() / 3) as u32;
                for v in &[v1, v2, v3, v4] {
                    positions.push(v[0] as f32);
                    positions.push(v[1] as f32);
                    positions.push(v[2] as f32);
                    normals.push(nx as f32);
                    normals.push(ny as f32);
                    normals.push(nz as f32);
                }
                // Two triangles: (0,1,2) and (0,2,3)
                indices.push(base);
                indices.push(base + 1);
                indices.push(base + 2);
                indices.push(base);
                indices.push(base + 2);
                indices.push(base + 3);
            }
        }
    }

    Ok(TriangleMesh {
        positions,
        normals,
        indices,
    })
}

fn tessellate_torus_solid(
    s: &TorusSolid,
    engine: &EvalEngine,
    segments: u32,
) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let aunit = s.aunit.as_deref().unwrap_or("rad");
    let rmin = resolve_opt_with_lunit(engine, &s.rmin, lunit);
    let rmax = resolve_with_lunit(engine, &s.rmax, lunit);
    let rtor = resolve_with_lunit(engine, &s.rtor, lunit);
    let startphi = units::angle_to_rad(resolve_opt(engine, &s.startphi), aunit);
    let deltaphi = match &s.deltaphi {
        Some(expr) => units::angle_to_rad(resolve(engine, expr), aunit),
        None => 2.0 * PI,
    };
    Ok(torus_mesh::tessellate_torus(
        rmin, rmax, rtor, startphi, deltaphi, segments,
    ))
}

fn tessellate_orb_solid(s: &OrbSolid, engine: &EvalEngine, segments: u32) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let r = resolve_with_lunit(engine, &s.r, lunit);
    Ok(sphere_mesh::tessellate_sphere(
        0.0, r, 0.0, 2.0 * PI, 0.0, PI, segments,
    ))
}

fn tessellate_polycone_solid(
    s: &PolyconeSolid,
    engine: &EvalEngine,
    segments: u32,
) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let aunit = s.aunit.as_deref().unwrap_or("rad");
    let startphi = units::angle_to_rad(resolve_opt(engine, &s.startphi), aunit);
    let deltaphi = match &s.deltaphi {
        Some(expr) => units::angle_to_rad(resolve(engine, expr), aunit),
        None => 2.0 * PI,
    };

    let planes: Vec<(f64, f64, f64)> = s
        .zplanes
        .iter()
        .map(|zp| {
            let z = resolve_with_lunit(engine, &zp.z, lunit);
            let rmin = resolve_opt_with_lunit(engine, &zp.rmin, lunit);
            let rmax = resolve_with_lunit(engine, &zp.rmax, lunit);
            (z, rmin, rmax)
        })
        .collect();

    Ok(polycone_mesh::tessellate_polycone(
        &planes, startphi, deltaphi, segments,
    ))
}

fn tessellate_xtru_solid(s: &XtruSolid, engine: &EvalEngine) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");

    let vertices: Vec<(f64, f64)> = s
        .vertices
        .iter()
        .map(|v| {
            let x = resolve_with_lunit(engine, &v.x, lunit);
            let y = resolve_with_lunit(engine, &v.y, lunit);
            (x, y)
        })
        .collect();

    let sections: Vec<(f64, f64, f64, f64)> = s
        .sections
        .iter()
        .map(|sec| {
            let z = resolve_with_lunit(engine, &sec.z_position, lunit);
            let xoff = resolve_with_lunit(engine, &sec.x_offset, lunit);
            let yoff = resolve_with_lunit(engine, &sec.y_offset, lunit);
            let scale = resolve(engine, &sec.scaling_factor);
            (z, xoff, yoff, scale)
        })
        .collect();

    Ok(xtru_mesh::tessellate_xtru(&vertices, &sections))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gdml::model::{DefineSection, Quantity};

    #[test]
    fn resolve_with_lunit_does_not_double_convert_length_expressions() {
        let mut engine = EvalEngine::new();
        let mut defines = DefineSection::default();
        defines.quantities.push(Quantity {
            name: "A".to_string(),
            r#type: Some("length".to_string()),
            value: "2".to_string(),
            unit: Some("cm".to_string()),
        });
        defines.quantities.push(Quantity {
            name: "B".to_string(),
            r#type: Some("length".to_string()),
            value: "3".to_string(),
            unit: Some("cm".to_string()),
        });
        engine.evaluate_all(&defines).unwrap();

        // A and B are already converted to mm in the eval engine.
        let expr_val = resolve_with_lunit(&engine, "A + B", "cm");
        assert!((expr_val - 50.0).abs() < 1e-9);

        // Literal values still respect the solid's lunit.
        let literal_val = resolve_with_lunit(&engine, "2.0", "cm");
        assert!((literal_val - 20.0).abs() < 1e-9);
    }

    #[test]
    fn boolean_cycle_is_reported_as_warning_instead_of_recursing_forever() {
        let solids = SolidSection {
            solids: vec![
                Solid::Boolean(BooleanSolid {
                    name: "A".to_string(),
                    operation: BooleanOp::Union,
                    first_ref: "B".to_string(),
                    second_ref: "B".to_string(),
                    position: None,
                    rotation: None,
                    first_position: None,
                    first_rotation: None,
                }),
                Solid::Boolean(BooleanSolid {
                    name: "B".to_string(),
                    operation: BooleanOp::Union,
                    first_ref: "A".to_string(),
                    second_ref: "A".to_string(),
                    position: None,
                    rotation: None,
                    first_position: None,
                    first_rotation: None,
                }),
            ],
        };

        let engine = EvalEngine::new();
        let (_meshes, warnings) = tessellate_all_solids(&solids, &engine, 24).unwrap();
        assert!(warnings
            .iter()
            .any(|w| w.contains("Cyclic boolean solid dependency detected")));
    }
}
