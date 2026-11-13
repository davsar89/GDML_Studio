# CAD import test fixtures

Small STEP and STL inputs for the CAD importer tests, with analytic expectations in
`expected.json`. They were produced by [`doc/cad-import/fixtures/generate.py`](../../../../doc/cad-import/fixtures/generate.py)
(see its README for the pinned environment) during the 2026-11-13 investigation. Every expected
volume, bound and matrix comes from a formula, never from reading the generated files back.

To regenerate, run the generator into an empty directory outside the repository and copy the
files listed here. Regeneration with another OCCT version may change the STEP text and
triangulations while representing the same geometry; the expectations stay valid.

Each STEP part's `family` in `expected.json` is what the importer must make of it: a GDML
solid, `<stock> - <n> cut-outs`, or `tessellated` (the lookalikes `eccentric_bore` and
`plate_17_holes` must stay meshes). `shaft_in_bore.step` holds a tube and a shaft that touch
along the bore without interfering.

The large curved STL outputs of the generator (sphere, torus, tube, bspline_roof) are not kept:
curved geometry is covered by the STEP fixtures.

| STL | Expected import result |
|---|---|
| `box_ascii.stl`, `box_binary.stl` | 1 body, 12 facets, 6000 mm³ |
| `binary_solid_header.stl` | binary despite a header starting with `solid`; as `box_binary.stl` |
| `unwelded.stl` | binary box, every facet with its own vertex copies; welds to 8 vertices |
| `one_flipped.stl`, `all_flipped.stl` | box with reversed winding; repaired to 6000 mm³ |
| `with_degenerate.stl` | box plus one facet with three identical vertices; the facet is dropped |
| `multi_solid.stl` | two ASCII `solid` blocks, boxes 30 mm apart; 2 bodies |
| `closed_cavity.stl` | cube of side 10 with a cubic void of side 6; 1 body, 784 mm³ |
| `open.stl` | box missing one facet; rejected as open |
