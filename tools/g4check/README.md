# g4check

Development tool, not part of the application: loads a GDML file in Geant4 and reports what
Geant4 makes of it. It is the authority for the CAD importer's output (see
[`doc/cad-import/PLAN.md`](../../doc/cad-import/PLAN.md)).

Per file it prints, one JSON object per line:

- `G4_SOLID`: every solid's Geant4 volume (Monte Carlo estimate for Booleans), bounds and facet count;
- `G4_PLACEMENT`: every placement's `CheckOverlaps` result (10,000 points, zero tolerance) and its
  rotation matrix and translation;
- `G4_RESULT`: load, overlap and navigation times, overlap and stuck-track counts, Geant4
  warnings and errors. Geant4 diagnostics go to stderr as `G4_DIAGNOSTIC`.

The exit status is 1 when Geant4 warns or fails, finds an overlap or a geantino gets stuck.
Overlap sampling finds problems; it cannot prove their absence.

```bash
source <geant4 install>/bin/geant4.sh   # any Geant4 built with GDML
tools/g4check/run.sh file.gdml [more.gdml ...]
```

The first run builds into `tools/g4check/build/`. Schema validation is off (it would fetch the
XSD). For Geant4 10.7 compatibility, run the same tool against a 10.7 install, for example on Olivia.
