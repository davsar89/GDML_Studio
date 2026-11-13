// Loads a GDML file in Geant4 and reports what Geant4 itself makes of it:
// solid volumes and bounds, overlaps of every placement, placement matrices,
// and geantinos shot through the world to catch navigation problems.
// One JSON object per line, prefixed by its kind. Exit status 1 when Geant4
// warns, fails, finds an overlap or a track gets stuck.
//
// Usage: g4check file.gdml [events=2000] [overlap_points=10000]
// Adapted from g4_check.cpp in the CAD import investigation evidence.
#include <G4Box.hh>
#include <G4BooleanSolid.hh>
#include <G4Event.hh>
#include <G4GDMLParser.hh>
#include <G4Geantino.hh>
#include <G4LogicalVolume.hh>
#include <G4ParticleGun.hh>
#include <G4PhysicalVolumeStore.hh>
#include <G4RunManager.hh>
#include <G4SolidStore.hh>
#include <G4StateManager.hh>
#include <G4Step.hh>
#include <G4SystemOfUnits.hh>
#include <G4TessellatedSolid.hh>
#include <G4Track.hh>
#include <G4UserSteppingAction.hh>
#include <G4VExceptionHandler.hh>
#include <G4VSolid.hh>
#include <G4VUserDetectorConstruction.hh>
#include <G4VUserPhysicsList.hh>
#include <G4VUserPrimaryGeneratorAction.hh>
#include <G4Version.hh>
#include <Randomize.hh>

#include <chrono>
#include <iomanip>
#include <iostream>
#include <string>

namespace {

double seconds_since(std::chrono::steady_clock::time_point t0) {
  return std::chrono::duration<double>(std::chrono::steady_clock::now() - t0).count();
}

struct CountingHandler : G4VExceptionHandler {
  int warnings = 0, errors = 0;
  G4bool Notify(const char* origin, const char* code, G4ExceptionSeverity severity,
                const char* description) override {
    if (severity == JustWarning) ++warnings; else ++errors;
    std::cerr << "G4_DIAGNOSTIC " << origin << " " << code << " " << description << "\n";
    return severity != JustWarning;  // abort on anything worse than a warning
  }
};

struct Detector : G4VUserDetectorConstruction {
  G4VPhysicalVolume* world;
  explicit Detector(G4VPhysicalVolume* w) : world(w) {}
  G4VPhysicalVolume* Construct() override { return world; }
};

struct TransportOnly : G4VUserPhysicsList {
  void ConstructParticle() override { G4Geantino::GeantinoDefinition(); }
  void ConstructProcess() override { AddTransportation(); }
  void SetCuts() override { SetCutsWithDefault(); }
};

// Geantinos start just inside a face of the world box and aim at random
// points of its central region, cycling through the six faces.
struct Gun : G4VUserPrimaryGeneratorAction {
  G4ParticleGun gun{1};
  G4ThreeVector lo, hi;
  Gun(G4ThreeVector l, G4ThreeVector h) : lo(l), hi(h) {
    gun.SetParticleDefinition(G4Geantino::Definition());
    gun.SetParticleEnergy(1 * GeV);
  }
  void GeneratePrimaries(G4Event* event) override {
    const int id = event->GetEventID();
    const int axis = id % 3;
    const G4ThreeVector centre = 0.5 * (lo + hi), half = 0.5 * (hi - lo);
    G4ThreeVector pos = centre, target;
    for (int i = 0; i < 3; ++i) target[i] = centre[i] + (2 * G4UniformRand() - 1) * 0.65 * half[i];
    pos[axis] = centre[axis] + ((id / 3) % 2 ? 1 : -1) * 0.95 * half[axis];
    gun.SetParticlePosition(pos);
    gun.SetParticleMomentumDirection((target - pos).unit());
    gun.GeneratePrimaryVertex(event);
  }
};

// A track is stuck when it makes more than 10 consecutive zero-length steps
// or more than 10000 steps in total.
struct StuckCounter : G4UserSteppingAction {
  long steps = 0, stuck = 0, zero_run = 0;
  void UserSteppingAction(const G4Step* step) override {
    ++steps;
    G4Track* track = step->GetTrack();
    zero_run = step->GetStepLength() < 1e-12 ? zero_run + 1 : 0;
    if (zero_run > 10 || track->GetCurrentStepNumber() > 10000) {
      ++stuck;
      zero_run = 0;
      track->SetTrackStatus(fStopAndKill);
    }
  }
};

}  // namespace

int main(int argc, char** argv) {
  if (argc < 2) {
    std::cerr << "usage: g4check file.gdml [events=2000] [overlap_points=10000]\n";
    return 2;
  }
  const int events = argc > 2 ? std::stoi(argv[2]) : 2000;
  const int overlap_points = argc > 3 ? std::stoi(argv[3]) : 10000;
  CLHEP::HepRandom::setTheSeed(17331);
  CountingHandler handler;
  G4StateManager::GetStateManager()->SetExceptionHandler(&handler);
  std::cout << std::setprecision(17);

  auto t0 = std::chrono::steady_clock::now();
  G4GDMLParser parser;
  parser.SetOverlapCheck(false);
  parser.Read(argv[1], false);  // no schema validation: it would fetch the XSD
  G4VPhysicalVolume* world = parser.GetWorldVolume();
  const double load_s = seconds_since(t0);
  std::cout << "G4_VERSION " << G4VERSION_NUMBER << "\n";

  for (G4VSolid* s : *G4SolidStore::GetInstance()) {
    if (auto* b = dynamic_cast<G4BooleanSolid*>(s)) {
      b->SetCubVolStatistics(1000000);
      b->SetCubVolEpsilon(0.0001);
    }
    G4ThreeVector lo, hi;
    s->BoundingLimits(lo, hi);
    std::cout << "G4_SOLID {\"name\":\"" << s->GetName() << "\",\"type\":\"" << s->GetEntityType()
              << "\",\"volume_mm3\":" << s->GetCubicVolume() << ",\"bbox_mm\":[" << lo.x() << ","
              << lo.y() << "," << lo.z() << "," << hi.x() << "," << hi.y() << "," << hi.z() << "]";
    if (auto* t = dynamic_cast<G4TessellatedSolid*>(s))
      std::cout << ",\"facets\":" << t->GetNumberOfFacets();
    std::cout << "}\n";
  }

  int overlaps = 0, placements = 0;
  auto t1 = std::chrono::steady_clock::now();
  for (G4VPhysicalVolume* p : *G4PhysicalVolumeStore::GetInstance()) {
    if (p == world) continue;
    ++placements;
    const bool overlap = p->CheckOverlaps(overlap_points, 0.0, false, 10);
    if (overlap) ++overlaps;
    const G4RotationMatrix r = p->GetObjectRotationValue();
    const G4ThreeVector v = p->GetObjectTranslation();
    std::cout << "G4_PLACEMENT {\"name\":\"" << p->GetName() << "\",\"volume\":\""
              << p->GetLogicalVolume()->GetName() << "\",\"overlap\":" << (overlap ? "true" : "false")
              << ",\"translation_mm\":[" << v.x() << "," << v.y() << "," << v.z() << "],\"rotation\":[";
    for (int i = 0; i < 3; ++i)
      for (int j = 0; j < 3; ++j) std::cout << (i || j ? "," : "") << r(i, j);
    std::cout << "]}\n";
  }
  const double overlap_s = seconds_since(t1);

  G4ThreeVector lo, hi;
  world->GetLogicalVolume()->GetSolid()->BoundingLimits(lo, hi);
  auto* run_manager = new G4RunManager;
  run_manager->SetUserInitialization(new Detector(world));
  run_manager->SetUserInitialization(new TransportOnly);
  run_manager->SetUserAction(new Gun(lo, hi));
  auto* stepping = new StuckCounter;
  run_manager->SetUserAction(stepping);
  run_manager->Initialize();
  auto t2 = std::chrono::steady_clock::now();
  run_manager->BeamOn(events);
  const double navigation_s = seconds_since(t2);

  std::cout << "G4_RESULT {\"version\":" << G4VERSION_NUMBER << ",\"load_s\":" << load_s
            << ",\"overlap_s\":" << overlap_s << ",\"navigation_s\":" << navigation_s
            << ",\"events\":" << events << ",\"placements\":" << placements
            << ",\"overlaps\":" << overlaps << ",\"steps\":" << stepping->steps
            << ",\"stuck\":" << stepping->stuck << ",\"warnings\":" << handler.warnings
            << ",\"errors\":" << handler.errors << "}\n";
  return overlaps || stepping->stuck || handler.warnings || handler.errors ? 1 : 0;
}
