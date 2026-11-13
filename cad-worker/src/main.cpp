// gdml-cad-worker [--volume-target X] [--chord <solid id>=<mm>]... [--only] < file.step > result.json
//
// Reads a STEP file on stdin and writes the JSON of output.cpp on stdout; no
// file path crosses the process boundary. Solid ids are "<label entry>.<solid
// index>"; `--chord` fixes the chordal deviation of one, and `--only` meshes
// just those, skipping the others and the interference check. Progress and a
// failure go to stderr, as `progress <phase> <done> <total>` and
// `error <message>` lines among whatever OpenCASCADE prints. Exit codes: 0
// done (per-solid failures are in the result), 2 bad arguments, 3 unreadable
// file, 4 internal error.
#include "worker.hpp"

#include <Message.hxx>
#include <Message_Messenger.hxx>
#include <Message_PrinterOStream.hxx>
#include <OSD.hxx>
#include <Standard_Failure.hxx>

#include <cstdlib>
#include <cstring>
#include <iostream>
#include <iterator>

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#define dup _dup
#define dup2 _dup2
#define fdopen _fdopen
#define fileno _fileno
#else
#include <unistd.h>
#endif

namespace worker {
void progress(const char* phase, size_t done, size_t total) {
  std::fprintf(stderr, "progress %s %zu %zu\n", phase, done, total);
  std::fflush(stderr);
}
}  // namespace worker

static int fail(int code, const std::string& message) {
  std::fprintf(stderr, "error %s\n", message.c_str());
  return code;
}

/// A positive number, or 0 when `text` is not one.
static double positive(const char* text) {
  char* end = nullptr;
  const double x = std::strtod(text, &end);
  return *text != '\0' && *end == '\0' && x > 0 && x < 1e300 ? x : 0;
}

int main(int argc, char** argv) {
  worker::Options options;
  for (int i = 1; i < argc; ++i) {
    const char* value = i + 1 < argc ? argv[i + 1] : "";
    if (!std::strcmp(argv[i], "--only")) {
      options.only = true;
    } else if (!std::strcmp(argv[i], "--volume-target") && (options.volume_target = positive(value)) > 0) {
      ++i;
    } else if (const char* eq = std::strchr(value, '='); !std::strcmp(argv[i], "--chord") && eq &&
                                                          positive(eq + 1) > 0) {
      options.chord_for[std::string(value, eq)] = positive(eq + 1);
      ++i;
    } else {
      return fail(2, std::string("bad argument ") + argv[i]);
    }
  }
#ifdef _WIN32
  _setmode(fileno(stdin), _O_BINARY);
  _setmode(fileno(stdout), _O_BINARY);
#endif
  // The result goes to the original stdout; anything else that writes to
  // stdout, OpenCASCADE's messages included, goes to stderr instead.
  std::fflush(stdout);
  std::FILE* out = fdopen(dup(fileno(stdout)), "wb");
  if (out == nullptr || dup2(fileno(stderr), fileno(stdout)) < 0) return fail(4, "cannot set up the output");
  Message::DefaultMessenger()->RemovePrinters(STANDARD_TYPE(Message_PrinterOStream));
  // Faults inside OpenCASCADE become exceptions, caught per solid.
  OSD::SetSignal(false);

  try {
    std::ios::sync_with_stdio(false);
    const std::string bytes{std::istreambuf_iterator<char>(std::cin), std::istreambuf_iterator<char>()};
    worker::Document doc = worker::read_step(bytes);
    size_t total = 0, done = 0;
    for (const auto& label : doc.labels) total += label.shapes.size();
    for (auto& label : doc.labels)
      for (size_t s = 0; s < label.shapes.size(); ++s) {
        const auto chord = options.chord_for.find(label.entry + "." + std::to_string(s));
        worker::progress("mesh", done++, total);
        if (options.only && chord == options.chord_for.end())
          label.solids.emplace_back();
        else
          label.solids.push_back(worker::analyze(label.shapes[s], options.volume_target,
                                                 chord == options.chord_for.end() ? 0 : chord->second));
      }
    if (!options.only) worker::interfere(doc);
    worker::write(out, doc);
    return 0;
  } catch (const worker::Unreadable& e) {
    return fail(3, e.what());
  } catch (const Standard_Failure& e) {
    return fail(4, std::string("OpenCASCADE: ") + e.what());
  } catch (const std::exception& e) {
    return fail(4, e.what());
  }
}
