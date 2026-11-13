// Exact properties and topology-stitched meshing with adaptive refinement.
#include "worker.hpp"

#include <BRepAdaptor_Surface.hxx>
#include <Bnd_Box.hxx>
#include <BRep_Tool.hxx>
#include <BRepBndLib.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepGProp.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepTools.hxx>
#include <GProp_GProps.hxx>
#include <NCollection_IndexedMap.hxx>
#include <Poly_PolygonOnTriangulation.hxx>
#include <Poly_Triangulation.hxx>
#include <ShapeAnalysis_ShapeTolerance.hxx>
#include <ShapeFix_Shape.hxx>
#include <Standard_ErrorHandler.hxx>
#include <Standard_Failure.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Vertex.hxx>
#include <TopTools_ShapeMapHasher.hxx>

#include <algorithm>
#include <cmath>
#include <map>
#include <sstream>

namespace worker {
namespace {

/// Refinement stops above this many facets, or after this many passes.
constexpr size_t FACET_BUDGET = 250000;
constexpr int MAX_PASSES = 10;

/// The angular deflection of every mesh. Refinement halves only the chord:
/// the volume error follows the chord, while finer angles crowd small
/// fillets with slivers that cross where faces meet tangentially.
constexpr double ANGLE_RAD = 0.2;

/// Below the shape's tolerance, BRepMesh stops following the chord on the
/// faces that set it; a pass there that keeps more than this fraction of the
/// volume error ends the refinement (where the chord limits the error,
/// halving it halves the error).
constexpr double STALL = 0.9;

/// What a face that could not be meshed is, and where: "a cone around (x, y, z) mm".
std::string describe(const TopoDS_Face& face) {
  static const char* const kinds[] = {"plane", "cylinder", "cone", "sphere", "torus", "Bezier surface",
                                      "B-spline surface", "surface of revolution", "extruded surface",
                                      "offset surface", "surface"};
  Bnd_Box box;
  BRepBndLib::Add(face, box);
  std::ostringstream text;
  text.precision(4);
  text << "a " << kinds[std::min(int(BRepAdaptor_Surface(face).GetType()), 10)];
  if (!box.IsVoid()) {
    const gp_Pnt c = box.CornerMin().XYZ() * 0.5 + box.CornerMax().XYZ() * 0.5;
    text << " around (" << c.X() << ", " << c.Y() << ", " << c.Z() << ") mm";
  }
  return text.str();
}

/// The mesh of `shape`'s current triangulation (algorithm C). Face nodes on an
/// edge are keyed by (edge, position along the edge's polygon) and nodes at
/// its ends by vertex, so faces sharing an edge share its nodes; the two
/// polygons of a seam edge close the seam the same way. Nodes on a
/// degenerated edge collapse onto its vertex, and facets that collapse with
/// them are dropped.
Mesh stitch(const TopoDS_Shape& shape) {
  Mesh m;
  NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> vertices, edges;
  TopExp::MapShapes(shape, TopAbs_VERTEX, vertices);
  TopExp::MapShapes(shape, TopAbs_EDGE, edges);
  std::vector<int64_t> vertex_node(vertices.Extent() + 1, -1);
  std::map<std::pair<int, int>, int64_t> edge_node;
  auto at = [&](uint32_t node) {
    return gp_Pnt(m.positions[3 * node], m.positions[3 * node + 1], m.positions[3 * node + 2]);
  };
  // A shared node takes the first face's copy of the point; the others must
  // lie within the B-rep tolerance of it.
  int mismatched = 0;
  auto share = [&](int64_t& slot, const gp_Pnt& p, double tolerance) {
    if (slot < 0) {
      m.positions.insert(m.positions.end(), {p.X(), p.Y(), p.Z()});
      slot = int64_t(m.positions.size() / 3 - 1);
    } else if (p.Distance(at(uint32_t(slot))) > std::max(2 * tolerance, 1e-6)) {
      ++mismatched;
    }
    return slot;
  };
  std::vector<std::string> unmeshed_faces;
  int unmeshed_edges = 0;
  for (TopExp_Explorer fx(shape, TopAbs_FACE); fx.More(); fx.Next()) {
    const TopoDS_Face& face = TopoDS::Face(fx.Current());
    TopLoc_Location location;
    const Handle(Poly_Triangulation)& tri = BRep_Tool::Triangulation(face, location);
    if (tri.IsNull()) {
      unmeshed_faces.push_back(describe(face));
      continue;
    }
    const gp_Trsf trsf = location.Transformation();
    auto point = [&](int i) { return tri->Node(i).Transformed(trsf); };
    std::vector<int64_t> node(tri->NbNodes() + 1, -1);
    for (TopExp_Explorer ex(face, TopAbs_EDGE); ex.More(); ex.Next()) {
      const TopoDS_Edge& edge = TopoDS::Edge(ex.Current());
      const Handle(Poly_PolygonOnTriangulation)& polygon =
          BRep_Tool::PolygonOnTriangulation(edge, tri, location);
      TopoDS_Vertex first, last;  // at the edge's first and last parameter
      TopExp::Vertices(edge, first, last);
      if (polygon.IsNull() || !polygon->HasParameters() || first.IsNull() || last.IsNull()) {
        ++unmeshed_edges;
        continue;
      }
      const NCollection_Array1<int>& nodes = polygon->Nodes();
      const int lo = nodes.Lower(), hi = nodes.Upper();
      // Positions count from `first` whichever way this polygon runs.
      const bool backward = polygon->Parameter(lo) > polygon->Parameter(hi);
      const int e = edges.FindIndex(edge);
      const double tolerance = BRep_Tool::Tolerance(edge);
      for (int k = lo; k <= hi; ++k) {
        const int position = backward ? hi - k : k - lo;
        const gp_Pnt p = point(nodes(k));
        if (BRep_Tool::Degenerated(edge) || position == 0)
          node[nodes(k)] = share(vertex_node[vertices.FindIndex(first)], p, BRep_Tool::Tolerance(first));
        else if (position == hi - lo)
          node[nodes(k)] = share(vertex_node[vertices.FindIndex(last)], p, BRep_Tool::Tolerance(last));
        else
          node[nodes(k)] = share(edge_node.emplace(std::make_pair(e, position), -1).first->second, p, tolerance);
      }
    }
    for (int i = 1; i <= tri->NbNodes(); ++i)
      if (node[i] < 0) share(node[i], point(i), 0);
    const bool reversed = face.Orientation() == TopAbs_REVERSED;
    for (int t = 1; t <= tri->NbTriangles(); ++t) {
      int a, b, c;
      tri->Triangle(t).Get(a, b, c);
      if (reversed) std::swap(b, c);
      const uint32_t na = uint32_t(node[a]), nb = uint32_t(node[b]), nc = uint32_t(node[c]);
      if (na == nb || nb == nc || nc == na) continue;
      m.triangles.insert(m.triangles.end(), {na, nb, nc});
    }
  }
  if (mismatched > 0)
    m.issues.push_back(std::to_string(mismatched) + " shared mesh nodes differ between faces by more than the B-rep tolerance");
  if (!unmeshed_faces.empty()) {
    std::string list = unmeshed_faces[0];
    for (size_t i = 1; i < std::min<size_t>(unmeshed_faces.size(), 3); ++i) list += "; " + unmeshed_faces[i];
    m.issues.push_back(std::to_string(unmeshed_faces.size()) + " faces could not be meshed (" + list +
                       (unmeshed_faces.size() > 3 ? "; ...)" : ")"));
  }
  if (unmeshed_edges > 0)
    m.issues.push_back(std::to_string(unmeshed_edges) + " face edges have no polygon to stitch along");

  // Signed volume about the first node, which keeps the sum well conditioned.
  if (!m.positions.empty()) {
    const gp_XYZ o = at(0).XYZ();
    double six = 0;
    for (size_t t = 0; t < m.triangles.size(); t += 3) {
      const gp_XYZ a = at(m.triangles[t]).XYZ() - o, b = at(m.triangles[t + 1]).XYZ() - o,
                   c = at(m.triangles[t + 2]).XYZ() - o;
      six += a.Dot(b.Crossed(c));
    }
    m.volume_mm3 = six / 6;
  }
  return m;
}

/// Mesh `shape` ever finer until the mesh volume is within `target` of
/// `exact`, starting from chord max(tolerance, min(0.05 mm, 1e-3 D)) and
/// halving it each pass, below the tolerance only while that helps
/// (STALL). A positive `chord` is used as it is.
Mesh refine(const TopoDS_Shape& shape, double exact, double diagonal, double tolerance, double target,
            double chord) {
  double c = chord > 0 ? chord : std::max(tolerance, std::min(0.05, 1e-3 * diagonal));
  Mesh m;
  double last = HUGE_VAL;
  for (int pass = 1; pass <= MAX_PASSES; ++pass, c /= 2) {
    BRepTools::Clean(shape);
    BRepMesh_IncrementalMesh mesher(shape, c, false, ANGLE_RAD, true);
    m = stitch(shape);
    m.chord_mm = c;
    const double error = std::abs(std::abs(m.volume_mm3) - exact) / exact;
    const bool stalled = c < tolerance && error > STALL * last;
    if (chord > 0 || !m.issues.empty() || error <= target || m.triangles.size() / 3 >= FACET_BUDGET || stalled)
      break;
    last = error;
  }
  return m;
}

/// Volume and area of `shape`.
std::array<double, 2> measure(const TopoDS_Shape& shape) {
  GProp_GProps volume, surface;
  BRepGProp::VolumeProperties(shape, volume);
  BRepGProp::SurfaceProperties(shape, surface);
  return {volume.Mass(), surface.Mass()};
}

}  // namespace

Solid analyze(TopoDS_Shape& shape, double volume_target, double chord) {
  Solid s;
  try {
    OCC_CATCH_SIGNALS
    s.valid = BRepCheck_Analyzer(shape).IsValid();
    s.tolerance_mm = ShapeAnalysis_ShapeTolerance().Tolerance(shape, 1);
    const std::array<double, 2> source = measure(shape);
    s.volume_mm3 = source[0];
    s.area_mm2 = source[1];
    if (!s.valid) {
      // ShapeFix within the solid's own tolerance; kept only if that makes it valid.
      Handle(ShapeFix_Shape) fix = new ShapeFix_Shape(shape);
      fix->SetPrecision(s.tolerance_mm);
      fix->SetMaxTolerance(s.tolerance_mm);
      fix->Perform();
      if (BRepCheck_Analyzer(fix->Shape()).IsValid()) {
        const std::array<double, 2> healed = measure(fix->Shape());
        std::ostringstream note;
        note.precision(2);
        note << "OpenCASCADE found its B-rep invalid; healing it within its tolerance changed its volume by "
             << 100 * (healed[0] / s.volume_mm3 - 1) << " %";
        s.issues.push_back(note.str());
        shape = fix->Shape();
        s.valid = true;
        s.volume_mm3 = healed[0];
        s.area_mm2 = healed[1];
      }
    }
    Bnd_Box box;
    BRepBndLib::AddOptimal(shape, box, false, false);
    if (box.IsVoid()) {
      s.issues.push_back("it has no geometry");
      return s;
    }
    double* b = s.bounds_mm.data();
    box.Get(b[0], b[1], b[2], b[3], b[4], b[5]);
    if (!(std::abs(s.volume_mm3) > 0)) {
      s.issues.push_back("it encloses no volume");
      return s;
    }
    const double diagonal = std::hypot(b[3] - b[0], b[4] - b[1], b[5] - b[2]);
    s.mesh = refine(shape, std::abs(s.volume_mm3), diagonal, s.tolerance_mm, volume_target, chord);
  } catch (const Standard_Failure& e) {
    s.issues.push_back(std::string("OpenCASCADE failed on it: ") + e.what());
  }
  return s;
}

}  // namespace worker
