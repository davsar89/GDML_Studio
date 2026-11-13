// STEP reading and XCAF traversal.
#include "worker.hpp"

#include <BRep_Builder.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRep_Tool.hxx>
#include <NCollection_Sequence.hxx>
#include <STEPCAFControl_ExternFile.hxx>
#include <STEPCAFControl_Reader.hxx>
#include <ShapeAnalysis_ShapeTolerance.hxx>
#include <ShapeFix_Solid.hxx>
#include <TCollection_AsciiString.hxx>
#include <TDF_Tool.hxx>
#include <TDataStd_Name.hxx>
#include <TDocStd_Document.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Shell.hxx>
#include <UnitsMethods_LengthUnit.hxx>
#include <XCAFDoc_DocumentTool.hxx>
#include <XCAFDoc_ShapeTool.hxx>
#include <gp_Trsf.hxx>

#include <algorithm>
#include <set>
#include <sstream>
#include <type_traits>

namespace worker {
namespace {

std::string entry(const TDF_Label& label) {
  TCollection_AsciiString text;
  TDF_Tool::Entry(label, text);
  return text.ToCString();
}

/// The label's name in UTF-8, or "" when it has none.
std::string name(const TDF_Label& label) {
  Handle(TDataStd_Name) attribute;
  if (!label.FindAttribute(TDataStd_Name::GetID(), attribute)) return {};
  return TCollection_AsciiString(attribute->Get()).ToCString();
}

std::array<double, 12> matrix(const TopLoc_Location& location) {
  const gp_Trsf t = location.Transformation();
  std::array<double, 12> m{};
  for (int i = 1; i <= 3; ++i)
    for (int j = 1; j <= 4; ++j) m[(i - 1) * 4 + (j - 1)] = t.Value(i, j);
  return m;
}

/// A part's solids, and the faces outside any solid sewn within their
/// tolerance: each closed shell becomes a solid, the rest is reported.
void part(const TopoDS_Shape& shape, Label& l) {
  for (TopExp_Explorer ex(shape, TopAbs_SOLID); ex.More(); ex.Next()) l.shapes.push_back(ex.Current());
  TopoDS_Compound loose;
  BRep_Builder().MakeCompound(loose);
  int faces = 0;
  for (TopExp_Explorer ex(shape, TopAbs_FACE, TopAbs_SOLID); ex.More(); ex.Next(), ++faces)
    BRep_Builder().Add(loose, ex.Current());
  if (faces == 0) {
    if (l.shapes.empty())
      l.issues.push_back(TopExp_Explorer(shape, TopAbs_EDGE).More() ? "it holds only curves, no solid"
                                                                     : "it holds no geometry");
    return;
  }
  BRepBuilderAPI_Sewing sewing(std::max(ShapeAnalysis_ShapeTolerance().Tolerance(loose, 1), 1e-7));
  sewing.Add(loose);
  sewing.Perform();
  int open = 0;
  for (TopExp_Explorer ex(sewing.SewedShape(), TopAbs_FACE); ex.More(); ex.Next()) ++open;
  for (TopExp_Explorer ex(sewing.SewedShape(), TopAbs_SHELL); ex.More(); ex.Next()) {
    const TopoDS_Shell& shell = TopoDS::Shell(ex.Current());
    if (!BRep_Tool::IsClosed(shell)) continue;
    ShapeFix_Solid fix;  // orients the shell outward
    fix.Init(BRepBuilderAPI_MakeSolid(shell).Solid());
    fix.Perform();
    l.shapes.push_back(fix.Solid());
    for (TopExp_Explorer f(shell, TopAbs_FACE); f.More(); f.Next()) --open;
  }
  if (open > 0)
    l.issues.push_back(std::to_string(open) + " of its " + std::to_string(faces) +
                       " faces outside any solid form no closed surface");
}

}  // namespace

Document read_step(const std::string& bytes) {
  Handle(TDocStd_Document) doc = new TDocStd_Document("BinXCAF");
  XCAFDoc_DocumentTool::SetLengthUnit(doc, 1.0, UnitsMethods_LengthUnit_Millimeter);
  STEPCAFControl_Reader reader;
  reader.SetNameMode(true);
  reader.SetColorMode(false);
  reader.SetLayerMode(false);
  reader.SetPropsMode(false);
  reader.SetGDTMode(false);
  reader.SetMatMode(false);
  reader.SetViewMode(false);
  std::istringstream stream(bytes);
  progress("read", 0, 1);
  if (reader.ReadStream("input.step", stream) != IFSelect_RetDone)
    throw Unreadable("it is not a STEP file OpenCASCADE can read");
  if (!reader.Transfer(doc)) throw Unreadable("no shape in it could be transferred");

  Document out;
  // The file came without its directory, so a part it keeps in another file
  // is missing unless OpenCASCADE happened to find one by that name.
  const auto& files = reader.ExternFiles();
  for (std::decay_t<decltype(files)>::Iterator it(files); it.More(); it.Next())
    if (it.Value()->GetLoadStatus() != IFSelect_RetDone)
      out.issues.push_back(std::string("the parts it keeps in the file '") + it.Key().ToCString() +
                           "' are missing");

  Handle(XCAFDoc_ShapeTool) tool = XCAFDoc_DocumentTool::ShapeTool(doc->Main());
  NCollection_Sequence<TDF_Label> roots;
  tool->GetFreeShapes(roots);
  std::set<std::string> seen;
  // Every label reachable from the free shapes, once.
  std::vector<TDF_Label> pending;
  for (int i = 1; i <= roots.Length(); ++i) {
    out.roots.push_back(entry(roots(i)));
    pending.push_back(roots(i));
  }
  while (!pending.empty()) {
    const TDF_Label label = pending.back();
    pending.pop_back();
    Label l;
    l.entry = entry(label);
    if (!seen.insert(l.entry).second) continue;
    l.name = name(label);
    if (XCAFDoc_ShapeTool::IsAssembly(label)) {
      l.assembly = true;
      NCollection_Sequence<TDF_Label> components;
      XCAFDoc_ShapeTool::GetComponents(label, components);
      for (int i = 1; i <= components.Length(); ++i) {
        TDF_Label target;
        if (!XCAFDoc_ShapeTool::GetReferredShape(components(i), target)) {
          l.issues.push_back("component " + entry(components(i)) + " refers to no shape");
          continue;
        }
        const TopLoc_Location location = XCAFDoc_ShapeTool::GetLocation(components(i));
        l.components.push_back({entry(target), matrix(location), location});
        pending.push_back(target);
      }
    } else {
      part(XCAFDoc_ShapeTool::GetShape(label), l);
    }
    out.labels.push_back(std::move(l));
  }
  return out;
}

}  // namespace worker
