// Source interference (algorithm I): placed solids whose interiors overlap,
// and by how much. Geant4 reports an overlap wherever they do.
#include "worker.hpp"

#include <BRepAlgoAPI_Common.hxx>
#include <BRepBndLib.hxx>
#include <BRepGProp.hxx>
#include <Bnd_Box.hxx>
#include <GProp_GProps.hxx>
#include <Standard_ErrorHandler.hxx>
#include <Standard_Failure.hxx>

#include <algorithm>
#include <atomic>
#include <chrono>
#include <mutex>
#include <thread>

namespace worker {
namespace {

/// Pairs not started when this time is up are counted as not checked.
constexpr double BUDGET_SECONDS = 60;

/// Pairs are checked concurrently; each Boolean can take a few hundred MB.
constexpr unsigned MAX_THREADS = 8;

/// Overlaps up to this fraction of the smaller volume are contact, not
/// interference: the Boolean of two touching solids leaves slivers.
constexpr double CONTACT_FRACTION = 1e-6;

struct Placed {
  std::vector<int> path;
  TopoDS_Shape shape;
  Bnd_Box box;
  double volume_mm3;
};

/// Every solid under label `entry`, which `at` places, in the world.
void place(const Document& doc, const std::string& entry, const TopLoc_Location& at, std::vector<int>& path,
           std::vector<Placed>& out) {
  auto l = std::find_if(doc.labels.begin(), doc.labels.end(), [&](const Label& l) { return l.entry == entry; });
  if (l == doc.labels.end()) return;
  for (size_t i = 0; i < l->components.size(); ++i) {
    path.push_back(int(i));
    place(doc, l->components[i].target, at * l->components[i].location, path, out);
    path.pop_back();
  }
  for (size_t s = 0; s < l->shapes.size(); ++s) {
    const std::optional<Solid>& solid = l->solids[s];
    if (!solid || solid->mesh.triangles.empty()) continue;
    Placed p{path, l->shapes[s].Moved(at), {}, std::abs(solid->volume_mm3)};
    p.path.push_back(int(s));
    BRepBndLib::Add(p.shape, p.box);
    p.box.Enlarge(solid->tolerance_mm);
    out.push_back(std::move(p));
  }
}

/// The volume common to `a` and `b`, or none when OpenCASCADE fails on them.
/// The Boolean leaves its arguments unchanged, so threads can share them.
std::optional<double> common_volume(const Placed& a, const Placed& b, bool parallel) {
  try {
    OCC_CATCH_SIGNALS
    BRepAlgoAPI_Common common;
    common.SetArguments({a.shape});
    common.SetTools({b.shape});
    common.SetNonDestructive(true);
    common.SetRunParallel(parallel);
    common.Build();
    if (common.HasErrors()) return std::nullopt;
    GProp_GProps overlap;
    BRepGProp::VolumeProperties(common.Shape(), overlap);
    return overlap.Mass();
  } catch (const Standard_Failure&) {
    return std::nullopt;
  }
}

}  // namespace

void interfere(Document& doc) {
  std::vector<Placed> placed;
  for (size_t r = 0; r < doc.roots.size(); ++r) {
    std::vector<int> path{int(r)};
    place(doc, doc.roots[r], TopLoc_Location(), path, placed);
  }
  std::vector<std::pair<size_t, size_t>> pairs;
  for (size_t i = 0; i < placed.size(); ++i)
    for (size_t j = i + 1; j < placed.size(); ++j)
      if (!placed[i].box.IsOut(placed[j].box)) pairs.emplace_back(i, j);

  const unsigned threads = std::clamp(std::thread::hardware_concurrency(), 1u, MAX_THREADS);
  const auto deadline = std::chrono::steady_clock::now() + std::chrono::duration<double>(BUDGET_SECONDS);
  std::vector<std::optional<double>> volume(pairs.size());
  std::atomic<size_t> next{0};
  std::mutex lock;
  size_t done = 0;
  progress("interfere", 0, pairs.size());
  std::vector<std::thread> pool;
  for (unsigned t = 0; t < threads; ++t)
    pool.emplace_back([&] {
      for (size_t k; (k = next++) < pairs.size() && std::chrono::steady_clock::now() < deadline;) {
        // A Boolean uses all threads itself only when there are fewer pairs.
        volume[k] = common_volume(placed[pairs[k].first], placed[pairs[k].second], pairs.size() < threads);
        std::lock_guard<std::mutex> hold(lock);
        progress("interfere", ++done, pairs.size());
      }
    });
  for (std::thread& t : pool) t.join();

  for (size_t k = 0; k < pairs.size(); ++k) {
    const Placed &a = placed[pairs[k].first], &b = placed[pairs[k].second];
    if (!volume[k]) ++doc.unchecked;
    else if (*volume[k] > CONTACT_FRACTION * std::min(a.volume_mm3, b.volume_mm3))
      doc.interferences.push_back({a.path, b.path, *volume[k]});
  }
}

}  // namespace worker
