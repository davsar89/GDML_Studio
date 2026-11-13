You are a senior engineer with deep knowledge of CAD geometry kernels (B-rep, STEP,
OpenCASCADE), computational geometry (meshing, mesh repair, primitive fitting) and
Geant4 geometry (GDML, solids, navigation, overlaps). You are working in the GDML
Studio repository: a Rust (Axum) backend, React/Three.js frontend and Electron
desktop app for viewing GDML geometry and editing its materials. If you received
this as gdml-studio-cad-import.zip, the repository is its GDML_Studio/ folder (a
snapshot of commit f736e31 without git history); all paths below are relative to it.

TASK
Investigate and write a plan for importing CAD files into GDML Studio so they become
GDML documents that the existing export can save. Formats: STEP and STL first; a
small number of closely related formats (for example IGES next to STEP, OBJ next to
STL) only where the extra cost is small and justified. Each CAD body should become a
native GDML primitive or a small CSG tree when a heuristic recognizes it and a check
against the exact geometry confirms the match, and a tessellated solid otherwise.
The import must be ergonomic: one click works with sensible defaults, and the user
can see and override what was chosen per part.

This is an investigation and planning task. Do not implement the feature in the
product code. Do run spikes and measurements to back your decisions.

READ FIRST
1. doc/cad-import/BRIEF.md: goal, formats, how the codebase loads and saves, the
   Geant4 rules the output must satisfy (with verified source citations), the
   tessellation and overlap trap, the open architecture question, and what
   validation must cover.
2. The code it points to: backend/src/gdml/model.rs, backend/src/api/handlers.rs
   (load_document, export_gdml, the scene-graph transform code),
   backend/src/mesh/tessellator.rs, backend/src/gdml/materials.rs (serialize_gdml),
   frontend/src/components/Toolbar.tsx, desktop/main.js, README.md.
Treat the brief as a starting point, not as truth: verify claims you rely on.

WHAT TO INVESTIGATE
- The geometry-kernel decision (native OpenCASCADE in the Rust backend, OpenCASCADE
  compiled to WebAssembly, a bundled sidecar, or pure Rust such as truck or a
  home-grown ISO 10303-21 parser). Check the current state and licenses of the
  candidates (it is late 2026; do not rely on memory). Measure on the fixtures and
  on at least one real-world STEP part and one real assembly: correctness, speed,
  memory, added installer size, and build effort for the Windows installer, the
  static-musl Ubuntu 20.04 .deb and the browser dev mode.
- Recognition: which primitives and CSG patterns are worth recognizing, how to
  verify each candidate against the exact B-rep, and the tolerances, tied to the
  file's own modelling uncertainty.
- Tessellation: chord and angular tolerance defaults, welding, degenerate-facet
  removal, validity checks, facet budgets, and whether to make tessellation
  conservative (each GDML solid a subset of its CAD solid) so that a clean CAD
  assembly cannot produce overlaps. Quantify the volume cost.
- Structure: generated world, assemblies vs flattening, instancing of repeated
  parts, units, mirrored instances, name sanitizing, materials, and the rotation
  convention (physvol angles are inverted relative to boolean and multiUnion angles
  in Geant4's reader; see BRIEF.md section 4).
- Ergonomics: open vs insert-into-volume, the options dialog, the per-part import
  report, overrides, progress and cancellation for large files.

VALIDATION
Generate fixtures (CadQuery, OCP or build123d in a local venv is fine) whose
expected volumes and bounding boxes come from analytic formulas, never from the
importer. Use the local Geant4 11.4.2 install with GDML (see BRIEF.md section 3) as
the authority: load the generated GDML, run the overlap check, compare volumes and
navigate geantinos. A spike that converts fixtures end to end and passes these
checks is the best evidence a plan can have.

DELIVERABLE: doc/cad-import/PLAN.md
1. Findings: tools assessed (versions, licenses, dates checked) and measured spike
   results in tables.
2. Chosen architecture and why; one sentence per rejected alternative.
3. The intermediate representation between CAD reading and GDML construction.
4. Recognition algorithms, tolerances and the acceptance check.
5. Tessellation strategy and its validity guarantees.
6. Mapping of structure, transforms, units, names and materials to GdmlDocument.
7. User flow: dialogs, defaults, report, overrides.
8. Packaging and CI impact per platform.
9. Phased implementation (STL to tessellated first, since it needs no CAD kernel),
   each phase with acceptance tests, effort estimate and risks.
10. Open questions only the user can answer (for example: is a larger installer
    or an LGPL dependency acceptable?).

RULES
- Prefer the simplest design that is correct. One owner per piece of domain logic;
  no duplicated logic, speculative abstractions or process ceremony. The plan should
  be as short as its content allows.
- Keep throwaway spikes outside the repository. The fixture generator may be kept
  under doc/cad-import/fixtures/ because the implementation will reuse it; keep
  generated files small.
- Work on the current branch; do not create branches, commit or push. Do not modify
  product code.
- Do not spawn sub-agents.
- Never use em dashes in anything you write; use commas, colons, parentheses or full
  stops. En dashes in numeric ranges are fine.
- Report numbers you measured as measured, and estimates as estimates. If a check
  fails or a spike does not work, say so in the plan.
