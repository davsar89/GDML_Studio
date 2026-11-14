// gdml-cad-worker: reads a STEP file with OpenCASCADE and writes, for each
// solid, its exact properties, a closed triangle mesh (algorithm C in
// doc/cad-import/PLAN.md) and, where one is proven equal to it, an exact GDML
// solid (algorithms D and E), and where placed solids interpenetrate or come
// closer than their meshes' chords (algorithm I). The backend turns the result
// into GDML.
#pragma once

#include <TopLoc_Location.hxx>
#include <TopoDS_Shape.hxx>
#include <gp_Ax1.hxx>
#include <gp_Trsf.hxx>

#include <algorithm>
#include <array>
#include <cmath>
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

/// A GDML solid in its own frame, with GDML's meaning of its numbers (mm,
/// rad, startphi 0; certify.cpp builds it). The backend writes it as it is.
struct Primitive {
  std::string solid;  // the GDML element: box, xtru, tube, cone, genericPolycone, sphere, orb, torus
  std::vector<std::pair<const char*, double>> values;  // its other attributes
  std::vector<std::array<double, 2>> points;           // xtru (x, y) vertices; genericPolycone (r, z)
  gp_Trsf frame;                                       // to the part's frame, rigid and right-handed
  double& value(const char* key);                      // throws when it has no such attribute
  double value(const char* key) const { return const_cast<Primitive*>(this)->value(key); }
};

/// The (r, z) meridian polygon of a tube, cone or genericPolycone in its
/// frame; empty for other solids.
std::vector<std::array<double, 2>> meridian(const Primitive& p);

/// A solid as `stock` minus the union of `tools`, proven equal to the
/// source within `epsilon_mm`.
struct Exact {
  Primitive stock;
  std::vector<Primitive> tools;
  double epsilon_mm = 0;
};

struct Solid {
  bool valid = false;
  double tolerance_mm = 0;
  double volume_mm3 = 0;  // exact, from the B-rep; negative when inside out
  double area_mm2 = 0;
  std::array<double, 6> bounds_mm{};  // exact: minimum x, y, z, then maximum
  Mesh mesh;
  std::optional<Exact> exact;
  std::string exact_reason;  // why there is no exact solid
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
  std::vector<Interference> contacts;  // closer than their meshes' chords, not interfering
  int unchecked = 0;                   // candidate pairs the interference check did not decide
  std::vector<std::string> issues;     // about the file as a whole
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

/// The exact GDML solid of an untouched B-rep `solid`, or none with the
/// reason in `why`.
std::optional<Exact> recognize(const TopoDS_Shape& solid, std::string& why);

/// The solid that `p`, or `e`, describes, in the part's frame.
TopoDS_Shape shape(const Primitive& p);
TopoDS_Shape shape(const Exact& e);

/// Whether `source` and `candidate` are the same solid within `eps` mm:
/// empty when they are, else why not.
std::string certify(const TopoDS_Shape& source, const TopoDS_Shape& candidate, double eps);

/// The angle between the lines along `a` and `b`, in [0, pi/2].
inline double tilt(const gp_Dir& a, const gp_Dir& b) {
  const double angle = a.Angle(b);
  return std::min(angle, M_PI - angle);
}

/// The point of `axis` nearest `p`.
inline gp_Pnt foot(const gp_Pnt& p, const gp_Ax1& axis) {
  const gp_Vec v(axis.Location(), p), z(axis.Direction());
  return axis.Location().Translated(z * v.Dot(z));
}

/// 3 x 4, row-major: rotation, then translation in mm.
std::array<double, 12> matrix(const gp_Trsf& t);

/// The volumes where placed solids overlap, and the pairs closer than their
/// meshes' chords.
void interfere(Document& doc);

/// The document as one JSON object.
void write(std::FILE* out, const Document& doc);

}  // namespace worker
