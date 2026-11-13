// What an exact solid's numbers mean, and the proof that it is the source
// solid (algorithm E in doc/cad-import/PLAN.md).
#include "worker.hpp"

#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepGProp.hxx>
#include <BRepGProp_Face.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <BRepTools.hxx>
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

double Primitive::value(const char* key) const {
  for (const auto& [k, v] : values)
    if (!std::strcmp(k, key)) return v;
  throw std::logic_error(std::string("a ") + solid + " has no " + key);
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

TopoDS_Face meridian(const std::vector<Node>& loop) {
  BRepBuilderAPI_MakeWire wire;
  for (size_t i = 0; i < loop.size(); ++i) {
    const Node &a = loop[i], &b = loop[(i + 1) % loop.size()];
    const gp_Pnt p(a.r, 0, a.z), q(b.r, 0, b.z);
    if (p.Distance(q) <= Precision::Confusion()) continue;
    if (a.mid) wire.Add(BRepBuilderAPI_MakeEdge(GC_MakeArcOfCircle(p, *a.mid, q).Value()).Edge());
    else wire.Add(BRepBuilderAPI_MakeEdge(p, q).Edge());
  }
  return BRepBuilderAPI_MakeFace(wire.Wire(), true).Face();
}

/// `face` swept about +z by `dphi` from +x towards +y (G4 startphi 0).
TopoDS_Shape revolve(const std::vector<Node>& loop, double dphi) {
  const TopoDS_Face face = meridian(loop);
  const gp_Ax1 z(gp::Origin(), gp::DZ());
  return dphi >= 2 * M_PI ? BRepPrimAPI_MakeRevol(face, z).Shape() : BRepPrimAPI_MakeRevol(face, z, dphi).Shape();
}

Node polar(double rho, double theta, std::optional<gp_Pnt> mid = {}) {
  return {rho * std::sin(theta), rho * std::cos(theta), mid};
}

gp_Pnt on_xz(const Node& n) { return {n.r, 0, n.z}; }

}  // namespace

// Box, tube and cone lengths are full lengths, centred; polycone points and
// xtru sections are coordinates; sphere theta is measured from +z
// (G4GDMLReadSolids).
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
  } else if (p.solid == "tube" || p.solid == "cone") {
    const bool tube = p.solid == "tube";
    const double h = v("z") / 2;
    s = revolve({{tube ? v("rmin") : v("rmin1"), -h},
                 {tube ? v("rmax") : v("rmax1"), -h},
                 {tube ? v("rmax") : v("rmax2"), h},
                 {tube ? v("rmin") : v("rmin2"), h}},
                v("deltaphi"));
  } else if (p.solid == "genericPolycone") {
    std::vector<Node> loop;
    for (const auto& [r, z] : p.points) loop.push_back({r, z});
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

double tilt(const gp_Dir& a, const gp_Dir& b) {
  const double angle = a.Angle(b);
  return std::min(angle, M_PI - angle);
}

/// Whether the lines `a` and `b` stay within `eps` of each other over a length `d`.
bool same_line(const gp_Ax1& a, const gp_Ax1& b, double eps, double d) {
  return tilt(a.Direction(), b.Direction()) * d + gp_Lin(a).Distance(b.Location()) <= eps;
}

/// Whether `a` and `b` lie on the same analytic surface within `eps`, over
/// a body of diagonal `d` (planes and cones without their orientation).
bool same(const BRepAdaptor_Surface& a, const BRepAdaptor_Surface& b, double eps, double d) {
  if (a.GetType() != b.GetType()) return false;
  switch (a.GetType()) {
    case GeomAbs_Plane:
      return tilt(a.Plane().Axis().Direction(), b.Plane().Axis().Direction()) * d +
                 a.Plane().Distance(b.Plane().Location()) <=
             eps;
    case GeomAbs_Cylinder:
      return same_line(a.Cylinder().Axis(), b.Cylinder().Axis(), eps, d) &&
             std::abs(a.Cylinder().Radius() - b.Cylinder().Radius()) <= eps;
    case GeomAbs_Cone:
      return same_line(a.Cone().Axis(), b.Cone().Axis(), eps, d) &&
             a.Cone().Apex().Distance(b.Cone().Apex()) <= eps &&
             std::abs(std::abs(a.Cone().SemiAngle()) - std::abs(b.Cone().SemiAngle())) * d <= eps;
    case GeomAbs_Sphere:
      return a.Sphere().Location().Distance(b.Sphere().Location()) +
                 std::abs(a.Sphere().Radius() - b.Sphere().Radius()) <=
             eps;
    case GeomAbs_Torus:
      return same_line(a.Torus().Axis(), b.Torus().Axis(), eps, d) &&
             a.Torus().Location().Distance(b.Torus().Location()) <= eps &&
             std::abs(a.Torus().MajorRadius() - b.Torus().MajorRadius()) <= eps &&
             std::abs(a.Torus().MinorRadius() - b.Torus().MinorRadius()) <= eps;
    default:
      return false;
  }
}

gp_Vec radial(const gp_Pnt& p, const gp_Ax1& axis) {
  const gp_Vec v(axis.Location(), p), z(axis.Direction());
  return v - z * v.Dot(z);
}

/// +1 where the material normal of `face` points away from the axis, centre
/// or minor circle of `surface` (along a plane's normal), -1 where it
/// points towards it, 0 where it is undefined. Constant over a face.
int side(const TopoDS_Face& face, const BRepAdaptor_Surface& surface) {
  double u0, u1, v0, v1;
  BRepTools::UVBounds(face, u0, u1, v0, v1);
  gp_Pnt p;
  gp_Vec n;
  BRepGProp_Face(face).Normal((u0 + u1) / 2, (v0 + v1) / 2, p, n);  // reversed with the face
  gp_Vec away;
  switch (surface.GetType()) {
    case GeomAbs_Plane: away = surface.Plane().Axis().Direction(); break;
    case GeomAbs_Cylinder: away = radial(p, surface.Cylinder().Axis()); break;
    case GeomAbs_Cone: away = radial(p, surface.Cone().Axis()); break;
    case GeomAbs_Sphere: away = gp_Vec(surface.Sphere().Location(), p); break;
    default: {
      const gp_Torus t = surface.Torus();
      const gp_Vec r = radial(p, t.Axis());
      if (r.Magnitude() == 0) return 0;
      away = gp_Vec(t.Location().Translated(r.Normalized() * t.MajorRadius()), p);
    }
  }
  const double dot = n.Dot(away);
  return dot > 0 ? 1 : dot < 0 ? -1 : 0;
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

// Both boundaries lying on one set of surfaces makes both solids unions of
// cells of that arrangement, so they can differ only by whole cells; each
// would change a surface's side, area or moment, or the volume.
std::string certify(const TopoDS_Shape& source, const TopoDS_Shape& candidate, double eps) {
  if (candidate.IsNull()) return "OpenCASCADE could not build it";
  Bnd_Box box;
  BRepBndLib::AddOptimal(source, box, false, false);
  double b[6];
  box.Get(b[0], b[1], b[2], b[3], b[4], b[5]);
  const gp_XYZ o((b[0] + b[3]) / 2, (b[1] + b[4]) / 2, (b[2] + b[5]) / 2);
  const double d = std::hypot(b[3] - b[0], b[4] - b[1], b[5] - b[2]);

  struct Side {
    bool any = false, mixed = false;
    int side = 0;
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
      auto u = std::find_if(surfaces.begin(), surfaces.end(), [&](const Surface& u) { return same(u.s, s, eps, d); });
      if (u == surfaces.end()) u = surfaces.insert(u, Surface{s, {}});
      Side& x = u->of[k];
      const int sd = side(face, u->s);
      if (sd == 0) return "a face on " + describe(u->s) + " has no defined normal";
      x.mixed |= x.any && x.side != sd;
      x.any = true;
      x.side = sd;
      GProp_GProps g;
      const double error = BRepGProp::SurfaceProperties(face, g, 1e-9);
      x.area += g.Mass();
      x.error += std::abs(error * g.Mass());
      x.moment += (g.CentreOfMass().XYZ() - o) * g.Mass();
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
    if (s.mixed || c.mixed || s.side != c.side) return "its faces on " + describe(u.s) + " face the other way";
    const double numeric = s.error + c.error;
    if (std::abs(s.area - c.area) > eps * c.perimeter + numeric)
      return "its area on " + describe(u.s) + " differs by " + number(c.area - s.area) + " mm2";
    if ((s.moment - c.moment).Modulus() > eps * (c.area + c.perimeter * d) + numeric * d)
      return "its faces on " + describe(u.s) + " lie elsewhere";
    area += c.area;
  }
  GProp_GProps vs, vc;
  const double es = BRepGProp::VolumeProperties(source, vs, 1e-9), ec = BRepGProp::VolumeProperties(candidate, vc, 1e-9);
  const double numeric = std::abs(es * vs.Mass()) + std::abs(ec * vc.Mass());
  if (std::abs(vs.Mass() - vc.Mass()) > eps * area + numeric)
    return "its volume differs by " + number(vc.Mass() - vs.Mass()) + " mm3";
  const gp_XYZ moment = (vs.CentreOfMass().XYZ() - o) * vs.Mass() - (vc.CentreOfMass().XYZ() - o) * vc.Mass();
  if (moment.Modulus() > (eps * area + numeric) * d) return "its volume lies elsewhere";
  return {};
}

}  // namespace worker
