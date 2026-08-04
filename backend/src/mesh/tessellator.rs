use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

use super::csg;
use super::primitives::{
    arb8_mesh, box_mesh, cone_mesh, cut_tube_mesh, elcone_mesh, ellipsoid_mesh, eltube_mesh,
    generic_polycone_mesh, hype_mesh, paraboloid_mesh, polycone_mesh, polyhedra_mesh, sphere_mesh,
    torus_mesh, trap_mesh, trd_mesh, tube_mesh, twisted_box_mesh, twisted_trap_mesh,
    twisted_tubs_mesh, xtru_mesh,
};
use super::types::TriangleMesh;
use crate::eval::engine::EvalEngine;
use crate::gdml::model::*;
use crate::gdml::units;

pub fn tessellate_all_solids(
    solids: &SolidSection,
    engine: &EvalEngine,
    segments: u32,
) -> Result<(HashMap<String, TriangleMesh>, Vec<String>)> {
    // Clamp the subdivision count to a safe range. `segments` comes straight from
    // the request body; 0 would make `2*PI/segments` divide by zero (NaN geometry)
    // and an unbounded value would blow up memory (sphere/torus are O(segments^2)).
    let segments = segments.clamp(3, 512);
    let mut meshes = HashMap::new();
    let mut warnings = Vec::new();

    // Build a name -> Solid lookup for boolean solid resolution
    let solid_map: HashMap<&str, &Solid> = solids.solids.iter().map(|s| (s.name(), s)).collect();

    // Phase 1: Tessellate all primitive solids
    for solid in &solids.solids {
        let name = solid.name().to_string();
        match solid {
            Solid::Boolean(_) | Solid::Scaled(_) | Solid::Reflected(_) | Solid::MultiUnion(_) => {} // skip for phase 2
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

    // Phase 2: Resolve composite solids (scaled, boolean — may reference each other)
    for solid in &solids.solids {
        match solid {
            Solid::MultiUnion(mu) => {
                let mut resolving = HashSet::new();
                match tessellate_multiunion_solid(
                    mu,
                    &solid_map,
                    &mut meshes,
                    engine,
                    segments,
                    &mut resolving,
                ) {
                    Ok(mesh) => {
                        meshes.insert(mu.name.clone(), mesh);
                    }
                    Err(e) => {
                        let msg = format!("Failed to tessellate multiUnion '{}': {}", mu.name, e);
                        tracing::warn!("{}", msg);
                        warnings.push(msg);
                    }
                }
            }
            Solid::Reflected(rs) => {
                let mut resolving = HashSet::new();
                match tessellate_reflected_solid(
                    rs,
                    &solid_map,
                    &mut meshes,
                    engine,
                    segments,
                    &mut resolving,
                ) {
                    Ok(mesh) => {
                        meshes.insert(rs.name.clone(), mesh);
                    }
                    Err(e) => {
                        let msg =
                            format!("Failed to tessellate reflected solid '{}': {}", rs.name, e);
                        tracing::warn!("{}", msg);
                        warnings.push(msg);
                    }
                }
            }
            Solid::Scaled(ss) => {
                let mut resolving = HashSet::new();
                match tessellate_scaled_solid(
                    ss,
                    &solid_map,
                    &mut meshes,
                    engine,
                    segments,
                    &mut resolving,
                ) {
                    Ok(mesh) => {
                        meshes.insert(ss.name.clone(), mesh);
                    }
                    Err(e) => {
                        let msg = format!("Failed to tessellate scaled solid '{}': {}", ss.name, e);
                        tracing::warn!("{}", msg);
                        warnings.push(msg);
                    }
                }
            }
            Solid::Boolean(bs) => {
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
                        let msg =
                            format!("Failed to tessellate boolean solid '{}': {}", bs.name, e);
                        tracing::warn!("{}", msg);
                        warnings.push(msg);
                    }
                }
            }
            _ => {}
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
        Solid::Trap(s) => tessellate_trap_solid(s, engine),
        Solid::Para(s) => tessellate_para_solid(s, engine),
        Solid::CutTube(s) => tessellate_cut_tube_solid(s, engine, segments),
        Solid::Polyhedra(s) => tessellate_polyhedra_solid(s, engine),
        Solid::Tessellated(s) => tessellate_tessellated_solid(s, engine),
        Solid::Ellipsoid(s) => tessellate_ellipsoid_solid(s, engine, segments),
        Solid::Eltube(s) => tessellate_eltube_solid(s, engine, segments),
        Solid::Tet(s) => tessellate_tet_solid(s, engine),
        Solid::GenericPolycone(s) => tessellate_generic_polycone_solid(s, engine, segments),
        Solid::Hype(s) => tessellate_hype_solid(s, engine, segments),
        Solid::Elcone(s) => tessellate_elcone_solid(s, engine, segments),
        Solid::Paraboloid(s) => tessellate_paraboloid_solid(s, engine, segments),
        Solid::GenericPolyhedra(s) => tessellate_generic_polyhedra_solid(s, engine),
        Solid::Arb8(s) => tessellate_arb8_solid(s, engine),
        Solid::TwistedTubs(s) => tessellate_twisted_tubs_solid(s, engine, segments),
        Solid::TwistedBox(s) => tessellate_twisted_box_solid(s, engine, segments),
        Solid::TwistedTrap(s) => tessellate_twisted_trap_solid(s, engine, segments),
        Solid::TwistedTrd(s) => tessellate_twisted_trd_solid(s, engine, segments),
        Solid::Scaled(_) => Err(anyhow::anyhow!("Scaled solids resolved in phase 2")),
        Solid::Reflected(_) => Err(anyhow::anyhow!("Reflected solids resolved in phase 2")),
        Solid::MultiUnion(_) => Err(anyhow::anyhow!("MultiUnion solids resolved in phase 2")),
        Solid::Boolean(_) => Err(anyhow::anyhow!("Boolean solids resolved in phase 2")),
    }
}

fn tessellate_scaled_solid(
    ss: &ScaledSolidDef,
    solid_map: &HashMap<&str, &Solid>,
    meshes: &mut HashMap<String, TriangleMesh>,
    engine: &EvalEngine,
    segments: u32,
    resolving: &mut HashSet<String>,
) -> Result<TriangleMesh> {
    if let Some(mesh) = meshes.get(&ss.name) {
        return Ok(mesh.clone());
    }

    if !resolving.insert(ss.name.clone()) {
        return Err(anyhow::anyhow!(
            "Cyclic scaled solid dependency detected at '{}'",
            ss.name
        ));
    }

    let result = (|| -> Result<TriangleMesh> {
        let inner_mesh = resolve_operand(
            &ss.solid_ref,
            solid_map,
            meshes,
            engine,
            segments,
            resolving,
        )?;

        // A <scaleref> resolves against the named <scale> defines. Without this
        // the reference fell through to the (1,1,1) defaults and the solid
        // rendered unscaled, with nothing to indicate it.
        let [sx, sy, sz] = match &ss.scale_ref {
            Some(name) => match engine.scale_values.get(name) {
                Some(v) => *v,
                None => {
                    engine.record_warning_public(format!(
                        "scaledSolid \"{}\" references scale \"{}\", which is not defined; \
                         rendering unscaled.",
                        ss.name, name
                    ));
                    [1.0, 1.0, 1.0]
                }
            },
            None => [
                resolve(engine, &ss.scale_x),
                resolve(engine, &ss.scale_y),
                resolve(engine, &ss.scale_z),
            ],
        };

        Ok(scale_mesh(&inner_mesh, sx, sy, sz))
    })();

    resolving.remove(&ss.name);
    result
}

fn scale_mesh(mesh: &TriangleMesh, sx: f64, sy: f64, sz: f64) -> TriangleMesh {
    let mut positions = mesh.positions.clone();
    let mut normals = mesh.normals.clone();
    let n_verts = positions.len() / 3;

    // Guard against a zero scale component: dividing the normal transform by zero
    // would yield NaN normals. The scaled geometry is degenerate at scale 0 either
    // way, but we keep the normals finite by treating the divisor as 1.
    let (dx, dy, dz) = (
        if sx == 0.0 { 1.0 } else { sx },
        if sy == 0.0 { 1.0 } else { sy },
        if sz == 0.0 { 1.0 } else { sz },
    );

    for i in 0..n_verts {
        positions[i * 3] = (positions[i * 3] as f64 * sx) as f32;
        positions[i * 3 + 1] = (positions[i * 3 + 1] as f64 * sy) as f32;
        positions[i * 3 + 2] = (positions[i * 3 + 2] as f64 * sz) as f32;

        // For non-uniform scaling, normals transform as (nx/sx, ny/sy, nz/sz)
        let nx = normals[i * 3] as f64 / dx;
        let ny = normals[i * 3 + 1] as f64 / dy;
        let nz = normals[i * 3 + 2] as f64 / dz;
        let len = (nx * nx + ny * ny + nz * nz).sqrt();
        if len > 1e-12 {
            normals[i * 3] = (nx / len) as f32;
            normals[i * 3 + 1] = (ny / len) as f32;
            normals[i * 3 + 2] = (nz / len) as f32;
        } else {
            // Keep normals unit-length even for degenerate input.
            normals[i * 3] = 0.0;
            normals[i * 3 + 1] = 0.0;
            normals[i * 3 + 2] = 1.0;
        }
    }

    // If any scale factor is negative, winding order flips — reverse triangle winding
    let neg_count = [sx, sy, sz].iter().filter(|&&v| v < 0.0).count();
    let mut indices = mesh.indices.clone();
    if neg_count % 2 == 1 {
        for tri in indices.chunks_exact_mut(3) {
            tri.swap(1, 2);
        }
    }

    TriangleMesh {
        positions,
        normals,
        indices,
    }
}

fn tessellate_multiunion_solid(
    mu: &MultiUnionSolid,
    solid_map: &HashMap<&str, &Solid>,
    meshes: &mut HashMap<String, TriangleMesh>,
    engine: &EvalEngine,
    segments: u32,
    resolving: &mut HashSet<String>,
) -> Result<TriangleMesh> {
    if let Some(mesh) = meshes.get(&mu.name) {
        return Ok(mesh.clone());
    }

    if !resolving.insert(mu.name.clone()) {
        return Err(anyhow::anyhow!(
            "Cyclic multiUnion dependency detected at '{}'",
            mu.name
        ));
    }

    let result = (|| -> Result<TriangleMesh> {
        if mu.nodes.is_empty() {
            return Err(anyhow::anyhow!("MultiUnion '{}' has no nodes", mu.name));
        }

        // Resolve and transform first node
        let first = &mu.nodes[0];
        let mut result_mesh = resolve_operand(
            &first.solid_ref,
            solid_map,
            meshes,
            engine,
            segments,
            resolving,
        )?;
        result_mesh =
            apply_placement_transform(&result_mesh, &first.position, &first.rotation, engine);

        // Iteratively union remaining nodes
        for node in &mu.nodes[1..] {
            let node_mesh = resolve_operand(
                &node.solid_ref,
                solid_map,
                meshes,
                engine,
                segments,
                resolving,
            )?;
            let node_mesh =
                apply_placement_transform(&node_mesh, &node.position, &node.rotation, engine);
            result_mesh = csg::union(&result_mesh, &node_mesh);
        }

        Ok(result_mesh)
    })();

    resolving.remove(&mu.name);
    result
}

fn tessellate_reflected_solid(
    rs: &ReflectedSolidDef,
    solid_map: &HashMap<&str, &Solid>,
    meshes: &mut HashMap<String, TriangleMesh>,
    engine: &EvalEngine,
    segments: u32,
    resolving: &mut HashSet<String>,
) -> Result<TriangleMesh> {
    if let Some(mesh) = meshes.get(&rs.name) {
        return Ok(mesh.clone());
    }

    if !resolving.insert(rs.name.clone()) {
        return Err(anyhow::anyhow!(
            "Cyclic reflected solid dependency detected at '{}'",
            rs.name
        ));
    }

    let result = (|| -> Result<TriangleMesh> {
        let inner_mesh = resolve_operand(
            &rs.solid_ref,
            solid_map,
            meshes,
            engine,
            segments,
            resolving,
        )?;

        let sx = resolve(engine, &rs.sx);
        let sy = resolve(engine, &rs.sy);
        let sz = resolve(engine, &rs.sz);

        let lunit = rs.lunit.as_deref().unwrap_or("mm");
        let aunit = rs.aunit.as_deref().unwrap_or("rad");

        let dx = resolve_with_lunit(engine, &rs.dx, lunit);
        let dy = resolve_with_lunit(engine, &rs.dy, lunit);
        let dz = resolve_with_lunit(engine, &rs.dz, lunit);

        let rx = resolve_with_aunit(engine, &rs.rx, aunit);
        let ry = resolve_with_aunit(engine, &rs.ry, aunit);
        let rz = resolve_with_aunit(engine, &rs.rz, aunit);

        // Apply scale (with reflection via negative values)
        let scaled = scale_mesh(&inner_mesh, sx, sy, sz);
        // Apply rotation + translation
        Ok(csg::transform_mesh(&scaled, [dx, dy, dz], [rx, ry, rz]))
    })();

    resolving.remove(&rs.name);
    result
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
        Solid::Scaled(ss) => {
            let mesh = tessellate_scaled_solid(ss, solid_map, meshes, engine, segments, resolving)?;
            meshes.insert(name.to_string(), mesh.clone());
            Ok(mesh)
        }
        Solid::Reflected(rs) => {
            let mesh =
                tessellate_reflected_solid(rs, solid_map, meshes, engine, segments, resolving)?;
            meshes.insert(name.to_string(), mesh.clone());
            Ok(mesh)
        }
        Solid::MultiUnion(mu) => {
            let mesh =
                tessellate_multiunion_solid(mu, solid_map, meshes, engine, segments, resolving)?;
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
            let unit = p.unit.as_deref().unwrap_or("mm");
            [
                resolve_opt_with_lunit(engine, &p.x, unit),
                resolve_opt_with_lunit(engine, &p.y, unit),
                resolve_opt_with_lunit(engine, &p.z, unit),
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
            let unit = r.unit.as_deref().unwrap_or("rad");
            [
                resolve_opt_with_aunit(engine, &r.x, unit),
                resolve_opt_with_aunit(engine, &r.y, unit),
                resolve_opt_with_aunit(engine, &r.z, unit),
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
///
/// **This deliberately diverges from Geant4 and is left as-is for now.**
/// `G4GDMLReadSolids::BoxRead` does `x = eval.Evaluate(attValue); x *= 0.5 *
/// lunit;` unconditionally, with no knowledge of where the value came from. So
/// for `<quantity name="a" type="length" value="5" unit="cm"/>` used as
/// `<box x="a" lunit="cm"/>`, Geant4 gives a 500 mm box and this gives 50 mm.
/// Two known consequences:
///
/// - sibling attributes of one element can end up in different unit regimes —
///   in `<box x="a" y="20" lunit="cm"/>`, `x` skips the conversion and `y` does
///   not;
/// - length-ness is contagious (`EvalEngine` marks any define referencing a
///   length symbol as one itself), so a dimensionless ratio like
///   `<constant name="k" value="a/b"/>` also loses its `lunit`, where the
///   anti-double-conversion rationale does not apply at all.
///
/// Matching Geant4 exactly would be the principled fix, but it changes how every
/// existing file renders and no shipped sample exercises the difference, so it
/// needs a corpus to validate against first. See
/// `resolve_with_lunit_does_not_double_convert_length_expressions` for the
/// behaviour this preserves.
fn resolve_with_lunit(engine: &EvalEngine, expr: &str, lunit: &str) -> f64 {
    let val = engine.resolve_value(expr);
    if engine.expression_uses_length_symbols(expr) {
        val
    } else {
        // Geant4 resolves lunit through G4UnitDefinition::GetValueOf and raises
        // a FatalException when the category is wrong, so a file naming an
        // unknown unit would not load there at all. Treating it as millimetres
        // renders wrong geometry with nothing else to go on, so say so.
        if units::length_factor(lunit).is_none() {
            engine.record_unit_warning(lunit, "length");
        }
        units::length_to_mm(val, lunit)
    }
}

fn resolve_opt_with_lunit(engine: &EvalEngine, expr: &Option<String>, lunit: &str) -> f64 {
    match expr {
        Some(s) => resolve_with_lunit(engine, s, lunit),
        None => 0.0,
    }
}

/// Resolve an angle expression, applying aunit conversion only for literal values.
/// If the expression references any symbols that are already angle values in
/// radians (converted `type="angle"` quantities), skip the aunit conversion to
/// avoid double-converting. Mirrors `resolve_with_lunit`.
fn resolve_with_aunit(engine: &EvalEngine, expr: &str, aunit: &str) -> f64 {
    let val = engine.resolve_value(expr);
    if engine.expression_uses_angle_symbols(expr) {
        val
    } else {
        if units::angle_factor(aunit).is_none() {
            engine.record_unit_warning(aunit, "angle");
        }
        units::angle_to_rad(val, aunit)
    }
}

fn resolve_opt_with_aunit(engine: &EvalEngine, expr: &Option<String>, aunit: &str) -> f64 {
    match expr {
        Some(s) => resolve_with_aunit(engine, s, aunit),
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
    let startphi = resolve_opt_with_aunit(engine, &s.startphi, aunit);
    let deltaphi = match &s.deltaphi {
        Some(expr) => resolve_with_aunit(engine, expr, aunit),
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
    let startphi = resolve_opt_with_aunit(engine, &s.startphi, aunit);
    let deltaphi = match &s.deltaphi {
        Some(expr) => resolve_with_aunit(engine, expr, aunit),
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
    let startphi = resolve_opt_with_aunit(engine, &s.startphi, aunit);
    let deltaphi = match &s.deltaphi {
        Some(expr) => resolve_with_aunit(engine, expr, aunit),
        None => 2.0 * PI,
    };
    let starttheta = resolve_opt_with_aunit(engine, &s.starttheta, aunit);
    let deltatheta = match &s.deltatheta {
        Some(expr) => resolve_with_aunit(engine, expr, aunit),
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

fn tessellate_arb8_solid(s: &Arb8Solid, engine: &EvalEngine) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let dz = resolve_with_lunit(engine, &s.dz, lunit);
    let vertices: [[f64; 2]; 8] = [
        [
            resolve_with_lunit(engine, &s.v1x, lunit),
            resolve_with_lunit(engine, &s.v1y, lunit),
        ],
        [
            resolve_with_lunit(engine, &s.v2x, lunit),
            resolve_with_lunit(engine, &s.v2y, lunit),
        ],
        [
            resolve_with_lunit(engine, &s.v3x, lunit),
            resolve_with_lunit(engine, &s.v3y, lunit),
        ],
        [
            resolve_with_lunit(engine, &s.v4x, lunit),
            resolve_with_lunit(engine, &s.v4y, lunit),
        ],
        [
            resolve_with_lunit(engine, &s.v5x, lunit),
            resolve_with_lunit(engine, &s.v5y, lunit),
        ],
        [
            resolve_with_lunit(engine, &s.v6x, lunit),
            resolve_with_lunit(engine, &s.v6y, lunit),
        ],
        [
            resolve_with_lunit(engine, &s.v7x, lunit),
            resolve_with_lunit(engine, &s.v7y, lunit),
        ],
        [
            resolve_with_lunit(engine, &s.v8x, lunit),
            resolve_with_lunit(engine, &s.v8y, lunit),
        ],
    ];
    Ok(arb8_mesh::tessellate_arb8(dz, vertices))
}

fn tessellate_twisted_tubs_solid(
    s: &TwistedTubsSolid,
    engine: &EvalEngine,
    segments: u32,
) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let aunit = s.aunit.as_deref().unwrap_or("rad");
    let rmin = resolve_opt_with_lunit(engine, &s.endinnerrad, lunit);
    let rmax = resolve_with_lunit(engine, &s.endouterrad, lunit);
    let zlen = resolve_with_lunit(engine, &s.zlen, lunit);
    let twist_angle = resolve_with_aunit(engine, &s.twistedangle, aunit);
    let phi = match &s.phi {
        Some(expr) => resolve_with_aunit(engine, expr, aunit),
        None => 2.0 * PI,
    };

    // G4GDMLReadSolids offers two parameterisations: end radii with `zlen`, or
    // mid radii with `negativeEndz`/`positiveEndz` when `zlen` is zero. Only the
    // first is modelled, and the parser defaults `zlen` to "0", so a file using
    // the second silently produced a zero-thickness (invisible) solid.
    if zlen.abs() < 1e-12 {
        anyhow::bail!(
            "twistedtubs \"{}\" uses the midinnerrad/negativeEndz/positiveEndz form \
             (zlen = 0), which is not supported. The solid is skipped rather than \
             drawn with zero thickness.",
            s.name
        );
    }

    // The radius is held constant along z, so this is a straight tube segment,
    // not the hyperboloid G4TwistedTubs actually builds -- that the real surface
    // is not a cylinder is clear from the reader offering both `endinnerrad` and
    // `midinnerrad`, which would be redundant otherwise. Note that for a full
    // 2*PI sweep the twist has no geometric effect at all here: rotating a
    // constant-radius circle about its own axis is the identity, so the mesh is
    // bit-for-bit a plain tube. G4TwistedTubs.cc is not among the vendored
    // reference sources, so the exact profile could not be established; warn
    // rather than guess at it.
    engine.record_warning_public(format!(
        "twistedtubs \"{}\" is approximated as a straight tube segment: its \
         inner and outer surfaces are really hyperboloids, so the waist near \
         z=0 is drawn too wide. Dimensions elsewhere are correct.",
        s.name
    ));

    Ok(twisted_tubs_mesh::tessellate_twisted_tubs(
        rmin,
        rmax,
        zlen,
        phi,
        twist_angle,
        segments,
    ))
}

fn tessellate_twisted_box_solid(
    s: &TwistedBoxSolid,
    engine: &EvalEngine,
    segments: u32,
) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let aunit = s.aunit.as_deref().unwrap_or("rad");
    let phi_twist = resolve_with_aunit(engine, &s.phi_twist, aunit);
    let x = resolve_with_lunit(engine, &s.x, lunit);
    let y = resolve_with_lunit(engine, &s.y, lunit);
    let z = resolve_with_lunit(engine, &s.z, lunit);
    Ok(twisted_box_mesh::tessellate_twisted_box(
        phi_twist, x, y, z, segments,
    ))
}

fn tessellate_twisted_trap_solid(
    s: &TwistedTrapSolid,
    engine: &EvalEngine,
    segments: u32,
) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let aunit = s.aunit.as_deref().unwrap_or("rad");
    let phi_twist = resolve_with_aunit(engine, &s.phi_twist, aunit);
    let z = resolve_with_lunit(engine, &s.z, lunit);
    let theta = resolve_with_aunit(engine, &s.theta, aunit);
    let phi_angle = resolve_with_aunit(engine, &s.phi, aunit);
    let y1 = resolve_with_lunit(engine, &s.y1, lunit);
    let x1 = resolve_with_lunit(engine, &s.x1, lunit);
    let x2 = resolve_with_lunit(engine, &s.x2, lunit);
    let y2 = resolve_with_lunit(engine, &s.y2, lunit);
    let x3 = resolve_with_lunit(engine, &s.x3, lunit);
    let x4 = resolve_with_lunit(engine, &s.x4, lunit);
    let alph = resolve_with_aunit(engine, &s.alph, aunit);
    Ok(twisted_trap_mesh::tessellate_twisted_trap(
        phi_twist, z, theta, phi_angle, y1, x1, x2, y2, x3, x4, alph, segments,
    ))
}

fn tessellate_twisted_trd_solid(
    s: &TwistedTrdSolid,
    engine: &EvalEngine,
    segments: u32,
) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let aunit = s.aunit.as_deref().unwrap_or("rad");
    let phi_twist = resolve_with_aunit(engine, &s.phi_twist, aunit);
    let x1 = resolve_with_lunit(engine, &s.x1, lunit);
    let x2 = resolve_with_lunit(engine, &s.x2, lunit);
    let y1 = resolve_with_lunit(engine, &s.y1, lunit);
    let y2 = resolve_with_lunit(engine, &s.y2, lunit);
    let z = resolve_with_lunit(engine, &s.z, lunit);
    // TwistedTrd is TwistedTrap with theta=0, phi=0, alph=0, x1=x2=trd.x1, x3=x4=trd.x2
    Ok(twisted_trap_mesh::tessellate_twisted_trap(
        phi_twist, z, 0.0, 0.0, y1, x1, x1, y2, x2, x2, 0.0, segments,
    ))
}

fn tessellate_tessellated_solid(s: &TessellatedSolid, engine: &EvalEngine) -> Result<TriangleMesh> {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    let lookup =
        |name: &str| -> Result<[f64; 3]> {
            engine.position_values.get(name).copied().ok_or_else(|| {
                anyhow::anyhow!("Tessellated vertex '{}' not found in defines", name)
            })
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

fn tessellate_polyhedra_solid(s: &PolyhedraSolid, engine: &EvalEngine) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let aunit = s.aunit.as_deref().unwrap_or("rad");
    let startphi = resolve_opt_with_aunit(engine, &s.startphi, aunit);
    let deltaphi = match &s.deltaphi {
        Some(expr) => resolve_with_aunit(engine, expr, aunit),
        None => 2.0 * PI,
    };
    // Clamp sides: 0/1/2 are degenerate and an unbounded value blows up memory.
    let numsides = (resolve(engine, &s.numsides) as u32).clamp(3, 512);

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

    Ok(polyhedra_mesh::tessellate_polyhedra(
        &planes, startphi, deltaphi, numsides,
    ))
}

fn tessellate_cut_tube_solid(
    s: &CutTubeSolid,
    engine: &EvalEngine,
    segments: u32,
) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let aunit = s.aunit.as_deref().unwrap_or("rad");
    let rmin = resolve_opt_with_lunit(engine, &s.rmin, lunit);
    let rmax = resolve_with_lunit(engine, &s.rmax, lunit);
    let z = resolve_with_lunit(engine, &s.z, lunit);
    let startphi = resolve_opt_with_aunit(engine, &s.startphi, aunit);
    let deltaphi = match &s.deltaphi {
        Some(expr) => resolve_with_aunit(engine, expr, aunit),
        None => 2.0 * PI,
    };
    let low_norm = [
        resolve_opt(engine, &s.low_x),
        resolve_opt(engine, &s.low_y),
        match &s.low_z {
            Some(expr) => resolve(engine, expr),
            None => -1.0,
        },
    ];
    let high_norm = [
        resolve_opt(engine, &s.high_x),
        resolve_opt(engine, &s.high_y),
        match &s.high_z {
            Some(expr) => resolve(engine, expr),
            None => 1.0,
        },
    ];
    Ok(cut_tube_mesh::tessellate_cut_tube(
        rmin, rmax, z, startphi, deltaphi, low_norm, high_norm, segments,
    ))
}

fn tessellate_para_solid(s: &ParaSolid, engine: &EvalEngine) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let aunit = s.aunit.as_deref().unwrap_or("rad");
    let x = resolve_with_lunit(engine, &s.x, lunit);
    let y = resolve_with_lunit(engine, &s.y, lunit);
    let z = resolve_with_lunit(engine, &s.z, lunit);
    let alpha = resolve_opt_with_aunit(engine, &s.alpha, aunit);
    let theta = resolve_opt_with_aunit(engine, &s.theta, aunit);
    let phi = resolve_opt_with_aunit(engine, &s.phi, aunit);
    // Para is Trap with uniform x/y dimensions
    Ok(trap_mesh::tessellate_trap(
        z, theta, phi, y, x, x, alpha, y, x, x, alpha,
    ))
}

fn tessellate_trap_solid(s: &TrapSolid, engine: &EvalEngine) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let aunit = s.aunit.as_deref().unwrap_or("rad");
    let z = resolve_with_lunit(engine, &s.z, lunit);
    let theta = resolve_opt_with_aunit(engine, &s.theta, aunit);
    let phi = resolve_opt_with_aunit(engine, &s.phi, aunit);
    let y1 = resolve_with_lunit(engine, &s.y1, lunit);
    let x1 = resolve_with_lunit(engine, &s.x1, lunit);
    let x2 = resolve_with_lunit(engine, &s.x2, lunit);
    let alpha1 = resolve_opt_with_aunit(engine, &s.alpha1, aunit);
    let y2 = resolve_with_lunit(engine, &s.y2, lunit);
    let x3 = resolve_with_lunit(engine, &s.x3, lunit);
    let x4 = resolve_with_lunit(engine, &s.x4, lunit);
    let alpha2 = resolve_opt_with_aunit(engine, &s.alpha2, aunit);
    Ok(trap_mesh::tessellate_trap(
        z, theta, phi, y1, x1, x2, alpha1, y2, x3, x4, alpha2,
    ))
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
    let startphi = resolve_opt_with_aunit(engine, &s.startphi, aunit);
    let deltaphi = match &s.deltaphi {
        Some(expr) => resolve_with_aunit(engine, expr, aunit),
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
        0.0,
        r,
        0.0,
        2.0 * PI,
        0.0,
        PI,
        segments,
    ))
}

fn tessellate_ellipsoid_solid(
    s: &EllipsoidSolid,
    engine: &EvalEngine,
    segments: u32,
) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let ax = resolve_with_lunit(engine, &s.ax, lunit);
    let by = resolve_with_lunit(engine, &s.by, lunit);
    let cz = resolve_with_lunit(engine, &s.cz, lunit);
    // When zcut is absent (None), default to full extent (-cz / +cz)
    let zcut1 = match &s.zcut1 {
        Some(expr) => resolve_with_lunit(engine, expr, lunit),
        None => -cz,
    };
    let zcut2 = match &s.zcut2 {
        Some(expr) => resolve_with_lunit(engine, expr, lunit),
        None => cz,
    };
    Ok(ellipsoid_mesh::tessellate_ellipsoid(
        ax, by, cz, zcut1, zcut2, segments,
    ))
}

fn tessellate_eltube_solid(
    s: &EltubeSolid,
    engine: &EvalEngine,
    segments: u32,
) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let dx = resolve_with_lunit(engine, &s.dx, lunit);
    let dy = resolve_with_lunit(engine, &s.dy, lunit);
    let dz = resolve_with_lunit(engine, &s.dz, lunit);
    Ok(eltube_mesh::tessellate_eltube(dx, dy, dz, segments))
}

fn tessellate_tet_solid(s: &TetSolid, engine: &EvalEngine) -> Result<TriangleMesh> {
    let lookup = |name: &str| -> Result<[f64; 3]> {
        engine
            .position_values
            .get(name)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("Tet vertex '{}' not found in defines", name))
    };

    let v1 = lookup(&s.vertex1)?;
    let v2 = lookup(&s.vertex2)?;
    let v3 = lookup(&s.vertex3)?;
    let v4 = lookup(&s.vertex4)?;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // Helper to compute face normal and emit a triangle
    let mut add_face = |a: [f64; 3], b: [f64; 3], c: [f64; 3]| {
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
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
        for v in &[a, b, c] {
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
    };

    // 4 faces of the tetrahedron with consistent outward winding
    // Compute centroid to orient normals outward
    let cx = (v1[0] + v2[0] + v3[0] + v4[0]) / 4.0;
    let cy = (v1[1] + v2[1] + v3[1] + v4[1]) / 4.0;
    let cz = (v1[2] + v2[2] + v3[2] + v4[2]) / 4.0;

    let faces: [[usize; 3]; 4] = [[0, 1, 2], [0, 2, 3], [0, 3, 1], [1, 3, 2]];
    let verts = [v1, v2, v3, v4];

    for face in &faces {
        let a = verts[face[0]];
        let b = verts[face[1]];
        let c = verts[face[2]];

        // Check if normal points away from centroid; if not, flip winding
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let nx = e1[1] * e2[2] - e1[2] * e2[1];
        let ny = e1[2] * e2[0] - e1[0] * e2[2];
        let nz = e1[0] * e2[1] - e1[1] * e2[0];

        // Dot with vector from centroid to face
        let to_face = [a[0] - cx, a[1] - cy, a[2] - cz];
        let dot = nx * to_face[0] + ny * to_face[1] + nz * to_face[2];

        if dot >= 0.0 {
            add_face(a, b, c);
        } else {
            add_face(a, c, b);
        }
    }

    Ok(TriangleMesh {
        positions,
        normals,
        indices,
    })
}

fn tessellate_polycone_solid(
    s: &PolyconeSolid,
    engine: &EvalEngine,
    segments: u32,
) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let aunit = s.aunit.as_deref().unwrap_or("rad");
    let startphi = resolve_opt_with_aunit(engine, &s.startphi, aunit);
    let deltaphi = match &s.deltaphi {
        Some(expr) => resolve_with_aunit(engine, expr, aunit),
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

fn tessellate_generic_polycone_solid(
    s: &GenericPolyconeSolid,
    engine: &EvalEngine,
    segments: u32,
) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let aunit = s.aunit.as_deref().unwrap_or("rad");
    let startphi = resolve_opt_with_aunit(engine, &s.startphi, aunit);
    let deltaphi = match &s.deltaphi {
        Some(expr) => resolve_with_aunit(engine, expr, aunit),
        None => 2.0 * PI,
    };

    // The rzpoints are a closed contour in the (r,z) half-plane, revolved about
    // z -- not a list of z-planes.
    let contour: Vec<(f64, f64)> = s
        .rzpoints
        .iter()
        .map(|rz| {
            (
                resolve_with_lunit(engine, &rz.r, lunit),
                resolve_with_lunit(engine, &rz.z, lunit),
            )
        })
        .collect();

    Ok(generic_polycone_mesh::tessellate_generic_polycone(
        &contour, startphi, deltaphi, segments, None,
    ))
}

fn tessellate_hype_solid(
    s: &HypeSolid,
    engine: &EvalEngine,
    segments: u32,
) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let aunit = s.aunit.as_deref().unwrap_or("rad");
    let rmin = resolve_opt_with_lunit(engine, &s.rmin, lunit);
    let rmax = resolve_with_lunit(engine, &s.rmax, lunit);
    let inst = resolve_opt_with_aunit(engine, &s.inst, aunit);
    let outst = resolve_opt_with_aunit(engine, &s.outst, aunit);
    let z = resolve_with_lunit(engine, &s.z, lunit);
    let hz = z * 0.5; // Geant4 convention: z is full length, halved for constructor
    Ok(hype_mesh::tessellate_hype(
        rmin, rmax, inst, outst, hz, segments,
    ))
}

fn tessellate_elcone_solid(
    s: &ElconeSolid,
    engine: &EvalEngine,
    segments: u32,
) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    // dx, dy are dimensionless ratios — NOT scaled by lunit
    let dx = resolve(engine, &s.dx);
    let dy = resolve(engine, &s.dy);
    let zmax = resolve_with_lunit(engine, &s.zmax, lunit);
    let zcut = resolve_with_lunit(engine, &s.zcut, lunit);
    Ok(elcone_mesh::tessellate_elcone(dx, dy, zmax, zcut, segments))
}

fn tessellate_paraboloid_solid(
    s: &ParaboloidSolid,
    engine: &EvalEngine,
    segments: u32,
) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let rlo = resolve_with_lunit(engine, &s.rlo, lunit);
    let rhi = resolve_with_lunit(engine, &s.rhi, lunit);
    let dz = resolve_with_lunit(engine, &s.dz, lunit);
    Ok(paraboloid_mesh::tessellate_paraboloid(
        rlo, rhi, dz, segments,
    ))
}

fn tessellate_generic_polyhedra_solid(
    s: &GenericPolyhedraSolid,
    engine: &EvalEngine,
) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let aunit = s.aunit.as_deref().unwrap_or("rad");
    let startphi = resolve_opt_with_aunit(engine, &s.startphi, aunit);
    let deltaphi = match &s.deltaphi {
        Some(expr) => resolve_with_aunit(engine, expr, aunit),
        None => 2.0 * PI,
    };
    // Clamp sides: 0/1/2 are degenerate and an unbounded value blows up memory.
    let numsides = (resolve(engine, &s.numsides) as u32).clamp(3, 512);

    // As with genericPolycone, the rzpoints are a closed contour, not z-planes.
    // The apothem -> corner radius conversion is kept: whether the generic form
    // should skip it (its Geant4 constructor takes different arguments) could
    // not be established from the vendored sources, so the existing behaviour
    // stands until it can be.
    let contour: Vec<(f64, f64)> = s
        .rzpoints
        .iter()
        .map(|rz| {
            (
                resolve_with_lunit(engine, &rz.r, lunit),
                resolve_with_lunit(engine, &rz.z, lunit),
            )
        })
        .collect();

    Ok(generic_polycone_mesh::tessellate_generic_polycone(
        &contour,
        startphi,
        deltaphi,
        numsides,
        Some(numsides),
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
    fn resolve_with_aunit_does_not_double_convert_angle_expressions() {
        let mut engine = EvalEngine::new();
        let mut defines = DefineSection::default();
        defines.quantities.push(Quantity {
            name: "ang".to_string(),
            r#type: Some("angle".to_string()),
            value: "90".to_string(),
            unit: Some("deg".to_string()),
        });
        engine.evaluate_all(&defines).unwrap();

        // The quantity is converted to radians at definition time, so it must
        // come through unchanged regardless of the solid's aunit.
        let half_pi = std::f64::consts::FRAC_PI_2;
        assert!((resolve_with_aunit(&engine, "ang", "rad") - half_pi).abs() < 1e-9);
        assert!((resolve_with_aunit(&engine, "ang", "deg") - half_pi).abs() < 1e-9);

        // Literal values still respect the solid's aunit.
        assert!((resolve_with_aunit(&engine, "90", "deg") - half_pi).abs() < 1e-9);
        assert!((resolve_with_aunit(&engine, "0.5", "rad") - 0.5).abs() < 1e-9);
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
