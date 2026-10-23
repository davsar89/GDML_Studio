use anyhow::Result;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::sync::LazyLock;

use super::model::*;
use crate::eval::dependency::dependency_order;

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
    serde_json::from_str(include_str!("../../data/nist_materials.json"))
        .expect("embedded nist_materials.json parses")
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

// ─── GDML Serializer ────────────────────────────────────────────────────────

/// Which section a raw element should be written into.
fn raw_section(raw: &RawElement) -> &str {
    match raw.section.as_str() {
        // "root" means it sat directly under <gdml>, e.g. <userinfo>.
        "root" => "gdml",
        s => s,
    }
}

/// Write a preserved-verbatim element inside the currently open section.
fn write_raw(writer: &mut Writer<Cursor<Vec<u8>>>, raw: &RawElement) -> Result<()> {
    writer.get_mut().write_all(b"\n    ")?;
    writer.get_mut().write_all(raw.xml.as_bytes())?;
    Ok(())
}

/// Write, and take out of `raws`, the preserved elements `at` selects by the
/// element each followed (`None`: the section start). Sections call this at
/// their start, after each named element, and at their end for any left over,
/// so every element goes back where it was declared ([`RawElement::after`]).
fn write_raws(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    raws: &mut Vec<&RawElement>,
    at: impl Fn(Option<&str>) -> bool,
) -> Result<()> {
    let (now, later) = raws.drain(..).partition(|r| at(r.after.as_deref()));
    *raws = later;
    for raw in now {
        write_raw(writer, raw)?;
    }
    Ok(())
}

pub fn serialize_gdml(doc: &GdmlDocument) -> Result<String> {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    // XML declaration
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

    if let Some(ref dt) = doc.order.doctype {
        // `from_escaped` writes the body verbatim; write_event wraps it as
        // `<!DOCTYPE ... >`.
        writer.write_event(Event::DocType(BytesText::from_escaped(dt)))?;
    }

    for text in &doc.order.prolog {
        writer.write_event(Event::Comment(BytesText::from_escaped(text)))?;
    }

    // <gdml> root. Reproduce the source's own attributes when we captured them;
    // fall back to the standard schema pair for documents built programmatically
    // (the handler tests) which have no source to copy.
    let mut gdml = BytesStart::new("gdml");
    if doc.root_attributes.is_empty() {
        gdml.push_attribute(("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"));
        gdml.push_attribute((
            "xsi:noNamespaceSchemaLocation",
            "http://service-spi.web.cern.ch/service-spi/app/releases/GDML/schema/gdml.xsd",
        ));
    } else {
        for (k, v) in &doc.root_attributes {
            gdml.push_attribute((k.as_str(), v.as_str()));
        }
    }
    writer.write_event(Event::Start(gdml))?;

    // Preserved-verbatim elements are re-emitted into the section they came
    // from (e.g. <opticalsurface> in <solids>, <skinsurface> in <structure>).
    let raw_in = |section: &str| -> Vec<&RawElement> {
        doc.raw_unknown
            .iter()
            .filter(|r| raw_section(r) == section)
            .collect()
    };

    write_comments(&mut writer, &doc.order, "root", Some("define"))?;
    let nested_defines = doc.materials_define.clone().unwrap_or_default();
    write_defines(
        &mut writer,
        &doc.defines,
        &mut raw_in("define"),
        &doc.order,
        &nested_defines,
    )?;
    write_comments(&mut writer, &doc.order, "root", Some("materials"))?;
    write_materials(
        &mut writer,
        &doc.materials,
        &mut raw_in("materials"),
        &doc.order,
        &doc.defines,
        doc.materials_define.as_ref(),
    )?;
    write_comments(&mut writer, &doc.order, "root", Some("solids"))?;
    write_solids(&mut writer, &doc.solids, &mut raw_in("solids"), &doc.order)?;
    write_comments(&mut writer, &doc.order, "root", Some("structure"))?;
    write_structure(
        &mut writer,
        &doc.structure,
        &mut raw_in("structure"),
        &doc.order,
    )?;
    write_comments(&mut writer, &doc.order, "root", Some("setup"))?;
    // Emit every setup the source carried, not just the selected one. Falls back
    // to the single selected setup for documents built programmatically.
    if doc.setups.is_empty() {
        write_setup(&mut writer, &doc.setup)?;
    } else {
        for setup in &doc.setups {
            write_setup(&mut writer, setup)?;
        }
    }

    // Elements that belong directly under <gdml> (e.g. <userinfo>).
    for raw in raw_in("gdml") {
        writer.get_mut().write_all(b"\n  ")?;
        writer.get_mut().write_all(raw.xml.as_bytes())?;
    }

    // Comments that were inside <gdml> with nothing after them.
    write_comments(&mut writer, &doc.order, "root", None)?;

    // </gdml>
    writer.write_event(Event::End(BytesEnd::new("gdml")))?;

    for text in &doc.order.epilog {
        writer.write_event(Event::Comment(BytesText::from_escaped(text)))?;
    }

    let result = writer.into_inner().into_inner();
    Ok(String::from_utf8(result)?)
}

/// The top-level `<define>` block: every define except those the source kept in
/// the nested `<materials><define>`.
fn write_defines(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    defines: &DefineSection,
    raws: &mut Vec<&RawElement>,
    order: &DocumentOrder,
    nested: &[String],
) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("define")))?;
    write_raws(writer, raws, |after| after.is_none())?;
    write_define_items(
        writer,
        defines,
        order,
        &|name| !nested.iter().any(|n| n == name),
        raws,
    )?;
    write_raws(writer, raws, |_| true)?;
    write_comments(writer, order, "define", None)?;
    writer.write_event(Event::End(BytesEnd::new("define")))?;
    Ok(())
}

/// The `<define>` nested inside `<materials>`, holding only what came from
/// there. Emitted even when empty, because a present-but-empty block is part of
/// the source's shape.
fn write_materials_define(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    defines: &DefineSection,
    order: &DocumentOrder,
    nested: &[String],
) -> Result<()> {
    if nested.is_empty() {
        writer.write_event(Event::Empty(BytesStart::new("define")))?;
        return Ok(());
    }
    writer.write_event(Event::Start(BytesStart::new("define")))?;
    write_define_items(
        writer,
        defines,
        order,
        &|name| nested.iter().any(|n| n == name),
        &mut Vec::new(),
    )?;
    writer.write_event(Event::End(BytesEnd::new("define")))?;
    Ok(())
}

/// Write the define items `include` selects. Shared by both blocks so the two
/// cannot drift apart in which attributes they emit.
///
/// Replays `order.define_slots`, the order the source declared these in,
/// whenever it lines up with the collections (why: see
/// `DocumentOrder::define_slots`), each preserved element after its define.
///
/// A document built programmatically carries no manifest; those fall back to
/// the grouped order, which is what the rest of the suite expects, and leave
/// `raws` to the caller.
fn write_define_items(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    defines: &DefineSection,
    order: &DocumentOrder,
    include: &dyn Fn(&str) -> bool,
    raws: &mut Vec<&RawElement>,
) -> Result<()> {
    let grouped = [
        (DefineKind::Constant, defines.constants.len()),
        (DefineKind::Quantity, defines.quantities.len()),
        (DefineKind::Variable, defines.variables.len()),
        (DefineKind::Expression, defines.expressions.len()),
        (DefineKind::Position, defines.positions.len()),
        (DefineKind::Rotation, defines.rotations.len()),
        (DefineKind::Scale, defines.scales.len()),
        (DefineKind::Matrix, defines.matrices.len()),
    ];
    // The manifest is appended alongside the collections, one entry per item in
    // the same relative order, so a per-kind cursor walks the two in step.
    // Check that fully before writing anything: a mismatch means the document
    // was assembled some other way, and a half-written block would be worse
    // than the grouped fallback.
    let mut cursor: HashMap<DefineKind, usize> = HashMap::new();
    let slots: Vec<(DefineKind, usize)> = order
        .define_slots
        .iter()
        .map(|slot| {
            let n = cursor.entry(slot.kind).or_default();
            *n += 1;
            (slot.kind, *n - 1)
        })
        .collect();
    let replay = slots.len() == grouped.iter().map(|(_, len)| len).sum::<usize>()
        && slots
            .iter()
            .zip(&order.define_slots)
            .all(|(&(kind, n), slot)| define_name(defines, kind, n) == Some(&slot.name));
    let items: Vec<(DefineKind, usize)> = if replay {
        slots
    } else {
        grouped
            .into_iter()
            .flat_map(|(kind, len)| (0..len).map(move |n| (kind, n)))
            .collect()
    };
    for (kind, n) in items {
        let name = define_name(defines, kind, n).map_or("", String::as_str);
        if include(name) {
            write_comments(writer, order, "define", Some(name))?;
            write_define(writer, defines, kind, n)?;
        }
        if replay {
            write_raws(writer, raws, |after| after == Some(name))?;
        }
    }
    Ok(())
}

/// Name of the `n`th define of `kind`, if there is one.
fn define_name(defines: &DefineSection, kind: DefineKind, n: usize) -> Option<&String> {
    match kind {
        DefineKind::Constant => defines.constants.get(n).map(|x| &x.name),
        DefineKind::Quantity => defines.quantities.get(n).map(|x| &x.name),
        DefineKind::Variable => defines.variables.get(n).map(|x| &x.name),
        DefineKind::Expression => defines.expressions.get(n).map(|x| &x.name),
        DefineKind::Position => defines.positions.get(n).map(|x| &x.name),
        DefineKind::Rotation => defines.rotations.get(n).map(|x| &x.name),
        DefineKind::Scale => defines.scales.get(n).map(|x| &x.name),
        DefineKind::Matrix => defines.matrices.get(n).map(|x| &x.name),
    }
}

/// The `n`th define of `kind` as its element.
fn write_define(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    defines: &DefineSection,
    kind: DefineKind,
    n: usize,
) -> Result<()> {
    match kind {
        DefineKind::Constant => {
            let c = &defines.constants[n];
            let mut elem = BytesStart::new("constant");
            elem.push_attribute(("name", c.name.as_str()));
            elem.push_attribute(("value", c.value.as_str()));
            writer.write_event(Event::Empty(elem))?;
        }
        DefineKind::Quantity => {
            let q = &defines.quantities[n];
            let mut elem = BytesStart::new("quantity");
            elem.push_attribute(("name", q.name.as_str()));
            push_opt(&mut elem, "type", &q.r#type);
            elem.push_attribute(("value", q.value.as_str()));
            push_opt(&mut elem, "unit", &q.unit);
            writer.write_event(Event::Empty(elem))?;
        }
        DefineKind::Variable => {
            let v = &defines.variables[n];
            let mut elem = BytesStart::new("variable");
            elem.push_attribute(("name", v.name.as_str()));
            elem.push_attribute(("value", v.value.as_str()));
            writer.write_event(Event::Empty(elem))?;
        }
        DefineKind::Expression => {
            let e = &defines.expressions[n];
            let mut elem = BytesStart::new("expression");
            elem.push_attribute(("name", e.name.as_str()));
            writer.write_event(Event::Start(elem))?;
            writer.write_event(Event::Text(BytesText::new(&e.value)))?;
            writer.write_event(Event::End(BytesEnd::new("expression")))?;
        }
        DefineKind::Position => {
            let p = &defines.positions[n];
            write_xyz(writer, "position", &p.name, [&p.x, &p.y, &p.z], &p.unit)?;
        }
        DefineKind::Rotation => {
            let r = &defines.rotations[n];
            write_xyz(writer, "rotation", &r.name, [&r.x, &r.y, &r.z], &r.unit)?;
        }
        DefineKind::Scale => {
            let s = &defines.scales[n];
            write_xyz(writer, "scale", &s.name, [&s.x, &s.y, &s.z], &None)?;
        }
        DefineKind::Matrix => {
            let m = &defines.matrices[n];
            let mut elem = BytesStart::new("matrix");
            elem.push_attribute(("name", m.name.as_str()));
            elem.push_attribute(("coldim", m.coldim.as_str()));
            elem.push_attribute(("values", m.values.as_str()));
            writer.write_event(Event::Empty(elem))?;
        }
    }
    Ok(())
}

fn write_materials(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    materials: &MaterialSection,
    raws: &mut Vec<&RawElement>,
    order: &DocumentOrder,
    defines: &DefineSection,
    nested: Option<&Vec<String>>,
) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("materials")))?;

    // The nested <define>: see `GdmlDocument::materials_define`.
    if let Some(names) = nested {
        write_materials_define(writer, defines, order, names)?;
    }
    write_raws(writer, raws, |after| after.is_none())?;

    for iso in &materials.isotopes {
        write_comments(writer, order, "materials", Some(&iso.name))?;
        let mut elem = BytesStart::new("isotope");
        elem.push_attribute(("name", iso.name.as_str()));
        push_opt(&mut elem, "N", &iso.n);
        push_opt(&mut elem, "Z", &iso.z);
        if let Some(ref av) = iso.atom_value {
            writer.write_event(Event::Start(elem))?;
            write_value(writer, "atom", av, &iso.atom_unit, &iso.atom_type)?;
            writer.write_event(Event::End(BytesEnd::new("isotope")))?;
        } else {
            writer.write_event(Event::Empty(elem))?;
        }
        write_raws(writer, raws, |after| after == Some(iso.name.as_str()))?;
    }

    for el in &materials.elements {
        write_comments(writer, order, "materials", Some(&el.name))?;
        let mut elem = BytesStart::new("element");
        elem.push_attribute(("name", el.name.as_str()));
        push_opt(&mut elem, "formula", &el.formula);
        push_opt(&mut elem, "Z", &el.z);
        if el.atom_value.is_some() || !el.fractions.is_empty() {
            writer.write_event(Event::Start(elem))?;
            if let Some(ref av) = el.atom_value {
                write_value(writer, "atom", av, &el.atom_unit, &el.atom_type)?;
            }
            for fr in &el.fractions {
                write_component(writer, "fraction", &fr.n, &fr.ref_name)?;
            }
            writer.write_event(Event::End(BytesEnd::new("element")))?;
        } else {
            writer.write_event(Event::Empty(elem))?;
        }
        write_raws(writer, raws, |after| after == Some(el.name.as_str()))?;
    }

    for mat in material_order(materials)?
        .into_iter()
        .map(|i| &materials.materials[i])
    {
        write_comments(writer, order, "materials", Some(&mat.name))?;
        let mut elem = BytesStart::new("material");
        elem.push_attribute(("name", mat.name.as_str()));
        push_opt(&mut elem, "formula", &mat.formula);
        push_opt(&mut elem, "Z", &mat.z);
        push_opt(&mut elem, "state", &mat.state);
        writer.write_event(Event::Start(elem))?;

        if let Some(ref d) = mat.density {
            write_value(writer, "D", &d.value, &d.unit, &None)?;
        }
        for (tag, quantity) in [
            ("Dref", &mat.density_ref),
            ("Tref", &mat.temperature_ref),
            ("Pref", &mat.pressure_ref),
            ("MEEref", &mat.mee_ref),
        ] {
            if let Some(quantity) = quantity {
                write_ref(writer, tag, quantity)?;
            }
        }
        for (tag, property) in [
            ("T", &mat.temperature),
            ("P", &mat.pressure),
            ("MEE", &mat.mee),
            ("RL", &mat.rl),
            ("AL", &mat.al),
        ] {
            if let Some(property) = property {
                write_value(writer, tag, &property.value, &property.unit, &None)?;
            }
        }
        for prop in &mat.properties {
            let mut e = BytesStart::new("property");
            e.push_attribute(("name", prop.name.as_str()));
            push_opt(&mut e, "ref", &prop.ref_name);
            push_opt(&mut e, "values", &prop.values);
            writer.write_event(Event::Empty(e))?;
        }
        if let Some(ref av) = mat.atom_value {
            write_value(writer, "atom", av, &mat.atom_unit, &mat.atom_type)?;
        }
        for comp in &mat.components {
            let (tag, n, ref_name) = match comp {
                MaterialComponent::Fraction { n, ref_name } => ("fraction", n, ref_name),
                MaterialComponent::Composite { n, ref_name } => ("composite", n, ref_name),
            };
            write_component(writer, tag, n, ref_name)?;
        }

        writer.write_event(Event::End(BytesEnd::new("material")))?;
        write_raws(writer, raws, |after| after == Some(mat.name.as_str()))?;
    }
    write_raws(writer, raws, |_| true)?;

    write_comments(writer, order, "materials", None)?;
    writer.write_event(Event::End(BytesEnd::new("materials")))?;
    Ok(())
}

/// A `<position>`, `<rotation>` or `<scale>`: its name when it has one, then
/// the components and unit that are set.
fn write_xyz(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    tag: &str,
    name: &str,
    xyz: [&Option<String>; 3],
    unit: &Option<String>,
) -> Result<()> {
    let mut elem = BytesStart::new(tag);
    if !name.is_empty() {
        elem.push_attribute(("name", name));
    }
    for (key, value) in ["x", "y", "z", "unit"]
        .into_iter()
        .zip(xyz.into_iter().chain([unit]))
    {
        push_opt(&mut elem, key, value);
    }
    writer.write_event(Event::Empty(elem))?;
    Ok(())
}

/// Push `key` when `value` is set.
fn push_opt(elem: &mut BytesStart, key: &str, value: &Option<String>) {
    if let Some(value) = value {
        elem.push_attribute((key, value.as_str()));
    }
}

/// `<tag value=".."/>` with the `unit` and `type` that are set.
fn write_value(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    tag: &str,
    value: &str,
    unit: &Option<String>,
    r#type: &Option<String>,
) -> Result<()> {
    let mut elem = BytesStart::new(tag);
    elem.push_attribute(("value", value));
    push_opt(&mut elem, "unit", unit);
    push_opt(&mut elem, "type", r#type);
    writer.write_event(Event::Empty(elem))?;
    Ok(())
}

/// `<fraction n=".." ref=".."/>` or `<composite ..>`.
fn write_component(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    tag: &str,
    n: &str,
    ref_name: &str,
) -> Result<()> {
    let mut elem = BytesStart::new(tag);
    elem.push_attribute(("n", n));
    elem.push_attribute(("ref", ref_name));
    writer.write_event(Event::Empty(elem))?;
    Ok(())
}

/// `<tag ref="name"/>`.
fn write_ref(writer: &mut Writer<Cursor<Vec<u8>>>, tag: &str, name: &str) -> Result<()> {
    let mut elem = BytesStart::new(tag);
    elem.push_attribute(("ref", name));
    writer.write_event(Event::Empty(elem))?;
    Ok(())
}

/// A placement's position: inline as `<tag>`, or by reference as `<tagref>`.
fn write_placement_pos(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    pos: &Option<PlacementPos>,
    tag: &str,
) -> Result<()> {
    match pos {
        Some(PlacementPos::Inline(p)) => {
            write_xyz(writer, tag, &p.name, [&p.x, &p.y, &p.z], &p.unit)
        }
        Some(PlacementPos::Ref(name)) => write_ref(writer, &format!("{tag}ref"), name),
        None => Ok(()),
    }
}

/// A placement's rotation: inline as `<tag>`, or by reference as `<tagref>`.
fn write_placement_rot(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    rot: &Option<PlacementRot>,
    tag: &str,
) -> Result<()> {
    match rot {
        Some(PlacementRot::Inline(r)) => {
            write_xyz(writer, tag, &r.name, [&r.x, &r.y, &r.z], &r.unit)
        }
        Some(PlacementRot::Ref(name)) => write_ref(writer, &format!("{tag}ref"), name),
        None => Ok(()),
    }
}

/// A placement's `<scale>`, or `<scaleref>` by reference.
fn write_placement_scale(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    scale: &Option<PlacementScale>,
) -> Result<()> {
    match scale {
        Some(PlacementScale::Inline(s)) => {
            write_xyz(writer, "scale", &s.name, [&s.x, &s.y, &s.z], &None)
        }
        Some(PlacementScale::Ref(name)) => write_ref(writer, "scaleref", name),
        None => Ok(()),
    }
}

fn write_solids(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    solids: &SolidSection,
    raws: &mut Vec<&RawElement>,
    order: &DocumentOrder,
) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("solids")))?;
    write_raws(writer, raws, |after| after.is_none())?;

    for solid in &solids.solids {
        write_comments(writer, order, "solids", Some(solid.name()))?;
        match solid {
            Solid::Box(b) => {
                let mut elem = BytesStart::new("box");
                elem.push_attribute(("name", b.name.as_str()));
                elem.push_attribute(("x", b.x.as_str()));
                elem.push_attribute(("y", b.y.as_str()));
                elem.push_attribute(("z", b.z.as_str()));
                push_opt(&mut elem, "lunit", &b.lunit);
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Tube(t) => {
                let mut elem = BytesStart::new("tube");
                elem.push_attribute(("name", t.name.as_str()));
                push_opt(&mut elem, "rmin", &t.rmin);
                elem.push_attribute(("rmax", t.rmax.as_str()));
                elem.push_attribute(("z", t.z.as_str()));
                push_opt(&mut elem, "startphi", &t.startphi);
                push_opt(&mut elem, "deltaphi", &t.deltaphi);
                push_opt(&mut elem, "aunit", &t.aunit);
                push_opt(&mut elem, "lunit", &t.lunit);
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Cone(c) => {
                let mut elem = BytesStart::new("cone");
                elem.push_attribute(("name", c.name.as_str()));
                push_opt(&mut elem, "rmin1", &c.rmin1);
                elem.push_attribute(("rmax1", c.rmax1.as_str()));
                push_opt(&mut elem, "rmin2", &c.rmin2);
                elem.push_attribute(("rmax2", c.rmax2.as_str()));
                elem.push_attribute(("z", c.z.as_str()));
                push_opt(&mut elem, "startphi", &c.startphi);
                push_opt(&mut elem, "deltaphi", &c.deltaphi);
                push_opt(&mut elem, "aunit", &c.aunit);
                push_opt(&mut elem, "lunit", &c.lunit);
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Sphere(s) => {
                let mut elem = BytesStart::new("sphere");
                elem.push_attribute(("name", s.name.as_str()));
                push_opt(&mut elem, "rmin", &s.rmin);
                elem.push_attribute(("rmax", s.rmax.as_str()));
                push_opt(&mut elem, "startphi", &s.startphi);
                push_opt(&mut elem, "deltaphi", &s.deltaphi);
                push_opt(&mut elem, "starttheta", &s.starttheta);
                push_opt(&mut elem, "deltatheta", &s.deltatheta);
                push_opt(&mut elem, "aunit", &s.aunit);
                push_opt(&mut elem, "lunit", &s.lunit);
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
                push_opt(&mut elem, "lunit", &t.lunit);
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Polycone(pc) => {
                let mut elem = BytesStart::new("polycone");
                elem.push_attribute(("name", pc.name.as_str()));
                push_opt(&mut elem, "startphi", &pc.startphi);
                push_opt(&mut elem, "deltaphi", &pc.deltaphi);
                push_opt(&mut elem, "aunit", &pc.aunit);
                push_opt(&mut elem, "lunit", &pc.lunit);
                writer.write_event(Event::Start(elem))?;
                for zp in &pc.zplanes {
                    let mut zelem = BytesStart::new("zplane");
                    zelem.push_attribute(("z", zp.z.as_str()));
                    push_opt(&mut zelem, "rmin", &zp.rmin);
                    zelem.push_attribute(("rmax", zp.rmax.as_str()));
                    writer.write_event(Event::Empty(zelem))?;
                }
                writer.write_event(Event::End(BytesEnd::new("polycone")))?;
            }
            Solid::Xtru(x) => {
                let mut elem = BytesStart::new("xtru");
                elem.push_attribute(("name", x.name.as_str()));
                push_opt(&mut elem, "lunit", &x.lunit);
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
                push_opt(&mut elem, "lunit", &ts.lunit);
                push_opt(&mut elem, "aunit", &ts.aunit);
                writer.write_event(Event::Start(elem))?;
                for facet in &ts.facets {
                    let (tag, vertices, r#type, lunit) = facet.parts();
                    let mut fe = BytesStart::new(tag);
                    for (i, v) in vertices.into_iter().enumerate() {
                        fe.push_attribute((format!("vertex{}", i + 1).as_str(), v));
                    }
                    if let Some(t) = r#type {
                        fe.push_attribute(("type", t));
                    }
                    if let Some(u) = lunit {
                        fe.push_attribute(("lunit", u));
                    }
                    writer.write_event(Event::Empty(fe))?;
                }
                writer.write_event(Event::End(BytesEnd::new("tessellated")))?;
            }
            Solid::Orb(o) => {
                let mut elem = BytesStart::new("orb");
                elem.push_attribute(("name", o.name.as_str()));
                elem.push_attribute(("r", o.r.as_str()));
                push_opt(&mut elem, "lunit", &o.lunit);
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Ellipsoid(e) => {
                let mut elem = BytesStart::new("ellipsoid");
                elem.push_attribute(("name", e.name.as_str()));
                elem.push_attribute(("ax", e.ax.as_str()));
                elem.push_attribute(("by", e.by.as_str()));
                elem.push_attribute(("cz", e.cz.as_str()));
                push_opt(&mut elem, "zcut1", &e.zcut1);
                push_opt(&mut elem, "zcut2", &e.zcut2);
                push_opt(&mut elem, "lunit", &e.lunit);
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::GenericPolyhedra(gp) => {
                let mut elem = BytesStart::new("genericPolyhedra");
                elem.push_attribute(("name", gp.name.as_str()));
                push_opt(&mut elem, "startphi", &gp.startphi);
                push_opt(&mut elem, "deltaphi", &gp.deltaphi);
                if !gp.numsides.is_empty() {
                    elem.push_attribute(("numsides", gp.numsides.as_str()));
                }
                push_opt(&mut elem, "aunit", &gp.aunit);
                push_opt(&mut elem, "lunit", &gp.lunit);
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
                push_opt(&mut elem, "lunit", &p.lunit);
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Elcone(e) => {
                let mut elem = BytesStart::new("elcone");
                elem.push_attribute(("name", e.name.as_str()));
                elem.push_attribute(("dx", e.dx.as_str()));
                elem.push_attribute(("dy", e.dy.as_str()));
                elem.push_attribute(("zmax", e.zmax.as_str()));
                elem.push_attribute(("zcut", e.zcut.as_str()));
                push_opt(&mut elem, "lunit", &e.lunit);
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Hype(h) => {
                let mut elem = BytesStart::new("hype");
                elem.push_attribute(("name", h.name.as_str()));
                push_opt(&mut elem, "rmin", &h.rmin);
                elem.push_attribute(("rmax", h.rmax.as_str()));
                push_opt(&mut elem, "inst", &h.inst);
                push_opt(&mut elem, "outst", &h.outst);
                elem.push_attribute(("z", h.z.as_str()));
                push_opt(&mut elem, "aunit", &h.aunit);
                push_opt(&mut elem, "lunit", &h.lunit);
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::GenericPolycone(gp) => {
                let mut elem = BytesStart::new("genericPolycone");
                elem.push_attribute(("name", gp.name.as_str()));
                push_opt(&mut elem, "startphi", &gp.startphi);
                push_opt(&mut elem, "deltaphi", &gp.deltaphi);
                push_opt(&mut elem, "aunit", &gp.aunit);
                push_opt(&mut elem, "lunit", &gp.lunit);
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
                push_opt(&mut elem, "lunit", &t.lunit);
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Eltube(e) => {
                let mut elem = BytesStart::new("eltube");
                elem.push_attribute(("name", e.name.as_str()));
                elem.push_attribute(("dx", e.dx.as_str()));
                elem.push_attribute(("dy", e.dy.as_str()));
                elem.push_attribute(("dz", e.dz.as_str()));
                push_opt(&mut elem, "lunit", &e.lunit);
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Polyhedra(ph) => {
                let mut elem = BytesStart::new("polyhedra");
                elem.push_attribute(("name", ph.name.as_str()));
                push_opt(&mut elem, "startphi", &ph.startphi);
                push_opt(&mut elem, "deltaphi", &ph.deltaphi);
                if !ph.numsides.is_empty() {
                    elem.push_attribute(("numsides", ph.numsides.as_str()));
                }
                push_opt(&mut elem, "aunit", &ph.aunit);
                push_opt(&mut elem, "lunit", &ph.lunit);
                writer.write_event(Event::Start(elem))?;
                for zp in &ph.zplanes {
                    let mut zelem = BytesStart::new("zplane");
                    zelem.push_attribute(("z", zp.z.as_str()));
                    push_opt(&mut zelem, "rmin", &zp.rmin);
                    zelem.push_attribute(("rmax", zp.rmax.as_str()));
                    writer.write_event(Event::Empty(zelem))?;
                }
                writer.write_event(Event::End(BytesEnd::new("polyhedra")))?;
            }
            Solid::CutTube(ct) => {
                let mut elem = BytesStart::new("cutTube");
                elem.push_attribute(("name", ct.name.as_str()));
                push_opt(&mut elem, "rmin", &ct.rmin);
                elem.push_attribute(("rmax", ct.rmax.as_str()));
                elem.push_attribute(("z", ct.z.as_str()));
                push_opt(&mut elem, "startphi", &ct.startphi);
                push_opt(&mut elem, "deltaphi", &ct.deltaphi);
                push_opt(&mut elem, "lowX", &ct.low_x);
                push_opt(&mut elem, "lowY", &ct.low_y);
                push_opt(&mut elem, "lowZ", &ct.low_z);
                push_opt(&mut elem, "highX", &ct.high_x);
                push_opt(&mut elem, "highY", &ct.high_y);
                push_opt(&mut elem, "highZ", &ct.high_z);
                push_opt(&mut elem, "aunit", &ct.aunit);
                push_opt(&mut elem, "lunit", &ct.lunit);
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Para(p) => {
                let mut elem = BytesStart::new("para");
                elem.push_attribute(("name", p.name.as_str()));
                elem.push_attribute(("x", p.x.as_str()));
                elem.push_attribute(("y", p.y.as_str()));
                elem.push_attribute(("z", p.z.as_str()));
                push_opt(&mut elem, "alpha", &p.alpha);
                push_opt(&mut elem, "theta", &p.theta);
                push_opt(&mut elem, "phi", &p.phi);
                push_opt(&mut elem, "aunit", &p.aunit);
                push_opt(&mut elem, "lunit", &p.lunit);
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Trap(t) => {
                let mut elem = BytesStart::new("trap");
                elem.push_attribute(("name", t.name.as_str()));
                elem.push_attribute(("z", t.z.as_str()));
                push_opt(&mut elem, "theta", &t.theta);
                push_opt(&mut elem, "phi", &t.phi);
                elem.push_attribute(("y1", t.y1.as_str()));
                elem.push_attribute(("x1", t.x1.as_str()));
                elem.push_attribute(("x2", t.x2.as_str()));
                push_opt(&mut elem, "alpha1", &t.alpha1);
                elem.push_attribute(("y2", t.y2.as_str()));
                elem.push_attribute(("x3", t.x3.as_str()));
                elem.push_attribute(("x4", t.x4.as_str()));
                push_opt(&mut elem, "alpha2", &t.alpha2);
                push_opt(&mut elem, "aunit", &t.aunit);
                push_opt(&mut elem, "lunit", &t.lunit);
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Torus(t) => {
                let mut elem = BytesStart::new("torus");
                elem.push_attribute(("name", t.name.as_str()));
                push_opt(&mut elem, "rmin", &t.rmin);
                elem.push_attribute(("rmax", t.rmax.as_str()));
                elem.push_attribute(("rtor", t.rtor.as_str()));
                push_opt(&mut elem, "startphi", &t.startphi);
                push_opt(&mut elem, "deltaphi", &t.deltaphi);
                push_opt(&mut elem, "aunit", &t.aunit);
                push_opt(&mut elem, "lunit", &t.lunit);
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::TwistedTubs(t) => {
                let mut elem = BytesStart::new("twistedtubs");
                elem.push_attribute(("name", t.name.as_str()));
                elem.push_attribute(("twistedangle", t.twistedangle.as_str()));
                push_opt(&mut elem, "endinnerrad", &t.endinnerrad);
                push_opt(&mut elem, "endouterrad", &t.endouterrad);
                push_opt(&mut elem, "zlen", &t.zlen);
                push_opt(&mut elem, "phi", &t.phi);
                // The zlen == 0 parameterisation.
                push_opt(&mut elem, "midinnerrad", &t.midinnerrad);
                push_opt(&mut elem, "midouterrad", &t.midouterrad);
                push_opt(&mut elem, "negativeEndz", &t.negative_endz);
                push_opt(&mut elem, "positiveEndz", &t.positive_endz);
                push_opt(&mut elem, "nseg", &t.nseg);
                push_opt(&mut elem, "totphi", &t.totphi);
                push_opt(&mut elem, "aunit", &t.aunit);
                push_opt(&mut elem, "lunit", &t.lunit);
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
                push_opt(&mut elem, "aunit", &t.aunit);
                push_opt(&mut elem, "lunit", &t.lunit);
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
                push_opt(&mut elem, "aunit", &t.aunit);
                push_opt(&mut elem, "lunit", &t.lunit);
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::TwistedBox(t) => {
                let mut elem = BytesStart::new("twistedbox");
                elem.push_attribute(("name", t.name.as_str()));
                elem.push_attribute(("PhiTwist", t.phi_twist.as_str()));
                elem.push_attribute(("x", t.x.as_str()));
                elem.push_attribute(("y", t.y.as_str()));
                elem.push_attribute(("z", t.z.as_str()));
                push_opt(&mut elem, "aunit", &t.aunit);
                push_opt(&mut elem, "lunit", &t.lunit);
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
                push_opt(&mut elem, "lunit", &a.lunit);
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::MultiUnion(mu) => {
                let mut elem = BytesStart::new("multiUnion");
                elem.push_attribute(("name", mu.name.as_str()));
                writer.write_event(Event::Start(elem))?;
                for node in &mu.nodes {
                    let mut node_elem = BytesStart::new("multiUnionNode");
                    push_opt(&mut node_elem, "name", &node.name);
                    writer.write_event(Event::Start(node_elem))?;
                    write_ref(writer, "solid", &node.solid_ref)?;
                    write_placement_pos(writer, &node.position, "position")?;
                    write_placement_rot(writer, &node.rotation, "rotation")?;
                    writer.write_event(Event::End(BytesEnd::new("multiUnionNode")))?;
                }
                writer.write_event(Event::End(BytesEnd::new("multiUnion")))?;
            }
            Solid::Reflected(rs) => {
                let mut elem = BytesStart::new("reflectedSolid");
                elem.push_attribute(("name", rs.name.as_str()));
                elem.push_attribute(("solid", rs.solid_ref.as_str()));
                elem.push_attribute(("sx", rs.sx.as_str()));
                elem.push_attribute(("sy", rs.sy.as_str()));
                elem.push_attribute(("sz", rs.sz.as_str()));
                elem.push_attribute(("rx", rs.rx.as_str()));
                elem.push_attribute(("ry", rs.ry.as_str()));
                elem.push_attribute(("rz", rs.rz.as_str()));
                elem.push_attribute(("dx", rs.dx.as_str()));
                elem.push_attribute(("dy", rs.dy.as_str()));
                elem.push_attribute(("dz", rs.dz.as_str()));
                push_opt(&mut elem, "aunit", &rs.aunit);
                push_opt(&mut elem, "lunit", &rs.lunit);
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Scaled(ss) => {
                let mut elem = BytesStart::new("scaledSolid");
                elem.push_attribute(("name", ss.name.as_str()));
                writer.write_event(Event::Start(elem))?;
                write_ref(writer, "solidref", &ss.solid_ref)?;
                write_placement_scale(writer, &ss.scale)?;
                writer.write_event(Event::End(BytesEnd::new("scaledSolid")))?;
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

                write_ref(writer, "first", &bs.first_ref)?;
                write_ref(writer, "second", &bs.second_ref)?;
                write_placement_pos(writer, &bs.position, "position")?;
                write_placement_rot(writer, &bs.rotation, "rotation")?;
                write_placement_pos(writer, &bs.first_position, "firstposition")?;
                write_placement_rot(writer, &bs.first_rotation, "firstrotation")?;

                writer.write_event(Event::End(BytesEnd::new(tag_name)))?;
            }
        }
        write_raws(writer, raws, |after| after == Some(solid.name()))?;
    }
    write_raws(writer, raws, |_| true)?;

    write_comments(writer, order, "solids", None)?;
    writer.write_event(Event::End(BytesEnd::new("solids")))?;
    Ok(())
}

/// Emit the comments anchored before `before` in `section`.
///
/// `BytesText::from_escaped` writes the body verbatim; `BytesText::new` would
/// escape it and turn a `<` inside a comment into `&lt;`. Going through
/// `write_event` rather than `write_raw` also keeps the writer's indentation
/// state correct.
fn write_comments(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    order: &DocumentOrder,
    section: &str,
    before: Option<&str>,
) -> Result<()> {
    for anchor in &order.anchors {
        if anchor.section == section && anchor.before.as_deref() == before {
            writer.write_event(Event::Comment(BytesText::from_escaped(&anchor.text)))?;
        }
    }
    Ok(())
}

/// Write an `<auxiliary>`, recursing into nested children.
///
/// Emitted self-closing when it has no children, so files that never used the
/// nested form round-trip byte-identically.
fn write_auxiliary(writer: &mut Writer<Cursor<Vec<u8>>>, aux: &Auxiliary) -> Result<()> {
    let mut ae = BytesStart::new("auxiliary");
    ae.push_attribute(("auxtype", aux.auxtype.as_str()));
    ae.push_attribute(("auxvalue", aux.auxvalue.as_str()));
    push_opt(&mut ae, "auxunit", &aux.auxunit);

    if aux.children.is_empty() {
        writer.write_event(Event::Empty(ae))?;
    } else {
        writer.write_event(Event::Start(ae))?;
        for child in &aux.children {
            write_auxiliary(writer, child)?;
        }
        writer.write_event(Event::End(BytesEnd::new("auxiliary")))?;
    }
    Ok(())
}

/// Export order for materials: each after the materials its components name.
/// `G4GDMLReadMaterials` resolves a component while reading its material, and
/// a name that is also an element means the element, so only the others
/// constrain the order.
pub fn material_order(materials: &MaterialSection) -> Result<Vec<usize>> {
    let index: HashMap<&str, usize> = materials
        .materials
        .iter()
        .enumerate()
        .map(|(i, m)| (m.name.as_str(), i))
        .collect();
    let deps: Vec<Vec<usize>> = materials
        .materials
        .iter()
        .map(|m| {
            m.components
                .iter()
                .filter_map(|c| {
                    let (MaterialComponent::Fraction { ref_name, .. }
                    | MaterialComponent::Composite { ref_name, .. }) = c;
                    if materials.elements.iter().any(|e| &e.name == ref_name) {
                        return None;
                    }
                    index.get(ref_name.as_str()).copied()
                })
                .collect()
        })
        .collect();
    dependency_order(&deps, "material", |i| &materials.materials[i].name)
}

/// Stable dependency order: Geant4 resolves a volumeref while reading its parent.
fn ordered_volumes(structure: &StructureSection) -> Result<Vec<&Volume>> {
    let volumes = &structure.volumes;
    let index: HashMap<&str, usize> = volumes
        .iter()
        .enumerate()
        .map(|(i, v)| (v.name.as_str(), i))
        .collect();
    let mut deps = Vec::with_capacity(volumes.len());
    for volume in volumes {
        let mut refs: Vec<String> = volume
            .physvols
            .iter()
            .filter(|pv| pv.file_ref.is_none())
            .map(|pv| pv.volume_ref.clone())
            .collect();
        if let Some(replica) = &volume.replica {
            refs.push(replica.volume_ref.clone());
        }
        // A placement inside a preserved element has the same dependency as a
        // direct physvol, even though a loop must not be expanded here.
        for raw in &volume.preserved {
            let mut reader = quick_xml::Reader::from_str(&raw.xml);
            loop {
                match reader.read_event()? {
                    Event::Empty(e) | Event::Start(e)
                        if e.local_name().as_ref() == b"volumeref" =>
                    {
                        for attr in e.attributes() {
                            let attr = attr?;
                            if attr.key.as_ref() == b"ref" {
                                refs.push(
                                    attr.decode_and_unescape_value(reader.decoder())?
                                        .into_owned(),
                                );
                            }
                        }
                    }
                    Event::Eof => break,
                    _ => {}
                }
            }
        }
        deps.push(
            refs.iter()
                .filter_map(|name| index.get(name.as_str()).copied())
                .collect(),
        );
    }
    let order = dependency_order(&deps, "volume", |i| &volumes[i].name)?;
    Ok(order.into_iter().map(|i| &volumes[i]).collect())
}

fn write_structure(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    structure: &StructureSection,
    raws: &mut Vec<&RawElement>,
    order: &DocumentOrder,
) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("structure")))?;
    write_raws(writer, raws, |after| after.is_none())?;

    for vol in ordered_volumes(structure)? {
        write_comments(writer, order, "structure", Some(&vol.name))?;
        let tag = if vol.assembly { "assembly" } else { "volume" };
        let mut elem = BytesStart::new(tag);
        elem.push_attribute(("name", vol.name.as_str()));
        writer.write_event(Event::Start(elem))?;

        for text in &vol.body_comments {
            writer.write_event(Event::Comment(BytesText::from_escaped(text)))?;
        }

        if !vol.assembly {
            write_ref(writer, "materialref", &vol.material_ref)?;
            write_ref(writer, "solidref", &vol.solid_ref)?;
        }

        for pv in &vol.physvols {
            let mut pv_elem = BytesStart::new("physvol");
            push_opt(&mut pv_elem, "name", &pv.name);
            push_opt(&mut pv_elem, "copynumber", &pv.copynumber);
            writer.write_event(Event::Start(pv_elem))?;

            if let Some(ref fref) = pv.file_ref {
                let mut fe = BytesStart::new("file");
                fe.push_attribute(("name", fref.name.as_str()));
                push_opt(&mut fe, "volname", &fref.volname);
                writer.write_event(Event::Empty(fe))?;
            } else {
                write_ref(writer, "volumeref", &pv.volume_ref)?;
            }
            write_placement_pos(writer, &pv.position, "position")?;
            write_placement_rot(writer, &pv.rotation, "rotation")?;
            write_placement_scale(writer, &pv.scale)?;

            writer.write_event(Event::End(BytesEnd::new("physvol")))?;
        }

        if let Some(ref replica) = vol.replica {
            let mut rv = BytesStart::new("replicavol");
            rv.push_attribute(("number", replica.number.as_str()));
            writer.write_event(Event::Start(rv))?;

            write_ref(writer, "volumeref", &replica.volume_ref)?;

            writer.write_event(Event::Start(BytesStart::new("replicate_along_axis")))?;

            write_xyz(writer, "direction", "", replica.direction.each_ref(), &None)?;

            write_value(writer, "width", &replica.width, &replica.width_unit, &None)?;
            write_value(
                writer,
                "offset",
                &replica.offset,
                &replica.offset_unit,
                &None,
            )?;

            writer.write_event(Event::End(BytesEnd::new("replicate_along_axis")))?;
            writer.write_event(Event::End(BytesEnd::new("replicavol")))?;
        }

        for aux in &vol.auxiliaries {
            write_auxiliary(writer, aux)?;
        }

        // Verbatim, after the physvols. GDML places no ordering constraint
        // between placements, and Geant4 reads them in one pass, so the
        // interleaving with <physvol> is not preserved; the elements are.
        for raw in &vol.preserved {
            write_raw(writer, raw)?;
        }

        writer.write_event(Event::End(BytesEnd::new(tag)))?;
        write_raws(writer, raws, |after| after == Some(vol.name.as_str()))?;
    }
    write_raws(writer, raws, |_| true)?;

    write_comments(writer, order, "structure", None)?;
    writer.write_event(Event::End(BytesEnd::new("structure")))?;
    Ok(())
}

fn write_setup(writer: &mut Writer<Cursor<Vec<u8>>>, setup: &SetupSection) -> Result<()> {
    let mut elem = BytesStart::new("setup");
    elem.push_attribute(("name", setup.name.as_str()));
    elem.push_attribute(("version", setup.version.as_str()));
    writer.write_event(Event::Start(elem))?;

    write_ref(writer, "world", &setup.world_ref)?;

    writer.write_event(Event::End(BytesEnd::new("setup")))?;
    Ok(())
}
