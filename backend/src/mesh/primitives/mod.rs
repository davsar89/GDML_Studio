// Tessellators take one parameter per GDML attribute of the solid they build —
// a trap has 11, a twisted trap 12 — and the emit helpers thread the three
// output buffers plus a vertex quad. Grouping them into structs would only
// rename the arguments, so the lint is turned off for this module rather than
// silenced function by function.
#![allow(clippy::too_many_arguments)]

pub mod arb8_mesh;
pub mod box_mesh;
pub mod cone_mesh;
pub mod cut_tube_mesh;
pub mod elcone_mesh;
pub mod ellipsoid_mesh;
pub mod eltube_mesh;
pub mod generic_polycone_mesh;
pub mod hype_mesh;
pub mod paraboloid_mesh;
pub mod polyhedra_mesh;
pub mod polycone_mesh;
pub mod sphere_mesh;
pub mod torus_mesh;
pub mod trap_mesh;
pub mod trd_mesh;
pub mod tube_mesh;
pub mod twisted_box_mesh;
pub mod twisted_trap_mesh;
pub mod twisted_tubs_mesh;
pub mod xtru_mesh;
