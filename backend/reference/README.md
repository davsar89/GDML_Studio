# Geant4 reference sources

Read-only copies of Geant4 sources, kept so the backend's behaviour can be
checked against the reference implementation instead of against guesswork.
Nothing here is compiled or linked — `cargo` never sees this directory.

**Version: Geant4 10.7.4 (`geant4.10.07.p04`), all files from one release.**

Taken from `source/` in the upstream tarball:

| File | Upstream path | Answers |
|---|---|---|
| `G4GDMLRead.cc` | `persistency/gdml/src/` | `<loop>` semantics (`LoopRead`) |
| `G4GDMLReadDefine.cc` | `persistency/gdml/src/` | `<define>` reading order, `<quantity>` units |
| `G4GDMLReadSolids.cc` | `persistency/gdml/src/` | solid attribute names and defaults |
| `G4GDMLReadStructure.cc` | `persistency/gdml/src/` | volumes, physvols, replicas, multi-file modules |
| `G4Polycone.cc` | `geometry/solids/specific/src/` | phi opening-angle rule, decreasing-z handling |
| `G4Polyhedra.cc` | `geometry/solids/specific/src/` | apothem vs circumradius convention |
| `G4TwistedTubs.cc` | `geometry/solids/specific/src/` | hyperboloidal surface, `zlen` parameterisation |
| `G4ExtrudedSolid.cc` | `geometry/solids/specific/src/` | xtru triangulation |
| `G4PVReplica.cc` | `geometry/volumes/src/` | replica axis validation |
| `G4ReplicaNavigation.cc` | `geometry/navigation/src/` | replica placement transforms |

## Why the version matters

Comments and tests across the backend cite these by line number
(`G4Polycone.cc:232`, `G4GDMLReadDefine.cc:601`). An earlier set mixed releases,
so a citation could be checked against a file that no longer said the same
thing. Keeping every file from a single release makes those references
verifiable.

If you replace these with a different Geant4 version, re-check the citations:

```bash
grep -rn "G4[A-Za-z]*\.cc:[0-9]" --include=*.rs ../src ../tests
```

Files are stored with upstream's LF endings. Line-ending normalisation would not
move any line numbers, so it is harmless here — only replacing a file with a
different release is.
