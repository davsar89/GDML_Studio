# Analytic CAD import fixtures

`generate.py` is reusable development tooling, not an application dependency. It writes STEP, STL and `expected.json` to the output directory supplied on the command line. Keep generated files outside the repository.

Tested environment: Python 3.12.14, `cadquery-ocp==7.8.1.1.post1`, `numpy==2.3.5`. The generator uses OCP directly; CadQuery itself is not required. A different OCCT version may produce different STEP text and triangulations while representing the same analytic geometry.

```sh
python3.12 -m venv /tmp/gdml-fixture-env
/tmp/gdml-fixture-env/bin/pip install -r doc/cad-import/fixtures/requirements.txt
/tmp/gdml-fixture-env/bin/python doc/cad-import/fixtures/generate.py /tmp/gdml-cad-fixtures
```

The generator obtains **no expected volume or bound from the importer or kernel mass-property routines**. It constructs geometry with OCP but calculates the truth independently from dimensions, integrals and transformed analytic vertices. Compare source-import results with `expected.json`; do not regenerate expected values by reading the generated STEP.

## Expected geometry

All dimensions below are millimetres except the explicit inch fixture. Centered solids use their natural origin; the extrusion uses its listed polygon, and the roof has its bottom at z=0.

| Fixture | Independent volume, mm³ | Independent bounds or construction |
|---|---|---|
| Box | `10*20*30 = 6000` | `[-5,-10,-15]` to `[5,10,15]` |
| Tube | `pi*(10²-8²)*20 = 720*pi` | x/y ±10, z ±10 |
| 270-degree tube sector | `3/4*pi*(10²-4²)*12 = 756*pi` | x/y ±10, z ±6 |
| Cone frustum | `pi*20*(10²+10*5+5²)/3` | x/y ±10, z ±10 |
| Sphere | `4*pi*10³/3` | all axes ±10 |
| Torus | `2*pi²*20*4² = 640*pi²` | x/y ±24, z ±4 |
| Polycone | `pi*(1000+1750/3)` | r=10 at z=-10 and 0, r=5 at z=10 |
| Concave extrusion | `1500` | Polygon `(0,0),(20,0),(20,5),(5,5),(5,15),(0,15)`, z ±5 |
| Counterbored plate | `40*30*10 - 228*pi` | Two r=3 through holes; r=5 counterbore of depth 3 over one hole |
| Rounded block | `abc+2r(ab+ac+bc)+pi*r²(a+b+c)+4*pi*r³/3` | Outer 20 by 16 by 12, r=2, inner a=16,b=12,c=8 |
| B-spline roof | `320*(10+4*3/9)` | `z=10+3*(1-(x/10)²)*(1-(y/8)²)` on x ±10,y ±8, bottom z=0; height max 13 |
| Closed cavity | `10³-6³ = 784` | Outer cube side 10, inner cube side 6; inner shell orientation matters |
| Inch box | `6*25.4³ = 98322.384` | 1 by 2 by 3 inches, centered |
| Shaft | `pi*8²*20` | Exact contact with tube cavity |
| Interfering shaft | `pi*8.1²*20` | Analytic intersection with tube `pi*(8.1²-8²)*20` |
| Nested assembly | `2*6000+1500 = 13500` | Two shared boxes, nested rotations about (1,2,3) and (2,1,3), mirrored asymmetric extrusion; bounds from transformed vertices |

The STEP writer must set both declared units and coordinate conversion for the inch fixture. Changing only the global unit label caused an incorrect scale during the investigation and was caught by these independent expectations.

## Mesh cases

STL cases include binary, ASCII, a binary header beginning with `solid`, two ASCII `solid` blocks, duplicate per-facet vertices, one reversed face, all reversed faces, one extra degenerate facet, a missing face, and curved/cavity inputs. Stored STL normals are intentionally zero; derive orientation from winding. `open.stl` must be rejected. An outer and inner shell must remain one body with a void, while the two separated boxes become two bodies.

Curved STL facets approximate the analytic construction, so their signed mesh volumes are expected to differ from the analytic solid volumes. An STL importer must preserve the mesh and report repair changes; it cannot reconstruct an exact curved boundary from information the file does not contain.

The separate evidence archive contains the throwaway recognition, conservative-contact and Geant4 validation spikes. Generated assets and those spikes are deliberately not installed in product source directories.
