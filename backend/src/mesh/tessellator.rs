use anyhow::Result;
use std::collections::HashMap;
use std::f64::consts::PI;

use super::csg;
use super::primitives::{box_mesh, cone_mesh, sphere_mesh, tube_mesh};
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
    let solid_map: HashMap<&str, &Solid> = solids
        .solids
        .iter()
        .map(|s| (s.name(), s))
        .collect();

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
            match tessellate_boolean_solid(bs, &solid_map, &mut meshes, engine, segments) {
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
        Solid::Boolean(_) => Err(anyhow::anyhow!("Boolean solids resolved in phase 2")),
    }
}

fn tessellate_boolean_solid(
    bs: &BooleanSolid,
    solid_map: &HashMap<&str, &Solid>,
    meshes: &mut HashMap<String, TriangleMesh>,
    engine: &EvalEngine,
    segments: u32,
) -> Result<TriangleMesh> {
    // Resolve first operand (may itself be a boolean)
    let first_mesh = resolve_operand(&bs.first_ref, solid_map, meshes, engine, segments)?;

    // Resolve second operand
    let second_mesh = resolve_operand(&bs.second_ref, solid_map, meshes, engine, segments)?;

    // Apply first solid transform if specified
    let first_mesh = apply_placement_transform(&first_mesh, &bs.first_position, &bs.first_rotation, engine);

    // Apply second solid transform (position/rotation of second relative to first)
    let second_mesh = apply_placement_transform(&second_mesh, &bs.position, &bs.rotation, engine);

    // Perform CSG operation
    let result = match bs.operation {
        BooleanOp::Subtraction => csg::subtract(&first_mesh, &second_mesh),
        BooleanOp::Union => csg::union(&first_mesh, &second_mesh),
        BooleanOp::Intersection => csg::intersect(&first_mesh, &second_mesh),
    };

    Ok(result)
}

fn resolve_operand(
    name: &str,
    solid_map: &HashMap<&str, &Solid>,
    meshes: &mut HashMap<String, TriangleMesh>,
    engine: &EvalEngine,
    segments: u32,
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
            let mesh = tessellate_boolean_solid(bs, solid_map, meshes, engine, segments)?;
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
/// If the expression resolves to a known variable (already converted to mm by the engine),
/// skip the lunit conversion to avoid double-converting.
fn resolve_with_lunit(engine: &EvalEngine, expr: &str, lunit: &str) -> f64 {
    let val = engine.resolve_value(expr);
    if engine.context.get(expr.trim()).is_some() {
        val // already in mm from engine
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

fn tessellate_tube_solid(s: &TubeSolid, engine: &EvalEngine, segments: u32) -> Result<TriangleMesh> {
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

fn tessellate_cone_solid(s: &ConeSolid, engine: &EvalEngine, segments: u32) -> Result<TriangleMesh> {
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

fn tessellate_sphere_solid(s: &SphereSolid, engine: &EvalEngine, segments: u32) -> Result<TriangleMesh> {
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
