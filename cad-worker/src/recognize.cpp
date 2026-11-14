// Exact GDML solids for B-rep solids (algorithm D in doc/cad-import/PLAN.md).
// Templates read a candidate off the solid's own surfaces and sections, a
// solid of revolution or a prism, or a stock minus cut-outs; only
// certify() decides whether it is the solid.
#include "worker.hpp"

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepGProp.hxx>
#include <BRepGProp_Face.hxx>
#include <BRepTools.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <GProp_GProps.hxx>
#include <ShapeAnalysis_ShapeTolerance.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <Standard_ErrorHandler.hxx>
#include <Standard_Failure.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Wire.hxx>
#include <gp_Ax3.hxx>
#include <gp_Lin.hxx>
#include <gp_Pln.hxx>
#include <gp_Pnt2d.hxx>
#include <gp_Vec2d.hxx>

#include <algorithm>
#include <cmath>

namespace worker {
namespace {

constexpr double MIN_EPS = 1e-7;           // mm, OCCT's Precision::Confusion()
constexpr double MAX_RELATIVE_EPS = 1e-4;  // of the diagonal: coarser solids are not recognized
constexpr double MIN_GAP = 1e-6;           // rad: narrower angular gaps read as full turns
constexpr size_t MAX_TOOLS = 16;
constexpr double TURN = 2 * M_PI;
const char* const BOX_SIDES[3] = {"x", "y", "z"};  // a box's lengths along its frame axes

struct Body {
  TopoDS_Shape shape;
  double eps, d;  // tolerance and diagonal, mm
  gp_Pnt centre;  // of the bounds
  std::vector<std::pair<TopoDS_Face, BRepAdaptor_Surface>> faces;
};

/// The least and greatest coordinates of `shape` in `frame`.
Bnd_Box bounds(const TopoDS_Shape& shape, const gp_Ax3& frame) {
  gp_Trsf to_frame;
  to_frame.SetTransformation(frame);
  Bnd_Box box;
  BRepBndLib::AddOptimal(shape.Moved(TopLoc_Location(to_frame)), box, false, false);
  return box;
}

Body body(const TopoDS_Shape& shape, double eps) {
  Body b{shape, eps, 0, {}, {}};
  const Bnd_Box box = bounds(shape, gp_Ax3());
  b.centre = gp_Pnt((box.CornerMin().XYZ() + box.CornerMax().XYZ()) / 2);
  b.d = box.CornerMin().Distance(box.CornerMax());
  for (TopExp_Explorer ex(shape, TopAbs_FACE); ex.More(); ex.Next())
    b.faces.emplace_back(TopoDS::Face(ex.Current()), BRepAdaptor_Surface(TopoDS::Face(ex.Current())));
  return b;
}

gp_Trsf placement(const gp_Ax3& frame) {
  gp_Trsf t;
  t.SetTransformation(frame);
  return t.Inverted();
}

gp_Ax3 axes(const gp_Trsf& t) {
  return gp_Ax3(gp_Pnt(t.TranslationPart()), gp_Dir(t.Value(1, 3), t.Value(2, 3), t.Value(3, 3)),
                gp_Dir(t.Value(1, 1), t.Value(2, 1), t.Value(3, 1)));
}

Primitive make(const char* solid, std::vector<std::pair<const char*, double>> values, const gp_Ax3& frame,
               std::vector<std::array<double, 2>> points = {}) {
  return {solid, std::move(values), std::move(points), placement(frame)};
}

/// A section edge in its plane's coordinates: a segment, or an arc.
struct Edge2 {
  gp_Pnt2d a, b;
  bool arc = false;
  gp_Pnt2d centre;
  double radius = 0;
};
using Loop = std::vector<Edge2>;

/// The region of `solid` in the rectangle [-w, w] x [-w, w] (or [0, w] in x
/// when `half`) of plane `at`, as each face's loops, outer first. Empty when
/// OpenCASCADE fails or an edge is neither a line nor a circle.
std::vector<std::vector<Loop>> section(const TopoDS_Shape& solid, const gp_Ax3& at, double w, bool half) {
  BRepAlgoAPI_Common common;
  common.SetArguments({solid});
  common.SetTools({BRepBuilderAPI_MakeFace(gp_Pln(at), half ? 0 : -w, w, -w, w).Face()});
  common.SetNonDestructive(true);
  common.Build();
  if (common.HasErrors()) return {};
  ShapeUpgrade_UnifySameDomain unify(common.Shape());
  unify.Build();
  const auto local = [&](const gp_Pnt& p) {
    const gp_Vec v(at.Location(), p);
    return gp_Pnt2d(v.Dot(at.XDirection()), v.Dot(at.YDirection()));
  };
  std::vector<std::vector<Loop>> out;
  for (TopExp_Explorer f(unify.Shape(), TopAbs_FACE); f.More(); f.Next()) {
    const TopoDS_Face& face = TopoDS::Face(f.Current());
    const TopoDS_Wire outer = BRepTools::OuterWire(face);
    std::vector<Loop> loops(1);
    for (TopExp_Explorer w(face, TopAbs_WIRE); w.More(); w.Next()) {
      Loop loop;
      for (BRepTools_WireExplorer e(TopoDS::Wire(w.Current()), face); e.More(); e.Next()) {
        const BRepAdaptor_Curve c(e.Current());
        double t0 = c.FirstParameter(), t1 = c.LastParameter();
        if (e.Orientation() == TopAbs_REVERSED) std::swap(t0, t1);
        Edge2 x{local(c.Value(t0)), local(c.Value(t1))};
        if (c.GetType() == GeomAbs_Circle) {
          x.arc = true;
          x.centre = local(c.Circle().Location());
          x.radius = c.Circle().Radius();
        } else if (c.GetType() != GeomAbs_Line) {
          return {};
        }
        loop.push_back(x);
      }
      if (w.Current().IsSame(outer)) loops[0] = loop;
      else loops.push_back(loop);
    }
    out.push_back(loops);
  }
  return out;
}

/// The corners of a loop of segments, without points on a straight run.
std::vector<gp_Pnt2d> corners(const Loop& loop, double eps) {
  std::vector<gp_Pnt2d> p;
  for (const Edge2& e : loop) p.push_back(e.a);
  for (size_t i = 0; i < p.size() && p.size() > 3;) {
    const gp_Pnt2d &a = p[(i + p.size() - 1) % p.size()], &b = p[i], &c = p[(i + 1) % p.size()];
    const gp_Vec2d ac(a, c);
    const double off = ac.Magnitude() > 0 ? std::abs(ac.Crossed(gp_Vec2d(a, b))) / ac.Magnitude() : a.Distance(b);
    if (off <= eps) p.erase(p.begin() + i);
    else ++i;
  }
  return p;
}

double area(const std::vector<gp_Pnt2d>& p) {
  double twice = 0;
  for (size_t i = 0; i < p.size(); ++i) twice += p[i].XY().Crossed(p[(i + 1) % p.size()].XY());
  return twice / 2;
}

/// The solid swept by the polygonal meridian `rz` ((r, z) in `frame`) about
/// the frame's z axis by `dphi` from its x axis: a tube, a cone, or else a
/// genericPolycone.
Primitive polygon_solid(std::vector<gp_Pnt2d> rz, gp_Ax3 frame, double dphi, double eps) {
  for (gp_Pnt2d& p : rz)
    if (std::abs(p.X()) <= eps) p.SetX(0);
  double z0 = HUGE_VAL, z1 = -HUGE_VAL;
  for (const gp_Pnt2d& p : rz) z0 = std::min(z0, p.Y()), z1 = std::max(z1, p.Y());
  std::vector<double> bottom, top;  // radii at either end
  bool two_levels = true;
  for (const gp_Pnt2d& p : rz) {
    if (std::abs(p.Y() - z0) <= eps) bottom.push_back(p.X());
    else if (std::abs(p.Y() - z1) <= eps) top.push_back(p.X());
    else two_levels = false;
  }
  // A single point at an end is an apex on the axis; Geant4 changes an inner
  // radius that is zero at one end only (G4Cons), so that is a polycone.
  const auto ends = [](std::vector<double>& r) {
    std::sort(r.begin(), r.end());
    return (r.size() == 2 || (r.size() == 1 && r[0] == 0)) ? std::array<double, 2>{r.front(), r.back()}
                                                           : std::array<double, 2>{-1, -1};
  };
  if (two_levels) {
    const auto [rmin1, rmax1] = ends(bottom);
    const auto [rmin2, rmax2] = ends(top);
    if (rmin1 >= 0 && rmin2 >= 0 && (rmin1 == 0) == (rmin2 == 0)) {
      frame.SetLocation(frame.Location().Translated(gp_Vec(frame.Direction()) * ((z0 + z1) / 2)));
      if (std::abs(rmin1 - rmin2) <= eps && std::abs(rmax1 - rmax2) <= eps)
        return make("tube", {{"rmin", rmin1}, {"rmax", rmax1}, {"z", z1 - z0}, {"deltaphi", dphi}}, frame);
      return make("cone",
                  {{"rmin1", rmin1}, {"rmax1", rmax1}, {"rmin2", rmin2}, {"rmax2", rmax2}, {"z", z1 - z0},
                   {"deltaphi", dphi}},
                  frame);
    }
  }
  std::vector<std::array<double, 2>> points;
  for (const gp_Pnt2d& p : rz) points.push_back({p.X(), p.Y()});
  return make("genericPolycone", {{"deltaphi", dphi}}, frame, points);
}

/// The solid of revolution with meridian `loops` ((r, z) in `frame`), swept
/// by `dphi`, or none with the reason in `why`.
std::optional<Primitive> revolved(const std::vector<Loop>& loops, gp_Ax3 frame, double dphi, double eps,
                                  std::string& why) {
  const bool arcs = std::any_of(loops.begin(), loops.end(), [](const Loop& l) {
    return std::any_of(l.begin(), l.end(), [](const Edge2& e) { return e.arc; });
  });
  const auto lift = [&](double z) {
    frame.SetLocation(frame.Location().Translated(gp_Vec(frame.Direction()) * z));
  };
  if (!arcs) {
    const std::vector<gp_Pnt2d> rz = loops.size() == 1 ? corners(loops[0], eps) : std::vector<gp_Pnt2d>();
    // G4GenericPolycone refuses a meridian narrower than kCarTolerance (1e-9 mm2).
    why = loops.size() > 1 ? "its meridian has a hole" : "its meridian is too thin for Geant4";
    return std::abs(area(rz)) < 1e-9 ? std::nullopt : std::optional(polygon_solid(rz, frame, dphi, eps));
  }
  // A torus: a full circle off the axis, perhaps with a concentric one inside.
  const auto circle = [&](const Loop& l) { return l.size() == 1 && l[0].arc && l[0].a.Distance(l[0].b) <= eps; };
  if (circle(loops[0]) && (loops.size() == 1 || (loops.size() == 2 && circle(loops[1]) &&
                                                 loops[1][0].centre.Distance(loops[0][0].centre) <= eps))) {
    const gp_Pnt2d c = loops[0][0].centre;
    const double rmax = loops[0][0].radius, rmin = loops.size() == 2 ? loops[1][0].radius : 0;
    // G4Torus refuses rtor < rmax + 1e3 kCarTolerance and zeroes rmin < 1e2 kCarTolerance.
    if (c.X() < rmax + 1e-6 || (rmin > 0 && (rmin < 1e-7 || rmin > rmax - 1e-7))) {
      why = "it is a torus Geant4 would change";
      return std::nullopt;
    }
    lift(c.Y());
    return make("torus", {{"rmin", rmin}, {"rmax", rmax}, {"rtor", c.X()}, {"deltaphi", dphi}}, frame);
  }
  // A sphere: arcs about one centre on the axis, radial segments between them.
  const Loop& l = loops[0];
  const auto arc = std::find_if(l.begin(), l.end(), [](const Edge2& e) { return e.arc; });
  if (arc == l.end() || loops.size() > 1 || std::abs(arc->centre.X()) > eps ||
      std::any_of(l.begin(), l.end(), [&](const Edge2& e) { return e.arc && e.centre.Distance(arc->centre) > eps; })) {
    why = "its meridian has arcs no GDML solid of revolution has";
    return std::nullopt;
  }
  const gp_Pnt2d c = arc->centre;
  double rmin = HUGE_VAL, rmax = 0, t0 = HUGE_VAL, t1 = -HUGE_VAL;
  for (const Edge2& e : l)
    if (e.arc) rmin = std::min(rmin, e.radius), rmax = std::max(rmax, e.radius);
  if (rmax - rmin <= eps) rmin = 0;
  for (const Edge2& e : l)
    if (e.arc && rmax - e.radius <= eps)
      for (const gp_Pnt2d& p : {e.a, e.b}) {
        const double t = std::atan2(p.X(), p.Y() - c.Y());
        t0 = std::min(t0, t), t1 = std::max(t1, t);
      }
  if (t0 * rmax <= eps) t0 = 0;
  if ((M_PI - t1) * rmax <= eps) t1 = M_PI;
  lift(c.Y());
  if (rmin == 0 && t0 == 0 && t1 == M_PI && dphi == TURN) return make("orb", {{"r", rmax}}, frame);
  return make("sphere",
              {{"rmin", rmin}, {"rmax", rmax}, {"deltaphi", dphi}, {"starttheta", t0}, {"deltatheta", t1 - t0}},
              frame);
}

/// The prism with cross-section `outer` (in plane `at`, centred between its
/// caps) and height `h`: a box, or else an xtru.
std::optional<Primitive> prism(const Loop& outer, const gp_Ax3& at, double h, double eps, std::string& why) {
  if (std::any_of(outer.begin(), outer.end(), [](const Edge2& e) { return e.arc; })) {
    why = "its cross-section has arcs";
    return std::nullopt;
  }
  std::vector<gp_Pnt2d> p = corners(outer, eps);
  if (p.size() == 4) {
    const gp_Vec2d x(p[0], p[1]), y(p[1], p[2]);
    bool square = true;
    for (size_t i = 0; i < 4; ++i) {
      const gp_Vec2d u(p[i], p[(i + 1) % 4]), v(p[(i + 1) % 4], p[(i + 2) % 4]);
      square &= std::abs(u.Dot(v)) <= eps * std::min(u.Magnitude(), v.Magnitude());
    }
    if (square) {
      const gp_XY centre = (p[0].XY() + p[2].XY()) / 2;
      const gp_Ax3 box(at.Location().Translated(gp_Vec(at.XDirection()) * centre.X() + gp_Vec(at.YDirection()) * centre.Y()),
                       at.Direction(), gp_Vec(at.XDirection()) * x.X() + gp_Vec(at.YDirection()) * x.Y());
      return make("box", {{"x", x.Magnitude()}, {"y", y.Magnitude()}, {"z", h}}, box);
    }
  }
  if (area(p) > 0) std::reverse(p.begin(), p.end());  // Geant4 expects clockwise
  std::vector<std::array<double, 2>> points;
  for (const gp_Pnt2d& q : p) points.push_back({q.X(), q.Y()});
  return make("xtru", {{"z1", -h / 2}, {"z2", h / 2}}, at, points);
}

/// The plane directions of `b`, most face area first.
std::vector<gp_Dir> plane_directions(const Body& b) {
  std::vector<std::pair<gp_Dir, double>> found;
  for (const auto& [face, s] : b.faces) {
    if (s.GetType() != GeomAbs_Plane) continue;
    const gp_Dir n = s.Plane().Axis().Direction();
    GProp_GProps g;
    BRepGProp::SurfaceProperties(face, g);
    auto it = std::find_if(found.begin(), found.end(), [&](const auto& f) { return tilt(f.first, n) * b.d <= b.eps; });
    if (it == found.end()) found.emplace_back(n, g.Mass());
    else it->second += g.Mass();
  }
  std::stable_sort(found.begin(), found.end(), [](const auto& x, const auto& y) { return x.second > y.second; });
  std::vector<gp_Dir> out;
  for (const auto& f : found) out.push_back(f.first);
  return out;
}

/// The prism along `n` whose cross-section is the outer loop of the section
/// of `b` halfway between its extremes along `n`.
std::optional<Primitive> prism_along(const Body& b, const gp_Dir& n, std::string& why) {
  const gp_Ax3 frame(b.centre, n);
  const Bnd_Box box = bounds(b.shape, frame);
  const double z0 = box.CornerMin().Z(), z1 = box.CornerMax().Z();
  const gp_Ax3 at(b.centre.Translated(gp_Vec(n) * ((z0 + z1) / 2)), n, frame.XDirection());
  const auto faces = section(b.shape, at, 2 * b.d, false);
  if (faces.empty()) {
    why = "OpenCASCADE could not section it";
    return std::nullopt;
  }
  // The face with the largest outline: the others lie in its holes.
  const auto outline = [&](const std::vector<Loop>& f) { return std::abs(area(corners(f[0], b.eps))); };
  const auto& largest = *std::max_element(faces.begin(), faces.end(),
                                          [&](const auto& x, const auto& y) { return outline(x) < outline(y); });
  return prism(largest[0], at, z1 - z0, b.eps, why);
}

/// The half-plane through the z axis of `frame` at `angle` from its x axis,
/// with plane coordinates (r, z).
gp_Ax3 meridian_plane(const gp_Ax3& frame, double angle) {
  const gp_Dir u(gp_Vec(frame.XDirection()) * std::cos(angle) + gp_Vec(frame.YDirection()) * std::sin(angle));
  return gp_Ax3(frame.Location(), u.Crossed(frame.Direction()), u);
}

std::optional<Primitive> revolution(const Body& b, std::string& why) {
  why = "its faces do not share one axis";
  std::optional<gp_Ax1> axis;
  for (const auto& [face, s] : b.faces) {
    if (s.GetType() == GeomAbs_Cylinder) axis = s.Cylinder().Axis();
    else if (s.GetType() == GeomAbs_Cone) axis = s.Cone().Axis();
    else if (s.GetType() == GeomAbs_Torus) axis = s.Torus().Axis();
    if (axis) break;
  }
  if (!axis) {  // spheres and planes: the planes' normal, or two planes' common line
    const auto sphere = std::find_if(b.faces.begin(), b.faces.end(),
                                     [](const auto& f) { return f.second.GetType() == GeomAbs_Sphere; });
    gp_Dir dir = gp::DZ();
    std::optional<gp_Dir> first;
    for (const auto& [face, s] : b.faces) {
      if (s.GetType() != GeomAbs_Plane) continue;
      const gp_Dir n = s.Plane().Axis().Direction();
      if (!first) dir = *(first = n);
      else if (tilt(*first, n) * b.d > b.eps) {
        dir = first->Crossed(n);
        break;
      }
    }
    axis = gp_Ax1(sphere->second.Sphere().Location(), dir);
  }
  const gp_Dir a = axis->Direction();
  const gp_Pnt o = foot(b.centre, *axis);
  const gp_Lin line(*axis);
  // Every face around the axis, across it (a cap) or along it (an angular cut).
  std::vector<gp_Dir> starts, ends;
  const auto add = [&](std::vector<gp_Dir>& rays, const gp_Dir& u) {
    if (std::none_of(rays.begin(), rays.end(), [&](const gp_Dir& r) { return r.Angle(u) * b.d <= b.eps; }))
      rays.push_back(u);
  };
  for (const auto& [face, s] : b.faces) {
    switch (s.GetType()) {
      case GeomAbs_Plane: {
        const gp_Pln plane = s.Plane();
        const gp_Dir n = plane.Axis().Direction();
        if (tilt(n, a) * b.d <= b.eps) break;
        if (std::abs(n.Dot(a)) * b.d > b.eps || plane.Distance(o) > b.eps) return std::nullopt;
        // An angular cut: on which side of the axis it lies, and which way it faces.
        double u0, u1, v0, v1;
        BRepTools::UVBounds(face, u0, u1, v0, v1);
        gp_Pnt p;
        gp_Vec out;
        BRepGProp_Face(face).Normal((u0 + u1) / 2, (v0 + v1) / 2, p, out);
        const gp_Dir w = n.Crossed(a);
        const Bnd_Box span = bounds(face, gp_Ax3(o, a, w));
        for (const double sign : {1.0, -1.0})
          if (sign * (sign > 0 ? span.CornerMax().X() : span.CornerMin().X()) > b.eps) {
            const gp_Dir u = sign > 0 ? w : w.Reversed();
            add(out.Dot(gp_Vec(a.Crossed(u))) < 0 ? starts : ends, u);
          }
        break;
      }
      case GeomAbs_Cylinder:
      case GeomAbs_Cone:
      case GeomAbs_Torus: {
        const gp_Ax1 own = s.GetType() == GeomAbs_Cylinder ? s.Cylinder().Axis()
                           : s.GetType() == GeomAbs_Cone   ? s.Cone().Axis()
                                                           : s.Torus().Axis();
        if (tilt(own.Direction(), a) * b.d + line.Distance(own.Location()) > b.eps) return std::nullopt;
        break;
      }
      default:
        if (line.Distance(s.Sphere().Location()) > b.eps) return std::nullopt;
    }
  }
  if (starts.size() != ends.size() || starts.size() > 1) {
    why = "it has more than one angular sector";
    return std::nullopt;
  }
  double dphi = TURN;
  gp_Ax3 frame(o, a);
  if (!starts.empty()) {
    const gp_Vec s(starts[0]), e(ends[0]);
    dphi = std::atan2(s.Crossed(e).Dot(gp_Vec(a)), s.Dot(e));
    if (dphi <= 0) dphi += TURN;
    if (dphi > TURN - MIN_GAP) {
      why = "its angular gap is too narrow for Geant4";
      return std::nullopt;
    }
    frame = gp_Ax3(o, a, starts[0]);
  }
  const auto faces = section(b.shape, meridian_plane(frame, dphi < TURN ? dphi / 2 : 0), 2 * b.d, true);
  if (faces.size() != 1) {
    why = faces.empty() ? "OpenCASCADE could not section it" : "its meridian section is not one region";
    return std::nullopt;
  }
  return revolved(faces[0], frame, dphi, b.eps, why);
}

long free_form(const TopoDS_Shape& shape) {
  long n = 0;
  for (TopExp_Explorer ex(shape, TopAbs_FACE); ex.More(); ex.Next())
    n += BRepAdaptor_Surface(TopoDS::Face(ex.Current())).GetType() > GeomAbs_Torus;
  return n;
}

/// A GDML solid equal to `b`, or none with the reason in `why`.
std::optional<Primitive> primitive(const Body& b, std::string& why) {
  if (free_form(b.shape) > 0) {
    why = "it has free-form faces";
    return std::nullopt;
  }
  const auto certified = [&](std::optional<Primitive>& p) {
    if (!p) return false;
    const std::string failed = certify(b.shape, shape(*p), b.eps);
    if (failed.empty()) return true;
    why = "the " + p->solid + " read off it fails: " + failed;
    return false;
  };
  if (std::any_of(b.faces.begin(), b.faces.end(), [](const auto& f) { return f.second.GetType() != GeomAbs_Plane; })) {
    auto p = revolution(b, why);
    return certified(p) ? p : std::nullopt;
  }
  // A prism along a direction that has every face across or along it.
  why = "no direction has all its faces across or along it";
  for (const gp_Dir& n : plane_directions(b)) {
    const bool fits = std::all_of(b.faces.begin(), b.faces.end(), [&](const auto& f) {
      const gp_Dir m = f.second.Plane().Axis().Direction();
      return std::min(tilt(m, n), std::abs(M_PI / 2 - m.Angle(n))) * b.d <= b.eps;
    });
    if (!fits) continue;
    auto p = prism_along(b, n, why);
    if (certified(p)) return p;
  }
  return std::nullopt;
}

/// A plane face of a tool that may lie on the stock, in the tool's frame: a
/// point on it, its outward normal, and the frame axis it lies across (0, 1,
/// 2), or -1 for an xtru side, which is not moved.
struct End {
  gp_Pnt at;
  gp_Dir out;
  int axis;
};

std::vector<End> ends(const Primitive& t) {
  std::vector<End> out;
  const auto across = [&](int k, double low, double high) {
    gp_XYZ e;
    e.SetCoord(k + 1, 1);
    out.push_back({gp_Pnt(e * low), gp_Dir(e.Reversed()), k});
    out.push_back({gp_Pnt(e * high), gp_Dir(e), k});
  };
  if (t.solid == "box") {
    for (const int k : {0, 1, 2}) across(k, -t.value(BOX_SIDES[k]) / 2, t.value(BOX_SIDES[k]) / 2);
  } else if (t.solid == "xtru") {
    across(2, t.value("z1"), t.value("z2"));
    for (size_t i = 0; i < t.points.size(); ++i) {  // clockwise: outward is to the left
      const auto [ax, ay] = t.points[i];
      const auto [bx, by] = t.points[(i + 1) % t.points.size()];
      out.push_back({gp_Pnt(ax, ay, t.value("z1")), gp_Dir(ay - by, bx - ax, 0), -1});
    }
  } else if (const auto rz = meridian(t); !rz.empty()) {
    const auto [low, high] =
        std::minmax_element(rz.begin(), rz.end(), [](const auto& p, const auto& q) { return p[1] < q[1]; });
    across(2, (*low)[1], (*high)[1]);
  }
  return out;
}

/// Moves each end of `tool` that lies on a face of `stock` outward by
/// `delta`, one end per call, then the rest. That removes nothing more as
/// long as the stock lies wholly inside that plane, and leaves no surface the
/// two share. False, with the reason in `why`, when an end lies on a face
/// that does not bound the stock, or an xtru side lies on the stock.
bool extend(Primitive& tool, const Body& stock, double delta, std::string& why) {
  for (const auto& [q, m, k] : ends(tool)) {
    const gp_Dir n = m.Transformed(tool.frame);
    const gp_Pnt p = q.Transformed(tool.frame);
    const bool on_stock = std::any_of(stock.faces.begin(), stock.faces.end(), [&](const auto& face) {
      const BRepAdaptor_Surface& s = face.second;
      return s.GetType() == GeomAbs_Plane && tilt(s.Plane().Axis().Direction(), n) * stock.d <= stock.eps &&
             s.Plane().Distance(p) <= stock.eps;
    });
    if (!on_stock) continue;
    if (bounds(stock.shape, gp_Ax3(p, n)).CornerMax().Z() > stock.eps) {
      why = "it ends on a face inside the stock";
      return false;
    }
    if (k < 0) {
      why = "a side of it lies on the stock";
      return false;
    }
    const double sign = m.Coord(k + 1), at = q.Coord(k + 1);
    if (tool.solid == "box") {
      tool.value(BOX_SIDES[k]) += delta;
      gp_Trsf shift;
      shift.SetTranslation(gp_Vec(n) * (delta / 2));
      tool.frame = shift * tool.frame;
    } else if (tool.solid == "xtru") {
      tool.value(sign > 0 ? "z2" : "z1") += sign * delta;
    } else {
      // Each end vertex moves along its side edge, extending that edge.
      std::vector<gp_Pnt2d> rz;
      for (const auto& [r, z] : meridian(tool)) rz.emplace_back(r, z);
      std::vector<gp_Pnt2d> moved = rz;
      for (size_t i = 0; i < rz.size(); ++i) {
        if (std::abs(rz[i].Y() - at) > stock.eps) continue;
        for (const size_t j : {(i + 1) % rz.size(), (i + rz.size() - 1) % rz.size()}) {
          const double dz = rz[i].Y() - rz[j].Y();
          if (std::abs(dz) <= stock.eps) continue;
          moved[i] = rz[i].Translated(gp_Vec2d(rz[j], rz[i]) * (delta / std::abs(dz)));
          if (moved[i].X() < 0) {
            why = "it narrows to a point beyond the stock";
            return false;
          }
        }
      }
      tool = polygon_solid(moved, axes(tool.frame), tool.value("deltaphi"), stock.eps);
    }
    return extend(tool, stock, delta, why);  // the moved end is off the stock now
  }
  return true;
}

/// The full solid of revolution about the largest cylinder or cone of `b`
/// that contains it, as a stock.
std::optional<Primitive> revolution_stock(const Body& b) {
  std::optional<gp_Ax1> axis;
  double largest = 0;
  for (const auto& [face, s] : b.faces) {
    if (s.GetType() != GeomAbs_Cylinder && s.GetType() != GeomAbs_Cone) continue;
    GProp_GProps g;
    BRepGProp::SurfaceProperties(face, g);
    if (g.Mass() > largest)
      largest = g.Mass(), axis = s.GetType() == GeomAbs_Cylinder ? s.Cylinder().Axis() : s.Cone().Axis();
  }
  if (!axis) return std::nullopt;
  // The meridian among eight that misses most cut-outs.
  const gp_Ax3 frame(foot(b.centre, *axis), axis->Direction());
  std::vector<Loop> best;
  double best_area = 0;
  for (int k = 0; k < 8; ++k) {
    const auto faces = section(b.shape, meridian_plane(frame, (k + 0.5) * TURN / 8), 2 * b.d, true);
    if (faces.size() != 1) continue;
    const double a = std::abs(area(corners(faces[0][0], b.eps)));
    if (a > best_area) best_area = a, best = {faces[0][0]};
  }
  std::string ignored;
  return best.empty() ? std::nullopt : revolved(best, frame, TURN, b.eps, ignored);
}

/// `b` as a stock minus at most MAX_TOOLS primitive cut-outs, or none with
/// the reason the first stock failed in `why`.
std::optional<Exact> csg(const Body& b, std::string& why) {
  why.clear();
  const auto fail = [&](const std::string& reason) {
    if (why.empty()) why = reason;
  };
  const double delta = std::max(1e-3 * b.d, 1e-3);
  // The stocks, built in turn: prisms along the three main plane directions,
  // then the solid of revolution.
  const std::vector<gp_Dir> dirs = plane_directions(b);
  const size_t prisms = std::min<size_t>(dirs.size(), 3);
  for (size_t k = 0; k <= prisms; ++k) {
    std::string ignored;
    const std::optional<Primitive> candidate = k < prisms ? prism_along(b, dirs[k], ignored) : revolution_stock(b);
    if (!candidate) continue;
    const Primitive& stock = *candidate;
    const Body block = body(shape(stock), b.eps);
    // A stock minus cut-outs lies within the stock: skip a stock it sticks out of.
    const gp_Ax3 frame = axes(stock.frame);
    const Bnd_Box in = bounds(b.shape, frame), out = bounds(block.shape, frame);
    const gp_XYZ low = in.CornerMin().XYZ() - out.CornerMin().XYZ(), high = out.CornerMax().XYZ() - in.CornerMax().XYZ();
    if (std::min({low.X(), low.Y(), low.Z(), high.X(), high.Y(), high.Z()}) < -b.eps) continue;
    BRepAlgoAPI_Cut cut;
    cut.SetArguments({block.shape});
    cut.SetTools({b.shape});
    cut.SetNonDestructive(true);
    cut.Build();
    if (cut.HasErrors()) continue;
    std::vector<TopoDS_Shape> cutouts;
    for (TopExp_Explorer s(cut.Shape(), TopAbs_SOLID); s.More(); s.Next()) {
      GProp_GProps volume, surface;
      BRepGProp::VolumeProperties(s.Current(), volume);
      BRepGProp::SurfaceProperties(s.Current(), surface);
      if (volume.Mass() > b.eps * surface.Mass()) cutouts.push_back(s.Current());  // not a sliver
    }
    if (cutouts.empty()) continue;
    if (cutouts.size() > MAX_TOOLS) {
      fail("a " + stock.solid + " minus " + std::to_string(cutouts.size()) + " cut-outs, more than " +
           std::to_string(MAX_TOOLS));
      continue;
    }
    Exact e{stock, {}, b.eps};
    for (size_t i = 0; i < cutouts.size(); ++i) {
      std::string reason;
      auto tool = primitive(body(cutouts[i], b.eps), reason);
      if (!tool || !extend(*tool, block, delta, reason)) {
        fail("cut-out " + std::to_string(i + 1) + " of a " + stock.solid + ": " + reason);
        break;
      }
      e.tools.push_back(*tool);
    }
    if (e.tools.size() != cutouts.size()) continue;
    const std::string failed = certify(b.shape, shape(e), b.eps);
    if (failed.empty()) return e;
    fail("a " + stock.solid + " minus " + std::to_string(e.tools.size()) + " cut-outs fails: " + failed);
  }
  fail("no stock to cut it from");
  return std::nullopt;
}

}  // namespace

std::optional<Exact> recognize(const TopoDS_Shape& solid, std::string& why) {
  try {
    OCC_CATCH_SIGNALS
    if (const long free = free_form(solid); free > 0) {
      why = std::to_string(free) + " of its faces are free-form";
      return std::nullopt;
    }
    const double eps = std::max(ShapeAnalysis_ShapeTolerance().Tolerance(solid, 1, TopAbs_FACE), MIN_EPS);
    const Body b = body(solid, eps);
    if (eps > MAX_RELATIVE_EPS * b.d) {
      why = "its tolerance is too large for its size";
      return std::nullopt;
    }
    std::string one, cut;
    if (auto p = primitive(b, one)) return Exact{*p, {}, eps};
    if (auto e = csg(b, cut)) return e;
    why = one + "; " + cut;
  } catch (const Standard_Failure& e) {
    why = std::string("OpenCASCADE failed on it: ") + e.what();
  }
  return std::nullopt;
}

}  // namespace worker
