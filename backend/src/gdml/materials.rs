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
    gdml.push_attribute(("xsi:noNamespaceSchemaLocation", "http://service-spi.web.cern.ch/service-spi/app/releases/GDML/schema/gdml.xsd"));
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

fn write_materials(writer: &mut Writer<Cursor<Vec<u8>>>, materials: &MaterialSection) -> Result<()> {
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
        }
    }

    writer.write_event(Event::End(BytesEnd::new("solids")))?;
    Ok(())
}

fn write_structure(writer: &mut Writer<Cursor<Vec<u8>>>, structure: &StructureSection) -> Result<()> {
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
