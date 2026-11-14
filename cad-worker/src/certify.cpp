// What an exact solid's numbers mean, and the proof that it is the source
// solid (algorithm E in doc/cad-import/PLAN.md).
#include "worker.hpp"

#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepGProp.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <Bnd_Box.hxx>
#include <GC_MakeArcOfCircle.hxx>
#include <GProp_GProps.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <gp_Lin.hxx>
#include <gp_Pln.hxx>

#include <algorithm>
#include <cmath>
#include <cstring>

namespace worker {

double& Primitive::value(const char* key) {
  for (auto& [k, v] : values)
    if (!std::strcmp(k, key)) return v;
  throw std::logic_error(std::string("a ") + solid + " has no " + key);
}

// Box, tube and cone lengths are full lengths, centred; polycone points and
// xtru sections are coordinates (G4GDMLReadSolids).
std::vector<std::array<double, 2>> meridian(const Primitive& p) {
  if (p.solid == "genericPolycone") return p.points;
  if (p.solid != "tube" && p.solid != "cone") return {};
  const bool tube = p.solid == "tube";
  const double h = p.value("z") / 2;
  return {{tube ? p.value("rmin") : p.value("rmin1"), -h},
          {tube ? p.value("rmax") : p.value("rmax1"), -h},
          {tube ? p.value("rmax") : p.value("rmax2"), h},
          {tube ? p.value("rmin") : p.value("rmin2"), h}};
}

std::array<double, 12> matrix(const gp_Trsf& t) {
  std::array<double, 12> m{};
  for (int i = 1; i <= 3; ++i)
    for (int j = 1; j <= 4; ++j) m[(i - 1) * 4 + (j - 1)] = t.Value(i, j);
  return m;
}

namespace {

/// A point of a meridian in the y = 0 plane, (r, z), joined to the next by
/// an arc through `mid` when there is one, else by a segment.
struct Node {
  double r, z;
  std::optional<gp_Pnt> mid;
};

/// The face bounded by `loop`, swept about +z by `dphi` from +x towards +y
/// (G4 startphi 0).
TopoDS_Shape revolve(const std::vector<Node>& loop, double dphi) {
  BRepBuilderAPI_MakeWire wire;
  for (size_t i = 0; i < loop.size(); ++i) {
    const Node &a = loop[i], &b = loop[(i + 1) % loop.size()];
    const gp_Pnt p(a.r, 0, a.z), q(b.r, 0, b.z);
    if (p.Distance(q) <= Precision::Confusion()) continue;
    if (a.mid) wire.Add(BRepBuilderAPI_MakeEdge(GC_MakeArcOfCircle(p, *a.mid, q).Value()).Edge());
    else wire.Add(BRepBuilderAPI_MakeEdge(p, q).Edge());
  }
  const TopoDS_Face face = BRepBuilderAPI_MakeFace(wire.Wire(), true).Face();
  const gp_Ax1 z(gp::Origin(), gp::DZ());
  return dphi >= 2 * M_PI ? BRepPrimAPI_MakeRevol(face, z).Shape() : BRepPrimAPI_MakeRevol(face, z, dphi).Shape();
}

Node polar(double rho, double theta, std::optional<gp_Pnt> mid = {}) {
  return {rho * std::sin(theta), rho * std::cos(theta), mid};
}

gp_Pnt on_xz(const Node& n) { return {n.r, 0, n.z}; }

}  // namespace

// Sphere theta is measured from +z (G4GDMLReadSolids).
TopoDS_Shape shape(const Primitive& p) {
  const auto v = [&](const char* key) { return p.value(key); };
  TopoDS_Shape s;
  if (p.solid == "box") {
    s = BRepPrimAPI_MakeBox(gp_Pnt(-v("x") / 2, -v("y") / 2, -v("z") / 2), v("x"), v("y"), v("z")).Shape();
  } else if (p.solid == "xtru") {
    BRepBuilderAPI_MakePolygon polygon;
    for (const auto& [x, y] : p.points) polygon.Add(gp_Pnt(x, y, v("z1")));
    polygon.Close();
    s = BRepPrimAPI_MakePrism(BRepBuilderAPI_MakeFace(polygon.Wire(), true).Face(),
                              gp_Vec(0, 0, v("z2") - v("z1")))
            .Shape();
  } else if (const auto rz = meridian(p); !rz.empty()) {
    std::vector<Node> loop;
    for (const auto& [r, z] : rz) loop.push_back({r, z});
    s = revolve(loop, v("deltaphi"));
  } else if (p.solid == "sphere" || p.solid == "orb") {
    const bool orb = p.solid == "orb";
    const double rmax = orb ? v("r") : v("rmax"), rmin = orb ? 0 : v("rmin");
    const double t0 = orb ? 0 : v("starttheta"), t1 = orb ? M_PI : t0 + v("deltatheta"), tm = (t0 + t1) / 2;
    std::vector<Node> loop = {polar(rmax, t0, on_xz(polar(rmax, tm))), polar(rmax, t1)};
    if (rmin > 0) loop.insert(loop.end(), {polar(rmin, t1, on_xz(polar(rmin, tm))), polar(rmin, t0)});
    else loop.push_back({0, 0});
    s = revolve(loop, orb ? 2 * M_PI : v("deltaphi"));
  } else if (p.solid == "torus") {
    const double c = v("rtor");
    const auto disk = [&](double r) {
      return revolve({{c + r, 0, gp_Pnt(c, 0, r)}, {c - r, 0, gp_Pnt(c, 0, -r)}}, v("deltaphi"));
    };
    s = disk(v("rmax"));
    if (v("rmin") > 0) s = BRepAlgoAPI_Cut(s, disk(v("rmin"))).Shape();
  } else {
    throw std::logic_error("no shape for a " + p.solid);
  }
  return s.Moved(TopLoc_Location(p.frame));
}

TopoDS_Shape shape(const Exact& e) {
  const TopoDS_Shape stock = shape(e.stock);
  if (e.tools.empty()) return stock;
  NCollection_List<TopoDS_Shape> tools;
  for (const Primitive& t : e.tools) tools.Append(shape(t));
  BRepAlgoAPI_Cut cut;
  cut.SetArguments({stock});
  cut.SetTools(tools);
  cut.SetNonDestructive(true);
  cut.Build();
  return cut.HasErrors() ? TopoDS_Shape() : cut.Shape();
}

namespace {

/// An upper bound on how far `b` lies from `a` within distance `r` of `o`;
/// infinite for different kinds (planes and cones without orientation).
double gap(const BRepAdaptor_Surface& a, const BRepAdaptor_Surface& b, const gp_Pnt& o, double r) {
  if (a.GetType() != b.GetType()) return HUGE_VAL;
  switch (a.GetType()) {
    case GeomAbs_Plane: {
      // Apart where b passes nearest o, then diverging by their tilt.
      const gp_Pln p = a.Plane(), q = b.Plane();
      const gp_Vec n(q.Axis().Direction());
      return p.Distance(o.Translated(n * gp_Vec(o, q.Location()).Dot(n))) +
             tilt(p.Axis().Direction(), q.Axis().Direction()) * r;
    }
    case GeomAbs_Cylinder: {
      const gp_Ax1 x = a.Cylinder().Axis(), y = b.Cylinder().Axis();
      return gp_Lin(x).Distance(foot(o, y)) + tilt(x.Direction(), y.Direction()) * r +
             std::abs(a.Cylinder().Radius() - b.Cylinder().Radius());
    }
    case GeomAbs_Cone: {
      const gp_Cone x = a.Cone(), y = b.Cone();
      return x.Apex().Distance(y.Apex()) + (tilt(x.Axis().Direction(), y.Axis().Direction()) +
                                            std::abs(std::abs(x.SemiAngle()) - std::abs(y.SemiAngle()))) *
                                               (x.Apex().Distance(o) + r);
    }
    case GeomAbs_Sphere:
      return a.Sphere().Location().Distance(b.Sphere().Location()) +
             std::abs(a.Sphere().Radius() - b.Sphere().Radius());
    case GeomAbs_Torus: {
      const gp_Torus x = a.Torus(), y = b.Torus();
      return x.Location().Distance(y.Location()) +
             tilt(x.Axis().Direction(), y.Axis().Direction()) * (x.MajorRadius() + x.MinorRadius()) +
             std::abs(x.MajorRadius() - y.MajorRadius()) + std::abs(x.MinorRadius() - y.MinorRadius());
    }
    default:
      return HUGE_VAL;
  }
}

std::string number(double x) {
  char text[32];
  std::snprintf(text, sizeof text, "%.6g", x + 0.0);  // no "-0"
  return text;
}

std::string describe(const BRepAdaptor_Surface& s) {
  switch (s.GetType()) {
    case GeomAbs_Plane: {
      const gp_Dir n = s.Plane().Axis().Direction();
      return "the plane normal to (" + number(n.X()) + ", " + number(n.Y()) + ", " + number(n.Z()) + ")";
    }
    case GeomAbs_Cylinder: return "the cylinder of radius " + number(s.Cylinder().Radius()) + " mm";
    case GeomAbs_Cone: return "the cone of half-angle " + number(std::abs(s.Cone().SemiAngle()) * 180 / M_PI) + " deg";
    case GeomAbs_Sphere: return "the sphere of radius " + number(s.Sphere().Radius()) + " mm";
    default:
      return "the torus of radii " + number(s.Torus().MajorRadius()) + " and " + number(s.Torus().MinorRadius()) +
             " mm";
  }
}

}  // namespace

// Rule 2 puts both boundaries on one set of surfaces, which makes both solids
// unions of cells of that arrangement; the volume of their intersection then
// shows whether any cell differs. Per-surface sums alone cannot: cells can
// trade places and keep every area and moment.
std::string certify(const TopoDS_Shape& source, const TopoDS_Shape& candidate, double eps) {
  if (candidate.IsNull()) return "OpenCASCADE could not build it";
  Bnd_Box box;
  BRepBndLib::AddOptimal(source, box, false, false);
  const gp_Pnt o((box.CornerMin().XYZ() + box.CornerMax().XYZ()) / 2);
  const double d = box.CornerMin().Distance(box.CornerMax());

  struct Side {
    bool any = false;
    double area = 0, error = 0, perimeter = 0;
    gp_XYZ moment;
  };
  struct Surface {
    BRepAdaptor_Surface s;
    Side of[2];  // source, candidate
  };
  std::vector<Surface> surfaces;
  const TopoDS_Shape solids[2] = {source, candidate};
  for (int k = 0; k < 2; ++k)
    for (TopExp_Explorer ex(solids[k], TopAbs_FACE); ex.More(); ex.Next()) {
      const TopoDS_Face& face = TopoDS::Face(ex.Current());
      const BRepAdaptor_Surface s(face);
      if (s.GetType() > GeomAbs_Torus) return k ? "it has a free-form face" : "a face is free-form";
      // Within eps / 2 of the first face, so any two faces of a surface are within eps.
      auto u = std::find_if(surfaces.begin(), surfaces.end(),
                            [&](const Surface& u) { return gap(u.s, s, o, d / 2) <= eps / 2; });
      if (u == surfaces.end()) u = surfaces.insert(u, Surface{s, {}});
      Side& x = u->of[k];
      x.any = true;
      GProp_GProps g;
      const double error = BRepGProp::SurfaceProperties(face, g, 1e-9);
      x.area += g.Mass();
      x.error += std::abs(error * g.Mass());
      x.moment += (g.CentreOfMass().XYZ() - o.XYZ()) * g.Mass();
      if (k) {
        GProp_GProps edges;
        BRepGProp::LinearProperties(face, edges);
        x.perimeter += edges.Mass();
      }
    }
  double area = 0;
  for (const Surface& u : surfaces) {
    const Side &s = u.of[0], &c = u.of[1];
    if (!s.any) return "it has " + describe(u.s) + ", which the part does not";
    if (!c.any) return "the part has " + describe(u.s) + ", which it does not";
    const double numeric = s.error + c.error;
    if (std::abs(s.area - c.area) > eps * c.perimeter + numeric)
      return "its area on " + describe(u.s) + " differs by " + number(c.area - s.area) + " mm2";
    if ((s.moment - c.moment).Modulus() > eps * (c.area + c.perimeter * d) + numeric * d)
      return "its faces on " + describe(u.s) + " lie elsewhere";
    area += c.area;
  }
  BRepAlgoAPI_Common common(source, candidate);
  if (common.HasErrors()) return "OpenCASCADE could not intersect it with the part";
  GProp_GProps vs, vc, vi;
  const double es = BRepGProp::VolumeProperties(source, vs, 1e-9), ec = BRepGProp::VolumeProperties(candidate, vc, 1e-9),
               ei = BRepGProp::VolumeProperties(common.Shape(), vi, 1e-9);
  const double allowed = eps * area + std::abs(es * vs.Mass()) + std::abs(ec * vc.Mass()) + std::abs(ei * vi.Mass());
  if (std::abs(vs.Mass() - vi.Mass()) > allowed || std::abs(vc.Mass() - vi.Mass()) > allowed)
    return "it differs from the part by " + number(vs.Mass() + vc.Mass() - 2 * vi.Mass()) + " mm3";
  return {};
}

}  // namespace worker
