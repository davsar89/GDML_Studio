use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

use super::csg;
use super::primitives::{
    arb8_mesh, box_mesh, cone_mesh, cut_tube_mesh, elcone_mesh, ellipsoid_mesh, eltube_mesh,
    face_normal, generic_polycone_mesh, hype_mesh, paraboloid_mesh, polycone_mesh, polyhedra_mesh,
    sphere_mesh, torus_mesh, trap_mesh, trd_mesh, tube_mesh, twisted_trap_mesh, twisted_tubs_mesh,
    xtru_mesh,
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
            _ => match tessellate_primitive(solid, engine, segments) {
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

    // Composite roots and nested operands use the same dispatch. Keep the
    // primitive pass so large standalone meshes enter the cache without a copy.
    for solid in &solids.solids {
        if !matches!(
            solid,
            Solid::Boolean(_) | Solid::Scaled(_) | Solid::Reflected(_) | Solid::MultiUnion(_)
        ) {
            continue;
        }
        match tessellate_solid(
            solid,
            &solid_map,
            &mut meshes,
            engine,
            segments,
            &mut HashSet::new(),
        ) {
            Ok(mesh) => {
                meshes.insert(solid.name().to_string(), mesh);
            }
            Err(e) => {
                let msg = format!("Failed to tessellate solid '{}': {}", solid.name(), e);
                tracing::warn!("{}", msg);
                warnings.push(msg);
            }
        }
    }

    Ok((meshes, warnings))
}

fn tessellate_primitive(solid: &Solid, engine: &EvalEngine, segments: u32) -> Result<TriangleMesh> {
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
            Some(name) => *engine
                .scale_values
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("Undefined scale '{name}'"))?,
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
            apply_placement_transform(&result_mesh, &first.position, &first.rotation, engine)?;

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
                apply_placement_transform(&node_mesh, &node.position, &node.rotation, engine)?;
            result_mesh = csg::boolean(&result_mesh, &node_mesh, &BooleanOp::Union);
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
            apply_placement_transform(&first_mesh, &bs.first_position, &bs.first_rotation, engine)?;

        // Apply second solid transform (position/rotation of second relative to first)
        let second_mesh =
            apply_placement_transform(&second_mesh, &bs.position, &bs.rotation, engine)?;

        Ok(csg::boolean(&first_mesh, &second_mesh, &bs.operation))
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

    let mesh = tessellate_solid(solid, solid_map, meshes, engine, segments, resolving)?;
    meshes.insert(name.to_string(), mesh.clone());
    Ok(mesh)
}

fn tessellate_solid(
    solid: &Solid,
    solid_map: &HashMap<&str, &Solid>,
    meshes: &mut HashMap<String, TriangleMesh>,
    engine: &EvalEngine,
    segments: u32,
    resolving: &mut HashSet<String>,
) -> Result<TriangleMesh> {
    match solid {
        Solid::Boolean(s) => {
            tessellate_boolean_solid(s, solid_map, meshes, engine, segments, resolving)
        }
        Solid::Scaled(s) => {
            tessellate_scaled_solid(s, solid_map, meshes, engine, segments, resolving)
        }
        Solid::Reflected(s) => {
            tessellate_reflected_solid(s, solid_map, meshes, engine, segments, resolving)
        }
        Solid::MultiUnion(s) => {
            tessellate_multiunion_solid(s, solid_map, meshes, engine, segments, resolving)
        }
        _ => tessellate_primitive(solid, engine, segments),
    }
}

fn apply_placement_transform(
    mesh: &TriangleMesh,
    pos: &Option<PlacementPos>,
    rot: &Option<PlacementRot>,
    engine: &EvalEngine,
) -> Result<TriangleMesh> {
    Ok(csg::transform_mesh(
        mesh,
        engine.resolve_position(pos)?,
        engine.resolve_rotation(rot)?,
    ))
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

/// mm per unit of a declared `lunit`; an unknown one warns and counts as mm.
fn lunit_factor(engine: &EvalEngine, lunit: &str) -> f64 {
    units::length_factor(lunit).unwrap_or_else(|| {
        engine.record_unit_warning(lunit, "length");
        1.0
    })
}

/// Geant4 applies lunit after evaluation, including expressions using quantities.
fn resolve_with_lunit(engine: &EvalEngine, expr: &str, lunit: &str) -> f64 {
    engine.resolve_value(expr) * lunit_factor(engine, lunit)
}

fn resolve_opt_with_lunit(engine: &EvalEngine, expr: &Option<String>, lunit: &str) -> f64 {
    match expr {
        Some(s) => resolve_with_lunit(engine, s, lunit),
        None => 0.0,
    }
}

/// The declared angle unit is also independent of the expression's symbols.
fn resolve_with_aunit(engine: &EvalEngine, expr: &str, aunit: &str) -> f64 {
    if units::angle_factor(aunit).is_none() {
        engine.record_unit_warning(aunit, "angle");
    }
    units::angle_to_rad(engine.resolve_value(expr), aunit)
}

fn resolve_opt_with_aunit(engine: &EvalEngine, expr: &Option<String>, aunit: &str) -> f64 {
    match expr {
        Some(s) => resolve_with_aunit(engine, s, aunit),
        None => 0.0,
    }
}

/// Resolve a `deltaphi` opening angle under Geant4's full-circle rule.
///
/// `G4Polycone.cc:232` treats an opening angle that is non-positive, or at or
/// above a full turn, as "nonsense value as representing no phi opening" and
/// sweeps the complete revolution:
///
/// ```text
/// if ( (phiTotal <= 0) || (phiTotal > twopi*(1-DBL_EPSILON)) )
/// { phiIsOpen = false; startPhi = 0.; endPhi = twopi; }
/// ```
///
/// The 2*pi default used to be applied only when the attribute was absent, so
/// an explicit `deltaphi="0"` -- legal GDML, and a solid object in Geant4 --
/// reached the meshers as a zero-width sweep. `polycone_mesh`, `polyhedra_mesh`
/// and `generic_polycone_mesh` already normalised it themselves; tube, cone,
/// sphere, torus, cuttube and twistedtubs did not, and tessellated to nothing.
/// Doing it here covers every phi-swept solid from one place.
///
/// Geant4 also forces `startPhi` to 0 inside that branch. That is deliberately
/// not reproduced: for the revolved solids it is geometrically inert (a full
/// turn sweeps the same surface wherever it starts), and for the faceted ones
/// -- polyhedra, genericPolyhedra -- it would rotate the facets to a different
/// orientation, which `G4Polyhedra.cc` is not vendored to confirm.
///
/// The rule is quoted from the one vendored source that states it. Geant4 puts
/// the other solids through an equivalent phi check, so applying it beyond
/// polycone is consistent but inferred rather than proven.
fn resolve_delta_phi(engine: &EvalEngine, expr: &Option<String>, aunit: &str) -> f64 {
    const TWO_PI: f64 = 2.0 * PI;
    let raw = match expr {
        Some(e) => resolve_with_aunit(engine, e, aunit),
        None => TWO_PI,
    };
    // A non-finite angle is nonsense by the same reading, and would otherwise
    // propagate NaN through every vertex of the mesh.
    if !raw.is_finite() || raw <= 0.0 || raw > TWO_PI * (1.0 - f64::EPSILON) {
        TWO_PI
    } else {
        raw
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
    let deltaphi = resolve_delta_phi(engine, &s.deltaphi, aunit);
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
    let deltaphi = resolve_delta_phi(engine, &s.deltaphi, aunit);
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
    let deltaphi = resolve_delta_phi(engine, &s.deltaphi, aunit);
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
    let twist_angle = resolve_with_aunit(engine, &s.twistedangle, aunit);

    // `fDPhi = totphi / nseg` when segmented, else the plain `phi`
    // (G4TwistedTubs.cc:129, G4GDMLReadSolids.cc:3330).
    let nseg = s
        .nseg
        .as_ref()
        .map(|e| engine.resolve_value(e))
        .unwrap_or(0.0);
    let deltaphi = if nseg >= 1.0 {
        match &s.totphi {
            Some(e) => resolve_with_aunit(engine, e, aunit) / nseg,
            None => 2.0 * PI / nseg,
        }
    } else {
        match &s.phi {
            Some(e) => resolve_with_aunit(engine, e, aunit),
            None => 2.0 * PI,
        }
    };

    // Two parameterisations, selected by `zlen` being non-zero. `zlen` is the
    // FULL length: the reader halves it (`zlen *= 0.5 * lunit`,
    // G4GDMLReadSolids.cc:3323) before passing it as `halfzlen`, so the solid
    // spans [-zlen/2, +zlen/2].
    let half = 0.5 * resolve_opt_with_lunit(engine, &s.zlen, lunit);
    let (rmin_mid, rmax_mid, z_neg, z_pos) = if half.abs() > 1e-12 {
        // End-radius form. The constructor converts to waist radii via
        // sqrt(rEnd^2 - (rEnd*sin(twist/2))^2), i.e. rEnd*|cos(twist/2)|
        // (G4TwistedTubs.cc:118-127).
        let cos_half = (0.5 * twist_angle).cos().abs();
        let r_in_end = resolve_opt_with_lunit(engine, &s.endinnerrad, lunit);
        let r_out_end = resolve_opt_with_lunit(engine, &s.endouterrad, lunit);
        (r_in_end * cos_half, r_out_end * cos_half, -half, half)
    } else {
        // Mid-radius form: radii are already the waist values and the z bounds
        // are given outright, so they need not be symmetric about the origin.
        let r_in = resolve_opt_with_lunit(engine, &s.midinnerrad, lunit);
        let r_out = resolve_opt_with_lunit(engine, &s.midouterrad, lunit);
        let z_neg = resolve_opt_with_lunit(engine, &s.negative_endz, lunit);
        let z_pos = resolve_opt_with_lunit(engine, &s.positive_endz, lunit);
        (r_in, r_out, z_neg, z_pos)
    };

    if (z_pos - z_neg).abs() < 1e-12 || rmax_mid <= 1e-12 {
        anyhow::bail!(
            "twistedtubs \"{}\" has no extent (outer radius {} mm over z {}..{} mm);              check that either zlen or midouterrad/negativeEndz/positiveEndz is set.",
            s.name,
            rmax_mid,
            z_neg,
            z_pos
        );
    }

    Ok(twisted_tubs_mesh::tessellate_twisted_tubs(
        rmin_mid,
        rmax_mid,
        z_neg,
        z_pos,
        deltaphi,
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
    // G4TwistedBox is G4VTwistedFaceted(phi, dz, 0, 0, dy, dx, dx, dy, dx, dx, 0)
    Ok(twisted_trap_mesh::tessellate_twisted_trap(
        phi_twist, z, 0.0, 0.0, y, x, x, y, x, x, 0.0, segments,
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

/// Append a planar convex polygon as a flat-shaded triangle fan.
fn push_polygon(mesh: &mut TriangleMesh, p: &[[f32; 3]]) {
    let n = face_normal(p[0], p[1], p[2]);
    let base = (mesh.positions.len() / 3) as u32;
    for q in p {
        mesh.positions.extend_from_slice(q);
        mesh.normals.extend_from_slice(&n);
    }
    for i in 1..p.len() as u32 - 1 {
        mesh.indices
            .extend_from_slice(&[base, base + i, base + i + 1]);
    }
}

/// Named positions scaled by a solid's or facet's own `lunit`: Geant4 reads
/// them as `GetPosition(..) * lunit` (G4GDMLReadSolids.cc TriangularRead,
/// QuadrangularRead, TetRead), even though the positions carry units too.
fn scaled_positions(
    engine: &EvalEngine,
    names: &[&str],
    lunit: Option<&str>,
) -> Result<Vec<[f64; 3]>> {
    let k = lunit_factor(engine, lunit.unwrap_or("mm"));
    names
        .iter()
        .map(|n| Ok(engine.named_position(n)?.map(|c| c * k)))
        .collect()
}

fn tessellate_tessellated_solid(s: &TessellatedSolid, engine: &EvalEngine) -> Result<TriangleMesh> {
    let mut mesh = TriangleMesh::new();
    for facet in &s.facets {
        let (_, names, r#type, lunit) = facet.parts();
        let mut v = scaled_positions(engine, &names, lunit)?;
        // RELATIVE: every vertex after the first is an offset from it
        // (G4TriangularFacet / G4QuadrangularFacet).
        if r#type == Some("RELATIVE") {
            let v0 = v[0];
            for p in &mut v[1..] {
                *p = [p[0] + v0[0], p[1] + v0[1], p[2] + v0[2]];
            }
        }
        let p: Vec<[f32; 3]> = v.iter().map(|q| q.map(|c| c as f32)).collect();
        push_polygon(&mut mesh, &p);
    }
    Ok(mesh)
}

fn tessellate_polyhedra_solid(s: &PolyhedraSolid, engine: &EvalEngine) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let aunit = s.aunit.as_deref().unwrap_or("rad");
    let startphi = resolve_opt_with_aunit(engine, &s.startphi, aunit);
    let deltaphi = resolve_delta_phi(engine, &s.deltaphi, aunit);
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
    let deltaphi = resolve_delta_phi(engine, &s.deltaphi, aunit);
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
    let deltaphi = resolve_delta_phi(engine, &s.deltaphi, aunit);
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
    let names = [&s.vertex1, &s.vertex2, &s.vertex3, &s.vertex4].map(|n| n.as_str());
    let v: Vec<[f32; 3]> = scaled_positions(engine, &names, s.lunit.as_deref())?
        .iter()
        .map(|q| q.map(|c| c as f32))
        .collect();
    let centroid = [0, 1, 2].map(|k| v.iter().map(|p| p[k]).sum::<f32>() / 4.0);

    // Each face wound so its normal points away from the centroid.
    let mut mesh = TriangleMesh::new();
    for [a, b, c] in [[0, 1, 2], [0, 2, 3], [0, 3, 1], [1, 3, 2]].map(|f| f.map(|i| v[i])) {
        let n = face_normal(a, b, c);
        let out = (0..3).map(|k| n[k] * (a[k] - centroid[k])).sum::<f32>() >= 0.0;
        push_polygon(&mut mesh, &if out { [a, b, c] } else { [a, c, b] });
    }
    Ok(mesh)
}

fn tessellate_polycone_solid(
    s: &PolyconeSolid,
    engine: &EvalEngine,
    segments: u32,
) -> Result<TriangleMesh> {
    let lunit = s.lunit.as_deref().unwrap_or("mm");
    let aunit = s.aunit.as_deref().unwrap_or("rad");
    let startphi = resolve_opt_with_aunit(engine, &s.startphi, aunit);
    let deltaphi = resolve_delta_phi(engine, &s.deltaphi, aunit);

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
    let deltaphi = resolve_delta_phi(engine, &s.deltaphi, aunit);

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
    let deltaphi = resolve_delta_phi(engine, &s.deltaphi, aunit);
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
    fn declared_lunit_applies_to_quantities_and_literals() {
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

        // Geant4 applies the solid's lunit after evaluating A + B.
        let expr_val = resolve_with_lunit(&engine, "A + B", "cm");
        assert!((expr_val - 500.0).abs() < 1e-9);

        // Literal values still respect the solid's lunit.
        let literal_val = resolve_with_lunit(&engine, "2.0", "cm");
        assert!((literal_val - 20.0).abs() < 1e-9);
    }

    #[test]
    fn declared_aunit_applies_to_quantities_and_literals() {
        let mut engine = EvalEngine::new();
        let mut defines = DefineSection::default();
        defines.quantities.push(Quantity {
            name: "ang".to_string(),
            r#type: Some("angle".to_string()),
            value: "90".to_string(),
            unit: Some("deg".to_string()),
        });
        engine.evaluate_all(&defines).unwrap();

        // The quantity's unit and the attribute's unit are separate operations.
        let half_pi = std::f64::consts::FRAC_PI_2;
        assert!((resolve_with_aunit(&engine, "ang", "rad") - half_pi).abs() < 1e-9);
        assert!((resolve_with_aunit(&engine, "ang", "deg") - half_pi.to_radians()).abs() < 1e-9);

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
