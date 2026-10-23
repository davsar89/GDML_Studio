# GDML Specification Reference

> Geometry Description Markup Language, schema version 3.1.7 (as shipped in Geant4 11.4 `persistency/gdml/schema/`)
> Sources: [CERN GDML](https://gdml.web.cern.ch/GDML/), [JeffersonLab XSD](https://github.com/JeffersonLab/gdml), [Geant4 Docs](https://geant4-userdoc.web.cern.ch/UsersGuides/ForApplicationDeveloper/html/Detector/Geometry/geomXML.html)

---

## 1. Document Structure

```xml
<?xml version="1.0" encoding="UTF-8" ?>
<gdml xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
      xsi:noNamespaceSchemaLocation="http://cern.ch/service-spi/app/releases/GDML/schema/gdml.xsd">
  <define> ... </define>
  <materials> ... </materials>
  <solids> ... </solids>
  <structure> ... </structure>
  <setup name="Default" version="1.0">
    <world ref="WorldVolumeName"/>
  </setup>
</gdml>
```

The schema requires the sections in this order: **define**, **materials**, **solids**, **structure**, an optional **userinfo**, then **setup**.

---

## 2. Define Section

### constant

```xml
<constant name="myconst" value="42.0"/>
```

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | xs:ID | yes | Unique identifier |
| `value` | Expression | yes | Numeric value or expression |

### variable

```xml
<variable name="myvar" value="2*myconst"/>
```

Same attributes as `constant`. Like a constant it is evaluated once, but a `<loop>` can change its value.

### quantity

```xml
<quantity name="myquant" type="length" value="100" unit="mm"/>
```

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | xs:ID | yes | Unique identifier |
| `value` | Expression | yes | Numeric value |
| `unit` | string | no | Unit string |
| `type` | string | no | Quantity type |

### expression

```xml
<expression name="myexpr">2*myconst + 10</expression>
```

Text content is the expression body. May reference other named constants/quantities.

### position

```xml
<position name="pos1" x="10" y="20" z="30" unit="mm"/>
```

| Attribute | Type | Default | Description |
|-----------|------|---------|-------------|
| `name` | xs:ID | | Unique identifier |
| `x` | Expression | "0.0" | X coordinate |
| `y` | Expression | "0.0" | Y coordinate |
| `z` | Expression | "0.0" | Z coordinate |
| `unit` | string | "mm" | Length unit |

### rotation

```xml
<rotation name="rot1" x="0" y="0" z="90" unit="deg"/>
```

Same structure as position. Default unit is `"rad"`.

### scale

```xml
<scale name="scale1" x="1.0" y="2.0" z="1.0"/>
```

| Attribute | Type | Default | Description |
|-----------|------|---------|-------------|
| `name` | xs:ID | | Unique identifier |
| `x` | Expression | "1.0" | X scale factor |
| `y` | Expression | "1.0" | Y scale factor |
| `z` | Expression | "1.0" | Z scale factor |

### matrix

```xml
<matrix name="mymatrix" coldim="2" values="1.0 2.0 3.0 4.0 5.0 6.0"/>
```

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | xs:ID | yes | Unique identifier |
| `coldim` | positive int | yes | Number of columns |
| `values` | string | yes | Space-separated values |

### loop

```xml
<loop for="i" from="0" to="9" step="1">
  <!-- Can contain: solid elements, volume, physvol, nested loop -->
</loop>
```

Geant4 reads a loop inside `<solids>`, `<structure>`, a `<volume>` or a `<paramvol>`, not inside `<define>` (`G4GDMLRead::LoopRead` callers).

| Attribute | Type | Description |
|-----------|------|-------------|
| `for` | string | Name of a defined `<variable>` (anything else is fatal) |
| `from` | int | Start value |
| `to` | Expression | End value |
| `step` | positive int | Increment |

---

## 3. Materials Section

### isotope

```xml
<isotope name="U235" Z="92" N="235">
  <atom value="235.044"/>
</isotope>
```

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | xs:ID | yes | Unique identifier |
| `Z` | double | yes | Atomic number |
| `N` | positive int | yes | Number of nucleons |
| `formula` | string | no | Chemical formula |
| `state` | string | no | `gas`, `liquid`, `solid`, `unknown` |

### element

Two forms:

```xml
<!-- Simple element -->
<element name="Oxygen" Z="8" formula="O">
  <atom value="16.0"/>
</element>

<!-- Element from isotope fractions -->
<element name="Uranium">
  <fraction n="0.9928" ref="U238"/>
  <fraction n="0.0072" ref="U235"/>
</element>
```

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | xs:ID | yes | Unique identifier |
| `Z` | double | no | Atomic number |
| `formula` | string | no | Chemical formula |

### material

Three forms:

**Form 1, single element:**
```xml
<material Z="7" name="Nitrogen" formula="N">
  <D value="0.00125"/>
  <atom value="14.01"/>
</material>
```

**Form 2, composite by atom count:**
```xml
<material name="Water" formula="H2O">
  <D value="1.0"/>
  <composite n="2" ref="Hydrogen"/>
  <composite n="1" ref="Oxygen"/>
</material>
```

**Form 3, fractional mixture:**
```xml
<material name="Air">
  <D value="0.0012"/>
  <fraction n="0.7" ref="Nitrogen"/>
  <fraction n="0.3" ref="Oxygen"/>
</material>
```

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | xs:ID | yes | Unique identifier |
| `Z` | double | no | Atomic number (simple materials) |
| `formula` | string | no | Chemical formula |
| `state` | string | no | `gas`, `liquid`, `solid`, `unknown` |

**Child elements:**

| Element | Description | Default Unit |
|---------|-------------|-------------|
| `<D value="..." unit="..."/>` | Density | g/cm3 |
| `<Dref ref="..."/>` | Reference to named density | |
| `<atom value="..." unit="..."/>` | Atomic mass | g/mole |
| `<composite n="2" ref="..."/>` | Element by atom count | |
| `<fraction n="0.7" ref="..."/>` | Mass fraction | |
| `<T value="..." unit="..."/>` | Temperature | K |
| `<P value="..." unit="..."/>` | Pressure | pascal |
| `<MEE value="..." unit="..."/>` | Mean excitation energy | eV |
| `<RL value="..." unit="..."/>` | Radiation length (schema only; Geant4 ignores it) | cm |
| `<AL value="..." unit="..."/>` | Absorption length (schema only; Geant4 ignores it) | cm |
| `<property name="..." ref="..."/>` | Material property (references matrix) | |

---

## 4. Solids Section

All solids have a required `name` attribute (xs:ID). Most accept `lunit` (default `"mm"`) and `aunit` (default `"rad"`).

**Important:** the lengths of box, tube, cutTube, cone, trd, trap, para, hype and the twisted solids (`x`, `y`, `z`, `x1`...`x4`, `y1`, `y2`, twistedtubs `zlen`) are **full lengths**; the Geant4 reader halves them. eltube, paraboloid and arb8 take half-lengths (`dx`, `dy`, `dz`) and ellipsoid semi-axes, passed through unchanged.

### 4.1 box

```xml
<box name="mybox" x="100" y="200" z="300" lunit="mm"/>
```

| Attribute | Description |
|-----------|-------------|
| `x` | Full length along X |
| `y` | Full length along Y |
| `z` | Full length along Z |

### 4.2 tube

```xml
<tube name="mytube" rmin="0" rmax="50" z="100" startphi="0" deltaphi="360" aunit="deg" lunit="mm"/>
```

| Attribute | Description |
|-----------|-------------|
| `rmin` | Inner radius |
| `rmax` | Outer radius |
| `z` | Full length in Z |
| `startphi` | Starting phi angle |
| `deltaphi` | Delta phi angle |

### 4.3 cutTube

Same as tube plus cut plane normals:

| Attribute | Description |
|-----------|-------------|
| `lowX`, `lowY`, `lowZ` | Normal vector at lower Z plane |
| `highX`, `highY`, `highZ` | Normal vector at upper Z plane |

### 4.4 cone

```xml
<cone name="mycone" rmin1="0" rmax1="50" rmin2="0" rmax2="25" z="100"
      startphi="0" deltaphi="360" aunit="deg" lunit="mm"/>
```

| Attribute | Description |
|-----------|-------------|
| `rmin1` | Inner radius at -z |
| `rmax1` | Outer radius at -z |
| `rmin2` | Inner radius at +z |
| `rmax2` | Outer radius at +z |
| `z` | Full height |
| `startphi` | Starting phi angle |
| `deltaphi` | Delta phi angle |

### 4.5 sphere

```xml
<sphere name="mysphere" rmin="0" rmax="100" startphi="0" deltaphi="360"
        starttheta="0" deltatheta="180" aunit="deg" lunit="mm"/>
```

| Attribute | Description |
|-----------|-------------|
| `rmin` | Inner radius |
| `rmax` | Outer radius |
| `startphi` | Starting phi angle |
| `deltaphi` | Delta phi angle |
| `starttheta` | Starting theta angle |
| `deltatheta` | Delta theta angle |

### 4.6 orb

```xml
<orb name="myorb" r="100" lunit="mm"/>
```

Full solid sphere. Only attribute: `r` (outer radius).

### 4.7 torus

```xml
<torus name="mytorus" rmin="1" rmax="4" rtor="20" startphi="0" deltaphi="360" aunit="deg" lunit="mm"/>
```

| Attribute | Description |
|-----------|-------------|
| `rmin` | Inside radius of tube cross-section |
| `rmax` | Outside radius of tube cross-section |
| `rtor` | Swept radius (center to tube center) |
| `startphi` | Starting phi angle |
| `deltaphi` | Delta phi angle |

### 4.8 trd (trapezoid)

```xml
<trd name="mytrd" x1="10" x2="20" y1="10" y2="20" z="30" lunit="mm"/>
```

| Attribute | Description |
|-----------|-------------|
| `x1` | X length at -z |
| `x2` | X length at +z |
| `y1` | Y length at -z |
| `y2` | Y length at +z |
| `z` | Z length |

### 4.9 trap (general trapezoid)

```xml
<trap name="mytrap" z="10" theta="0" phi="0" y1="15" x1="10" x2="10" alpha1="0"
      y2="15" x3="10" x4="10" alpha2="0" aunit="rad" lunit="mm"/>
```

| Attribute | Description |
|-----------|-------------|
| `z` | Z length |
| `theta` | Polar angle of line joining face centers at -z and +z |
| `phi` | Azimuthal angle of that line |
| `y1` | Y length at -z |
| `x1` | X length at y=-y1 at -z face |
| `x2` | X length at y=+y1 at -z face |
| `alpha1` | Angle from center at y=-y1 to center at y=+y1 of -z face |
| `y2` | Y length at +z |
| `x3` | X length at y=-y2 at +z face |
| `x4` | X length at y=+y2 at +z face |
| `alpha2` | Same angle for +z face |

### 4.10 para (parallelepiped)

```xml
<para name="mypara" x="10" y="10" z="10" alpha="0.5" theta="0.3" phi="0.1" aunit="rad" lunit="mm"/>
```

| Attribute | Description |
|-----------|-------------|
| `x`, `y`, `z` | Lengths along axes |
| `alpha` | Angle between the Y axis and the line joining the centres of the -y and +y faces |
| `theta` | Polar angle of line joining face centers |
| `phi` | Azimuthal angle of that line |

### 4.11 polycone

```xml
<polycone name="mypcone" startphi="0" deltaphi="360" aunit="deg" lunit="mm">
  <zplane rmin="0" rmax="10" z="-50"/>
  <zplane rmin="0" rmax="20" z="0"/>
  <zplane rmin="0" rmax="10" z="50"/>
</polycone>
```

Child `<zplane>` elements (minimum 2): `rmin` (default "0.0"), `rmax`, `z`.

### 4.12 genericPolycone

```xml
<genericPolycone name="mygpcone" startphi="0" deltaphi="360" aunit="deg" lunit="mm">
  <rzpoint r="10" z="-50"/>
  <rzpoint r="20" z="0"/>
  <rzpoint r="10" z="50"/>
</genericPolycone>
```

Child `<rzpoint>` elements (minimum 3): `r`, `z`.

### 4.13 polyhedra

Same as polycone plus `numsides` attribute (number of polygon sides).

### 4.14 genericPolyhedra

Same as genericPolycone plus `numsides` attribute.

### 4.15 ellipsoid

```xml
<ellipsoid name="myellipsoid" ax="10" by="15" cz="20" zcut1="-15" zcut2="15" lunit="mm"/>
```

| Attribute | Description |
|-----------|-------------|
| `ax` | Semi-axis along X |
| `by` | Semi-axis along Y |
| `cz` | Semi-axis along Z |
| `zcut1` | Lower cut plane (default 0) |
| `zcut2` | Upper cut plane (default 0) |

Both 0 means uncut; omitting only one puts that cut at z = 0 (`G4Ellipsoid::CheckParameters`).

### 4.16 eltube (elliptical tube)

```xml
<eltube name="myeltube" dx="10" dy="15" dz="20" lunit="mm"/>
```

`dx`, `dy`: X/Y semi-axes. `dz`: half-length in Z.

### 4.17 elcone (elliptical cone)

```xml
<elcone name="myelcone" dx="2" dy="3" zmax="20" zcut="15" lunit="mm"/>
```

`dx`, `dy`: dimensionless semi-axis slopes (not scaled by `lunit`). `zmax`: apex height. `zcut`: the solid is cut at z = ±`zcut`.

### 4.18 hype (hyperbolic tube)

```xml
<hype name="myhype" rmin="1" rmax="5" inst="0.5" outst="0.7" z="40" lunit="mm"/>
```

`rmin`, `rmax`: radii. `inst`, `outst`: inner/outer stereo angles. `z`: length.

### 4.19 paraboloid

```xml
<paraboloid name="myparab" rlo="10" rhi="15" dz="20" lunit="mm"/>
```

`rlo`: radius at -dz. `rhi`: radius at +dz. `dz`: half Z length.

### 4.20 arb8 (arbitrary trapezoid, 8 vertices)

```xml
<arb8 name="myarb8" v1x="1" v1y="1" v2x="2" v2y="2" v3x="3" v3y="3" v4x="4" v4y="4"
      v5x="10" v5y="10" v6x="11" v6y="11" v7x="12" v7y="12" v8x="13" v8y="13" dz="20" lunit="mm"/>
```

`v1`-`v4`: vertices at -dz face. `v5`-`v8`: vertices at +dz face. `dz`: half Z length.

### 4.21 tet (tetrahedron)

```xml
<tet name="mytet" vertex1="v1" vertex2="v2" vertex3="v3" vertex4="v4"/>
```

Vertex attributes reference position elements from the define section.

### 4.22 xtru (extruded solid)

```xml
<xtru name="myxtru" lunit="mm">
  <twoDimVertex x="0" y="0"/>
  <twoDimVertex x="10" y="0"/>
  <twoDimVertex x="5" y="10"/>
  <section zOrder="0" zPosition="-20" xOffset="0" yOffset="0" scalingFactor="1.0"/>
  <section zOrder="1" zPosition="20" xOffset="0" yOffset="0" scalingFactor="1.0"/>
</xtru>
```

`<twoDimVertex>` (min 3): 2D polygon outline. `<section>` (min 2): Z slices with offset and scale.

### 4.23 tessellated

```xml
<tessellated name="mytess">
  <triangular vertex1="v1" vertex2="v2" vertex3="v3" type="ABSOLUTE"/>
  <quadrangular vertex1="v4" vertex2="v5" vertex3="v6" vertex4="v7" type="ABSOLUTE"/>
</tessellated>
```

Vertices reference positions. `type`: `ABSOLUTE` (default) or `RELATIVE`. Vertices must be listed anti-clockwise from outside.

### 4.24 Twisted Solids

**twistedbox**: `PhiTwist`, `x`, `y`, `z`
**twistedtrd**: `PhiTwist`, `x1`, `x2`, `y1`, `y2`, `z`
**twistedtrap**: `PhiTwist`, `z`, `Theta`, `Phi`, `y1`, `x1`, `x2`, `y2`, `x3`, `x4`, `Alph`
**twistedtubs**: `twistedangle`, `endinnerrad`, `endouterrad`, `zlen`, `phi` (Geant4 also reads `midinnerrad`/`midouterrad`, `negativeEndz`/`positiveEndz`, and `nseg` with `totphi`)

### 4.25 Boolean Solids

```xml
<union name="myunion">
  <first ref="solid1"/>
  <second ref="solid2"/>
  <position name="pos" x="10" y="0" z="0"/>
  <rotation name="rot" x="0" y="0" z="0"/>
</union>

<subtraction name="mysub">
  <first ref="solid1"/>
  <second ref="solid2"/>
  <positionref ref="someposition"/>
  <rotationref ref="somerotation"/>
</subtraction>

<intersection name="myint">
  <first ref="solid1"/>
  <second ref="solid2"/>
</intersection>
```

Position/rotation define the placement of the second solid in the first solid's coordinate system. Optional `firstposition`/`firstrotation` for the first solid.

### 4.26 multiUnion

```xml
<multiUnion name="mymultiunion">
  <multiUnionNode name="node1">
    <solid ref="solid1"/>
    <position x="0" y="0" z="0"/>
  </multiUnionNode>
  <multiUnionNode name="node2">
    <solid ref="solid2"/>
    <position x="10" y="0" z="0"/>
  </multiUnionNode>
</multiUnion>
```

### 4.27 reflectedSolid

```xml
<reflectedSolid name="myreflected" solid="originalSolid" sx="1" sy="1" sz="-1"
                rx="0" ry="0" rz="0" dx="0" dy="0" dz="0"/>
```

`sx/sy/sz`: scale (use -1 for reflection). `rx/ry/rz`: rotation. `dx/dy/dz`: translation.

### 4.28 scaledSolid

```xml
<scaledSolid name="myscaled">
  <solidref ref="originalSolid"/>
  <scale name="s" x="2.0" y="2.0" z="1.0"/>
</scaledSolid>
```

---

## 5. Optical Surfaces

### opticalsurface

```xml
<opticalsurface name="surf1" model="unified" finish="polished" type="dielectric_dielectric" value="1.0">
  <property name="REFLECTIVITY" ref="reflectivityMatrix"/>
</opticalsurface>
```

**Model values:** `glisur`, `unified`, `LUT`, `DAVIS`, `dichroic`

**Type values:** `dielectric_metal`, `dielectric_dielectric`, `dielectric_LUT`, `dielectric_LUTDAVIS`, `dielectric_dichroic`, `firsov`, `x_ray`

**Finish values:**

- `polished`, `polishedfrontpainted`, `polishedbackpainted`, `ground`, `groundfrontpainted`, `groundbackpainted`
- LUT model: `polished`, `etched` or `ground` followed by one of `lumirrorair`, `lumirrorglue`, `air`, `teflonair`, `tioair`, `tyvekair`, `vm2000air`, `vm2000glue` (e.g. `groundtyvekair`)
- DAVIS model: `Rough_LUT`, `RoughTeflon_LUT`, `RoughESR_LUT`, `RoughESRGrease_LUT`, `Polished_LUT`, `PolishedTeflon_LUT`, `PolishedESR_LUT`, `PolishedESRGrease_LUT`

The reader also accepts each enum's integer value; an unknown `model` becomes `dichroic` and an unknown `type` becomes `x_ray`.

---

## 6. Structure Section

### volume

```xml
<volume name="MyVolume">
  <materialref ref="Air"/>
  <solidref ref="mybox"/>
  <auxiliary auxtype="SensDet" auxvalue="Tracker"/>
  <physvol> ... </physvol>
</volume>
```

Required children: `<materialref>`, `<solidref>`. Optional: `<physvol>`, `<divisionvol>`, `<replicavol>`, `<paramvol>`, `<auxiliary>`, `<loop>`.

### physvol (physical volume placement)

```xml
<physvol name="pv1" copynumber="0">
  <volumeref ref="ChildVolume"/>
  <position name="pvpos" x="10" y="0" z="0" unit="mm"/>
  <rotation name="pvrot" x="0" y="0" z="0.5" unit="rad"/>
</physvol>

<!-- Or reference external file -->
<physvol>
  <file name="subdetector.gdml" volname="SpecificVolume"/>
  <positionref ref="somePosition"/>
</physvol>
```

| Child | Description |
|-------|-------------|
| `<volumeref ref="..."/>` | Volume being placed |
| `<file name="..." volname="..."/>` | External GDML file inclusion |
| `<position .../>` or `<positionref .../>` | Placement position |
| `<rotation .../>` or `<rotationref .../>` | Placement rotation |
| `<scale .../>` or `<scaleref .../>` | Reflection: only components of ±1 take effect; Geant4 drops any other positive scale and refuses a scaled reflection (`G4ReflectionFactory::Place`) |

### assembly

```xml
<assembly name="MyAssembly">
  <physvol>
    <volumeref ref="Part1"/>
    <positionref ref="pos1"/>
  </physvol>
</assembly>
```

Groups volumes without a boundary solid. No `materialref`/`solidref`. Cannot be used as world volume.

### bordersurface

```xml
<bordersurface name="myborder" surfaceproperty="surf1">
  <physvolref ref="pv1"/>
  <physvolref ref="pv2"/>
</bordersurface>
```

### skinsurface

```xml
<skinsurface name="myskin" surfaceproperty="surf1">
  <volumeref ref="SomeVolume"/>
</skinsurface>
```

### auxiliary

```xml
<auxiliary auxtype="Color" auxvalue="Red"/>
<auxiliary auxtype="Region" auxvalue="MyRegion" auxunit="mm">
  <auxiliary auxtype="gamcut" auxvalue="0.1"/>
</auxiliary>
```

`auxtype` is free text the application interprets. Geant4's writer emits only `SensDet` and the `gammaECut`/`electronECut`/`positronECut`/`protonECut` production cuts; Geant4 example G01 reads `Region` with `volume`, `gamcut`, `ecut`, `poscut`, `pcut` children. GDML Studio colours a volume from a `Color` auxiliary.

---

## 7. Replicas and Parameterised Volumes

### replicavol

```xml
<replicavol number="10">
  <volumeref ref="SliceVolume"/>
  <replicate_along_axis>
    <direction x="0" y="0" z="1"/>
    <width value="10" unit="mm"/>
    <offset value="0" unit="mm"/>
  </replicate_along_axis>
</replicavol>
```

### divisionvol

```xml
<divisionvol number="10" axis="kZAxis" width="5" offset="0" unit="mm">
  <volumeref ref="SliceVolume"/>
</divisionvol>
```

Axis values: `kXAxis`, `kYAxis`, `kZAxis`, `kRho`, `kPhi`.

### paramvol

```xml
<paramvol ncopies="3">
  <volumeref ref="ParamVolume"/>
  <parameterised_position_size>
    <parameters number="1">
      <position name="p1" x="0" y="0" z="-30" unit="mm"/>
      <box_dimensions x="10" y="10" z="10" lunit="mm"/>
    </parameters>
  </parameterised_position_size>
</paramvol>
```

Dimension types: `box_dimensions`, `tube_dimensions`, `cone_dimensions`, `sphere_dimensions`, `orb_dimensions`, `torus_dimensions`, `ellipsoid_dimensions`, `para_dimensions`, `hype_dimensions`, `trap_dimensions`, `trd_dimensions`, `polycone_dimensions`, `polyhedra_dimensions`.

---

## 8. Setup Section

```xml
<setup name="Default" version="1.0">
  <world ref="WorldVolume"/>
</setup>
```

The `<world>` element references the top-level volume. Cannot be an assembly volume.

---

## 9. XSD Schema Files

The GDML schema is split across multiple XSD files ([JeffersonLab/gdml](https://github.com/JeffersonLab/gdml)):

| File | Contents |
|------|----------|
| `gdml.xsd` | Main schema, includes all others |
| `gdml_core.xsd` | Core types (expressions, references, auxiliary) |
| `gdml_define.xsd` | Define section types |
| `gdml_materials.xsd` | Materials section types |
| `gdml_solids.xsd` | All solid type definitions |
| `gdml_replicas.xsd` | Replica volume types |
| `gdml_parameterised.xsd` | Parameterised volume types |
| `gdml_extensions.xsd` | Loop element and extensions |
