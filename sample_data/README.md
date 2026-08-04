# Sample GDML Data

Ten GDML files are available in this directory. All follow the GDML schema and
contain `<define>`, `<materials>`, `<solids>` and `<structure>` sections.

| File | Size | Description |
|------|------|-------------|
| `BgoDetModel_v2_00.gdml` | 160 KB | BGO detector model |
| `NaiDetModelWithMLI_v2_00.gdml` | 169 KB | NaI detector model with MLI |
| `solids.gdml` | 15 KB | Widest solid-type coverage — exercises most GDML primitives |
| `pinhole_lab.gdml` | 13 KB | Boolean/CSG-heavy geometry with nested replicas |
| `pod_asm.gdml` | 12 KB | POD assembly; includes materials with `state="gas"` |
| `fermi_simple_elements_satellite.gdml` | 7.6 KB | Fermi satellite simple geometry |
| `test_all_features.gdml` | 6.5 KB | Test file exercising all solid types |
| `test_modular_child.gdml` | 1.6 KB | Child module for the `<file>`-inclusion demo |
| `test_modular_mother.gdml` | 1.4 KB | Parent module — open this one to test multi-file loading |
| `pod_asm_tessellated.gdml` | **6.7 MB** | Fully tessellated POD assembly (44,408 facets) |

> `pod_asm_tessellated.gdml` is by far the largest file here — larger than
> everything else in the repository combined. It is the stress-test fixture for
> tessellated-solid handling; expect a noticeably slower load.

## Provenance

These geometries are third-party in origin and are included as test fixtures:

- `BgoDetModel_v2_00.gdml`, `NaiDetModelWithMLI_v2_00.gdml` — GRESS mass-model
  files (Fermi/GLAST lineage), per their own file headers.
- `fermi_simple_elements_satellite.gdml` — GDML translation of a Fastrad file,
  produced by the GDML module of Fastrad 3.6.1.0, per its file header.
- `solids.gdml` — the Geant4 GDML persistency test file.

Redistribution terms for these files have not been established; treat them as
test fixtures rather than as part of the project's own source.
