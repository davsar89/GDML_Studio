use anyhow::Result;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use std::io::Cursor;
use std::sync::LazyLock;

use super::model::*;

// ─── NIST Material Database ─────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NistComponent {
    #[serde(rename = "type")]
    pub comp_type: String,
    #[serde(rename = "ref")]
    pub ref_name: String,
    pub n: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NistMaterial {
    pub name: String,
    pub density: f64,
    pub state: String,
    pub category: String,
    pub z: Option<u32>,
    pub atom_value: Option<f64>,
    pub formula: Option<String>,
    pub components: Vec<NistComponent>,
}

static NIST_MATERIALS: LazyLock<Vec<NistMaterial>> = LazyLock::new(|| {
    let data = include_str!("../../data/nist_materials.json");
    serde_json::from_str(data).expect("Failed to parse nist_materials.json")
});

pub fn find_nist_material(name: &str) -> Option<&'static NistMaterial> {
    NIST_MATERIALS.iter().find(|m| m.name == name)
}

pub fn search_nist_materials(query: &str, category: Option<&str>) -> Vec<&'static NistMaterial> {
    let q = query.to_uppercase();
    NIST_MATERIALS
        .iter()
        .filter(|m| {
            if let Some(cat) = category {
                if m.category != cat {
                    return false;
                }
            }
            if q.is_empty() {
                return true;
            }
            let name_match = m.name.to_uppercase().contains(&q);
            let formula_match = m
                .formula
                .as_ref()
                .map(|f| f.to_uppercase().contains(&q))
                .unwrap_or(false);
            name_match || formula_match
        })
        .collect()
}

// NIST_MATERIALS data is loaded from ../../data/nist_materials.json via LazyLock above.
// The old static array has been replaced. Marker for next section:
// G4_H (see nist_materials.json)

// ─── GDML Serializer ────────────────────────────────────────────────────────

pub fn serialize_gdml(doc: &GdmlDocument) -> Result<String> {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    // XML declaration
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

    // <gdml> root with namespace
    let mut gdml = BytesStart::new("gdml");
    gdml.push_attribute(("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"));
    gdml.push_attribute((
        "xsi:noNamespaceSchemaLocation",
        "http://service-spi.web.cern.ch/service-spi/app/releases/GDML/schema/gdml.xsd",
    ));
    writer.write_event(Event::Start(gdml))?;

    write_defines(&mut writer, &doc.defines)?;
    write_materials(&mut writer, &doc.materials)?;
    write_solids(&mut writer, &doc.solids)?;
    write_structure(&mut writer, &doc.structure)?;
    write_setup(&mut writer, &doc.setup)?;

    // </gdml>
    writer.write_event(Event::End(BytesEnd::new("gdml")))?;

    let result = writer.into_inner().into_inner();
    Ok(String::from_utf8(result)?)
}

fn write_defines(writer: &mut Writer<Cursor<Vec<u8>>>, defines: &DefineSection) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("define")))?;

    for c in &defines.constants {
        let mut elem = BytesStart::new("constant");
        elem.push_attribute(("name", c.name.as_str()));
        elem.push_attribute(("value", c.value.as_str()));
        writer.write_event(Event::Empty(elem))?;
    }

    for q in &defines.quantities {
        let mut elem = BytesStart::new("quantity");
        elem.push_attribute(("name", q.name.as_str()));
        if let Some(ref t) = q.r#type {
            elem.push_attribute(("type", t.as_str()));
        }
        elem.push_attribute(("value", q.value.as_str()));
        if let Some(ref u) = q.unit {
            elem.push_attribute(("unit", u.as_str()));
        }
        writer.write_event(Event::Empty(elem))?;
    }

    for v in &defines.variables {
        let mut elem = BytesStart::new("variable");
        elem.push_attribute(("name", v.name.as_str()));
        elem.push_attribute(("value", v.value.as_str()));
        writer.write_event(Event::Empty(elem))?;
    }

    for e in &defines.expressions {
        let mut elem = BytesStart::new("expression");
        elem.push_attribute(("name", e.name.as_str()));
        writer.write_event(Event::Start(elem))?;
        writer.write_event(Event::Text(BytesText::new(&e.value)))?;
        writer.write_event(Event::End(BytesEnd::new("expression")))?;
    }

    for p in &defines.positions {
        let mut elem = BytesStart::new("position");
        elem.push_attribute(("name", p.name.as_str()));
        if let Some(ref x) = p.x {
            elem.push_attribute(("x", x.as_str()));
        }
        if let Some(ref y) = p.y {
            elem.push_attribute(("y", y.as_str()));
        }
        if let Some(ref z) = p.z {
            elem.push_attribute(("z", z.as_str()));
        }
        if let Some(ref u) = p.unit {
            elem.push_attribute(("unit", u.as_str()));
        }
        writer.write_event(Event::Empty(elem))?;
    }

    for r in &defines.rotations {
        let mut elem = BytesStart::new("rotation");
        elem.push_attribute(("name", r.name.as_str()));
        if let Some(ref x) = r.x {
            elem.push_attribute(("x", x.as_str()));
        }
        if let Some(ref y) = r.y {
            elem.push_attribute(("y", y.as_str()));
        }
        if let Some(ref z) = r.z {
            elem.push_attribute(("z", z.as_str()));
        }
        if let Some(ref u) = r.unit {
            elem.push_attribute(("unit", u.as_str()));
        }
        writer.write_event(Event::Empty(elem))?;
    }

    writer.write_event(Event::End(BytesEnd::new("define")))?;
    Ok(())
}

fn write_materials(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    materials: &MaterialSection,
) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("materials")))?;

    for el in &materials.elements {
        let mut elem = BytesStart::new("element");
        elem.push_attribute(("name", el.name.as_str()));
        if let Some(ref f) = el.formula {
            elem.push_attribute(("formula", f.as_str()));
        }
        if let Some(ref z) = el.z {
            elem.push_attribute(("Z", z.as_str()));
        }
        if let Some(ref av) = el.atom_value {
            writer.write_event(Event::Start(elem))?;
            let mut atom = BytesStart::new("atom");
            atom.push_attribute(("value", av.as_str()));
            writer.write_event(Event::Empty(atom))?;
            writer.write_event(Event::End(BytesEnd::new("element")))?;
        } else {
            writer.write_event(Event::Empty(elem))?;
        }
    }

    for mat in &materials.materials {
        let mut elem = BytesStart::new("material");
        elem.push_attribute(("name", mat.name.as_str()));
        if let Some(ref f) = mat.formula {
            elem.push_attribute(("formula", f.as_str()));
        }
        if let Some(ref z) = mat.z {
            elem.push_attribute(("Z", z.as_str()));
        }
        writer.write_event(Event::Start(elem))?;

        if let Some(ref d) = mat.density {
            let mut de = BytesStart::new("D");
            de.push_attribute(("value", d.value.as_str()));
            if let Some(ref u) = d.unit {
                de.push_attribute(("unit", u.as_str()));
            }
            writer.write_event(Event::Empty(de))?;
        }
        if let Some(ref dr) = mat.density_ref {
            let mut dref = BytesStart::new("Dref");
            dref.push_attribute(("ref", dr.as_str()));
            writer.write_event(Event::Empty(dref))?;
        }
        if let Some(ref t) = mat.temperature {
            let mut te = BytesStart::new("T");
            te.push_attribute(("value", t.value.as_str()));
            if let Some(ref u) = t.unit {
                te.push_attribute(("unit", u.as_str()));
            }
            writer.write_event(Event::Empty(te))?;
        }
        if let Some(ref p) = mat.pressure {
            let mut pe = BytesStart::new("P");
            pe.push_attribute(("value", p.value.as_str()));
            if let Some(ref u) = p.unit {
                pe.push_attribute(("unit", u.as_str()));
            }
            writer.write_event(Event::Empty(pe))?;
        }
        if let Some(ref av) = mat.atom_value {
            let mut atom = BytesStart::new("atom");
            atom.push_attribute(("value", av.as_str()));
            writer.write_event(Event::Empty(atom))?;
        }
        for comp in &mat.components {
            match comp {
                MaterialComponent::Fraction { n, ref_name } => {
                    let mut fe = BytesStart::new("fraction");
                    fe.push_attribute(("n", n.as_str()));
                    fe.push_attribute(("ref", ref_name.as_str()));
                    writer.write_event(Event::Empty(fe))?;
                }
                MaterialComponent::Composite { n, ref_name } => {
                    let mut ce = BytesStart::new("composite");
                    ce.push_attribute(("n", n.as_str()));
                    ce.push_attribute(("ref", ref_name.as_str()));
                    writer.write_event(Event::Empty(ce))?;
                }
            }
        }

        writer.write_event(Event::End(BytesEnd::new("material")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("materials")))?;
    Ok(())
}

fn write_placement_pos(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    pos: &PlacementPos,
    inline_tag: &str,
    ref_tag: &str,
) -> Result<()> {
    match pos {
        PlacementPos::Inline(p) => {
            let mut pe = BytesStart::new(inline_tag);
            if !p.name.is_empty() {
                pe.push_attribute(("name", p.name.as_str()));
            }
            if let Some(ref x) = p.x {
                pe.push_attribute(("x", x.as_str()));
            }
            if let Some(ref y) = p.y {
                pe.push_attribute(("y", y.as_str()));
            }
            if let Some(ref z) = p.z {
                pe.push_attribute(("z", z.as_str()));
            }
            if let Some(ref u) = p.unit {
                pe.push_attribute(("unit", u.as_str()));
            }
            writer.write_event(Event::Empty(pe))?;
        }
        PlacementPos::Ref(name) => {
            let mut pr = BytesStart::new(ref_tag);
            pr.push_attribute(("ref", name.as_str()));
            writer.write_event(Event::Empty(pr))?;
        }
    }
    Ok(())
}

fn write_placement_rot(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    rot: &PlacementRot,
    inline_tag: &str,
    ref_tag: &str,
) -> Result<()> {
    match rot {
        PlacementRot::Inline(r) => {
            let mut re = BytesStart::new(inline_tag);
            if !r.name.is_empty() {
                re.push_attribute(("name", r.name.as_str()));
            }
            if let Some(ref x) = r.x {
                re.push_attribute(("x", x.as_str()));
            }
            if let Some(ref y) = r.y {
                re.push_attribute(("y", y.as_str()));
            }
            if let Some(ref z) = r.z {
                re.push_attribute(("z", z.as_str()));
            }
            if let Some(ref u) = r.unit {
                re.push_attribute(("unit", u.as_str()));
            }
            writer.write_event(Event::Empty(re))?;
        }
        PlacementRot::Ref(name) => {
            let mut rr = BytesStart::new(ref_tag);
            rr.push_attribute(("ref", name.as_str()));
            writer.write_event(Event::Empty(rr))?;
        }
    }
    Ok(())
}

fn write_solids(writer: &mut Writer<Cursor<Vec<u8>>>, solids: &SolidSection) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("solids")))?;

    for solid in &solids.solids {
        match solid {
            Solid::Box(b) => {
                let mut elem = BytesStart::new("box");
                elem.push_attribute(("name", b.name.as_str()));
                elem.push_attribute(("x", b.x.as_str()));
                elem.push_attribute(("y", b.y.as_str()));
                elem.push_attribute(("z", b.z.as_str()));
                if let Some(ref u) = b.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Tube(t) => {
                let mut elem = BytesStart::new("tube");
                elem.push_attribute(("name", t.name.as_str()));
                if let Some(ref v) = t.rmin {
                    elem.push_attribute(("rmin", v.as_str()));
                }
                elem.push_attribute(("rmax", t.rmax.as_str()));
                elem.push_attribute(("z", t.z.as_str()));
                if let Some(ref v) = t.startphi {
                    elem.push_attribute(("startphi", v.as_str()));
                }
                if let Some(ref v) = t.deltaphi {
                    elem.push_attribute(("deltaphi", v.as_str()));
                }
                if let Some(ref u) = t.aunit {
                    elem.push_attribute(("aunit", u.as_str()));
                }
                if let Some(ref u) = t.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Cone(c) => {
                let mut elem = BytesStart::new("cone");
                elem.push_attribute(("name", c.name.as_str()));
                if let Some(ref v) = c.rmin1 {
                    elem.push_attribute(("rmin1", v.as_str()));
                }
                elem.push_attribute(("rmax1", c.rmax1.as_str()));
                if let Some(ref v) = c.rmin2 {
                    elem.push_attribute(("rmin2", v.as_str()));
                }
                elem.push_attribute(("rmax2", c.rmax2.as_str()));
                elem.push_attribute(("z", c.z.as_str()));
                if let Some(ref v) = c.startphi {
                    elem.push_attribute(("startphi", v.as_str()));
                }
                if let Some(ref v) = c.deltaphi {
                    elem.push_attribute(("deltaphi", v.as_str()));
                }
                if let Some(ref u) = c.aunit {
                    elem.push_attribute(("aunit", u.as_str()));
                }
                if let Some(ref u) = c.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Sphere(s) => {
                let mut elem = BytesStart::new("sphere");
                elem.push_attribute(("name", s.name.as_str()));
                if let Some(ref v) = s.rmin {
                    elem.push_attribute(("rmin", v.as_str()));
                }
                elem.push_attribute(("rmax", s.rmax.as_str()));
                if let Some(ref v) = s.startphi {
                    elem.push_attribute(("startphi", v.as_str()));
                }
                if let Some(ref v) = s.deltaphi {
                    elem.push_attribute(("deltaphi", v.as_str()));
                }
                if let Some(ref v) = s.starttheta {
                    elem.push_attribute(("starttheta", v.as_str()));
                }
                if let Some(ref v) = s.deltatheta {
                    elem.push_attribute(("deltatheta", v.as_str()));
                }
                if let Some(ref u) = s.aunit {
                    elem.push_attribute(("aunit", u.as_str()));
                }
                if let Some(ref u) = s.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Trd(t) => {
                let mut elem = BytesStart::new("trd");
                elem.push_attribute(("name", t.name.as_str()));
                elem.push_attribute(("x1", t.x1.as_str()));
                elem.push_attribute(("y1", t.y1.as_str()));
                elem.push_attribute(("x2", t.x2.as_str()));
                elem.push_attribute(("y2", t.y2.as_str()));
                elem.push_attribute(("z", t.z.as_str()));
                if let Some(ref u) = t.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Polycone(pc) => {
                let mut elem = BytesStart::new("polycone");
                elem.push_attribute(("name", pc.name.as_str()));
                if let Some(ref v) = pc.startphi {
                    elem.push_attribute(("startphi", v.as_str()));
                }
                if let Some(ref v) = pc.deltaphi {
                    elem.push_attribute(("deltaphi", v.as_str()));
                }
                if let Some(ref u) = pc.aunit {
                    elem.push_attribute(("aunit", u.as_str()));
                }
                if let Some(ref u) = pc.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Start(elem))?;
                for zp in &pc.zplanes {
                    let mut zelem = BytesStart::new("zplane");
                    zelem.push_attribute(("z", zp.z.as_str()));
                    if let Some(ref v) = zp.rmin {
                        zelem.push_attribute(("rmin", v.as_str()));
                    }
                    zelem.push_attribute(("rmax", zp.rmax.as_str()));
                    writer.write_event(Event::Empty(zelem))?;
                }
                writer.write_event(Event::End(BytesEnd::new("polycone")))?;
            }
            Solid::Xtru(x) => {
                let mut elem = BytesStart::new("xtru");
                elem.push_attribute(("name", x.name.as_str()));
                if let Some(ref u) = x.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Start(elem))?;
                for v in &x.vertices {
                    let mut velem = BytesStart::new("twoDimVertex");
                    velem.push_attribute(("x", v.x.as_str()));
                    velem.push_attribute(("y", v.y.as_str()));
                    writer.write_event(Event::Empty(velem))?;
                }
                for s in &x.sections {
                    let mut selem = BytesStart::new("section");
                    selem.push_attribute(("zOrder", s.z_order.as_str()));
                    selem.push_attribute(("zPosition", s.z_position.as_str()));
                    selem.push_attribute(("xOffset", s.x_offset.as_str()));
                    selem.push_attribute(("yOffset", s.y_offset.as_str()));
                    selem.push_attribute(("scalingFactor", s.scaling_factor.as_str()));
                    writer.write_event(Event::Empty(selem))?;
                }
                writer.write_event(Event::End(BytesEnd::new("xtru")))?;
            }
            Solid::Tessellated(ts) => {
                let mut elem = BytesStart::new("tessellated");
                elem.push_attribute(("name", ts.name.as_str()));
                writer.write_event(Event::Start(elem))?;
                for facet in &ts.facets {
                    match facet {
                        TessellatedFacet::Triangular {
                            vertex1,
                            vertex2,
                            vertex3,
                            r#type,
                        } => {
                            let mut fe = BytesStart::new("triangular");
                            fe.push_attribute(("vertex1", vertex1.as_str()));
                            fe.push_attribute(("vertex2", vertex2.as_str()));
                            fe.push_attribute(("vertex3", vertex3.as_str()));
                            if let Some(ref t) = r#type {
                                fe.push_attribute(("type", t.as_str()));
                            }
                            writer.write_event(Event::Empty(fe))?;
                        }
                        TessellatedFacet::Quadrangular {
                            vertex1,
                            vertex2,
                            vertex3,
                            vertex4,
                            r#type,
                        } => {
                            let mut fe = BytesStart::new("quadrangular");
                            fe.push_attribute(("vertex1", vertex1.as_str()));
                            fe.push_attribute(("vertex2", vertex2.as_str()));
                            fe.push_attribute(("vertex3", vertex3.as_str()));
                            fe.push_attribute(("vertex4", vertex4.as_str()));
                            if let Some(ref t) = r#type {
                                fe.push_attribute(("type", t.as_str()));
                            }
                            writer.write_event(Event::Empty(fe))?;
                        }
                    }
                }
                writer.write_event(Event::End(BytesEnd::new("tessellated")))?;
            }
            Solid::Orb(o) => {
                let mut elem = BytesStart::new("orb");
                elem.push_attribute(("name", o.name.as_str()));
                elem.push_attribute(("r", o.r.as_str()));
                if let Some(ref u) = o.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Ellipsoid(e) => {
                let mut elem = BytesStart::new("ellipsoid");
                elem.push_attribute(("name", e.name.as_str()));
                elem.push_attribute(("ax", e.ax.as_str()));
                elem.push_attribute(("by", e.by.as_str()));
                elem.push_attribute(("cz", e.cz.as_str()));
                if let Some(ref v) = e.zcut1 {
                    elem.push_attribute(("zcut1", v.as_str()));
                }
                if let Some(ref v) = e.zcut2 {
                    elem.push_attribute(("zcut2", v.as_str()));
                }
                if let Some(ref u) = e.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::GenericPolyhedra(gp) => {
                let mut elem = BytesStart::new("genericPolyhedra");
                elem.push_attribute(("name", gp.name.as_str()));
                if let Some(ref v) = gp.startphi {
                    elem.push_attribute(("startphi", v.as_str()));
                }
                if let Some(ref v) = gp.deltaphi {
                    elem.push_attribute(("deltaphi", v.as_str()));
                }
                elem.push_attribute(("numsides", gp.numsides.as_str()));
                if let Some(ref u) = gp.aunit {
                    elem.push_attribute(("aunit", u.as_str()));
                }
                if let Some(ref u) = gp.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Start(elem))?;
                for rz in &gp.rzpoints {
                    let mut rze = BytesStart::new("rzpoint");
                    rze.push_attribute(("r", rz.r.as_str()));
                    rze.push_attribute(("z", rz.z.as_str()));
                    writer.write_event(Event::Empty(rze))?;
                }
                writer.write_event(Event::End(BytesEnd::new("genericPolyhedra")))?;
            }
            Solid::Paraboloid(p) => {
                let mut elem = BytesStart::new("paraboloid");
                elem.push_attribute(("name", p.name.as_str()));
                elem.push_attribute(("rlo", p.rlo.as_str()));
                elem.push_attribute(("rhi", p.rhi.as_str()));
                elem.push_attribute(("dz", p.dz.as_str()));
                if let Some(ref u) = p.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Elcone(e) => {
                let mut elem = BytesStart::new("elcone");
                elem.push_attribute(("name", e.name.as_str()));
                elem.push_attribute(("dx", e.dx.as_str()));
                elem.push_attribute(("dy", e.dy.as_str()));
                elem.push_attribute(("zmax", e.zmax.as_str()));
                elem.push_attribute(("zcut", e.zcut.as_str()));
                if let Some(ref u) = e.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Hype(h) => {
                let mut elem = BytesStart::new("hype");
                elem.push_attribute(("name", h.name.as_str()));
                if let Some(ref v) = h.rmin {
                    elem.push_attribute(("rmin", v.as_str()));
                }
                elem.push_attribute(("rmax", h.rmax.as_str()));
                if let Some(ref v) = h.inst {
                    elem.push_attribute(("inst", v.as_str()));
                }
                if let Some(ref v) = h.outst {
                    elem.push_attribute(("outst", v.as_str()));
                }
                elem.push_attribute(("z", h.z.as_str()));
                if let Some(ref u) = h.aunit {
                    elem.push_attribute(("aunit", u.as_str()));
                }
                if let Some(ref u) = h.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::GenericPolycone(gp) => {
                let mut elem = BytesStart::new("genericPolycone");
                elem.push_attribute(("name", gp.name.as_str()));
                if let Some(ref v) = gp.startphi {
                    elem.push_attribute(("startphi", v.as_str()));
                }
                if let Some(ref v) = gp.deltaphi {
                    elem.push_attribute(("deltaphi", v.as_str()));
                }
                if let Some(ref u) = gp.aunit {
                    elem.push_attribute(("aunit", u.as_str()));
                }
                if let Some(ref u) = gp.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Start(elem))?;
                for rz in &gp.rzpoints {
                    let mut rze = BytesStart::new("rzpoint");
                    rze.push_attribute(("r", rz.r.as_str()));
                    rze.push_attribute(("z", rz.z.as_str()));
                    writer.write_event(Event::Empty(rze))?;
                }
                writer.write_event(Event::End(BytesEnd::new("genericPolycone")))?;
            }
            Solid::Tet(t) => {
                let mut elem = BytesStart::new("tet");
                elem.push_attribute(("name", t.name.as_str()));
                elem.push_attribute(("vertex1", t.vertex1.as_str()));
                elem.push_attribute(("vertex2", t.vertex2.as_str()));
                elem.push_attribute(("vertex3", t.vertex3.as_str()));
                elem.push_attribute(("vertex4", t.vertex4.as_str()));
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Eltube(e) => {
                let mut elem = BytesStart::new("eltube");
                elem.push_attribute(("name", e.name.as_str()));
                elem.push_attribute(("dx", e.dx.as_str()));
                elem.push_attribute(("dy", e.dy.as_str()));
                elem.push_attribute(("dz", e.dz.as_str()));
                if let Some(ref u) = e.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Polyhedra(ph) => {
                let mut elem = BytesStart::new("polyhedra");
                elem.push_attribute(("name", ph.name.as_str()));
                if let Some(ref v) = ph.startphi {
                    elem.push_attribute(("startphi", v.as_str()));
                }
                if let Some(ref v) = ph.deltaphi {
                    elem.push_attribute(("deltaphi", v.as_str()));
                }
                elem.push_attribute(("numsides", ph.numsides.as_str()));
                if let Some(ref u) = ph.aunit {
                    elem.push_attribute(("aunit", u.as_str()));
                }
                if let Some(ref u) = ph.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Start(elem))?;
                for zp in &ph.zplanes {
                    let mut zelem = BytesStart::new("zplane");
                    zelem.push_attribute(("z", zp.z.as_str()));
                    if let Some(ref v) = zp.rmin {
                        zelem.push_attribute(("rmin", v.as_str()));
                    }
                    zelem.push_attribute(("rmax", zp.rmax.as_str()));
                    writer.write_event(Event::Empty(zelem))?;
                }
                writer.write_event(Event::End(BytesEnd::new("polyhedra")))?;
            }
            Solid::CutTube(ct) => {
                let mut elem = BytesStart::new("cutTube");
                elem.push_attribute(("name", ct.name.as_str()));
                if let Some(ref v) = ct.rmin {
                    elem.push_attribute(("rmin", v.as_str()));
                }
                elem.push_attribute(("rmax", ct.rmax.as_str()));
                elem.push_attribute(("z", ct.z.as_str()));
                if let Some(ref v) = ct.startphi {
                    elem.push_attribute(("startphi", v.as_str()));
                }
                if let Some(ref v) = ct.deltaphi {
                    elem.push_attribute(("deltaphi", v.as_str()));
                }
                if let Some(ref v) = ct.low_x {
                    elem.push_attribute(("lowX", v.as_str()));
                }
                if let Some(ref v) = ct.low_y {
                    elem.push_attribute(("lowY", v.as_str()));
                }
                if let Some(ref v) = ct.low_z {
                    elem.push_attribute(("lowZ", v.as_str()));
                }
                if let Some(ref v) = ct.high_x {
                    elem.push_attribute(("highX", v.as_str()));
                }
                if let Some(ref v) = ct.high_y {
                    elem.push_attribute(("highY", v.as_str()));
                }
                if let Some(ref v) = ct.high_z {
                    elem.push_attribute(("highZ", v.as_str()));
                }
                if let Some(ref u) = ct.aunit {
                    elem.push_attribute(("aunit", u.as_str()));
                }
                if let Some(ref u) = ct.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Para(p) => {
                let mut elem = BytesStart::new("para");
                elem.push_attribute(("name", p.name.as_str()));
                elem.push_attribute(("x", p.x.as_str()));
                elem.push_attribute(("y", p.y.as_str()));
                elem.push_attribute(("z", p.z.as_str()));
                if let Some(ref v) = p.alpha {
                    elem.push_attribute(("alpha", v.as_str()));
                }
                if let Some(ref v) = p.theta {
                    elem.push_attribute(("theta", v.as_str()));
                }
                if let Some(ref v) = p.phi {
                    elem.push_attribute(("phi", v.as_str()));
                }
                if let Some(ref u) = p.aunit {
                    elem.push_attribute(("aunit", u.as_str()));
                }
                if let Some(ref u) = p.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Trap(t) => {
                let mut elem = BytesStart::new("trap");
                elem.push_attribute(("name", t.name.as_str()));
                elem.push_attribute(("z", t.z.as_str()));
                if let Some(ref v) = t.theta {
                    elem.push_attribute(("theta", v.as_str()));
                }
                if let Some(ref v) = t.phi {
                    elem.push_attribute(("phi", v.as_str()));
                }
                elem.push_attribute(("y1", t.y1.as_str()));
                elem.push_attribute(("x1", t.x1.as_str()));
                elem.push_attribute(("x2", t.x2.as_str()));
                if let Some(ref v) = t.alpha1 {
                    elem.push_attribute(("alpha1", v.as_str()));
                }
                elem.push_attribute(("y2", t.y2.as_str()));
                elem.push_attribute(("x3", t.x3.as_str()));
                elem.push_attribute(("x4", t.x4.as_str()));
                if let Some(ref v) = t.alpha2 {
                    elem.push_attribute(("alpha2", v.as_str()));
                }
                if let Some(ref u) = t.aunit {
                    elem.push_attribute(("aunit", u.as_str()));
                }
                if let Some(ref u) = t.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Torus(t) => {
                let mut elem = BytesStart::new("torus");
                elem.push_attribute(("name", t.name.as_str()));
                if let Some(ref v) = t.rmin {
                    elem.push_attribute(("rmin", v.as_str()));
                }
                elem.push_attribute(("rmax", t.rmax.as_str()));
                elem.push_attribute(("rtor", t.rtor.as_str()));
                if let Some(ref v) = t.startphi {
                    elem.push_attribute(("startphi", v.as_str()));
                }
                if let Some(ref v) = t.deltaphi {
                    elem.push_attribute(("deltaphi", v.as_str()));
                }
                if let Some(ref u) = t.aunit {
                    elem.push_attribute(("aunit", u.as_str()));
                }
                if let Some(ref u) = t.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::TwistedTubs(t) => {
                let mut elem = BytesStart::new("twistedtubs");
                elem.push_attribute(("name", t.name.as_str()));
                elem.push_attribute(("twistedangle", t.twistedangle.as_str()));
                if let Some(ref v) = t.endinnerrad {
                    elem.push_attribute(("endinnerrad", v.as_str()));
                }
                elem.push_attribute(("endouterrad", t.endouterrad.as_str()));
                elem.push_attribute(("zlen", t.zlen.as_str()));
                if let Some(ref v) = t.phi {
                    elem.push_attribute(("phi", v.as_str()));
                }
                if let Some(ref u) = t.aunit {
                    elem.push_attribute(("aunit", u.as_str()));
                }
                if let Some(ref u) = t.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::TwistedTrd(t) => {
                let mut elem = BytesStart::new("twistedtrd");
                elem.push_attribute(("name", t.name.as_str()));
                elem.push_attribute(("PhiTwist", t.phi_twist.as_str()));
                elem.push_attribute(("x1", t.x1.as_str()));
                elem.push_attribute(("x2", t.x2.as_str()));
                elem.push_attribute(("y1", t.y1.as_str()));
                elem.push_attribute(("y2", t.y2.as_str()));
                elem.push_attribute(("z", t.z.as_str()));
                if let Some(ref u) = t.aunit {
                    elem.push_attribute(("aunit", u.as_str()));
                }
                if let Some(ref u) = t.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::TwistedTrap(t) => {
                let mut elem = BytesStart::new("twistedtrap");
                elem.push_attribute(("name", t.name.as_str()));
                elem.push_attribute(("PhiTwist", t.phi_twist.as_str()));
                elem.push_attribute(("z", t.z.as_str()));
                elem.push_attribute(("Theta", t.theta.as_str()));
                elem.push_attribute(("Phi", t.phi.as_str()));
                elem.push_attribute(("y1", t.y1.as_str()));
                elem.push_attribute(("x1", t.x1.as_str()));
                elem.push_attribute(("x2", t.x2.as_str()));
                elem.push_attribute(("y2", t.y2.as_str()));
                elem.push_attribute(("x3", t.x3.as_str()));
                elem.push_attribute(("x4", t.x4.as_str()));
                elem.push_attribute(("Alph", t.alph.as_str()));
                if let Some(ref u) = t.aunit {
                    elem.push_attribute(("aunit", u.as_str()));
                }
                if let Some(ref u) = t.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::TwistedBox(t) => {
                let mut elem = BytesStart::new("twistedbox");
                elem.push_attribute(("name", t.name.as_str()));
                elem.push_attribute(("PhiTwist", t.phi_twist.as_str()));
                elem.push_attribute(("x", t.x.as_str()));
                elem.push_attribute(("y", t.y.as_str()));
                elem.push_attribute(("z", t.z.as_str()));
                if let Some(ref u) = t.aunit {
                    elem.push_attribute(("aunit", u.as_str()));
                }
                if let Some(ref u) = t.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Arb8(a) => {
                let mut elem = BytesStart::new("arb8");
                elem.push_attribute(("name", a.name.as_str()));
                elem.push_attribute(("dz", a.dz.as_str()));
                elem.push_attribute(("v1x", a.v1x.as_str()));
                elem.push_attribute(("v1y", a.v1y.as_str()));
                elem.push_attribute(("v2x", a.v2x.as_str()));
                elem.push_attribute(("v2y", a.v2y.as_str()));
                elem.push_attribute(("v3x", a.v3x.as_str()));
                elem.push_attribute(("v3y", a.v3y.as_str()));
                elem.push_attribute(("v4x", a.v4x.as_str()));
                elem.push_attribute(("v4y", a.v4y.as_str()));
                elem.push_attribute(("v5x", a.v5x.as_str()));
                elem.push_attribute(("v5y", a.v5y.as_str()));
                elem.push_attribute(("v6x", a.v6x.as_str()));
                elem.push_attribute(("v6y", a.v6y.as_str()));
                elem.push_attribute(("v7x", a.v7x.as_str()));
                elem.push_attribute(("v7y", a.v7y.as_str()));
                elem.push_attribute(("v8x", a.v8x.as_str()));
                elem.push_attribute(("v8y", a.v8y.as_str()));
                if let Some(ref u) = a.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Boolean(bs) => {
                let tag_name = match bs.operation {
                    BooleanOp::Subtraction => "subtraction",
                    BooleanOp::Union => "union",
                    BooleanOp::Intersection => "intersection",
                };
                let mut elem = BytesStart::new(tag_name);
                elem.push_attribute(("name", bs.name.as_str()));
                writer.write_event(Event::Start(elem))?;

                let mut first = BytesStart::new("first");
                first.push_attribute(("ref", bs.first_ref.as_str()));
                writer.write_event(Event::Empty(first))?;

                let mut second = BytesStart::new("second");
                second.push_attribute(("ref", bs.second_ref.as_str()));
                writer.write_event(Event::Empty(second))?;

                if let Some(ref pos) = bs.position {
                    write_placement_pos(writer, pos, "position", "positionref")?;
                }
                if let Some(ref rot) = bs.rotation {
                    write_placement_rot(writer, rot, "rotation", "rotationref")?;
                }
                if let Some(ref pos) = bs.first_position {
                    write_placement_pos(writer, pos, "firstposition", "firstpositionref")?;
                }
                if let Some(ref rot) = bs.first_rotation {
                    write_placement_rot(writer, rot, "firstrotation", "firstrotationref")?;
                }

                writer.write_event(Event::End(BytesEnd::new(tag_name)))?;
            }
        }
    }

    writer.write_event(Event::End(BytesEnd::new("solids")))?;
    Ok(())
}

fn write_structure(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    structure: &StructureSection,
) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("structure")))?;

    for vol in &structure.volumes {
        let mut elem = BytesStart::new("volume");
        elem.push_attribute(("name", vol.name.as_str()));
        writer.write_event(Event::Start(elem))?;

        let mut mref = BytesStart::new("materialref");
        mref.push_attribute(("ref", vol.material_ref.as_str()));
        writer.write_event(Event::Empty(mref))?;

        let mut sref = BytesStart::new("solidref");
        sref.push_attribute(("ref", vol.solid_ref.as_str()));
        writer.write_event(Event::Empty(sref))?;

        for pv in &vol.physvols {
            let mut pv_elem = BytesStart::new("physvol");
            if let Some(ref n) = pv.name {
                pv_elem.push_attribute(("name", n.as_str()));
            }
            writer.write_event(Event::Start(pv_elem))?;

            if let Some(ref fref) = pv.file_ref {
                let mut fe = BytesStart::new("file");
                fe.push_attribute(("name", fref.name.as_str()));
                if let Some(ref vn) = fref.volname {
                    fe.push_attribute(("volname", vn.as_str()));
                }
                writer.write_event(Event::Empty(fe))?;
            } else {
                let mut vref = BytesStart::new("volumeref");
                vref.push_attribute(("ref", pv.volume_ref.as_str()));
                writer.write_event(Event::Empty(vref))?;
            }

            match &pv.position {
                Some(PlacementPos::Inline(p)) => {
                    let mut pe = BytesStart::new("position");
                    if !p.name.is_empty() {
                        pe.push_attribute(("name", p.name.as_str()));
                    }
                    if let Some(ref x) = p.x {
                        pe.push_attribute(("x", x.as_str()));
                    }
                    if let Some(ref y) = p.y {
                        pe.push_attribute(("y", y.as_str()));
                    }
                    if let Some(ref z) = p.z {
                        pe.push_attribute(("z", z.as_str()));
                    }
                    if let Some(ref u) = p.unit {
                        pe.push_attribute(("unit", u.as_str()));
                    }
                    writer.write_event(Event::Empty(pe))?;
                }
                Some(PlacementPos::Ref(name)) => {
                    let mut pr = BytesStart::new("positionref");
                    pr.push_attribute(("ref", name.as_str()));
                    writer.write_event(Event::Empty(pr))?;
                }
                None => {}
            }

            match &pv.rotation {
                Some(PlacementRot::Inline(r)) => {
                    let mut re = BytesStart::new("rotation");
                    if !r.name.is_empty() {
                        re.push_attribute(("name", r.name.as_str()));
                    }
                    if let Some(ref x) = r.x {
                        re.push_attribute(("x", x.as_str()));
                    }
                    if let Some(ref y) = r.y {
                        re.push_attribute(("y", y.as_str()));
                    }
                    if let Some(ref z) = r.z {
                        re.push_attribute(("z", z.as_str()));
                    }
                    if let Some(ref u) = r.unit {
                        re.push_attribute(("unit", u.as_str()));
                    }
                    writer.write_event(Event::Empty(re))?;
                }
                Some(PlacementRot::Ref(name)) => {
                    let mut rr = BytesStart::new("rotationref");
                    rr.push_attribute(("ref", name.as_str()));
                    writer.write_event(Event::Empty(rr))?;
                }
                None => {}
            }

            writer.write_event(Event::End(BytesEnd::new("physvol")))?;
        }

        // Write replicavol if present
        if let Some(ref replica) = vol.replica {
            let mut rv = BytesStart::new("replicavol");
            rv.push_attribute(("number", replica.number.as_str()));
            writer.write_event(Event::Start(rv))?;

            let mut vref = BytesStart::new("volumeref");
            vref.push_attribute(("ref", replica.volume_ref.as_str()));
            writer.write_event(Event::Empty(vref))?;

            writer.write_event(Event::Start(BytesStart::new("replicate_along_axis")))?;

            let mut dir = BytesStart::new("direction");
            if let Some(ref x) = replica.direction[0] {
                dir.push_attribute(("x", x.as_str()));
            }
            if let Some(ref y) = replica.direction[1] {
                dir.push_attribute(("y", y.as_str()));
            }
            if let Some(ref z) = replica.direction[2] {
                dir.push_attribute(("z", z.as_str()));
            }
            writer.write_event(Event::Empty(dir))?;

            let mut w = BytesStart::new("width");
            w.push_attribute(("value", replica.width.as_str()));
            if let Some(ref u) = replica.width_unit {
                w.push_attribute(("unit", u.as_str()));
            }
            writer.write_event(Event::Empty(w))?;

            let mut o = BytesStart::new("offset");
            o.push_attribute(("value", replica.offset.as_str()));
            if let Some(ref u) = replica.offset_unit {
                o.push_attribute(("unit", u.as_str()));
            }
            writer.write_event(Event::Empty(o))?;

            writer.write_event(Event::End(BytesEnd::new("replicate_along_axis")))?;
            writer.write_event(Event::End(BytesEnd::new("replicavol")))?;
        }

        for aux in &vol.auxiliaries {
            let mut ae = BytesStart::new("auxiliary");
            ae.push_attribute(("auxtype", aux.auxtype.as_str()));
            ae.push_attribute(("auxvalue", aux.auxvalue.as_str()));
            writer.write_event(Event::Empty(ae))?;
        }

        writer.write_event(Event::End(BytesEnd::new("volume")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("structure")))?;
    Ok(())
}

fn write_setup(writer: &mut Writer<Cursor<Vec<u8>>>, setup: &SetupSection) -> Result<()> {
    let mut elem = BytesStart::new("setup");
    elem.push_attribute(("name", setup.name.as_str()));
    elem.push_attribute(("version", setup.version.as_str()));
    writer.write_event(Event::Start(elem))?;

    let mut world = BytesStart::new("world");
    world.push_attribute(("ref", setup.world_ref.as_str()));
    writer.write_event(Event::Empty(world))?;

    writer.write_event(Event::End(BytesEnd::new("setup")))?;
    Ok(())
}
