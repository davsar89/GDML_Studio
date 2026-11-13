// The result as JSON. Its fields are owned by the backend's serde structs
// (backend/src/cad/worker.rs).
#include "worker.hpp"

#include <cmath>
#include <type_traits>

namespace worker {
namespace {

// Declared first so that the templates below find them.
void put(std::string& o, const Component& c);
void put(std::string& o, const Mesh& m);
void put(std::string& o, const Solid& s);
void put(std::string& o, const std::optional<Solid>& s);
void put(std::string& o, const Label& l);
void put(std::string& o, const Interference& i);

void put(std::string& o, bool b) { o += b ? "true" : "false"; }

template <class T, std::enable_if_t<std::is_arithmetic<T>::value, int> = 0>
void put(std::string& o, T x) {
  if constexpr (std::is_integral<T>::value) {
    o += std::to_string(x);
  } else if (!std::isfinite(x)) {
    o += "null";
  } else {
    char text[32];  // %.17g reads back as the same double (C locale)
    std::snprintf(text, sizeof text, "%.17g", x);
    o += text;
  }
}

void put(std::string& o, const std::string& s) {
  o += '"';
  for (unsigned char c : s) {
    char escape[8];
    if (c == '"' || c == '\\') (o += '\\') += char(c);
    else if (c < 0x20) std::snprintf(escape, sizeof escape, "\\u%04x", c), o += escape;
    else o += char(c);  // UTF-8 passes through
  }
  o += '"';
}

template <class Items>
auto put(std::string& o, const Items& items) -> decltype(items.begin(), void()) {
  o += '[';
  for (const auto& item : items) {
    if (o.back() != '[') o += ',';
    put(o, item);
  }
  o += ']';
}

void fields(std::string&) {}

template <class T, class... Rest>
void fields(std::string& o, const char* key, const T& value, const Rest&... rest) {
  if (o.back() != '{') o += ',';
  ((o += '"') += key) += "\":";
  put(o, value);
  fields(o, rest...);
}

template <class... Fields>
void object(std::string& o, const Fields&... f) {
  o += '{';
  fields(o, f...);
  o += '}';
}

void put(std::string& o, const Component& c) {
  object(o, "target", c.target, "matrix", c.matrix);
}

void put(std::string& o, const Mesh& m) {
  object(o, "positions", m.positions, "triangles", m.triangles, "chord_mm", m.chord_mm, "issues", m.issues);
}

void put(std::string& o, const Solid& s) {
  object(o, "valid", s.valid, "tolerance_mm", s.tolerance_mm, "volume_mm3", s.volume_mm3,
         "area_mm2", s.area_mm2, "bounds_mm", s.bounds_mm, "mesh", s.mesh, "issues", s.issues);
}

void put(std::string& o, const std::optional<Solid>& s) {
  if (s) put(o, *s);
  else o += "null";
}

void put(std::string& o, const Label& l) {
  object(o, "entry", l.entry, "name", l.name, "assembly", l.assembly, "components", l.components,
         "solids", l.solids, "issues", l.issues);
}

void put(std::string& o, const Interference& i) {
  object(o, "a", i.a, "b", i.b, "volume_mm3", i.volume_mm3);
}

}  // namespace

void write(std::FILE* out, const Document& doc) {
  std::string json;
  object(json, "roots", doc.roots, "labels", doc.labels, "interferences", doc.interferences, "unchecked",
         doc.unchecked, "issues", doc.issues);
  std::fwrite(json.data(), 1, json.size(), out);
  std::fflush(out);
}

}  // namespace worker
