# GDML Solid Implementation Progress

Reference: `backend/reference/geant4/G4GDMLReadSolids.cc`

## Already Supported (before this effort)
- [x] Box
- [x] Tube (tubs)
- [x] Cone (cons)
- [x] Sphere
- [x] Trd
- [x] Polycone
- [x] Xtru (ExtrudedSolid)
- [x] Boolean (union, subtraction, intersection)

## Tier 1 - Very Common
1. [x] **Orb** - Full solid sphere (r only)
2. [x] **Torus** - Ring/donut (rmin, rmax, rtor, startphi, deltaphi)
3. [x] **Trap** - General trapezoid (z, theta, phi, y1, x1, x2, alpha1, y2, x3, x4, alpha2)
4. [x] **Para** - Parallelepiped (x, y, z, alpha, theta, phi)
5. [x] **CutTube** - Tube cut at angles (tube params + low/high normal vectors)
6. [x] **Polyhedra** - Polygonal solid (startphi, deltaphi, numsides + zplanes)

## Tier 2 - Common
7. [x] **Ellipsoid** - Ellipsoid with z-cuts (ax, ay, az, zcut1, zcut2)
8. [ ] **Eltube** - Elliptical tube (dx, dy, dz)
9. [ ] **Tet** - Tetrahedron (4 vertex positions)
10. [ ] **GenericPolycone** - Polycone from (r,z) pairs
11. [ ] **Hype** - Hyperbolic tube (rmin, rmax, inst, outst, z)

## Tier 3 - Moderate Use
12. [ ] **Elcone** - Elliptical cone (dx, dy, zmax, zcut)
13. [ ] **Paraboloid** - Parabolic solid (rlo, rhi, dz)
14. [ ] **GenericPolyhedra** - Polyhedra from (r,z) pairs
15. [ ] **Arb8/GenTrap** - Arbitrary 8-vertex solid
16. [x] **Tessellated** - Triangular/quad facet solid

## Tier 4 - Rare / Specialized
17. [ ] **TwistedTubs** - Twisted tube
18. [ ] **TwistedBox** - Twisted box
19. [ ] **TwistedTrap** - Twisted trapezoid
20. [ ] **TwistedTrd** - Twisted trd
21. [ ] **ScaledSolid** - Scale transform on existing solid
22. [ ] **ReflectedSolid** - Mirror transform on existing solid
23. [ ] **MultiUnion** - Multi-solid boolean union
