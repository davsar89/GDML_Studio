# gdml-cad-worker

The STEP reader of GDML Studio's CAD import (`doc/cad-import/PLAN.md`, algorithms C to F and I):
a small C++17 program on [Open CASCADE Technology](https://github.com/Open-Cascade-SAS/OCCT)
(OCCT). The backend runs it for each import (`backend/src/cad/worker.rs`), with the STEP file
on stdin. It writes on stdout, as JSON, the assembly tree (labels, components and their
placement matrices), for each solid its validity, exact volume, area and bounds, a closed
triangle mesh stitched along the B-rep edges, and its exact GDML solid (a primitive, or a
stock minus up to 16 primitive cut-outs, each with its frame) when `certify.cpp` proves it
equal to the solid, or why not. It also writes the placed solids whose interiors overlap
(`BRepAlgoAPI_Common` on up to 8 threads, for at most 60 s; the pairs left over are counted)
and those closer than their meshes' chordal deviations.
The Rust serde structs in `worker.rs` own that format.

```
gdml-cad-worker [--volume-target X] [--chord <label entry>.<solid>=<mm>]... [--only]
```

Meshes are refined until their volume is within `X` (default 1e-3) of the exact one;
`--chord` fixes the chordal deviation of a solid instead, and `--only` meshes just the solids
given with `--chord` and skips the overlap check. Progress goes to stderr as
`progress <phase> <done> <total>` lines (phases `read`, `mesh`, which includes recognition,
and `interfere`). Exit codes: 0 done, 2 bad arguments, 3 unreadable file, 4 internal error,
with `error <message>` on stderr.

## Build (Linux)

```bash
cad-worker/build-linux.sh
```

needs Docker. It builds in Ubuntu 20.04 (so the result runs on glibc 2.31 and newer):
OCCT once into `third_party/` (downloaded and checked against `occt-source.txt`, configured by
`occt-options.cmake`; about 30 minutes), then the worker and `recognize-test` (run by
`ctest`: the certificate on known solids and near misses), and stages `dist/cad/` with the
worker, the OCCT libraries it loads (`lib/`, found through its `$ORIGIN/lib` RUNPATH) and the
licenses. The desktop package ships `dist/cad/` as `resources/cad/`; for development, point
the backend at it with `GDML_CAD_WORKER=$PWD/cad-worker/dist/cad/gdml-cad-worker`
(`run.sh` does this when it exists).

Windows (Phase 6) uses the same `occt-options.cmake` and `CMakeLists.txt` with MSVC.

## License

OCCT is used unmodified as shared libraries under the LGPL 2.1 with the Open CASCADE
exception; `build-linux.sh` writes the notice into `dist/cad/licenses/`.
