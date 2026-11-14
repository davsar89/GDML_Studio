// certify() accepts a solid's own GDML description and rejects near misses
// (doc/cad-import/PLAN.md, algorithm E); recognize() reads sectors and
// shells. Run by ctest.
#include "../src/worker.hpp"

#include <gp_Ax1.hxx>

#include <cmath>

using namespace worker;

namespace {

int failures = 0;

void check(bool ok, const std::string& what) {
  std::printf("%s %s\n", ok ? "ok  " : "FAIL", what.c_str());
  failures += !ok;
}

/// Whether certify() accepts `candidate` for `source` as it should.
void certifies(const TopoDS_Shape& source, const Exact& candidate, bool expected, const std::string& what,
               double eps = 1e-7) {
  const std::string why = certify(source, shape(candidate), eps);
  check(why.empty() == expected, what + (why.empty() ? "" : ": " + why));
}

gp_Trsf moved(double x, double y, double z, double turn = 0) {
  gp_Trsf t;
  t.SetRotation(gp_Ax1(gp::Origin(), gp::DZ()), turn);
  t.SetTranslationPart(gp_Vec(x, y, z));
  return t;
}

Primitive box(double x, double y, double z, gp_Trsf at = {}) {
  return {"box", {{"x", x}, {"y", y}, {"z", z}}, {}, at};
}

Primitive tube(double rmin, double rmax, double z, double dphi, gp_Trsf at = {}) {
  return {"tube", {{"rmin", rmin}, {"rmax", rmax}, {"z", z}, {"deltaphi", dphi}}, {}, at};
}

Primitive sphere(double rmin, double rmax) {
  return {"sphere", {{"rmin", rmin}, {"rmax", rmax}, {"deltaphi", 2 * M_PI}, {"starttheta", 0}, {"deltatheta", M_PI}},
          {}, {}};
}

Primitive l_shape(double corner) {
  return {"xtru", {{"z1", -5}, {"z2", 5}}, {{0, 0}, {0, 20}, {corner, 20}, {corner, corner}, {20, corner}, {20, 0}}, {}};
}

}  // namespace

int main() {
  const TopoDS_Shape block = shape(box(10, 20, 30));
  certifies(block, {box(10, 20, 30), {}, 0}, true, "a box is its box");
  certifies(block, {box(10, 20, 30.001, moved(0, 0, 0.0005)), {}, 0}, false, "a face 1e-3 mm off");
  certifies(shape(Exact{box(10, 20, 30), {box(2, 2, 2)}, 0}), {box(10, 20, 30), {}, 0}, false,
            "a box with a cavity against the plain box");
  certifies(shape(l_shape(5)), {l_shape(5), {}, 0}, true, "an L is its xtru");
  certifies(shape(l_shape(5)), {l_shape(5.001), {}, 0}, false, "an xtru vertex 1e-3 mm off");

  // A half tube shares every surface with the other half; only sides differ.
  const TopoDS_Shape half = shape(tube(4, 10, 12, M_PI));
  certifies(half, {tube(4, 10, 12, M_PI, moved(0, 0, 0, M_PI)), {}, 0}, false, "the other half of a tube");
  std::string why;
  const auto read = recognize(half, why);
  check(read && read->stock.solid == "tube" && std::abs(read->stock.value("deltaphi") - M_PI) < 1e-12,
        "a half tube reads as a tube with deltaphi pi " + why);

  const TopoDS_Shape shell = shape(sphere(5, 10));
  certifies(shell, {sphere(0, 10), {}, 0}, false, "a spherical shell against the ball");
  const auto hollow = recognize(shell, why);
  check(hollow && hollow->stock.solid == "sphere" && std::abs(hollow->stock.value("rmin") - 5) < 1e-12,
        "a spherical shell reads as a sphere with rmin 5 " + why);
  // Eight cubes out of a slab, on the diagonals or off them: every surface
  // carries the same area and moment either way, but the solids differ.
  std::vector<Primitive> diagonal, off;
  for (const double x : {-6, -2, 2, 6})
    for (const double y : {-6, -2, 2, 6}) (std::abs(x) == std::abs(y) ? diagonal : off).push_back(box(1, 1, 1, moved(x, y, 0)));
  certifies(shape(Exact{box(20, 20, 10), diagonal, 0}), {box(20, 20, 10), off, 0}, false,
            "cut-outs swapped between symmetric places");

  // Axis and radius each within eps, together 1.5 eps off.
  const TopoDS_Shape rod = shape(tube(0, 10, 20, 2 * M_PI));
  certifies(rod, {tube(0, 10 + 0.75e-5, 20, 2 * M_PI, moved(0.75e-5, 0, 0)), {}, 0}, false,
            "a rod 0.75 eps wider and 0.75 eps off its axis", 1e-5);
  certifies(rod, {tube(0, 10 + 0.1e-5, 20, 2 * M_PI, moved(0.1e-5, 0, 0)), {}, 0}, true,
            "a rod 0.1 eps wider and 0.1 eps off its axis", 1e-5);
  // Geant4 refuses a genericPolycone meridian of less than 1e-9 mm2.
  const Primitive ring{"genericPolycone", {{"deltaphi", 2 * M_PI}}, {{10, 0}, {10.00002, 0}, {10, 0.00002}}, {}};
  const bool thin = !recognize(shape(ring), why) && why.find("too thin") != std::string::npos;
  check(thin, "a ring of 2e-10 mm2 section stays tessellated: " + why);

  // A pocket open through a side of the stock: its xtru side would lie on the stock's face.
  const Primitive pocket{"xtru", {{"z1", 1}, {"z2", 5}}, {{10, -5}, {15, -5}, {15, 10}, {20, 10}, {20, -10}, {10, -10}}, {}};
  const bool open = !recognize(shape(Exact{box(40, 30, 10), {pocket}, 0}), why) && why.find("side of it") != std::string::npos;
  check(open, "a pocket open through a side stays tessellated: " + why);
  return failures ? 1 : 0;
}
