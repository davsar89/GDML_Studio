// gdml-cad-worker: reads a STEP file with OpenCASCADE and writes, for each
// solid, its exact properties and a closed triangle mesh (algorithm C in
// doc/cad-import/PLAN.md), and where placed solids interpenetrate (algorithm I).
// The backend turns the result into GDML.
#pragma once

#include <TopLoc_Location.hxx>
#include <TopoDS_Shape.hxx>

#include <array>
#include <cstdint>
#include <cstdio>
#include <map>
#include <optional>
#include <stdexcept>
#include <string>
#include <vector>

namespace worker {

/// What the command line asks for (main.cpp).
struct Options {
  double volume_target = 1e-3;              // mesh volume error that refinement aims for
  std::map<std::string, double> chord_for;  // solid id "<entry>.<index>" -> fixed chord in mm
  bool only = false;                        // mesh only those solids, skip the interference check
};

/// A solid's face triangulations stitched into one mesh: one node per B-rep
/// vertex, per edge-polygon node and per face-interior node, so the mesh is
/// closed wherever the B-rep is, without any tolerance.
struct Mesh {
  std::vector<double> positions;    // x, y, z per node, in mm
  std::vector<uint32_t> triangles;  // three nodes per facet, wound by face orientation
  double chord_mm = 0;
  double volume_mm3 = 0;  // signed, by the divergence theorem (not written)
  std::vector<std::string> issues;
};

struct Solid {
  bool valid = false;
  double tolerance_mm = 0;
  double volume_mm3 = 0;  // exact, from the B-rep; negative when inside out
  double area_mm2 = 0;
  std::array<double, 6> bounds_mm{};  // exact: minimum x, y, z, then maximum
  Mesh mesh;
  std::vector<std::string> issues;
};

/// An instance of `target` (a label entry) placed in an assembly.
struct Component {
  std::string target;
  std::array<double, 12> matrix{};  // 3 x 4, row-major: rotation and translation in mm
  TopLoc_Location location;         // the same (not written)
};

/// An XCAF shape label: an assembly of components, or a part with solids.
struct Label {
  std::string entry, name;
  bool assembly = false;
  std::vector<Component> components;
  std::vector<TopoDS_Shape> shapes;          // a part's solids, healed by analyze (not written)
  std::vector<std::optional<Solid>> solids;  // their analysis, in the same order; none when skipped
  std::vector<std::string> issues;           // what of the part is not imported, and why
};

/// Placed solids whose interiors overlap, each named by its path: the root
/// index, the component index at each assembly level, then the solid index.
struct Interference {
  std::vector<int> a, b;
  double volume_mm3 = 0;
};

struct Document {
  std::vector<Label> labels;
  std::vector<std::string> roots;  // entries of the free (top-level) labels
  std::vector<Interference> interferences;
  int unchecked = 0;                // candidate pairs the interference check did not decide
  std::vector<std::string> issues;  // about the file as a whole
};

/// The file is not STEP, or nothing in it could be transferred (exit code 3).
struct Unreadable : std::runtime_error {
  using std::runtime_error::runtime_error;
};

/// `progress <phase> <done> <total>` on stderr, which the backend shows.
void progress(const char* phase, size_t done, size_t total);

/// Read STEP bytes into their labels, in mm whatever unit the file uses.
Document read_step(const std::string& bytes);

/// Validity, exact properties and a mesh of `solid`, healed first when
/// OpenCASCADE finds it invalid (and replaced by the healed shape). The mesh
/// volume is within `volume_target` of the exact one, or it is the finest
/// mesh tried; a positive `chord` fixes the chordal deviation instead.
Solid analyze(TopoDS_Shape& solid, double volume_target, double chord);

/// The volumes where placed solids overlap.
void interfere(Document& doc);

/// The document as one JSON object.
void write(std::FILE* out, const Document& doc);

}  // namespace worker
