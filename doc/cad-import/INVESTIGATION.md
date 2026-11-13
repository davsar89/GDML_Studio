> **Investigation record.** Written by ChatGPT Pro on 2026-11-13 from [BRIEF.md](BRIEF.md) and kept unchanged below. The spike sources, raw measurements and Geant4 logs it cites are in `cad-import-evidence.zip`, which is kept outside the repository. The implementation plan built on this record, with its corrections, is [PLAN.md](PLAN.md).

# CAD import plan

Investigation date: **2026-11-13**. Repository: supplied `f736e31` snapshot. This is a plan supported by external spikes, not a product implementation. Product files are unchanged. Measurements below are observations on one Linux host; proposed defaults, budgets and effort are explicitly recommendations or estimates.

## 1. Findings and measured evidence

### Decision

Ship **STL to tessellated GDML first, entirely in Rust**. Add STEP through a **bundled native C++ worker using OCCT 8.0.1**, launched by the Rust backend. Recognize simple analytic bodies and bounded CSG, reconstruct each candidate in OCCT, and accept it only after geometric verification. Otherwise mesh the original B-rep. Build the existing `GdmlDocument` and use the existing export.

Do not promise that ordinary CAD tessellation preserves assembly clearances. The experiments demonstrate new overlaps between exact touching solids. General conservative meshing is not a small extension to OCCT's mesher. Prefer exact recognition, check the actual generated solids, and stop on unresolved intersections or invalid meshes. A conservative option is justified only for shapes for which containment can be established and its volume cost reported.

### Tools assessed

All versions and licenses were checked on the investigation date against the linked upstream sources or package registries. A release date is not a claim that a project is abandoned.

| Candidate | Version or state checked | License | Finding |
|---|---|---|---|
| Native OCCT | **8.0.1**, released 2026-07-30 | Top-level LGPL 2.1 with OCCT exception; see header conflict below | STEP/XCAF, exact surfaces, booleans, validation, healing and meshing in one kernel; selected subject to distribution clearance. [1] |
| Rust `opencascade` / `opencascade-sys` | **0.3.0**, published 2026-08-24; bundled `occt-sys` **7.8.1** | LGPL 2.1 | Usable native binding, but still C++ build/link obligations and additional XCAF work; no advantage over a narrow worker for this application. [2] |
| `occt-import-js` | npm **0.0.23**, 2024-12-03; underlying OCCT **7.6.1**, Emscripten 3.1.69 | LGPL 2.1 | Measured. Returns meshes and hierarchy, not an exact-shape API for candidate verification. Upstream reverted its kernel version after a meshing regression. Main's 0.0.24 package declaration is not the published npm release. [3] |
| `opencascade.js` | npm latest **1.1.1**, 2020-09-27; beta **2.0.0-beta.b5ff984**, 2023-03-23 | LGPL 2.1 only | Broad kernel bindings could support verification, but require a maintained custom build and a separate WASM performance spike. Not benchmarked here. [4] |
| `truck-stepio` / `truck-meshalgo` | **0.3.0** / **0.4.0**; repository active through 2026-11-02 | Apache 2.0 | Compiled and exercised the upstream shell-import approach. Several important fixture failures below; not selected as the production CAD kernel. [5] |
| Foxtrot | Experimental STEP viewer; last repository push 2022-09-06 | MIT or Apache 2.0 | Reader/viewer work, not an established exact-solid verification kernel; source assessment only. [6] |
| Home-grown ISO 10303-21 parser | No implementation proposed | Our code | Reading entity syntax does not supply trimmed NURBS, topology, robust booleans, units or assembly semantics. |
| OCP / CadQuery, spike tooling | OCP **8.0.1.1.0** for current-kernel experiments; `cadquery-ocp` **7.8.1.1.post1** and CadQuery **2.7.0** for fixture generation | Wrapper/tooling Apache 2.0; OCCT's terms still apply | Convenient development tools. No Python runtime in the shipped application. [7] |
| Rust STL / predicate helpers | `stl_io` **0.11.0**; `robust` **1.2.0** | MIT; MIT or Apache 2.0 | Proposed parser and adaptive orientation predicates. Neither is a complete mesh validator. The STL spike used Python, not these crates. [8] |

**Current license caveat:** the downloaded OCCT 8.0.1 source contains commercial-only notices in the eight `TKGeomBase` files identified by upstream issue 1564, despite the repository's LGPL declaration. The issue remained open with no clarification at this check. Treat redistribution clearance as a release gate: obtain upstream clarification, a corrected release, or appropriate licensing; do not edit away the notices or assume the header exception resolves this contradiction. This changes shipping readiness, not the measured technical result. [14]

### Method and inputs

Host: Xeon Platinum 8370C at 2.80 GHz, 9 visible CPUs, 9.73 GiB RAM, Linux/glibc 2.41, GCC 14.2, Rust 1.100, Python 3.12.14, Node 24.22.1. Native measurements use a small optimized C++ executable against OCCT 8.0.1 shared libraries from the OCP wheel. WASM measurements use Node with the published `occt-import-js` package. This compares usable import paths, **not identical OCCT versions or identical work**: native also validates B-reps, measures exact volumes and computes bounds; WASM creates JavaScript mesh arrays. Browser responsiveness and an Electron installer were not benchmarked. WASM and truck outputs were compared through mesh/structure metrics, not taken through the full Geant4 conversion pipeline.

The final timing run is serial, with the source build stopped. Small cases and the real part have three fresh-process samples; assemblies have one. Earlier contended runs, including two 180-second WASM assembly timeouts, are retained in the evidence but excluded from headline timings. RSS is the process high-water mark, not added installer size or peak memory of the whole application.

`fixtures/generate.py` generates 15 single-body STEP cases, a nested assembly, and 14 STL cases outside the repository. Expected dimensions, volumes and assembly matrices are calculated from formulas, never from imported geometry. Coverage includes a box, tube, 270-degree tube sector, cone, sphere, torus, polycone, concave extrusion, drilled/counterbored plate, rounded block, closed cavity and an inch-unit box. The curved roof is a B-spline representation of

`z = 10 + 3(1 - (x/10)^2)(1 - (y/8)^2)`, with `x in [-10,10]`, `y in [-8,8]`, bottom `z=0`.

Its independent volume is `320 * (10 + 4*3/9) = 3626.666666666667 mm^3`, and its true maximum height is 13 mm. Using its control-pole maximum of 22 mm as a bounding-box oracle would be wrong. The inch fixture is `1 x 2 x 3 in`, volume **98,322.384 mm³**. Independent truth caught an initial fixture-writer error that labelled millimetre coordinates as inches; both the writer's coordinate conversion and declared unit were then corrected.

Real inputs are Apache-2.0 files from TheRobotStudio/SO-ARM100, pinned to commit `a758567c3978dfeefe282ede0500085a48fe8f78`. These are practical interoperability tests, not analytic ground truth. [9]

| Input | Repository path | Bytes | Native structure/validity |
|---|---|---:|---|
| Real part | `STEP/SO101/Upper_arm_SO101.step` | 551,437 | 1 solid, B-rep valid |
| SO100 assembly | `STEP/SO100/Follower_Specific/SO_5DOF_ARM100_Assembly.step` | 7,038,498 | 15 unique solids, 19 solid occurrences; all B-reps valid |
| SO101 assembly | `STEP/SO101/SO101 Assembly.step` | 20,496,014 | 90 unique solids, 266 solid occurrences; one invalid B-rep and 12 non-solid leaves |

#### Import performance

| Input | Engine | Process wall time, s | Peak RSS, MiB | Facets |
|---|---|---:|---:|---:|
| Box | Native OCCT 8.0.1 | 1.009 (0.152 to 1.157) | 41.5 | 12 unique-definition total |
| Box | WASM OCCT 7.6.1 | 12.736 (8.882 to 23.052) | 304.5 | 12 occurrence total |
| Tube | Native OCCT 8.0.1 | 0.418 (0.158 to 0.789) | 42.0 | 504 unique-definition total |
| Tube | WASM OCCT 7.6.1 | 11.178 (5.996 to 11.796) | 345.6 | 504 occurrence total |
| Real part | Native OCCT 8.0.1 | 2.700 (2.192 to 3.323) | 62.2 | 19,288 unique-definition total |
| Real part | WASM OCCT 7.6.1 | 4.681 (3.618 to 6.363) | 379.2 | 19,258 occurrence total |
| SO100 assembly | Native OCCT 8.0.1 | 60.665 | 118.3 | 221,995 unique-definition total |
| SO100 assembly | WASM OCCT 7.6.1 | 61.284 | 722.4 | 287,184 occurrence total |
| SO101 assembly | Native OCCT 8.0.1 | 93.469 | 250.7 | 744,244 unique-definition total |
| SO101 assembly | WASM OCCT 7.6.1 | 171.973 | 1541.0 | 1,493,397 occurrence total |

Times include process startup and exit; parentheses give the three-sample range, not a confidence interval. Shared-host scheduling produced substantial variance, so these figures should not be used to promise a speed ratio. The definition sharing and memory differences are more stable architectural evidence.

The SO101 source also has 11 compound leaves without solids and one shell leaf (`User_Library-LED_0603_R_Pocket002`); the invalid solid is `XG-ZIJI_16`. These were reported separately, not counted as successful solids. The real-part file has two compound leaves without solids. Distinguishing empty product nodes from open surface geometry is required in the production importer.

The SO101 WASM path emits meshes per occurrence; native meshes shared definitions once. Facet counts are therefore explicitly labelled. The two paths also have slightly different triangulations. Native bounds and volumes refer to the B-rep, while WASM only exposes mesh bounds and volumes. Neither successful read status nor positive signed volume establishes suitability for Geant4.

#### End-to-end correctness

The workstation paths in the brief were unavailable in this environment. A local **Geant4 11.4.2** C++ runtime with GDML was assembled from conda-forge binaries and used directly, with its version checked at runtime (`G4VERSION_NUMBER=1142`). The available `geant4-pybind` wheel was 11.4.1 and was not used as the authority. Every test below passes its generated GDML through the repository's real parser, `serialize_gdml`, a second parse and the existing preview tessellator before Geant4 reads it. No product serializer was replaced.

The validator uses 10,000 overlap samples per placement, zero reporting tolerance, seed 17331 and 2,000 transported geantinos per document. It records loads, volumes, bounds, placement matrices, diagnostic warnings, navigation steps and stuck tracks. Boolean volumes use Geant4's Monte Carlo estimator with one million samples, and are labelled estimates. XML schema fetching was disabled; offline XSD validation and a Geant4 10.7 runtime remain release gates. Geant4 overlap sampling is authoritative evidence of a detected problem, not proof that an undetected problem is absent. [10]

| Test group | Measured outcome |
|---|---|
| 15 STEP bodies, selected representation plus forced tessellation | All 30 documents pass with no warnings or stuck tracks. 10 bodies selected native solids/CSG; 5 fell back to tessellation. Sector and polycone recognition were not implemented in the spike. |
| Analytic checks | Maximum imported fixture B-rep relative volume error is **2.63e-13**; maximum analytic-bound discrepancy is **1.01e-7 mm**. Native primitive Geant4 volumes agree to approximately `2e-16` relative. Counterbored plate: analytic **11,283.716875 mm³**, Geant4 MC estimate **11,283.977130 mm³**, +0.00231%. |
| Cavity | One 24-facet tessellated solid, outer cube side 10 and inward inner shell side 6: volume **784 mm³**; center outside material, wall point inside, center-to-wall distance **3 mm**. Inner shells work in 11.4.2. |
| Structure and rotations | Composed placement matrices and transformed analytic bounds agree within **7.11e-15** (bounds in mm). Nested assemblies, repeated box instances, two non-principal rotations and an asymmetric reflected extrusion pass. Separate boolean and multiUnion probes confirm their opposite angle convention to physvol. This transform test builds GDML from known fixture matrices; a general XCAF-to-GDML assembly mapper is not implemented. |
| Contacts and negative cases | Exact tube/shaft contact and three conservative meshes pass. A deliberate CAD interference and all three ordinary independently tessellated contacts fail overlap checks as expected. They do not produce stuck tracks, demonstrating why navigation alone is insufficient. |
| STL | 13 accepted fixtures round-trip and pass Geant4, including binary headers starting with `solid`, winding repair, duplicate vertices, separate bodies and a cavity. The open cube is rejected. No curved primitive fitting is attempted. |
| Real SO101 part | **19,288 facets**, 9,586 welded vertices; closed and oriented. CAD volume **117,417.073686 mm³**, mesh/Geant4 volume **117,396.480412 mm³**, loss **0.01754%**. Geant4 passes without warnings. |
| Real SO100 assembly | **Fails**, correctly: two of 19 occurrence meshes remain open after degenerate removal; neighbour-cell welding from `1e-6` to `0.01 mm` does not close them. Geant4 independently warns of holes in those same solids and finds 14 overlapping placements. 2,000 geantinos still finish without stuck tracks. |
| SO100 source intersections | 44 bounding-box candidate pairs tested with OCCT Common; 12 positive volumes above `1e-5 mm³`, including 11 above 1 mm³. Largest **187.457 mm³**. The `0.000609 mm³` result needs tolerance-aware classification. Pair volumes must not be summed as a union volume. |

Before the CSG cost experiment below, this is **56 documents and 112,000 geantinos**: 51 positive passes, four deliberately invalid contact cases and one real assembly failure. The real assembly's GDML load took 44.51 s and reached 901.6 MiB RSS; this flattened diagnostic document is about 51 MiB. Valid B-reps are not enough to guarantee valid exported meshes. Full self-intersection tests, certified surface-distance bounds and broad real-model healing were not implemented in the spike. The fixtures and SO101 inputs use AP214; the SO100 assembly uses AP242. AP203 and mixed-unit assembly regressions remain implementation gates.

Two recognition checks are particularly informative. Extending a counterbore through its interior floor by 0.001 mm produced a symmetric difference of **0.0502655 mm³**, exceeding its **0.0020508 mm³** allowance, and was rejected. Extending only the exterior end produced zero difference and was accepted. Two solids with the same 992 mm³ volume and the same outer bounding box, but different cavity locations, produced a **16 mm³** symmetric difference and were rejected. Volume and bounding-box matching alone is unsafe.

The truck shell-import experiment read the box correctly in 0.0047 s, but returned an irregular mesh for the rounded block (1,796.77 versus 3,685.90 mm³) and an open/oriented-only tube sector. Without an additional semantics layer it returned an inch-box volume of 6 instead of 98,322.384, separate positive cavity-shell volumes of 1000 and 216 instead of material volume 784, and assembly definitions without their repeated placements. These last three are limitations of the exercised importer path, not proof that truck cannot implement those semantics. Real-part and assembly attempts hit 90-second limits during the initial contended run; they are not fair comparative latency figures.

#### Size and build evidence

| Measurement | Result and limit |
|---|---|
| Native worker proof | C++ executable 56,848 bytes; latest rebuild 22.63 s under contention. A separately compiled **static-musl Rust executable successfully launches it** and reads the box. This verifies the process boundary on this host, not Ubuntu 20.04 compatibility. |
| Available native dependency closure | 49 shared libraries, **207,978,856 bytes after stripping (198.34 MiB)**, summed gzip size **24,499,815 bytes (23.36 MiB)**. Includes STEP, IGES, boolean support and avoidable visualization/image dependencies inherited from the wheel. This is a measured prototype payload, not an installer delta or a minimal OCCT build. |
| Current Linux ABI | Host-built worker requires **GLIBC_2.34** and therefore fails the Ubuntu 20.04 baseline requirement. Rebuild against that platform; do not ship this spike binary. |
| Headless source build | OCCT 8.0.1 configuration succeeds, roughly 18 s without PCH and 15 s with it. STEP/IGES/XCAF/meshing/boolean closure selects about 4,967 build actions. Bounded PCH build stopped after **300 s**, with 581 actions reported. No clean source-built release payload was completed. |
| WASM runtime | `occt-import-js` JS + WASM + worker: **7,701,220 bytes**, approximately **3,110,807 bytes gzip**. Does not provide the exact-shape operations required by this plan. |
| Python distribution proxy | OCP 8 environment including NumPy/pip: **474,328,049 bytes**. This is not a portable Python installer and excludes additional application integration. |
| Windows and `.deb` installers | **Not built or measured.** Their real added size, signing, clean-machine startup and CI time are implementation gates, not completed investigation results. |

## 2. Architecture and ownership

Use one version-pinned `gdml-cad-worker` executable and private OCCT shared libraries, packaged alongside the backend. The **backend owns its lifecycle** in desktop and browser dev mode. Electron continues to start the backend; the frontend sends bytes and job options. Browser dev mode already has a Rust backend, so client-side WASM is not required. [11]

| Owner | Responsibility |
|---|---|
| C++ worker | STEP/XCAF and later IGES reading, units/tolerance extraction, exact B-rep validity, candidate generation/reconstruction/verification, CAD meshing and CAD intersection diagnostics. Retain exact bodies for per-part retries during the job. |
| Rust import module | STL parsing, one shared mesh validator/limited repair path for STL and worker meshes, job limits/cancellation, import report, matrix-to-GDML conversion, materials/names, `GdmlDocument` construction and atomic commit. |
| Existing backend pipeline | Define evaluation, preview meshes and scene graph, material editing, `serialize_gdml` and export. Factor common document preparation from `load_document`; do not serialize and reparse as the product import architecture. |
| React UI | File selection, settings, progress, staging report, highlighting and overrides. No independent kernel, primitive classifier or geometry acceptance rules. |

Use a small versioned protocol with bounded control messages and binary `f64` vertex / integer index buffers in a backend-owned temporary job directory. Do not send millions of numbers as JSON. Treat worker paths and lengths as untrusted inputs. Start one worker per CAD job; no persistent service, worker pool or plugin framework initially. Store exact shapes only in the worker, referenced by opaque IDs. Cancellation terminates the process and its children when a kernel call cannot cooperate. Clean temporary data on success, cancellation and restart.

Rejected alternatives, one sentence each:

- **OCCT linked into Rust:** same kernel quality, but couples the static-musl backend and every Rust build to C++ ABI/linking, and makes kernel crashes and cancellation application-wide.
- **OCCT/WASM in the browser or Electron renderer:** current mesh-only packaging cannot verify candidates, measured assembly memory is high, and maintaining full current-kernel bindings adds work without satisfying a browser-only requirement.
- **WASM inside the backend:** preserves process portability but retains binding/build work and unmeasured exact-kernel performance, while solving no requirement the native worker cannot meet.
- **Bundled Python/OCP:** convenient spike tooling, but the measured environment is larger than the native library closure and adds Python runtime packaging.
- **Truck as the CAD kernel:** current exercised topology/meshing failures and missing exact-solid verification confidence make it a higher-risk route to correct STEP import.
- **Foxtrot or a home-grown Part 21 reader:** neither supplies the mature exact-geometry operations this task requires.

Scope is STEP `.step/.stp` and binary/ASCII STL. IGES is a later, small reader addition only if users supply legacy IGES workloads; sewing open surface collections and weak structure still need tests, so it is not free. Defer OBJ, PLY and 3MF until demand is established: ngons, shell interpretation, units, materials and object semantics add testing even when triangle parsing is easy. Simulation interchange formats are out of scope.

## 3. Intermediate representation

Keep one import-only IR between reading and GDML construction. It is not a second editable document model or a lossless CAD archive.

| Record | Required fields |
|---|---|
| Source | File name/hash/format, reader version, source units and uncertainty contexts, import settings, warnings and provenance. |
| Body definition | Stable job ID, source label/path, local coordinate frame, exact volume/area/bounds when available, vertex/edge/face tolerances, validity and repair history, optional CAD material/color hints, opaque exact-shape handle. |
| Representation | Accepted analytic primitive or bounded CSG DAG with numeric parameters and active operand matrices; otherwise indexed closed triangle mesh with shell membership and source-face IDs. Include candidate residuals, symmetric difference, volume change and validation status. |
| Occurrence | Stable ID and parent assembly path, definition ID, active `3x3` matrix plus translation in mm, label, optional material/color override. Preserve shared definitions. |
| Diagnostics | Body or occurrence IDs, issue code, severity, locations/pairs, tolerance, measured values, repair/override choices, plus separate states for source validity, representation validity and overlap validation. |

All geometry remains `f64` until preview generation. Preserve matrices until the GDML boundary, not Euler angles in the IR. Mesh inputs have no exact B-rep or inferred source accuracy. A body with multiple disconnected material regions becomes separate bodies unless the source explicitly requires otherwise; an outer shell and its nested void shells stay one body. Multiple bodies in one CAD part need a structural grouping, not a fused solid.

## 4. Recognition and acceptance

### Tolerances

Normalize length and uncertainty units together. Retain STEP's `UNCERTAINTY_MEASURE_WITH_UNIT` per representation and OCCT's actual vertex/edge/face tolerances; reading may increase them. The fixtures have `1e-7 mm` tolerances. The real upper arm declares **0.01 mm**, with imported vertex/edge tolerances up to **0.00910651 mm**. A universal micron threshold is therefore inappropriate. [12]

For a candidate region use `tau = max(source uncertainty, relevant local topology tolerances, numeric floor)`, with a numeric floor such as `max(1e-9 mm, 32 ulp(coordinate scale))`. Start with a proposed recognition distance allowance `epsilon = 5 tau`, but require `epsilon <= 1e-4 D` and `epsilon <= 0.01 f`, where `D` is body diagonal and `f` is the smallest relevant wall/gap/tool size. If the cap cannot be met, allow only demonstrably identical analytic support/topology or decline the approximate fit. Do not silently loosen tolerances to increase recognition. Derive angular allowances from distance over feature span, rather than a fixed degree threshold.

### Worthwhile families

| Order | Recognition algorithm | GDML and exclusions |
|---|---|---|
| 1 | Cluster coplanar faces, identify three orthogonal opposing plane pairs; check complete trimmed boundary and local frame. Merge split faces logically. | `box`; do not require exactly six STEP faces. |
| 1 | Cluster coaxial cylindrical/conical supports and planar ends, infer radii, end stations, axis and angular cut planes from trims. | Full/partial `tube`, `cone`; reject skew axes, missing caps and incompatible trims. |
| 1 | Check spherical/toroidal supports, axis and angular coverage; distinguish inner shells and radial/angular caps. | `orb`, `sphere`, `torus`; no sphere fit based only on mesh samples. |
| 2 | Detect coaxial piecewise linear meridian bounded by planes/cones/cylinders; order stations and preserve concave radial profile. | `polycone` or `genericPolycone`; use tessellation for spline revolutions initially. |
| 2 | Two parallel congruent polygon caps and straight ruled side faces; simple non-self-intersecting section. | `xtru`; sections with holes use verified subtraction or tessellation. |
| 2 | Recognize stock, then cylindrical through/blind drills, counterbores, simple box pockets and a few analytic bosses from boundary supports and adjacency. | Small subtraction/union tree. Extend through-tools only beyond exterior stock faces. Preserve blind floors exactly. |
| Later, only with measured benefit | Planar wedges/trapezoids and restricted eight-vertex shapes. | `trd`, `trap`, `arb8`, after their parameter and planar-face constraints are verified. |

Do not reverse engineer a feature history, remove fillets, approximate threads, or fit arbitrary NURBS to primitives by default. Seam-split analytic faces should be recognized by common support geometry. Analytic-looking B-splines can be considered later with the same acceptance gate. STL remains tessellated initially; a planar box replacement can be exact relative to its mesh, but a curved fit discards the input's faceted boundary and is not automatically faithful.

### Acceptance gate

1. Require a valid, closed source solid. Reject unresolved open shells, missing transfer roots and failed healing; never hide them by counting only successfully read solids.
2. Reconstruct the candidate B-rep from exactly the parameters and transforms intended for GDML. Validate it and compare connected components, cavities, boundary support types and trim coverage. Check parameter restrictions such as positive dimensions and legal angular ranges.
3. Bound boundary deviation with analytic support/trim checks for the supported families. Sample extrema and curves as additional diagnostics, not as a Hausdorff proof. If boundary coverage or a numerical result is ambiguous, fall back.
4. Compute **both** differences `source minus candidate` and `candidate minus source`, without a loose fuzzy Boolean that can erase the discrepancy. Require successful, valid results. Proposed volume allowance: `Vdiff <= min(A * epsilon, 1e-5 * V)`, in addition to the boundary/topology checks. Record both directional volumes, residuals and tolerance. Matching total volume is insufficient.
5. Validate the actual exported representation and placement, including Geant4 regression tests for its parameter family. A kernel comparison cannot catch a reversed GDML rotation or a length/half-length mapping mistake.

This is a conservative acceptance policy using a floating-point CAD kernel, not a theorem of exact equality. A failed recognition attempt simply selects the validated mesh. Keep verified alternate representations for the report; an override may choose tessellation or another accepted candidate, but cannot bypass its geometric acceptance gate.

### CSG cost and proposed bounds

The additional cost spike uses an 80 by 80 by 10 mm plate with 3, 8, 16 or 32 separated radius-2 through holes. It compares sequential subtractions with one subtraction of a voxelized multiUnion of tools. Each document goes through the existing Rust parser/export/preview and 20,000 Geant4 geantinos. All eight pass overlaps/navigation without warnings, adding **160,000 tracks**. This is one sample per configuration; Monte Carlo volume reporting was disabled for these timing runs. The analytic target volume is `64000 - n*pi*2^2*10`.

| Holes | Rust round-trip + preview, chain / multiUnion, s | Geant4 navigation, chain / multiUnion, s | Final preview facets, chain / multiUnion |
|---:|---:|---:|---:|
| 3 | 0.039 / 0.026 | 0.116 / 0.079 | 1,348 / 1,231 |
| 8 | 0.030 / 0.015 | 0.172 / 0.100 | 4,036 / 3,188 |
| 16 | 0.058 / 0.103 | 0.410 / 0.075 | 8,034 / 6,610 |
| 32 | 0.187 / 0.494 | 2.012 / 0.155 | 16,874 / 13,364 |

Geant4 benefits from the multiUnion in this workload; the existing preview folds unions through its BSP and does not share that advantage at 32 tools. Proposed first-release limits are **8 Boolean operations, depth 4, and at most 16 tool instances in a multiUnion**, with per-body preview work/facet limits as an additional gate. Prefer `stock minus multiUnion(tools)` for verified drill arrays. These limits are deliberately below the largest tested case, not a claim that every 16-operand shape is cheap. Intersecting tools, thin slivers and arbitrary orientations need regression tests before broadening them.

## 5. Tessellation and its guarantees

### Defaults and measured error

Proposed starting settings for CAD: absolute chord target `min(0.05 mm, 0.001 D)`, raised to `5 tau` if the source cannot justify that resolution; angular target **0.2 rad**. Show both actual values. Mesh each shared body once. Compare signed mesh material volume with exact B-rep volume and target **at most 0.1% absolute relative difference**. Halve chord and angular targets on failure, down to the source uncertainty floor, within the facet/time budget. This volume target is a quality heuristic, not a surface-distance or containment guarantee.

Measured forced-tessellation results, OCCT 8.0.1, without analytic recognition:

| Fixture | 0.05 mm / 0.2 rad: facets, volume loss | 0.025 mm / 0.1 rad | 0.0125 mm / 0.05 rad |
|---|---:|---:|---:|
| Sphere R=10 | 2,020; 0.71589% | 8,000; 0.18418% | 32,202; 0.04588% |
| Cone frustum | 568; 0.53413% | 1,508; 0.14619% | 4,280; 0.03829% |
| Torus R=20, r=4 | 8,694; 0.30361% | 31,752; 0.08286% | 127,008; 0.02072% |
| Tube R=10, r=8 | 504; 0.16570% | 1,008; 0.04144% | 2,016; 0.01036% |
| B-spline roof | 620; 0.10727% | 1,192; 0.07011% | 2,720; 0.03031% |
| Rounded block | 2,796; 0.02530% | 9,452; 0.00648% | 34,348; 0.00168% |

Box, planar extrusion and planar cavity have zero tessellation volume error. The counterbored plate's mesh instead **gains 0.01052%** at the first setting because faceted bores add material. Even a mesh with net volume loss can protrude locally. Recognition avoids both the curved-surface error and thousands of facets.

### One shared mesh validation path

Use CAD edge identity and compatible edge discretizations before proximity welding. Then use a neighbour-cell spatial weld with a bounded displacement, not rounded-cell equality or a transitive chain that can collapse a feature. Never weld between separate bodies. For CAD, tie the permitted movement to local modelling tolerance and wall/gap scale. For STL, use a visible proposed numerical weld tolerance `max(1e-9 mm, 1e-7 D)` only where topology is preserved; STL has no declared modelling uncertainty. A larger repair tolerance is an explicit geometry-changing override.

After welding, remove repeated-index and too-small/too-narrow facets, duplicate facets and zero-area triangles, then **revalidate closure**. Use Geant4's edge-length and minimum-altitude criteria, with a margin above its surface tolerance, rather than area alone. The SO100 failure demonstrates why deleting degenerates cannot be the final step. Repair from CAD topology or remesh; do not invent patches across unknown holes. Limit automatic sewing/p-curve repair to the source tolerance, record its displacement and volume effect, and revalidate the repaired B-rep. A larger geometry-changing repair requires an explicit override and the same checks. [13]

Require finite coordinates, bounded indices, exactly two oppositely directed faces per edge, manifold vertex links, consistent orientation, nonzero material volume, and no improper triangle intersections. A BVH supplies broad phase; adaptive orientation predicates and explicit coplanar/shared-edge cases supply narrow phase. `robust` is a predicate dependency, not evidence that this complete validator already exists. Check disconnected shell containment with unambiguous ray/winding tests; reject intersecting or touching shell ambiguities. Outer shells face out of material and nested cavity shells face into the void. Do not split a cavity into a second positive body. Recheck all of this after any repair.

Proposed soft budgets: **250,000 facets per body, one million unique facets per import**, plus a **2 GiB import-job memory ceiling** and user-visible time limit. These are initial product limits, not benchmark-proven safe maxima: the existing Rust model, XML defines, preview expansion and browser buffers create several copies. Estimate those copies before allocating. Offer finer/coarser settings or explicit exclusions when a budget is hit, never silent decimation or missing parts. Preserve the existing document until the staged import is valid. Measure large-file upload, export and reopen before committing these limits.

### Conservative meshing and overlaps

For an exact annular bore `R=10, r=8, h=20 mm` and a touching shaft `r=8`, ordinary independently inscribed polygons with half-step phase offset overlap. The CAD intersection is zero. Replacing the bore's inner polygon by a circumscribed polygon makes the bore material a subset of its CAD solid; the inscribed shaft is also a subset.

| Segments n | Total facets | Ordinary intersection, mm³ | Ordinary bore/shaft loss | Conservative bore loss | Conservative intersection |
|---:|---:|---:|---:|---:|---:|
| 32 | 384 | 9.642808 | 0.64131% each | 2.35480% | 0 |
| 64 | 768 | 2.419448 | 0.16056% each | 0.58893% | 0 |
| 128 | 1,536 | 0.605409 | 0.04015% each | 0.14725% | 0 |

At 64 segments the maximum ordinary bore intrusion is **0.00963635 mm**. Conservative shaft loss remains 0.16056%. The analytic areas are `n R^2 sin(2 pi/n)/2` for the outer inscribed polygon and `n r^2 tan(pi/n)` for the circumscribed hole. All six mesh cases were also loaded and checked in Geant4, with the expected failures and passes.

**Do not make universal conservative meshing a first-release promise.** Convex analytic surfaces and this extruded-circle construction admit explicit containment arguments. Concave patches, saddle surfaces, trim boundaries and thin walls do not become safe merely by placing vertices inside, shifting along normals or shrinking toward a centroid. General certification needs bounded surface patches or a much more expensive volumetric/interval method and can erase small features. The volume deficit and the material filling new gaps can affect simulation results; gaps are not universally physically benign, contrary to the brief's shorthand.

Default to accurate meshes with validation. Detect original CAD interferences using occurrence AABBs followed by exact-kernel intersections and tolerance-aware contact classification. Check generated representations separately, including containment and coplanar contacts, so tessellation-created intersections can be distinguished from source defects. Refine the implicated parts or use verified primitives. If contact cannot be resolved within tolerance/budget, present a blocking diagnostic and the available overrides. A later **local conservative option** may support only certified families, with per-part volume deficit and resulting gap size. A clean CAD assembly alone never earns an overlap-free label.

For final pair checks, Rust's mesh BVH handles tessellated pairs, with triangle intersections plus containment tests to catch a wholly enclosed body. The worker checks analytic/CSG pairs against their reconstructed B-reps. For a potentially colliding mixed pair, construct a faceted B-rep of the validated mesh and use kernel intersection, with a strict work budget; this expensive fallback is a phase-4 performance gate. An inconclusive check stays unresolved. Do not check against the approximate viewer mesh and call the actual analytic GDML solid validated.

A committed normal import requires a validated closed oriented representation with reported approximation metrics and no unresolved checks. Finite sampling does not establish mathematical non-overlap or universal surface error. Reports must say which checks ran, which were inconclusive and whether Geant4 itself was run; the desktop need not bundle Geant4 to perform the import.

## 6. Mapping into `GdmlDocument`

The current model already has the needed solid, assembly, placement, reflection, auxiliary and material types. `DocumentOrder::define_slots` can be empty for generated documents, and the serializer orders dependencies. Import should construct these objects directly, prepare preview data off the application lock, then atomically replace the document with `unsaved=true`. The current load path tolerates preview tessellation failures as warnings with missing meshes; imports must treat a missing required preview or failed geometry validation as a staging failure. [11]

| Concern | Mapping and rule |
|---|---|
| World | Generate a `BoxSolid` and ordinary world `Volume`. Keep source coordinates by default; each half-extent is `max(abs(min), abs(max)) + margin`, with proposed margin `max(10 mm, 0.01 scene diagonal)`. Offer recentering for distant origins. World bounds include every transformed occurrence and mesh. |
| Assemblies | Preserve CAD hierarchy with `Volume.assembly=true`, no material or artificial envelope. Geant4 imprints assemblies, so retain original paths in the report. Offer flattening as an export/compatibility choice, composing transforms once. Do not use an assembly as a material-bearing mother. |
| Instances | One solid per body definition, one logical volume per `(definition, material)` combination, multiple `PhysVol`s. An instance material override clones the logical volume, not its solid. Assign stable names and copy numbers; preserve instance-to-source mapping in the report and parent-volume auxiliary metadata because `PhysVol` has no auxiliary field today. |
| Multiple bodies | Split compound solids into separate definitions under their part grouping. Report open sheets/edges explicitly; distinguish non-geometric empty product nodes from failed solid transfers. Resolve external STEP references only from supplied files and report missing references. Never silently omit a body. |
| Units | All lengths and positions in mm, angles explicitly rad or deg. Convert representation-local units and uncertainty before composition; never assume the first STEP context applies globally. STL defaults to mm, with dimensions and a unit override displayed. |
| Tessellation | Shared `Position` definitions in mm, `TessellatedSolid` with `ABSOLUTE` triangular references and consistent material-outward winding. Omit facet `lunit`: Geant4's per-facet unit handling can multiply already-scaled positions. A solid-level tessellated unit is not a substitute. [17] |
| Coordinates | Rebase body-local vertices near their own origin while composing compensating occurrence translations. Keep exact numeric values in `f64`; do not construct GDML from Three.js or `f32` preview buffers. Flag very large scene offsets before preview precision is lost. |
| Names | Prefix by kind and stable source ID, restrict to `[A-Za-z_][A-Za-z0-9_]*`, remove the literal substring `0x`, bound length, resolve collisions deterministically in every namespace. Preserve the original Unicode label as escaped auxiliary text/report data. Geant4's default reader strips names at `0x`. [16] |
| Materials | Explicit world vacuum and clearly named `CAD_UNASSIGNED` part placeholder by default, with an unassigned-material banner. It is a syntactically valid low-density placeholder, not an inferred physical material. Allow default and per-part NIST assignment. Preserve CAD density/designation as hints requiring confirmation; color does not identify a material. |
| Material implementation | Reuse the 309-entry NIST table. Its definition-building logic currently lives in `frontend/src/utils/nistImport.ts`; move that preparation to one backend helper used by import and the existing UI, rather than translating it independently into C++. |
| Save | Existing `export_gdml` / `serialize_gdml`, desktop save IPC and browser download. Suggest the source basename with `.gdml`. Keep the original CAD untouched. Store concise provenance/report summary in supported auxiliaries, not the exact CAD payload. |

### Rotation and reflection convention

For active CAD rotation `M`, Geant4 builds `R = Rz(z) Ry(y) Rx(x)` from GDML angles. A physvol places with **`R.inverse()`**, whereas boolean second operands and multiUnion nodes use **`R`**. Therefore decompose **`M^T` for physvol**, and **`M` for boolean/multiUnion**. Do not negate three general Euler angles independently. Implement this once at the backend GDML boundary and test the reconstructed matrix, including near-gimbal-lock cases. Existing Three.js placement uses the corresponding inverse convention. [15], [17], [18]

For an orthogonal reflected occurrence, factor `M = Q S`, where `S = diag(-1,1,1)` and `Q = M S` is a proper rotation. Emit the physvol rotation for `Q^T` and sign-only scale `S`. Geant4 may represent this using a different reflection plane plus a compensating rotation, so validate the **composed transform**, not the reported rotation alone. For baked reflected meshes, reverse winding. Non-unit scale/shear must be baked into a new definition and revalidated or rejected; the current scene/Geant4 reflection placement path is not arbitrary affine scaling. Bake mirrored boolean operands into a reflected definition or use tessellation, rather than pretending an improper matrix is a rotation. [15], [19]

## 7. User flow

**Open is the initial feature.** Extend the existing file selector to GDML, STL and STEP and preserve its unsaved-work handling. A recognized CAD file starts a staged import with sensible defaults; no mandatory expert dialog. Add an optional **Import options** control before or during staging: units for STL, default material, chord/angular quality, recognition on/off, assembly preservation and resource limits. Display the chosen settings after import so one click remains reviewable.

Use binary/streamed upload, not `File.text()` or the existing JSON map for binary STL. Proposed routes are create job, read progress/report, update selected-part options, commit and cancel; keep the job state small and backend-owned. The current **64 MiB request limit** also affects reopening large exported GDML through JSON. Fix upload/reopen buffering and explicit limits in the same phase, instead of introducing a CAD path that exports files the application cannot reopen. [11]

The report has one row per definition, expandable to occurrences: original name/path, chosen representation, recognized alternatives/rejection reason, source tolerance, exact and output volume with signed change, facet count, repairs and maximum movement, material, source collisions and generated-geometry collisions. Selecting a row highlights its parts. Show a clear overall state: ready, approximation warning, material unassigned, invalid, cancelled or validation incomplete. Include a per-part reason when it cannot be imported.

Overrides are **Auto**, **Tessellated**, accepted primitive/CSG choice, quality, unit/material and explicit exclusion. Recompute only affected definitions and incident overlap checks, preserve current material choices by stable IDs, then rebuild the staged document atomically. A requested primitive that fails verification stays unavailable with the measured reason. Excluding a part is a visible user choice. Do not silently substitute an empty mesh, fill a hole or remove an interfering fastener.

Progress phases are reading/transferring, classifying, meshing, validating and preparing preview, with body/byte counts where available. Use indeterminate progress inside opaque kernel calls. Cancellation keeps the prior document and selection intact; propose a tested one-second worker-termination target, bounded memory and a configurable job timeout. Retry with coarser quality or exclusions after a budget failure. Preserve committed report metadata with the document; retain exact worker data only for the active import/override session, with a clear need to reread the source after it is released.

**Insert into volume is a later phase.** Select an ordinary mother volume, preview a rigid transform, and validate every inserted daughter is contained and does not intersect existing siblings. Import the CAD contents, not its generated world. Reuse existing definitions/materials only after explicit name/identity checks, and apply all changes as one reversible transaction. An arbitrary tessellated mother needs actual containment tests, not its bounding box; defer unsupported containment rather than mislabel it safe.

## 8. Packaging and CI

Keep a single worker protocol and kernel version across platforms. Installer size budgets below are provisional planning estimates; only the prototype Linux payload in section 1 was measured.

| Platform | Proposed packaging/build | Measurement status and acceptance gate |
|---|---|---|
| Windows installer | MSVC x64 release worker, private OCCT DLLs and required runtime beside the packaged backend; electron-builder `extraResources`. Build a headless kernel with only the toolkit dependency closure. | No Windows build here. **Estimate:** 20–45 MiB added compressed, 100–250 MiB installed until measured. Test clean supported Windows VM, Unicode/space paths, no development runtimes, import/save/reopen, cancellation and app shutdown. |
| Ubuntu 20.04+ `.deb` | Keep the Rust backend static-musl. Build the separate worker and C++ dependencies against glibc 2.31 baseline, private libraries and `$ORIGIN` RPATH, no global `LD_LIBRARY_PATH` change. Disable unnecessary graphics/image dependencies. | Process-boundary spike passes on this host, but current worker needs glibc 2.34. **Estimate:** 20–35 MiB compressed, 100–220 MiB installed. Gate on actual package delta, `readelf` symbol requirements, dependency audit, clean Ubuntu 20.04 launch without a development environment. |
| Browser dev | Vite continues to call Axum; backend discovers an adjacent worker or an explicit development path. Provide a pinned development binary or documented CMake build, with a clear unavailable-kernel error. STL needs no worker. | Native and Node-WASM measurements are available; browser UI and worker discovery were not exercised. No WASM download is required by the chosen design. |

Cache OCCT builds by source hash, toolchain and options. Compile the small worker separately; do not rebuild OCCT on each Cargo change or npm install. Windows/Linux installers need their own CI jobs, which current CI does not provide. Share the source manifest, notices, patches and build recipe. Once the source-license conflict is resolved, preserve LGPL notices and the OCCT exception, provide the corresponding library source/build information and permit replacement of dynamically linked libraries. A process boundary does not erase dependency licensing. [1], [14]

CI layers, each with a concrete purpose:

1. **Fast Rust tests:** STL size detection, finite data, manifold/vertex-link/coplanar intersection cases, cavities, winding, deterministic names, matrices, document round-trip and resource limits. Test the shared validator once, not separate STL/STEP copies.
2. **Pinned CAD integration:** regenerate analytic fixtures, check exact-volume/bounds formulas, units and transfer counts, rejected lookalikes, mirrored/nested/repeated occurrences, timeout/cancel and malformed CAD. Keep public real models as pinned downloads with SHA-256 verification, not large repository fixtures.
3. **Geant4 integration:** 11.4.2 GDML read, offline XSD, warnings as failures for valid cases, overlap checks, analytic/MC-aware volume and bound comparisons, cavity/location probes, deterministic geantinos including grazing and near-boundary tracks. Add a 10.7 compatibility runtime job because the brief requires it; it was not executed here.
4. **Packaged application:** Windows and Ubuntu 20.04 clean-machine imports, preview, material override, export/reopen, installer delta, whole-app peak memory, large files near limits and worker cleanup. Tests must observe the actual saved GDML and final preview, not just IR values.

## 9. Phased implementation

Effort estimates are **engineer-days for one engineer familiar with this repository**, including tests and integration, excluding upstream licensing response time. Resolve dependency distribution and platform feasibility early while STL work proceeds. These are estimates, not measured implementation times.

| Phase | Scope and exit acceptance | Estimate | Principal risk |
|---|---|---:|---|
| 1. STL vertical slice | Byte upload, mesh IR/validator, bounded safe repairs, shell nesting, generated world, direct `GdmlDocument`, report/material defaults and save/reopen. All 14 current STL fixtures produce the expected pass/reject result; add self-intersection, non-manifold vertex, nan/inf, truncated counts, extreme units and cavity-island cases. Geant4-valid positives have no warnings. | **8–12 d** | A robust coplanar/self-intersection validator and memory bounds are substantial work; a triangle parser is not the feature. |
| 2. Worker and platform proof | Versioned native worker, pinned OCCT, source-license clearance, exact units/validity/structure IR, crash/cancel. Actual Windows and Ubuntu 20.04 packages pass clean-machine smoke tests and record added size. | **6–10 d** | License ambiguity, baseline ABI, kernel build closure and installer dependencies. |
| 3. STEP tessellation and report | AP203/AP214/AP242 solids, multi-body parts, shared instances/nesting, transformations/reflections, source interferences, CAD mesh repair/remesh, final mesh validation, per-part retries. All generated fixtures and real part round-trip; both real assemblies yield complete, honest validity reports with no silently missing bodies. Resolve or explicitly reject the two SO100 open meshes. | **10–15 d** | Real CAD topology, healing changing geometry, large-file memory, contact validation. |
| 4. Analytic recognition | Box/tube/cone/sphere/torus, then polycone and extrusion; reconstruct/check gate with boundary/topology and both differences. Accept intended exact cases; reject wrong-cavity and counterbore-floor cases; validate generated analytic/mixed pairs and rotated/split-face/near-match variants against Geant4. | **6–9 d** | False recognition from tolerances, trim coverage and incorrect parameter conventions. |
| 5. Bounded CSG and quality controls | Drills/counterbores/pockets/bosses under measured limits, user overrides, adaptive refinement and complete generated-representation intersection handling. Benchmark both Geant4 and the existing preview at the limit; contact cases must not quietly pass. | **5–8 d** | Viewer BSP growth, coincident surfaces and costly verification. |
| 6. Insert into volume | Placement preview, actual containment/sibling checks, material/name mapping and atomic undo/commit. Tests include rotated mothers, nonconvex mothers, invalid insert and cancellation. | **4–6 d** | Containment inside arbitrary existing GDML/CSG geometry. |
| Optional IGES | Enable the existing worker's IGES reader only after representative user files exist; classify open surfaces honestly, sew within declared tolerances, pass the same solid/mesh acceptance path. | **2–4 d** | Input is often surfaces rather than closed solids; marginal reader cost understates repair cost. |

Core STEP/STL release through phase 5: **35–54 engineer-days**, approximately 7–11 working weeks. Insert and IGES are additional. Universal conservative meshing is excluded from this estimate; certify a restricted family only when its benefit exceeds exact primitive recognition. No phase is accepted merely because the viewer draws a plausible object.

## 10. Decisions only the user can make

1. Is the measured order of **23 MiB compressed / 198 MiB installed** for a prototype native dependency payload acceptable, subject to a smaller audited production build and actual installer measurements?
2. Is an LGPL dependency acceptable, and is a commercial license an option if the current source-notice conflict cannot be cleared? Upstream clarification itself is engineering follow-up, not something the user should guess.
3. What volume/mass error and clearance change are acceptable for the intended simulations? The proposed 0.1% volume target is a default, not a physics requirement; conservative gaps also need a material policy.
4. Should unassigned imported parts use a conspicuous vacuum-like placeholder, or is there a domain-specific default material? Geometry can import in one click without inventing composition.
5. What is the largest expected assembly and lowest supported machine, and are insertion and legacy IGES actually needed for the first release?

## Sources and reproducibility

The accompanying `cad-import-evidence.zip` contains throwaway spike source, raw result JSON/logs, dependency metadata, the Geant4 validator and a reproduction guide. It is kept outside the repository. Headline tables cite `results/benchmark-final.json`, `conversion.json`, `quality.json`, `conservative.json`, `validation*.json`, `so100-*.json`, `sizes.json`, `stripped-size.json` and `abi.json` in that archive. Initial failures are retained with their corrected results. No production binaries or third-party CAD models are included.

The retained fixture generator is [fixtures/generate.py](fixtures/generate.py); [fixtures/README.md](fixtures/README.md) gives the pinned environment and independent oracles. Generate files to a directory outside the repository. Real-file SHA-256 values:

| File | SHA-256 |
|---|---|
| `Upper_arm_SO101.step` | `efa19a6dd2ccb459248500c76629cfa840630e7e15d9e146394d31da1525dd61` |
| `SO_5DOF_ARM100_Assembly.step` | `6dda8ce9858b2a460082eb192678ba4dcfc062d8bbacc160111bf2019b8c6658` |
| `SO101 Assembly.step` | `f754be35708382b05ce65768ae33c612f91dca11ad19d14807dc637eb944ba3d` |

[1]: https://github.com/Open-Cascade-SAS/OCCT/releases/tag/V8.0.1
[2]: https://crates.io/crates/opencascade/0.3.0
[3]: https://github.com/kovacsv/occt-import-js/releases
[4]: https://www.npmjs.com/package/opencascade.js
[5]: https://github.com/ricosjp/truck
[6]: https://github.com/Formlabs/foxtrot
[7]: https://github.com/CadQuery/OCP
[8]: https://github.com/georust/robust
[9]: https://github.com/TheRobotStudio/SO-ARM100/tree/a758567c3978dfeefe282ede0500085a48fe8f78/STEP
[10]: https://geant4.web.cern.ch/documentation/dev/bfad_html/ForApplicationDevelopers/Detector/Geometry/geomOverlap.html
[11]: ../../backend/src/gdml/model.rs
[12]: https://occt3d.com/dev/doc/overview/html/occt_user_guides__step.html
[13]: https://github.com/Geant4/geant4/blob/v11.4.2/source/geometry/solids/specific/src/G4TriangularFacet.cc
[14]: https://github.com/Open-Cascade-SAS/OCCT/issues/1564
[15]: https://github.com/Geant4/geant4/blob/v11.4.2/source/persistency/gdml/src/G4GDMLReadStructure.cc
[16]: https://github.com/Geant4/geant4/blob/v11.4.2/source/persistency/gdml/src/G4GDMLRead.cc
[17]: https://github.com/Geant4/geant4/blob/v11.4.2/source/persistency/gdml/src/G4GDMLReadSolids.cc
[18]: https://github.com/Geant4/geant4/blob/v11.4.2/source/persistency/gdml/src/G4GDMLReadDefine.cc
[19]: https://github.com/Geant4/geant4/blob/v11.4.2/source/geometry/volumes/src/G4ReflectionFactory.cc

Additional primary references: [OCCT license](https://github.com/Open-Cascade-SAS/OCCT/blob/V8.0.1/LICENSE_LGPL_21.txt), [OCCT exception](https://github.com/Open-Cascade-SAS/OCCT/blob/V8.0.1/OCCT_LGPL_EXCEPTION.txt), [published WASM metadata](https://registry.npmjs.org/occt-import-js/0.0.23), [stl_io source/license](https://github.com/hmeyer/stl_io), [Geant4 define rotations](https://github.com/Geant4/geant4/blob/v11.4.2/source/persistency/gdml/src/G4GDMLReadDefine.cc), [Geant4 solids/booleans/multiUnion/facet reader](https://github.com/Geant4/geant4/blob/v11.4.2/source/persistency/gdml/src/G4GDMLReadSolids.cc), [Geant4 tessellation](https://github.com/Geant4/geant4/blob/v11.4.2/source/geometry/solids/specific/src/G4TessellatedSolid.cc), [Geant4 reflection factory](https://github.com/Geant4/geant4/blob/v11.4.2/source/geometry/volumes/src/G4ReflectionFactory.cc).

Repository integration was checked in [handlers.rs](../../backend/src/api/handlers.rs), [tessellator.rs](../../backend/src/mesh/tessellator.rs), [materials.rs](../../backend/src/gdml/materials.rs), [main.rs](../../backend/src/main.rs), [Toolbar.tsx](../../frontend/src/components/Toolbar.tsx), [Scene.tsx](../../frontend/src/components/Viewport/Scene.tsx), [nistImport.ts](../../frontend/src/utils/nistImport.ts), [desktop/main.js](../../desktop/main.js), [desktop/package.json](../../desktop/package.json) and [README.md](../../README.md). Source links are pinned where versions affect behavior; mutable documentation and registry state were checked on the date above.
