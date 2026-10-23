# GDML Expressions, Units, and Advanced Features

> Sources: [CLHEP Evaluator](https://apc.u-paris.fr/~franco/g4doxy/html/class_hep_tool_1_1_evaluator.html), [Geant4 SystemOfUnits](https://github.com/Geant4/geant4/blob/master/source/externals/clhep/include/CLHEP/Units/SystemOfUnits.h), [GDML User Guide v2.7](https://indico.ihep.ac.cn/event/12361/contributions/17680/attachments/8526/9724/GDMLmanual.pdf)

---

## 1. Expression Syntax

GDML expressions are evaluated by `G4GDMLEvaluator`, which wraps CLHEP's `HepTool::Evaluator`.

### Operators

| Operator | Description |
|----------|-------------|
| `+`, `-`, `*`, `/` | Arithmetic |
| `^`, `**` | Exponentiation |
| `( )` | Grouping |
| `==`, `!=`, `>`, `>=`, `<`, `<=` | Comparison |
| `&&`, `\|\|` | Logical |

### Mathematical Functions

| Function | Args | Description |
|----------|------|-------------|
| `abs(x)` | 1 | Absolute value |
| `min(a,b)` | 2 | Minimum |
| `max(a,b)` | 2 | Maximum |
| `sqrt(x)` | 1 | Square root |
| `pow(x,y)` | 2 | Power |
| `sin(x)` | 1 | Sine |
| `cos(x)` | 1 | Cosine |
| `tan(x)` | 1 | Tangent |
| `asin(x)` | 1 | Arcsine |
| `acos(x)` | 1 | Arccosine |
| `atan(x)` | 1 | Arctangent |
| `atan2(y,x)` | 2 | Two-argument arctangent |
| `sinh(x)` | 1 | Hyperbolic sine |
| `cosh(x)` | 1 | Hyperbolic cosine |
| `tanh(x)` | 1 | Hyperbolic tangent |
| `exp(x)` | 1 | Exponential |
| `log(x)` | 1 | Natural logarithm |
| `log10(x)` | 1 | Base-10 logarithm |

### Built-in Constants

| Constant | Value |
|----------|-------|
| `pi` | 3.14159265358979323846 |
| `e` | 2.7182818284590452354 |
| `gamma` | 0.577215664901532861 (Euler-Mascheroni) |
| `radian` / `rad` | 1.0 |
| `degree` / `deg` | pi/180 |

### Matrix Access

A `<matrix>` cell is read as `m[row,col]`, or `m[i]` for a single-row or single-column matrix (1-based; `G4GDMLEvaluator::SolveBrackets`). GDML Studio does the same (`eval/engine.rs`).

### Expression Usage

Expressions can be used anywhere a numeric value is expected: solid dimensions, positions, rotations, material properties.

---

## 2. Units

Values come from CLHEP `SystemOfUnits.h`; internal base units are mm, ns, MeV, radian. Geant4 looks a unit up in two places:

- a `unit`/`lunit`/`aunit` attribute goes to `G4UnitsTable` (name or symbol). `lunit` must be a Length and `aunit` an Angle unit, or Geant4 aborts. The table covers the length, angle, time, energy, mass and pressure units below plus composites such as `g/cm3` and `g/mole`, but symbols differ: `Ang`, `fm`, `meV`, `dL`, `eplus` exist only there, while `micron`, `cc`, `Gs`, `kGs` and multiples such as `kBq`, `kW`, `kN` exist only in the evaluator;
- a name inside an expression is a variable of the evaluator (CLHEP `setSystemOfUnits.cc`); the tables below list those.

Neither knows `inch`, `foot`, `radians` or `degrees`.

### Length Units (default `lunit="mm"`)

| Unit | Symbol | Value in mm |
|------|--------|-------------|
| `fermi` | fm | 1e-12 |
| `angstrom` | Ang | 1e-7 |
| `nm` | nm | 1e-6 |
| `um` / `micrometer` | um | 1e-3 (`micron` only inside expressions) |
| `mm` | mm | 1.0 |
| `cm` | cm | 10.0 |
| `decimeter` | | 100.0 (only inside expressions; there is no `dm`) |
| `m` | m | 1000.0 |
| `km` | km | 1e6 |
| `parsec` | pc | ~3.0857e19 |

### Angle Units (default `aunit="rad"`)

| Unit | Symbol | Value in rad |
|------|--------|-------------|
| `rad` | rad | 1.0 |
| `mrad` | mrad | 0.001 |
| `deg` | deg | pi/180 |
| `sr` | sr | 1.0 (steradian; a solid angle, not a valid `aunit`) |

### Energy Units

| Unit | Value in MeV |
|------|-------------|
| `millielectronvolt` (attribute: `meV`) | 1e-9 |
| `eV` | 1e-6 |
| `keV` | 1e-3 |
| `MeV` | 1.0 |
| `GeV` | 1e3 |
| `TeV` | 1e6 |
| `PeV` | 1e9 |
| `J` | ~6.2415e12 |

### Time Units

| Unit | Value in ns |
|------|-------------|
| `ps` | 0.001 |
| `ns` | 1.0 |
| `us` | 1e3 |
| `ms` | 1e6 |
| `s` | 1e9 |
| `minute` | 6e10 |
| `hour` | 3.6e12 |
| `day` | 8.64e13 |
| `year` | ~3.1536e16 (365 days) |

### Mass Units

| Unit | Value |
|------|-------|
| `mg` | milligram |
| `g` | gram |
| `kg` | kilogram |

### Pressure Units

| Unit | Description |
|------|-------------|
| `Pa` / `pascal` | Pascal (default) |
| `bar` | 1e5 Pa |
| `atm` / `atmosphere` | ~1.01325e5 Pa |

Default units of material properties (`<D>`, `<atom>`, `<T>`, `<P>`, `<MEE>`): see
[gdml-specification.md §3](gdml-specification.md#3-materials-section).

### Magnetic Field

| Unit | Description |
|------|-------------|
| `T` / `tesla` | Tesla |
| `Gs` / `gauss` | Gauss (1e-4 T) |
| `kGs` / `kilogauss` | Kilogauss |

### Electric Units

| Unit | Description |
|------|-------------|
| `C` / `coulomb` | Coulomb |
| `A` | Ampere |
| `V` | Volt |
| `kV` | Kilovolt |
| `MV` | Megavolt |
| `ohm` | Ohm |
| `F` | Farad |

### Area and Volume

| Unit | Description |
|------|-------------|
| `mm2`, `cm2`, `m2`, `km2` | Area |
| `barn`, `mbarn`, `microbarn`, `nanobarn`, `picobarn` | Cross-section area |
| `mm3`, `cm3`/`cc`, `m3`, `km3` | Volume |
| `L`, `cL`, `mL` | Litre units |

### Other

| Unit | Description |
|------|-------------|
| `Bq`, `kBq`, `MBq`, `GBq` | Radioactivity (Becquerel) |
| `Ci`, `mCi`, `uCi` | Curie |
| `Gy`, `kilogray`, `milligray`, `microgray` | Absorbed dose (Gray) |
| `Sv` | Dose equivalent (Sievert) |
| `Hz`, `kHz`, `MHz` | Frequency |
| `W`, `kW`, `MW`, `GW` | Power |
| `N`, `kN` | Force |

---

## 3. External Entity / File Inclusion

### Method 1: XML DOCTYPE Entities (shared scope)

```xml
<!DOCTYPE gdml [
  <!ENTITY materials SYSTEM "materials.xml">
  <!ENTITY solids SYSTEM "solids.xml">
]>
<gdml ...>
  <define>...</define>
  &materials;
  &solids;
  ...
</gdml>
```

Textual substitution. Constants/variables from the parent file are accessible.

### Method 2: `<file>` Element in `<physvol>` (separate scope)

Syntax: see [`<physvol>`](gdml-specification.md#physvol-physical-volume-placement). Each included file has its own evaluation scope. Parent constants/variables are NOT available.

GDML Studio keeps a DOCTYPE but does not expand its entities (an upload endpoint resolving `SYSTEM` entities is XXE); it warns instead, and supports Method 2 through multi-file upload (`gdml/parser.rs`, `Event::DocType`).

---

## 4. GDML-to-Geant4 Class Mapping

### Solids

| GDML Tag | Geant4 Class |
|----------|-------------|
| `<box>` | `G4Box` |
| `<tube>` | `G4Tubs` |
| `<cutTube>` | `G4CutTubs` |
| `<cone>` | `G4Cons` |
| `<sphere>` | `G4Sphere` |
| `<orb>` | `G4Orb` |
| `<torus>` | `G4Torus` |
| `<para>` | `G4Para` |
| `<trd>` | `G4Trd` |
| `<trap>` | `G4Trap` |
| `<polycone>` | `G4Polycone` |
| `<genericPolycone>` | `G4GenericPolycone` |
| `<polyhedra>` | `G4Polyhedra` |
| `<genericPolyhedra>` | `G4Polyhedra` (r-z constructor; there is no `G4GenericPolyhedra`) |
| `<eltube>` | `G4EllipticalTube` |
| `<ellipsoid>` | `G4Ellipsoid` |
| `<elcone>` | `G4EllipticalCone` |
| `<paraboloid>` | `G4Paraboloid` |
| `<hype>` | `G4Hype` |
| `<tet>` | `G4Tet` |
| `<arb8>` | `G4GenericTrap` |
| `<xtru>` | `G4ExtrudedSolid` |
| `<tessellated>` | `G4TessellatedSolid` |
| `<twistedbox>` | `G4TwistedBox` |
| `<twistedtrap>` | `G4TwistedTrap` |
| `<twistedtrd>` | `G4TwistedTrd` |
| `<twistedtubs>` | `G4TwistedTubs` |
| `<union>` | `G4UnionSolid` |
| `<subtraction>` | `G4SubtractionSolid` |
| `<intersection>` | `G4IntersectionSolid` |
| `<multiUnion>` | `G4MultiUnion` |
| `<scaledSolid>` | `G4ScaledSolid` |
| `<reflectedSolid>` | `G4ReflectedSolid` |

### Structure

| GDML Tag | Geant4 Class |
|----------|-------------|
| `<volume>` | `G4LogicalVolume` |
| `<physvol>` | `G4PVPlacement` |
| `<assembly>` | `G4AssemblyVolume` |
| `<replicavol>` | `G4PVReplica` |
| `<divisionvol>` | `G4PVDivision` |
| `<paramvol>` | `G4PVParameterised` |

---

## 5. Common Patterns and Pitfalls

### Top Pitfalls

1. **Overlapping/protruding daughter volumes**: daughters must fit within mother
2. **Unit mismatches**: forgetting to set `lunit`/`aunit` (defaults: mm, rad)
3. **Half-length confusion**: most GDML solids take full lengths, but eltube, paraboloid and arb8 take half-lengths (see [gdml-specification.md §4](gdml-specification.md#4-solids-section))
4. **Omitted deltaphi**: the Geant4 reader defaults `deltaphi` (and a sphere's `deltatheta`) to 0, not 2*pi. Tube, cutTube, cone, sphere and torus then refuse the solid ("Invalid dphi"); polycone and polyhedra read 0 as a full turn. Always set it
5. **Name collisions**: names are `xs:ID`, unique across the document, and a define may not reuse a name the evaluator already has (`m`, `deg`, `pi`, ...): Geant4 aborts with "Redefinition of constant or variable"
6. **Entity vs file scope**: DOCTYPE entities share parent scope; `<file>` elements don't
7. **Floating-point at boundaries**: daughter surfaces should not coincide with mother surfaces (use small gap)
8. **Missing material references**: every volume needs a materialref
9. **Expression evaluation order**: Geant4 reads defines in document order, so a forward reference is fatal there (GDML Studio orders defines by dependency and accepts it)
10. **Not validating against schema**: use `xsi:noNamespaceSchemaLocation` to enable validation

---

## 6. Visualization Tools

| Tool | Description |
|------|-------------|
| **ROOT** | `TGeoManager::Import("file.gdml"); gGeoManager->GetTopVolume()->Draw("ogl")` |
| **Geant4** | Load via `G4GDMLParser`, visualize with OpenGL/Qt/HepRep/VRML/DAWN drivers |
| **pyg4ometry** | Python library with VTK visualization |
