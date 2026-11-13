# CAD import (STEP, STL) for GDML Studio: implementation plan

## Context

GDML Studio should open CAD files (STEP, STL) and turn them into Geant4-valid GDML that the
existing export saves: exact GDML primitives or small CSG trees where a heuristic can be proven
right, `<tessellated>` solids otherwise. ChatGPT Pro investigated this from [BRIEF.md](BRIEF.md)
(prompt: [PROMPT.md](PROMPT.md)) and delivered:

- its plan, kept as [INVESTIGATION.md](INVESTIGATION.md): architecture, recognition and
  tessellation rules, phases;
- `cad-import-evidence.zip` (outside the repository): spike code, fixtures with analytic
  `expected.json`, a Geant4 harness (`g4_check.cpp`), raw results and logs. Its 56 Geant4
  documents (51 passing, 4 deliberately failing contact cases, 1 real-assembly failure;
  112,000 geantinos) back the plan; `verified-summary.json` matches the headline numbers.

This plan reviews that work against the code and the Geant4 sources, and turns it into an
executable implementation. Decisions taken on 2026-11-13: **placeholder material plus optional
picker**, **OCCT 8.0.x shipped as LGPL**, **Linux first, built so that Windows is only a build
and packaging addition later**.

## 1. Review of the investigation

### Holds up (checked)

- **Architecture:** STL and all GDML construction in Rust, STEP through a separate native C++
  worker on OpenCASCADE (OCCT), launched by the backend. This keeps the static-musl backend and
  isolates kernel crashes. The rejected alternatives (WASM is mesh-only and needs 6x the memory;
  truck failed fixtures; Python is too heavy to ship) are supported by the measurements.
- **Rotation conventions** (10.7.4 sources in the repo): `GetRotationMatrix` builds
  `R = Rz Ry Rx` (`G4GDMLReadDefine.cc:153`). A physvol uses `R.inverse()`, then applies the
  scale (`G4GDMLReadStructure.cc:476-477`). Boolean and multiUnion operands use `R` directly
  (`G4GDMLReadSolids.cc:193`, `:870`).
- **Facet `lunit`** multiplies positions that are already scaled (TriangularRead returns
  `vertex * lunit`), and `tessellator.rs:658` mirrors that, so emitted facets carry no `lunit`.
- **Degenerate facets:** `G4TriangularFacet` rejects an edge or altitude `<= kCarTolerance`
  (1e-9 mm), and `AddFacet` drops the facet (checked in the 11.4.2 source).
- **Names:** the reader cuts names at `0x` (`G4GDMLRead.cc:107`).
- **Upload limit:** `main.rs:49` caps request bodies at 64 MiB, so large exported tessellated
  GDML could not be reopened.
- **Geant4 10.7.4 support** (reference sources) for genericPolycone, multiUnion, recursive
  assemblies (`AddPlacedAssembly`) and physvol reflection.
- **OCCT license issue #1564** is open; the eight conflicting headers have been in every
  release since 7.7. Decision: ship unmodified OCCT 8.0.1 shared libraries under the declared
  LGPL and record the issue.
- **Fixture generator:** expected values come from formulas, independent of any importer.

### Corrections

1. **Degenerate triangles must be repaired without changing the topology, not deleted.** The
   SO100 "unfixable" failure (`so100-repair.json`) still had one facet removed, a minimum
   altitude of 0.0 and an open mesh at every weld tolerance from 1e-6 to 0.01 mm. That is the
   signature of a *cap* triangle (three distinct, nearly collinear vertices): deleting it leaves
   a sliver hole. The standard repair is an edge flip for caps and an edge collapse for needles
   (as in CGAL's mesh-repair routines). It is expected to close SO100; this becomes a Phase 3
   acceptance test.
2. **CAD meshes are stitched from B-rep topology, not welded by proximity.** BRepMesh
   discretizes each edge once for both adjacent faces, so indexing nodes by (vertex) and
   (edge, node index) through `BRep_Tool::PolygonOnTriangulation` gives a watertight mesh with
   no tolerance.
3. **No NIST logic in the backend.** The backend already accepts NIST names as volume
   materials (`handlers.rs:228-239`), and Geant4 resolves undefined `G4_*` names through
   `G4NistManager` (`G4GDMLReadMaterials.cc:976`). The world uses `G4_Galactic` by name. A part
   material picked at import is prepared by the existing frontend `prepareNistMaterial`, given
   an explicit element list, and sent with the request.
4. **Verification adds a support-set equivalence check** (exact, analytic and cheap) to the dual
   difference volume. A difference-volume check alone can accept a needle-shaped defect (large
   distance, tiny volume); sampling and Hausdorff estimates are not needed.
5. **Recognition uses one axisymmetric recognizer** (a meridian section through the axis) for
   tube, cone, genericPolycone, orb, sphere and torus instead of per-family code. **CSG is stock
   minus the connected components of (stock minus part)**, each component recognized by the
   same recognizer. There is no feature-specific code for drills, counterbores or pockets.

### Simplifications

- No import jobs, staging, commit protocol or temporary job directories. The worker runs once
  per import, reads the file on stdin and writes the result on stdout. The import replaces the
  document the same way Open does; the session keeps the source bytes and the IR for re-imports
  and overrides.
- No coordinate rebasing: GDML values stay exact `f64`; the `f32` preview error is cosmetic.
- Overlap checks on the generated geometry: tessellated contact pairs are checked by reusing the
  self-intersection code; analytic-versus-mesh contacts on curved surfaces are flagged, not
  certified.
- The small fixtures are committed, so CI needs no OCP or Python CAD environment.

## 2. Architecture

```
Frontend  Open(.gdml | .stl | .step/.stp)          poll GET /api/import/progress, POST /api/import/cancel
   | POST /api/import?filename=..&options=..  (raw bytes)
   v
Backend (Rust) cad::import
   |- STL  : cad::stl     -> triangle soup per ASCII solid block
   |- STEP : cad::worker  -> gdml-cad-worker (stdin: file; stdout: JSON with meshes; stderr: progress)
   |- cad::mesh     weld (STL only), degenerate repair, orientation, shells, self-intersection, volume
   |- cad::build    IR -> GdmlDocument (world, assemblies, LVs, physvols, solids, names, materials)
   '- finish_load   (shared with Open) evaluate, preview-tessellate -> LoadedDocument{unsaved, import session}
Export: unchanged serialize_gdml
```

| Owner | Responsibility |
|---|---|
| `cad-worker` (C++17, OCCT 8.0.1) | STEP/XCAF read, units, assembly tree, validity and healing, exact volume, area and bounds, topology-stitched meshing with adaptive refinement, recognition plus verification, source interference and contact detection |
| `backend/src/cad/` (Rust) | STL parse, the one shared mesh validator and repairer, transforms, names, materials, `GdmlDocument` construction, report, session and re-import |
| Frontend | file routing, options, progress and cancel, report panel, overrides. No geometry rules |

## 3. Repository changes

New:
- `backend/src/cad/{mod.rs, ir.rs, stl.rs, mesh.rs, intersect.rs, worker.rs, build.rs, transform.rs, names.rs, report.rs}`
- `backend/tests/cad_import_test.rs`; `backend/tests/data/cad/` (STEP fixtures, small STL
  fixtures and `expected.json` from the evidence zip; a README naming the generator command)
- `cad-worker/` (`CMakeLists.txt`, `occt-options.cmake`, `occt-source.txt` with version, URL
  and SHA-256, `build-linux.sh`, `src/{main,read,mesh,recognize,verify,interfere,output}.cpp`,
  `README.md`)
- `tools/g4check/` (dev-only Geant4 harness adapted from evidence `g4_check.cpp`: load, solid
  volumes and bounds, `CheckOverlaps` per placement, placement matrices, geantinos, exception
  counts, JSON lines, nonzero exit on failure; `CMakeLists.txt`, `run.sh`, `README.md`)
- `frontend/src/utils/cadImport.ts`, `frontend/src/components/TreePanel/ImportPanel.tsx` (+ test)
- `doc/cad-import/PLAN.md` (this plan), `doc/cad-import/INVESTIGATION.md` (ChatGPT's
  `PLAN.md`, unchanged, with a header pointing to the evidence zip kept outside the repo),
  `doc/cad-import/fixtures/` (generator, README, requirements from the plan zip)

Modified:
- `backend/src/api/handlers.rs` (factor `finish_load`; import handlers), `routes.rs`,
  `state/app_state.rs` (`ImportSession`, `ImportControl`), `main.rs` + `config.rs` (body
  limit), `backend/Cargo.toml` (+ `robust`)
- `frontend/src/{api/client.ts, store/types.ts, store/index.ts, components/Toolbar.tsx,
  components/TreePanel/TreePanel.tsx, utils/nistImport.ts}`
- `desktop/package.json` (Linux `extraResources: cad/`), `run.sh` (`GDML_CAD_WORKER` in dev),
  `.github/workflows/ci.yml` (Linux worker job), `README.md` (CAD import section)

The investigation's archives stay outside the repository (the plan archive duplicates the repo);
only the pieces above were extracted from them.

## 4. Phases

Each phase ends with all tests green and the app usable.

### Phase 0: groundwork (no behavior change)
1. Extract the docs, fixtures and harness listed in section 3.
2. Split `handlers.rs::load_document` (`:350`): parsing, module merging and loop expansion stay
   there; the rest becomes `finish_load(document, render, warnings, segments) ->
   Result<LoadedDocument, ApiError>` (evaluate defines, `tessellate_all_solids`, warnings,
   world check). Open and import both use it.
3. Replace the 64 MiB literal with `config::MAX_UPLOAD_BYTES = 1 GiB` (single-user localhost tool).
4. Build `tools/g4check` against a local Geant4 11.4.2 install with GDML; run it on
   `sample_data/pod_asm_tessellated.gdml` as a baseline.

### Phase 1: STL import, end to end (Rust only, both platforms)
1. `cad/ir.rs`: `CadImport { source, definitions, assemblies, roots, issues, interferences,
   contacts }`. A `Definition` holds an id, a label, exact props (STEP only), a mesh (`f64`
   vertices, `u32` triangles, chord), an optional verified `Primitive` with its body frame,
   rejected candidates and issues. `Child { target, transform: Affine (active, mm), label }`.
2. `cad/stl.rs`: algorithm A (section 5).
3. `cad/mesh.rs`: algorithm B; `cad/intersect.rs`: BVH plus robust triangle-triangle test.
4. `cad/transform.rs` (algorithm G), `cad/names.rs`, `cad/build.rs` (algorithm H),
   `cad/report.rs`.
5. API: `POST /api/import` (STL branch), `GET /api/document/import-report`,
   `POST /api/import/update`, all through `finish_load`.
6. Frontend: open routing, `cadImport.ts`, `ImportPanel`, unassigned-material chip, and
   `prepareNistMaterial(nist, existing = store elements)`, called with `[]` for a new import.

Acceptance:
- The committed STL fixtures give the evidence's outcomes. `open.stl` is rejected;
  `multi_solid.stl` gives 2 bodies; `closed_cavity.stl` gives 1 body of 784 mm³; the flipped,
  unwelded and degenerate cases are repaired; `binary_solid_header.stl` parses as binary.
- Synthetic unit cases: a needle and a cap degenerate are repaired and the mesh stays closed;
  self-intersecting, NaN, truncated-binary and non-manifold-edge inputs are rejected; an island
  inside a cavity becomes 2 bodies.
- Round trip: import, export, reopen through `upload-multi`; the preview volume equals the IR
  mesh volume (1e-9 relative).
- `g4check` (11.4.2) on every accepted fixture: no warnings, overlaps or stuck tracks.
- A synthetic 250k-facet STL imports, previews, saves and reopens. Record the times; add a
  numeric-literal fast path in the evaluator only if positions dominate.

Status (2026-11-13): done; all acceptance items pass.
- Built as planned, except that the IR holds only what STL needs (meshes, shells, repairs,
  contacts; no assemblies, primitives or transforms). `transform.rs` moves to Phase 2, whose
  `assembly.step` is the first input that places parts anywhere but at the identity.
- Bodies whose surfaces touch or cross are kept, reported in the panel and as load warnings
  (Geant4 reports an overlap where they interpenetrate), using the self-intersection test.
- The Import tab stages its options (STL unit, part material, included parts) and applies
  them with Re-import. The last unit and part material start the next import.
- Viewport fix: after a reload that keeps the solid names (a re-import), the camera fit
  measured the previous geometry, so a new STL unit left the view inside the part.
- `g4check` with Geant4 11.4.2 on the GDML exported from all 9 accepted STL fixtures and from
  the evidence's OCCT-tessellated sphere, torus, tube and B-spline roof: exit 0, no warnings,
  errors, overlaps or stuck tracks; Geant4's solid volumes equal the IR mesh volumes (6000 mm³
  per box, 784 mm³ for the cavity). The curved meshes are below their analytic volumes by
  their chord error (2.9e-3, 2.2e-3, 6.8e-4 and 5.2e-4 relative).
- Round trip: the saved GDML reads back with every vertex the same `f64` and every facet the
  same triangle.
- 249,000-facet sphere (release build): import 1.40 s, save 0.19 s (32 MB of GDML), reopen
  0.57 s. The evaluator's numeric-literal fast path already covers the positions.

### Phase 2: CAD worker and Linux packaging proof
1. `cad-worker/occt-options.cmake` (one owner of the OCCT configuration, reusable on Windows):
   shared libraries; all modules OFF; `BUILD_ADDITIONAL_TOOLKITS = TKDESTEP TKXCAF TKMesh TKBO
   TKBool TKPrim TKShHealing` (OCCT resolves the closure, including the TKV3d and TKService that
   TKXCAF needs); FreeType, FreeImage, OpenGL, TBB, VTK, Draw, docs and samples OFF; Release.
   This matches the evidence's configured cache.
2. `cad-worker/build-linux.sh`: in Docker `ubuntu:20.04` (glibc 2.31, focal GCC), download
   and verify the OCCT 8.0.1 tarball per `occt-source.txt`, build it once into
   `cad-worker/third_party/` (gitignored), then build the worker. Offload this one-time build
   to david-local if the workstation is busy.
3. Worker v0 (algorithm C): STEP from stdin, XCAF traversal, per-solid validity, exact props,
   tolerance, stitched meshes with adaptive refinement, JSON on stdout. No
   recognition yet.
4. `cad/worker.rs`: locate the worker (`GDML_CAD_WORKER`, else
   `<current_exe dir>/cad/gdml-cad-worker{EXE_SUFFIX}`); spawn without a shell; feed stdin from
   a thread; parse progress from stderr into `ImportControl`; read stdout; `Child::kill` on
   cancel; map exit codes. A missing worker gives "STEP import is not available in this build".
5. Packaging: Linux `extraResources` copies `cad/` (worker, `lib/*.so*`, `licenses/`). The
   worker carries an RPATH of `$ORIGIN/lib`; `run.sh` exports `GDML_CAD_WORKER` for dev.

Acceptance:
- `box.step`, `inch_box.step` (98,322.384 mm³) and `assembly.step` (instances, nesting, mirror)
  import in the app as tessellated GDML.
- In a clean `ubuntu:20.04` container: `gdml-cad-worker < box.step` succeeds; `readelf -V`
  shows nothing above GLIBC_2.31; `ldd` shows only system and private libraries.
- Desktop smoke passes; the `.deb` size delta is recorded in `doc/cad-import/PLAN.md`; the LGPL
  texts, exception and source pointer ship in `cad/licenses/`.

Status (2026-11-13): done; all acceptance items pass.
- Deviations: the worker writes plain JSON (meshes inline, `%.17g`) instead of the binary
  container, and takes no arguments; its volume target is fixed at 1e-3 until Phase 3 adds the
  quality presets. Progress lines, cancel and `ImportControl` move to Phase 3 with the progress
  UI. Placements are flattened: every occurrence is a physvol of the world, named
  `<base>_pv<k>` with copy number `k`, its position, rotation and scale named `_pos`, `_rot`,
  `_scl` (the schema requires the names). A non-rigid (scaled) occurrence is rejected, since
  G4ReflectionFactory keeps only the sign of a scale.
- Build: OCCT 8.0.1 needs GCC 10 (GCC 9.4 rejects its defaulted noexcept constructors). Focal's
  `gcc-10` uses the system `libstdc++6`, so no private runtime ships. OCCT builds in 25 min
  (`-j7`). In a clean `ubuntu:20.04` the worker runs, every library resolves to `cad/lib` or
  libc, libm, libstdc++ and libgcc_s, and the newest symbols it needs are GLIBC_2.29 and
  GLIBCXX_3.4.26.
- Size: `cad/` is 61.6 MB installed; the `.deb` grows by 17.6 MB (102.1 to 119.7 MB). The
  `.deb`, extracted in a clean `ubuntu:20.04`, imports `assembly.step` through its static
  backend and `resources/cad/`; the desktop smoke test passes on the packaged app.
- Accuracy: on the 15 single-part STEP fixtures, OCCT's volumes are within 1e-9 of the formulas
  and the meshes within 1e-3 (`cad_import_test`, needs `GDML_CAD_WORKER`). `g4check` (11.4.2)
  on all 16 exported fixtures: no warnings, errors, overlaps or stuck tracks; Geant4's volumes
  are within 8.3e-4 of the formulas. `assembly.step` gives 2 definitions, 3 placements and
  13,500 mm³; Geant4's placement matrices match the generator's within 4.5e-13 (rotation) and
  1.8e-11 mm (translation), limited by the 12 significant digits the file gives directions.
- Five real-world models (0.16 to 0.78 MB, up to 16 parts) import in 0.26 to 5.0 s with nothing
  rejected or repaired, meshes within 6.4e-4 of the CAD volumes; the worker peaks at 85 MB.
- CI: a Linux job builds the worker (OCCT cached on `occt-source.txt` and
  `occt-options.cmake`) and runs the STEP tests; it runs for the first time on the next push.
- For Phase 3: STEP imports do not yet check contacts between placed parts (in a 16-part
  aircraft model Geant4 finds 11 placements overlapping, as the parts interpenetrate in the
  source). The 1e-3 target can make heavy meshes (a grooved shaft: 189,500 facets, 23.7 MB of
  GDML, 50 s to load in Geant4), so the quality presets matter.

### Phase 3: complete tessellated STEP import
1. Worker: multi-body parts become separate definitions under the part. Closed non-solid shells
   are sewn into solids; open shells and empty nodes are reported, not dropped. Healing uses
   `ShapeFix_Shape` within the source tolerance, then revalidates and records the volume change.
   Invalid B-reps are flagged. Unresolved external references are reported. Source
   interference and contact detection (algorithm I) runs with a time budget.
2. Rust: hierarchy as `<assembly>` (or flattened), instances, reflections, and stable ids,
   names and copy numbers (algorithm H). Contact pairs between tessellated bodies are checked
   with `intersect.rs` (two meshes in the world frame).
3. Bodies that fail Rust validation (thin-wall self-intersection, leftover degenerates) are
   re-run once with `--chord-for id=chord/2`, then reported.
4. Overrides: exclude a part, per-part quality; `/api/import/update` reuses the session bytes
   and preserves materials (section 6).
5. Progress UI with Cancel.

Acceptance:
- STEP fixtures: exact volume vs `expected.json` at 1e-9 relative, bounds within 1e-6 mm, mesh
  volume within the target. Assembly instance count and composed matrices match within 1e-12;
  `g4check` placement matrices agree within 1e-9.
- SO101 upper arm (fetched by SHA-256 into a gitignored dir; tests marked `#[ignore]`): closed,
  loss at most 0.1 %, Geant4 clean.
- SO100: the two previously open meshes close after degenerate repair, or the report names the
  face and the reason. The 12 source interferences are reported (largest 187.457 mm³).
- SO101 assembly: 90 unique solids and 266 occurrences. `XG-ZIJI_16` is healed or visibly
  reported; the 12 non-solid leaves are listed; nothing is silently missing.
- Cancel takes under 1 s and keeps the previous document.

Status (2026-11-13): done; all acceptance items pass, with the deviations below.
- Worker: loose faces are sewn into solids; empty labels, labels with only curves, faces
  outside any solid and missing external files are reported; an invalid B-rep is healed with
  `ShapeFix_Shape` within its tolerance, kept only if that makes it valid, with the volume
  change noted. Bounds are exact (`BRepBndLib::AddOptimal`). Arguments `--volume-target`,
  `--chord <entry>.<solid>=<mm>` and `--only` (mesh the listed solids, skip the overlap check)
  replace `--chord-for`; output stays JSON; progress lines `read`, `mesh`, `interfere`.
- Refinement deviates from algorithm C: it halves only the chord, at a fixed 0.2 rad.
  Halving the angle too crowded small fillets with slivers that crossed where faces meet
  tangentially (SO100 `Mirror`: 25 to 42 crossing pairs at 0.1 rad; chord 0.025 mm at 0.2 rad
  is clean with a third of the facets). Below the solid's tolerance, refinement stops once a
  pass keeps more than 90 % of the volume error, since BRepMesh no longer follows the chord on
  faces whose tolerance exceeds it. In SO101, `Component2` (thin and swept, tolerance 0.004 mm)
  stays at -4.99e-3 at chord 0.004 mm and -4.85e-3 at 1e-4 mm with 22 % more facets; only the
  angle helps (0.05 rad: -7.4e-4 with 1.38 M facets). This cut SO101's meshing from 162 to
  90 s and its facets from 1.58 M to 1.25 M. The Import panel flags a part whose mesh misses
  the target (8 in SO101: 6 such thin swept parts and 2 screws).
- Algorithm I: `BRepAlgoAPI_Common` on the placed solids whose boxes, grown by the tolerance,
  overlap; an interference is an overlap above 1e-6 of the smaller volume. Pairs run on up to
  8 threads; those not started within 60 s are counted as unchecked. Contacts came with
  Phase 4; between meshes, the Rust mesh-crossing check reports what Geant4 sees.
- Rust: assemblies kept as `<assembly>` volumes (those with nothing included left out) or
  flattened; ids `<label entry>.<solid>.<body>`. Placed meshes are checked for crossings
  (a BVH over the facets in the common box); a pair counts only where triangles pierce each
  other by more than 1e-7 mm, so faces in contact, also after a reflection or an inexact
  rotation, do not. Each interference and each crossing it does not explain is a load
  warning naming the volumes. A solid whose mesh fails validation is meshed once more at
  half the chord (`--only`), then imported whole or not at all; the reason names an
  unmeshable face and leads with an invalid B-rep.
- Overrides: exclude a part, chord per part; a re-import reads the file again only when the
  volume target or a chord changes. Progress and Cancel sit in the toolbar.
- STEP fixtures: as in Phase 2, and the exact bounds are within 1e-6 mm of the formulas.
  Keep and flatten give the same placements within 1e-12 (`cad_import_test`). Geant4
  11.4.2 composes the keep-mode placements of `assembly.step` (an assembly inside an
  assembly) to the flattened ones within 7.1e-15, with no warnings.
- The SO-ARM100 models (`backend/tests/data/real/fetch.sh` downloads them by commit and
  SHA-256; ignored tests `so10*` and `cancelling_*`), on this workstation while other jobs
  loaded it:
  - Upper arm: closed, mesh -1.75e-4 from the CAD volume; its two empty labels are listed.
    Geant4: no warnings, overlaps or stuck tracks; its solid volume equals the mesh volume.
  - SO100: 15 solids in 19 placements; 13 imported, 2 rejected with their reasons
    (`Wrist_Roll_Pitch_08i`: an unmeshable cone face; `Rotation_Pitch_08i`: 11 open edges,
    also at half the chord). 11 interferences, the largest 187.457 mm³; the investigation's
    12th, 0.0006 mm³, is below 1e-6 of the smaller solid. Import 38 s. Geant4 (keep mode,
    22.9 MB, loads in 18.9 s) finds 9 overlapping volume pairs, all among the 11 the import
    warns of (7 interferences and 2 of 4 mesh crossings), nothing else, and no stuck tracks.
  - SO101: the worker reads 90 solids in 266 occurrences. 83 solids are imported; 7 are
    rejected: `XG-ZIJI_16` (invalid B-rep that healing does not fix), 4 SMD packages whose
    faces intersect (238 crossing pairs in `SOT-23` at every chord from 0.0049 to 0.02 mm) and
    2 whose meshes stay open; the 12 leaves without a solid are listed. 220 s (meshing 90 s,
    overlap check 74 s with 599 of 793 pairs unchecked), 1.25 M facets, about 154 MB of GDML.
  - Cancel during meshing: the import stops within 1 s and the previous document stays.
- The desktop backend exits when the app's pipe to it closes, cancelling a running import
  (worker killed): no backend or worker outlives a crashed, killed or upgraded app.

### Phase 4: exact GDML solids (algorithms D, E, F)
1. Worker: `recognize.cpp` reads a candidate off each valid, unhealed solid; `certify.cpp`
   owns what GDML numbers mean (`shape()`) and the certificate. Each solid's output carries
   `exact: {epsilon_mm, stock, tools}` (each primitive: GDML values in mm and rad and a
   right-handed 3 x 4 frame in the solid's frame) or `exact_reason`. The mesh is always output,
   so switching representation never re-runs the worker.
2. Rust: `ir::Analytic`, emitted by `build.rs` when the part's representation is Auto. The
   names `<base>`, `<base>_stock`, `<base>_tool<k>` and `<base>_tools` are claimed either way,
   so volume and solid names stay when switching. Physvols place the primitive's frame
   (`at.after(frame)`); Boolean operands place theirs relative to the stock (algorithm G).
3. Fixtures (generator, committed): the families on oblique axes, faces split at seams, a
   hollow theta-cut sphere sector, a hollow torus sector, and lookalikes: a bore 0.01 mm off
   its axis (tessellated) and one tapered by 0.1 degree (a correct cone). `expected.json` holds
   each part's `family`.

Acceptance: each fixture gets its family and every lookalike its correct outcome; Geant4
volumes of the primitives equal the formulas; the exact solids sit where the CAD solids do.

Status (2026-11-14): done.
- All 25 STEP part fixtures get their family (`step_parts_have_their_formula_volumes_and_families`),
  and the viewer's mesh of each document solid is within 3 % of the formula (32 segments per
  turn make a sphere 1.6 % small), so the GDML numbers mean what was certified.
- `recognize-test` (ctest; `build-linux.sh` runs it): the certificate accepts true candidates
  and rejects a face 1e-3 mm off, an xtru vertex 1e-3 mm off, a box with a cavity against the
  plain box, the other half of a half tube and a spherical shell against the ball; a half
  tube reads as deltaphi pi, the shell as rmin 5.
- Geant4 11.4.2 (`g4check`) on the GDML exported from every STEP fixture: no warnings, overlaps
  or stuck tracks; primitive volumes equal the formulas within 6e-13. Sampling 2e6 points with
  Geant4's navigator, each part's exact and tessellated exports disagree only by the mesh sag:
  0 mm³ for every planar part, including a pocket turned 30 degrees (which pins the operand
  rotation convention), and 1.6 to 13.6 mm³ for curved ones.
- Deviations: no `recognize` option and no progress phase of its own. Recognition runs in the
  `mesh` phase on every valid solid; the per-part choice (Auto, Tessellated, Exclude)
  replaces the option.
- The oblique sphere fixture found a bug: `gp_Dir::Crossed` normalizes, which made every sweep
  but 1.5 pi wrong; sweeps use `gp_Vec`.

### Phase 5: bounded CSG (algorithm D, CSG)
A solid that is a stock primitive minus at most 16 primitive cut-outs is written as
`subtraction(stock, tool)` or `subtraction(stock, multiUnion(tools))`, certified as a whole.

Acceptance: `hole_plate` and `closed_cavity` become CSG with Geant4 Monte Carlo volumes within
1 % of their formulas; a flange with 6 bolt holes (revolution stock), a turned blind pocket and
a counterbore floor 1e-3 mm off its expected depth (its true floor is kept) become CSG; a
17-hole plate and the filleted and B-spline parts stay tessellated with their reasons.

Status (2026-11-14): done.
- `hole_plate`: box minus multiUnion(tube, genericPolycone), the counterbore and its hole
  being one stepped tool; `closed_cavity`: box minus box; `flange`: tube minus 6 tubes;
  `pocket_block`: box minus a turned box; `counterbore_shift` keeps its floor 2.001 mm deep;
  `plate_17_holes`: "a box minus 17 cut-outs, more than 16". Geant4 Monte Carlo volumes are
  within 9e-5 of the formulas (hole_plate 11282.72 against 11283.72 mm³).
- A plate with 16 holes (box minus 16 tubes) imports in 0.58 s, its preview included. Geant4
  tracks 200,000 geantinos through it in 0.47 s; through its tessellated version (4,108
  facets), 18.8 s for the same 642,000 steps.
- Real models (worker alone, this workstation under load; recognition time is the sum over
  solids):

  | File | Solids | Exact | Families | Recognition | Worker |
  |---|---|---|---|---|---|
  | SO100 | 15 | 1 | genericPolycone | 3.5 s | 18 s |
  | SO101 | 90 | 10 | box 2, xtru 7, box - 1 cut-out | 14 s | 126 s |
  | SO101 upper arm | 1 | 0 | (B-spline faces) | 0 s | 0.8 s |
  | 7 local instrument files | 80 | 57 | box 21, xtru 20, tube 10, xtru - 2 cut-outs 5, genericPolycone 1 | 3.7 s | 7.7 s |

  The robot arm parts are mostly free-form or filleted. The instrument parts that stay
  tessellated are mostly trays and frames whose cut-out is two primitives (a wide pocket, then
  a narrower window), or whose tool ends on an inner face of the stock: both deferred below.
  Two measures halved recognition time: stocks are built in turn (the eight meridian sections
  of a revolution stock only after the prisms fail), and a stock the solid sticks out of is
  skipped before its Boolean.
- Deferred (fallback: tessellated): a connected cut-out made of several primitives, tools
  that intersect or end on inner faces of the stock, oblique xtru, B-spline-encoded analytic
  faces, sectors of revolution stocks.

### Phase 6: Windows worker and installer
Use the same `occt-options.cmake` and worker `CMakeLists.txt` with MSVC, through a PowerShell
script equivalent to `build-linux.sh`. Ship the DLLs and the app-local MSVC runtime in `cad/`
via the win `extraResources`. Add a CI job if wanted. Test on a clean Windows VM, including a
user profile path with non-ASCII characters.

Acceptance: the full STEP fixture suite passes on Windows; the installer delta is recorded;
installing over an older version and over the same version, with the app running and
importing, replaces it cleanly.

Later, unscheduled: insert into a selected volume (containment and sibling checks); IGES (the
same worker plus sewing); conservative meshing for concave cylinder and cone faces (radial
vertex offset `r/cos(dtheta/2)`, which keeps the solid a subset of its CAD shape);
OBJ and 3MF.

## 5. Algorithms

**A. STL parse.** If the length is `84 + 50 n` (n read at offset 80), the file is binary, even
when the header starts with `solid`. Otherwise it is parsed as ASCII, token-based: keywords are
case-insensitive, CRLF and scientific notation are accepted, and each `solid` block becomes a
group named after it. Exactly 3 vertices per facet. Stored normals are ignored (winding is the
truth). Non-finite values, truncation, an empty file and an unknown format are rejected. The
unit factor (option, default mm) is applied here.

**B. Mesh pipeline** (shared by STL and worker meshes):
1. STL only: merge bit-identical vertices. If open edges remain, merge *boundary* vertices
   within `max(1e-9 mm, 1e-7 D)` (nearest representative, 27-cell grid, no transitive chains),
   recording the count and maximum displacement. A larger repair tolerance is an explicit,
   geometry-changing override.
2. Drop index-degenerate triangles (this is an edge collapse) and duplicate triangles; an
   opposite-oriented duplicate pair is an internal double wall, so both go.
3. Geometric degenerates, with `eps = 1e-7 mm` (100 x Geant4's 1e-9 mm):
   - needle (an edge `<= eps`): collapse it to its midpoint if the link condition holds;
   - cap (altitude `<= eps`): flip the longest edge `(u,v)` with the neighbour `(v,u,x)` to
     `(u,x,w), (x,v,w)`, unless edge `(w,x)` already exists.

   Iterate to a fixed point with a bounded iteration count. Leftovers make the body invalid.
4. Split non-manifold vertices; reject edges with other than 2 incident triangles (open or
   non-manifold) with the counts and the largest gap.
5. Make orientation consistent per component (BFS; non-orientable is rejected).
6. Shells: signed volume per component, nesting depth by ray parity (robust ray-triangle test,
   retried with a new direction on edge or vertex hits). Even depth faces outward, odd depth
   faces into the void. Each outer shell plus its depth+1 shells is one body (one
   `G4TessellatedSolid`; works in 11.4.2, to be checked in 10.7). Depth-2 islands are
   separate bodies.
7. Self-intersection: a median-split AABB BVH, exact `robust::orient3d`/`orient2d`
   triangle-triangle tests; shared-vertex and shared-edge pairs count only for real crossings
   or coplanar fold-overs.
8. Final gate: closed, oriented, every edge and altitude `> eps`, positive volume per body.
   Output the indexed mesh plus a `RepairLog`.

**C. Worker meshing.** `BRepMesh_IncrementalMesh(solid, chord, false, angle, true)`. Global
node ids are keyed by `TopoDS_Vertex`, by (edge, polygon node index) taken from
`PolygonOnTriangulation`, including both polygons of seam edges, and by face-interior nodes.
Degenerated edges map to their vertex. Triangles are flipped on `TopAbs_REVERSED` faces. If an
edge has no polygon, fall back to a weld within that edge's tolerance and flag it. Refinement:
start at `chord = clamp(min(0.05 mm, 1e-3 D), tau, ...)` and `angle = 0.2 rad`; while
`|V_mesh - V_exact| / V_exact > target` (default 1e-3; presets 1e-2, 1e-3, 1e-4) and facets are
below 250k per body, halve both and re-mesh after `BRepTools::Clean`. Record the final values.

**D. Recognition** (worker `recognize.cpp`, per valid unhealed solid, in its definition frame).
Only native analytic faces count (plane, cylinder, cone, sphere, torus): a B-spline or offset
face means tessellated, since OCCT's canonical recognition only fits on a fixed 21 x 21 grid
(`GeomConvert_SurfToAnaSurf.cxx:660`). Templates propose; only the certificate (E) decides.
- **Revolution**, when any face is curved. The axis is that of a cylinder, cone or torus, else
  a sphere's centre with the planes' normal. Every face must be coaxial, a cap or a phi cut;
  the phi cuts' material sides give the start ray (frame x, startphi 0) and deltaphi. The
  section with the half-plane at mid-sweep (`BRepAlgoAPI_Common`, then
  `ShapeUpgrade_UnifySameDomain` to merge seam splits) must be one face; its loops give:

  | Meridian | GDML solid |
  |---|---|
  | rectangle | tube |
  | trapezoid with horizontal ends, inner radii both zero or both positive | cone |
  | other polygon | genericPolycone |
  | arcs about one centre on the axis, radial segments | sphere (rmin, theta range), or orb |
  | circle or two concentric circles off the axis | torus |

  Geant4 changes some parameters behind the reader's back, so: a cone with one zero inner
  radius is a genericPolycone (`G4Cons.cc:120`); a torus needs `rtor >= rmax + 1e-6 mm` and
  `rmin = 0` or `>= 1e-7 mm` (`G4Torus.cc`); full sweeps are exactly 2 pi; gaps below 1e-6 rad
  are refused (the viewer reads them as full turns).
- **Prism**, all faces planar. Plane directions by decreasing area; the first along which every
  face is across or along and whose mid-extent section's outer loop gives a certified
  candidate: a box (four right angles) or an xtru (written clockwise, as Geant4 expects).
- **CSG.** Stocks in turn: prisms along the three main plane directions (the outer loop of the
  mid section), then the full revolution about the largest cylinder or cone (the largest
  single-face meridian of eight). A stock the solid sticks out of is skipped. The cut-outs are
  the solids of `Cut(stock, S)` (slivers of volume below eps times area ignored), at most 16,
  each read by the templates above and certified alone. A tool end on a stock plane moves
  outward by `delta = max(1e-3 D, 1e-3 mm)`, but only when the whole stock lies inside that
  plane: then the subtraction removes nothing more and leaves Geant4 no coincident surface.
  Box faces and xtru ends shift; end vertices of revolution profiles move along their side
  edges. Blind floors stay. The whole tree is then certified; the first stock that passes
  wins, and a failure reports the first stock's reason.

**E. Certificate** (`certify.cpp`). `shape()` builds each primitive from exactly its GDML numbers
and frame (revolving its meridian, extruding its polygon; CSG is one Cut with all tools), so
source and candidate compare surface to surface. `certify(S, C, eps)` accepts only if:
1. every face of both is native analytic;
2. the faces fall into the same surfaces, each compared with its cluster's first face over the
   diagonal D (planes: `|n1 x n2| D + offset`; cylinders, cones, tori: axis tilt times D plus
   offset, and radii, or apex and half-angle; spheres: centre and radius; all `<= eps`), and
   every surface has faces of both;
3. on each surface every face of each solid has the same material side
   (`BRepGProp_Face::Normal` at the UV centre against the plane normal, or away from the axis,
   centre or minor circle);
4. on each surface, area within `eps P + num` and first moment within `eps (A + P D) + num`, P
   being the candidate's edge length there;
5. volume within `eps A + num` and its first moment within `(eps A + num) D`; the `num` terms
   are the error estimates of the adaptive `BRepGProp` integrations (Eps 1e-9).

Both boundaries then lie on one finite set of surfaces, so both solids are unions of cells of
that arrangement and can differ only by whole cells; each changes some surface's side, area
or moment, or the volume. Complements fail 3, wrong trims 4, eccentric or needle defects 2. An
empty Boolean difference is never the evidence: fuzzy values stop at `Precision::Confusion()`
= 1e-7 mm (`BOPAlgo_Options.cxx:107`).

**F. Tolerances.** `eps = max(the solid's largest face tolerance, 1e-7 mm)`; a solid with
`eps > 1e-4 D` is not recognized. Directions match within eps over D. Genuine primitives deviate
by about 1e-12 mm; the lookalikes by 1e-3 mm or more.

**G. Transforms.**
- Decompose `R = Rz(g) Ry(b) Rx(a)`: `b = atan2(-R20, hypot(R00, R10))`,
  `a = atan2(R21, R22)`, `g = atan2(R10, R00)`. At gimbal lock (`|cos b| < 1e-12`):
  `g = 0`, `a = atan2(-R12, R11)`.
- Physvol: `R = M^T`. Boolean and multiUnion operands: `R = M`.
- Reflection (`det M < 0`): `S = diag(-1,1,1)`, `Q = M S`, physvol rotation from `Q^T`,
  inline `<scale x=-1>` (Geant4 composes `[Q, t] S = M`).
- Angles are written in deg, positions in mm.
- Unit tests rebuild `R` from the *formatted and reparsed* strings and require 1e-12 agreement,
  using random, gimbal and reflected cases.

**H. Document mapping.**
- **World:** a box with half extents `max(|min|, |max|) + max(10 mm, 1 % of the diagonal)`
  over the exact and mesh bounds, material `G4_Galactic` by name, source coordinates kept.
- **Hierarchy:** CAD assemblies become `<assembly>` volumes (shared sub-assemblies are placed
  repeatedly); "flatten" composes the transforms once and places every leaf in the world.
- **Logical volumes:** one solid and one LV per definition; physvol transform = occurrence
  transform x primitive frame (identity for meshes); `copynumber` = occurrence index.
- **Ordering:** solids are pushed in dependency order (operands first) and volumes bottom-up
  with the world last, because Geant4 reads each section in one pass.
- **Names:** `[A-Za-z_][A-Za-z0-9_]*`, `0x` removed, deterministic `_2`, `_3` suffixes;
  `<base>` (solid), `<base>_lv`, `<base>_asm`, `<base>_pv<k>`, tessellated vertices `t<k>_<i>`
  (short, since there may be millions). Original labels stay in the report.
- **Tessellated solids:** positions with `unit="mm"`; triangular facets with no `type`
  (ABSOLUTE by default, `G4GDMLReadSolids.cc:2629`) and no `lunit`. Numbers use Rust's
  shortest round-trip `f64` formatting.
- **Materials:**
  - by default, a defined `CAD_UNASSIGNED` (Z=1, density 1e-25 g/cm3, A=1.00794 g/mole,
    state gas);
  - when picked, the prepared `{material, elements}` from the options, validated with the
    existing `validate_element` and `validate_material_values`.

  The unassigned count is derived in the frontend from `volumes`, not stored.

**I. Interference and contact** (worker `interfere.cpp`).
- Candidate pairs are placed solids whose boxes, grown by their tolerance, overlap.
- Interference: `BRepAlgoAPI_Common` volume `> 1e-6 min(Va, Vb)`.
- Contact: otherwise, the triangulations (the GDML meshes) come within
  `chord_a + chord_b + max(tolerance)` of each other (`BRepExtrema_ShapeProximity`), so every
  gap narrower than a chord is found. The report lists the contacts of an exact and a
  tessellated part, for the current choice: there the mesh can cut into the exact solid by up
  to its chord. (`BRepExtrema_DistShapeShape` took 7 of the 9.6 s of one 7-solid assembly;
  the proximity test costs nothing measurable.)
- Up to 8 threads; pairs not started within 60 s are reported as not checked.

## 6. API, state and worker protocol

- `POST /api/import?filename=&options=` (raw body) returns the `DocumentSummary`; a busy
  import gives 409; a cancelled one gives 409 "cancelled".
- `GET /api/import/progress` returns `{phase, done, total}`; `POST /api/import/cancel`;
  `GET /api/document/import-report`.
- `POST /api/import/update {options, overrides}` re-runs the worker only when
  worker-affecting options changed (volume target, angle, chord, recognition, interference,
  per-part chord); otherwise it rebuilds from the session IR. Afterwards it copies
  `doc.materials` and each definition's `material_ref` from the previous document, using
  `ImportSession.lv_of_def`.
- State: `LoadedDocument.import: Option<ImportSession { bytes, filename, format, options,
  overrides, ir, lv_of_def }>`. `AppState.import_control: Arc<ImportControl { progress, child
  }>` sits outside the `RwLock` hold, so progress and cancel answer during a long import.
- Options JSON: `stl_unit_mm`, `volume_target`, `angle_rad`, `chord_mm?`,
  `recognize: none|primitives|csg`, `hierarchy: keep|flatten`, `interference`,
  `part_material?`, `overrides{def_id: {representation: auto|tessellated|exclude, chord_mm?}}`.
- Worker:
  - invocation: `gdml-cad-worker` with the file on stdin; Phase 2 takes no arguments, and the
    options it gains later (`--volume-target X --angle A [--chord C] --recognize R
    --interference on|off [--chord-for id=v,...]`) are added with the features that need them;
  - stdout: one JSON object (labels, components with 3 x 4 matrices, solids with their meshes
    inline, numbers as `%.17g`). The planned binary container was not built: the 7.4 MB of
    JSON for a 189,500-facet part parse in 27 ms, next to 3.2 s in the worker;
  - stderr: `error <message>` lines (`progress <phase> <done> <total>` comes with Phase 3);
  - exit codes: 0 ok (per-body failures are inside the JSON), 3 unreadable file, 4 internal
    error (2, bad arguments, once there are arguments);
  - definition ids: `<XCAF label entry>.<solid index>.<body index>`, stable across runs;
  - the Rust serde structs and `cad_import_test.rs` own and enforce the contract.

## 7. UI

- Open accepts `.gdml,.stl,.step,.stp`. A single CAD file goes to `cadImport.ts`; GDML files
  go through the existing flow; a mixed selection is an error. The existing unsaved-work
  prompt applies.
- One click imports with the last-used options (localStorage, wrapped in try/catch). While it
  runs, the toolbar shows "Importing: meshing 12/90" and Cancel.
- The **Import** tab in `TreePanel` appears for imported documents.
  - Summary: counts by representation, total facets, estimated GDML size, interferences,
    and an "N parts unassigned" chip.
  - Options form: quality preset, recognition, hierarchy, STL unit, part material (existing
    `SearchableCombobox` over NIST).
  - Per-definition table: label, instances, representation dropdown, CAD vs GDML volume and
    error, facets, repairs, issues. Clicking a row calls `setSelectedVolume`.
  - Re-import button.

## 8. Windows readiness (rules applied from Phase 1)

- The worker is standard C++17 plus OCCT only: no POSIX headers, `fork`, signals or
  `getrusage`. Binary stdin/stdout use `_setmode` under `_WIN32`. No file path crosses the
  process boundary, which avoids codepage problems with Unicode paths. Numbers are formatted
  with `%.17g` in the C locale.
- One OCCT option file and one worker CMake project serve both platforms; only the thin
  build scripts differ.
- Backend: `EXE_SUFFIX`, `PathBuf`, no shell, `Child::kill`, and
  `creation_flags(CREATE_NO_WINDOW)` under `cfg(windows)` (otherwise a console window flashes
  on every import).
- Packaging is per-platform `extraResources` with the same `cad/` layout (Linux `lib/` plus
  RPATH; Windows DLLs beside the exe). The frontend has no platform code.

## 9. Verification

- `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`; frontend `npx tsc -b`,
  `npm run lint`, `npm test`.
- STEP suite: `GDML_CAD_WORKER=... cargo test --test cad_import_test -- --include-ignored`
  (CI Linux job with cached OCCT keyed on `occt-source.txt` and `occt-options.cmake`).
- `tools/g4check/run.sh` on every GDML exported from the fixtures and the real models (local
  11.4.2). Zero warnings, overlaps and stuck tracks, except the expected failures
  (`interfering_shaft`, independently tessellated contacts).
- Geant4 10.7.4 on Olivia for compatibility: tessellated cavity, genericPolycone, multiUnion,
  reflected nested assembly.
- In the app (`./run.sh`, desktop `npm start`): open `assembly.step`, the SO101 part and an
  STL; check the tree, report, material picker, overrides, save and reopen. Install the `.deb`
  in a clean focal container.

## 10. Risks and gates

- OCCT 8.0.1 may not build with focal's GCC. Fallback: a newer GCC with a private
  `libstdc++` in `cad/lib` (decided in Phase 2).
- `STEPCAFControl_Reader::ReadStream` may behave unexpectedly. Fallback: the backend writes
  the bytes to a temp file with an ASCII name.
- A tessellated cavity may not work in 10.7. Fallback: emit cavities as a subtraction of
  tessellated solids.
- Very large assemblies: per-body and per-import facet budgets (250k and 1M, soft), measured
  memory, GDML size shown before saving (51 MiB took 44 s to load in Geant4).
- License issue #1564: redistribute OCCT unmodified and record the issue in
  `doc/cad-import/PLAN.md` and `cad/licenses/`.
- False recognition: the verification gate plus the lookalike fixtures are the protection.
