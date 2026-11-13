# CAD import to GDML: investigation brief

Context for planning STEP/STL (and a few related formats) import into GDML Studio.
Facts about the codebase were checked on 2026-11-13 at commit `f736e31`; re-check
anything you rely on, since the code moves.

## 1. Goal

A user opens a STEP or STL file and gets a Geant4-valid GDML geometry that they can
inspect, assign materials to, and save with the existing GDML export. Each CAD body
becomes the most faithful representation that can be shown to be correct:

1. a native GDML primitive (box, tube, cone, sphere, torus, polycone, xtru, ...),
   or a small CSG tree of them, when a heuristic recognizes it **and** a check
   against the exact CAD geometry confirms it;
2. otherwise a `<tessellated>` solid.

The task at hand is an evidence-backed plan (`doc/cad-import/PLAN.md`), not the
finished feature.

## 2. Formats in scope

| Format | Kind | Notes |
|---|---|---|
| STEP `.step` `.stp` (AP203/AP214/AP242) | exact B-rep, assemblies, units, names, colors | primary |
| STL `.stl` (binary and ASCII) | triangle soup | primary |
| IGES `.igs` `.iges` | exact B-rep, weak assembly support | nearly free once a STEP reader built on a CAD kernel exists |
| OBJ `.obj` (maybe PLY or 3MF) | mesh | nearly free once the STL path exists; 3MF carries units and objects |

Keep the list short. Add a format only if the marginal cost is small and you can
name who needs it. Simulation-geometry formats (ROOT TGeo, Geant4 text geometry
`.tg`, FLUKA, MCNP) are a different problem with existing converters: out of scope
unless one turns out to be nearly free, and then argue for it in one paragraph.

### What STL carries (and lacks)

- No units, no topology, no assembly, no materials, no exact surfaces. Names only
  via ASCII `solid <name>`. ASCII files may hold several `solid` blocks. A binary
  header may itself start with `solid`; tell the formats apart by size
  (`84 + 50 n` bytes).
- Usual defects: unwelded vertices, cracks, non-manifold edges, inconsistent winding,
  stored normals that disagree with the winding (ignore them), degenerate triangles,
  self-intersections, several shells per file (separate bodies, or a body with an
  internal cavity).
- Units must come from the user (default mm); show the resulting bounding box so
  a wrong guess (inch, m) is obvious at a glance.
- Primitive recognition on a mesh means fitting an approximation of an
  approximation. Planar polyhedra (box, trd, trap, arb8) can be matched exactly;
  curved fits need a strict tolerance or should be skipped. Decide whether it is
  worth doing at all.

### What STEP carries

- Exact B-rep: `MANIFOLD_SOLID_BREP`, `BREP_WITH_VOIDS`, `CLOSED_SHELL`,
  `ADVANCED_FACE` on `PLANE`, `CYLINDRICAL_SURFACE`, `CONICAL_SURFACE`,
  `SPHERICAL_SURFACE`, `TOROIDAL_SURFACE`, `SURFACE_OF_REVOLUTION`,
  `SURFACE_OF_LINEAR_EXTRUSION`, `B_SPLINE_SURFACE_WITH_KNOTS` (and rational forms),
  bounded by edge loops of trimmed curves. Open shells (`SHELL_BASED_SURFACE_MODEL`)
  are not solids.
- Units in `GLOBAL_UNIT_ASSIGNED_CONTEXT` (`SI_UNIT` with a prefix, or
  `CONVERSION_BASED_UNIT` such as inch), plus `UNCERTAINTY_MEASURE_WITH_UNIT`, the
  modelling tolerance that defines what "coincident" means for that file.
- Assemblies: `PRODUCT_DEFINITION` per part, `NEXT_ASSEMBLY_USAGE_OCCURRENCE` per
  instance, placement via `CONTEXT_DEPENDENT_SHAPE_REPRESENTATION` and
  `ITEM_DEFINED_TRANSFORMATION`. One part definition can be instanced many times.
  Colors via `STYLED_ITEM`. Materials are rarely present (AP242 can carry a
  material designation and density).
- Real-world quirks: periodic faces split at seams (a cylinder as two half faces),
  one analytic surface split into many faces, sliver faces, fillets and chamfers
  everywhere, threads as helical B-spline sweeps, tolerance violations, several
  bodies per part, mirrored instances (determinant -1).
- Robust trimmed-surface tessellation, healing, volume properties and booleans need
  a mature geometry kernel. In practice that means OpenCASCADE (OCCT, LGPL-2.1 with
  an exception). Check the current state of alternatives yourself (Section 6).

## 3. GDML Studio today

| Layer | Path | Role |
|---|---|---|
| Backend | `backend/` (Rust, Axum) | parse, evaluate, tessellate, edit, serialize |
| Frontend | `frontend/` (React, React Three Fiber, Zustand) | viewer, tree, material editor |
| Desktop | `desktop/` (Electron) | starts the backend on a free port and shows the frontend it serves |

- Ships as a Windows installer and an Ubuntu 20.04+ `.deb` that bundle everything;
  the user installs no Rust, Node or Python. The Linux backend is a **static musl**
  binary. A browser mode (`run.sh`, Vite proxy) is the dev setup and must keep working.
- It is a viewer plus material editor: solids and volumes cannot be edited in the UI.
- **Open:** `frontend/src/components/Toolbar.tsx` (`input.accept = '.gdml'`) reads
  files **as text** and posts them to `/api/files/upload-multi`
  (`UploadFilesRequest { files: name -> text, main_file, segments }`).
  `backend/src/api/handlers.rs::load_document` parses (`gdml/parser.rs`) into
  `GdmlDocument` (`gdml/model.rs`), evaluates defines (`eval/engine.rs`), tessellates
  every solid (`mesh/tessellator.rs::tessellate_all_solids`) and stores a
  `LoadedDocument` (`state/app_state.rs`, has an `unsaved` flag). It refuses
  non-`.gdml` names today. Binary STL will need bytes, not text.
- **Save:** `/api/document/export` calls `gdml/materials.rs::serialize_gdml`. The
  desktop saves through the `save-file` IPC in `desktop/main.js`; the browser
  downloads.
- **Model:** `GdmlDocument` is the single in-memory model. `Solid` has a variant for
  every solid Geant4 reads from GDML, including `Tessellated` (facets reference
  `<define><position>` vertices), `Boolean`, `MultiUnion`, `Xtru`, `Polycone`,
  `GenericPolycone`, `Arb8`. `Volume.assembly` models `<assembly>`; `PhysVol` has
  position, rotation and scale (reflection). Programmatically built documents are
  anticipated: an empty `DocumentOrder::define_slots` falls back to grouped order.
  So an importer can build a `GdmlDocument` and reuse the existing tessellator,
  viewer and export unchanged.
- **Materials:** a built-in NIST table (`backend/data/nist_materials.json`, 309
  Geant4 predefined materials) with a frontend lookup (`frontend/src/utils/nistImport.ts`)
  can drive per-part material assignment.
- **Reference:** `sample_data/pod_asm_tessellated.gdml` (6.7 MB, 44,408 facets) is a
  tessellated version of `sample_data/pod_asm.gdml` (primitives): a useful picture
  of what CAD-derived output looks like, and of its load cost.
- **Tests:** `cargo test` (`backend/tests/`: round trip, correctness, mesh
  orientation, integration), frontend Vitest, desktop smoke test. CI runs
  `cargo fmt --check`, `clippy -D warnings`, tests, `tsc -b`, lint.
- **Geant4 sources:** `backend/reference/geant4/` holds Geant4 10.7.4 files cited by
  line in comments. On the user's workstation a full Geant4 11.4.2 source tree and an
  install **with GDML** are available. Elsewhere, build Geant4 with
  `-DGEANT4_USE_GDML=ON`, or state in the plan that the Geant4 checks were not run.
  Generated GDML must load in both 10.7 and 11.x.
- **Local tooling:** Python 3.13 (miniconda) with numpy only; no OCCT, CadQuery,
  FreeCAD, trimesh or gmsh installed (a venv with `cadquery-ocp` or `build123d`
  is fine for spikes and fixtures). Node 20.20. 8 cores.

## 4. Geant4 rules the output must satisfy (verified in the sources)

- **Placement rotation is inverted for physvols but not for booleans.**
  `G4GDMLReadDefine::GetRotationMatrix` builds `R = Rz(z) Ry(y) Rx(x)` (rotateX, then
  rotateY, then rotateZ; `G4GDMLReadDefine.cc:153`). A `<physvol>` uses
  `R.inverse()` as the daughter's active rotation (`G4GDMLReadStructure.cc:476`).
  A boolean's second solid and a `multiUnionNode` use `R` itself
  (`G4GDMLReadSolids.cc:193`, `:870`). The same angles therefore mean opposite
  rotations in the two places. CAD transforms are active (daughter in mother), so
  physvol angles come from decomposing `M^T`, boolean angles from `M`. The viewer
  follows the same convention (comment near `handlers.rs:990`). This is the most
  likely silent bug; test it with non-principal rotation axes.
- **Degenerate facets punch holes.** A too-small or too-narrow `G4TriangularFacet` is
  flagged undefined with a warning (`G4TriangularFacet.cc:110`), and
  `G4TessellatedSolid::AddFacet` rejects it (`G4TessellatedSolid.cc:211`, GeomSolids1002),
  leaving the surface open. Remove degenerate triangles after welding, keeping the
  mesh watertight.
- **Facet orientation:** vertices anticlockwise seen from outside (outward normal by
  the right-hand rule). Default vertex type is ABSOLUTE (`G4GDMLReadSolids.cc:1644`).
- **Names:** the GDML reader truncates a name at the first `0x`
  (`G4GDMLRead.cc:107`). Sanitize CAD names: unique, XML-safe, no `0x`.
- **No overlaps, daughters inside mothers.** Geant4 does not detect overlaps while
  tracking; it silently gives wrong physics. Geant4's own overlap check
  (`G4PVPlacement::CheckOverlaps`, `/geometry/test/run`) is the ground truth.
- **A CAD assembly has no world.** Generate a world box (bounding box plus margin,
  configurable material). Subassemblies map naturally to `<assembly>` (no envelope)
  or can be flattened; an envelope solid would need to contain its children without
  touching siblings, which CAD does not provide.
- **Cavities:** check in `G4TessellatedSolid.cc` (and by test) whether one
  tessellated solid may hold an outer shell plus inward-facing inner shells, or
  whether a void must become a boolean subtraction or a daughter volume.
- **Cost:** tessellated solids are voxelized, but navigation cost grows with facet
  count; a boolean evaluates all its operands and nesting compounds it;
  `G4MultiUnion` is voxelized. Measure before choosing limits.

## 5. Why recognition matters physically, and the tessellation trap

Tessellation moves curved surfaces. A cylinder of radius R as an inscribed n-gon
loses the fraction `1 - n sin(2 pi/n) / (2 pi)` of its cross-section (0.64 % at
n = 32, 0.16 % at n = 64); the sagitta is `R (1 - cos(pi/n))`. That is a mass error,
and in assemblies it is an **overlap** source: convex surfaces move inward, but
concave ones (the wall of a bore) move into the cavity, so a shaft in a bore,
tessellated independently, can intrude into the bore wall.

One property worth evaluating: **conservative tessellation**, where every generated
solid is a subset of its exact CAD solid (vertices on the surface for convex regions,
facet planes kept on the material side for concave ones, saddles handled
explicitly). Then a CAD assembly without interference cannot produce GDML overlaps;
thin gaps of mother material are physically benign, overlaps are not. Weigh it
against the extra volume loss and the complexity, and report the volume deficit per
part rather than hiding it.

CAD assemblies also contain real interference (press fits, nominal threads, fasteners
modelled without clearance). Detecting it (bounding-box prefilter, then a kernel
boolean intersection volume) and reporting it is part of the job; fixing it may be
user-driven.

Planar-faced solids tessellate with **zero** geometric error, so recognition pays off
mainly on curved surfaces, plus readability and file size.

## 6. Open architecture question (the crux)

Where does the geometry kernel run, given the static musl backend, Windows and
Ubuntu 20.04 packages, browser dev mode and "nothing to install"? Candidates:

- **A. Native OCCT linked into the Rust backend** (cxx/bindgen bindings, or a small
  C++ shim built by `build.rs`). Fast; hard on musl static linking, MSVC, CI time
  and binary size.
- **B. OCCT compiled to WebAssembly**, run in the frontend (identical in Electron and
  browser) or in Electron's Node process. `occt-import-js` exists (STEP/IGES/BREP to
  meshes with per-face ranges, names, colors, hierarchy) but exposes no analytic
  surface parameters; recognition would need a custom Emscripten shim. wasm32 memory
  limits and speed need measuring.
- **C. Sidecar process** (native OCCT CLI, or Python with OCP) bundled in the
  installers. Easiest to prototype, heaviest to ship.
- **D. Pure Rust** (truck, foxtrot, or a home-grown ISO 10303-21 parser). A Part 21
  parser plus analytic-surface extraction is small; robust trimmed-NURBS
  tessellation and booleans are not. Check what is mature as of now.

Hybrids are possible (Rust Part 21 reader for structure and analytic surfaces,
kernel only for tessellation and verification), at the risk of two readers
disagreeing about the same file.

Whatever wins, keep **one owner** for each piece of domain logic. The backend owns
`GdmlDocument`, the evaluator and the tessellator, so document construction belongs
there, fed by a neutral intermediate representation (parts, instances and
transforms, per-face surface type and parameters, triangulation, exact volume). Put
recognition next to whatever can verify it.

STL (and OBJ) parsing, welding, validation and `<tessellated>` emission need no
kernel and fit in the Rust backend. That path can ship first.

## 7. Recognition: heuristics with proof

Never accept a heuristic without checking it. A candidate primitive or CSG tree is
accepted only when it agrees with the exact B-rep within a tolerance tied to the
file's modelling uncertainty (for example symmetric-difference volume relative to
part volume, plus every B-rep face lying on a candidate surface). Otherwise fall
back to tessellation. Justify the check you choose. Report the choice and the
measured error per part.

A plausible ladder, cheapest first:

1. **Whole-solid match.** box (three orthogonal pairs of parallel planes); tube with
   rmin and phi cut (coaxial cylinders, perpendicular caps); cutTube (tilted caps);
   cone; sphere or orb; torus; revolved profiles to polycone or genericPolycone
   (straight profile segments are exact, arcs are not); prisms to xtru (two parallel
   caps with the same polygon, side faces parallel to the axis), or to
   trd/trap/arb8/para for eight-vertex hexahedra.
2. **Stock minus features.** One primitive minus recognized holes, counterbores and
   pockets (or plus bosses), as a boolean chain or as stock minus a `multiUnion` of
   the tools. Bound the operand count by measured Geant4 cost.
3. **Tessellation.** Kernel triangulation with a chord tolerance in mm and an angular
   tolerance; a valid B-rep gives a watertight mesh because faces share edge
   discretization. Then weld, drop degenerates, check closed and consistently
   oriented, positive signed volume, compare with the exact volume, emit.

## 8. Ergonomics (starting points, refine them)

- Open accepts `.gdml`, `.step`, `.stp`, `.stl` (plus whatever extra formats you
  justify), in the toolbar, the desktop dialogs and by drag and drop.
- **Open as new document** (world generated) first; **insert into a selected
  volume** with a placement and containment/overlap check later.
- One click must work with sensible defaults; options for those who need them:
  length unit (STL; live bounding-box readout), chord tolerance, recognition level
  (off, primitives, primitives plus CSG), keep or flatten hierarchy, world material
  and margin, default material, merge identical parts into one logical volume with
  several placements (STEP gives instancing for free).
- An **import report** per part: representation chosen, facet count, exact vs GDML
  volume and relative error, warnings (open shell, healed, degenerates removed,
  overlaps with X). Per-part overrides (force tessellated, change tolerance) re-run
  without re-reading the file.
- Material per part with the existing NIST lookup; a guess from STEP names, colors
  or AP242 data is shown as a guess, never applied silently.
- The viewer shows the **GDML interpretation** (the generated document through the
  existing tessellator), not the CAD mesh, so users see what Geant4 will see.
- Large files (STEP assemblies over 100 MB, millions of triangles): progress,
  cancellation, work off the async workers (`load_document` already uses
  `spawn_blocking`), memory bounds.

## 9. Validation the plan must specify

- A dev-only fixture generator (not shipped) producing STEP and STL with expected
  values derived **analytically**, never from the importer: box; tube with bore and
  phi cut; cone frustum; sphere; torus; revolved profile; L-profile extrusion; plate
  with through holes and a counterbore; filleted block; freeform B-spline part;
  assembly with repeated instances, a nested subassembly, rotations about
  non-principal axes, a mirrored instance and inch units; shaft in bore; deliberately
  overlapping parts; STL in ASCII, binary, multi-solid, unwelded, open and with
  flipped winding.
- Round trip: import, serialize, re-parse with the existing parser, tessellate,
  compare per-part world-space volume, bounding box and centroid with CAD truth.
- Geant4 as the authority: a small program built against the local Geant4 install
  that reads the GDML, runs the overlap check on every placement, compares solid
  volumes with CAD volumes, and shoots geantinos to catch stuck tracks and
  navigation warnings.
- At least one real-world STEP part and one real assembly (not generated), to meet
  the quirks of Section 2.
