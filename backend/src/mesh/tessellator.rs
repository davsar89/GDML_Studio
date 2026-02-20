use anyhow::Result;
use std::collections::HashMap;
use std::f64::consts::PI;

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

    for solid in &solids.solids {
        let name = solid.name().to_string();
        match tessellate_solid(solid, engine, segments) {
            Ok(mesh) => {
                meshes.insert(name, mesh);
            }
            Err(e) => {
                let msg = format!("Failed to tessellate solid '{}': {}", name, e);
                tracing::warn!("{}", msg);
                warnings.push(msg);
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
