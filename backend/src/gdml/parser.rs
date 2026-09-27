use anyhow::{bail, Context, Result};
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use quick_xml::Writer;
use std::path::Path;

use super::model::*;

pub fn parse_gdml(path: &Path) -> Result<GdmlDocument> {
    let raw = std::fs::read(path).context("Failed to read GDML file")?;
    let filename = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    parse_gdml_from_bytes(&raw, filename)
}

/// Parse GDML from raw bytes with a given filename.
pub fn parse_gdml_from_bytes(raw: &[u8], filename: String) -> Result<GdmlDocument> {
    let mut reader = Reader::from_reader(raw);
    reader.config_mut().trim_text(true);

    let mut defines = DefineSection::default();
    let mut materials = MaterialSection::default();
    let mut solids = SolidSection::default();
    let mut structure = StructureSection::default();
    let mut setups: Vec<SetupSection> = Vec::new();
    // Elements recognized by name but not interpreted; preserved verbatim.
    let mut raw_unknown: Vec<RawElement> = Vec::new();
    let mut root_attributes: Vec<(String, String)> = Vec::new();
    // Names of defines seen inside <materials><define>, so the writer can put
    // them back there instead of folding them into the top-level block.
    let mut materials_define: Option<Vec<String>> = None;
    let mut order = DocumentOrder::default();
    // Comments seen since the last element; they anchor to whatever comes next.
    let mut pending_comments: Vec<String> = Vec::new();
    let mut seen_root = false;
    // Unsupported constructs that had to be dropped entirely (warn the user).
    let mut skipped_unsupported: Vec<String> = Vec::new();

    let mut section = Section::None;
    let mut buf = Vec::new();

    loop {
        buf.clear();
        let event = reader.read_event_into(&mut buf);
        // A self-closed element has no body and no end tag: reading one for it
        // would consume the enclosing element's children instead.
        let open = matches!(event, Ok(Event::Start(_)));
        match event {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                let local = e.local_name();
                let tag = local.as_ref();
                anchor_pending_comments(&mut pending_comments, &mut order, section, e, tag);
                note_define(section, &mut order, &mut materials_define, e, tag);
                let in_define = matches!(section, Section::Define | Section::MaterialsDefine);
                let in_materials = section == Section::Materials;
                let in_solids = section == Section::Solids;
                match tag {
                    b"define" => {
                        if in_materials {
                            // Present-but-empty is meaningful: it still has to
                            // round-trip as an empty <define/>.
                            materials_define.get_or_insert_with(Vec::new);
                            if open {
                                section = Section::MaterialsDefine;
                            }
                        } else if open {
                            section = Section::Define;
                        }
                    }
                    b"materials" if open => section = Section::Materials,
                    b"solids" if open => section = Section::Solids,
                    b"structure" if open => section = Section::Structure,
                    // Capture the root's own attributes so the writer can
                    // reproduce them instead of substituting a hardcoded pair.
                    b"gdml" => {
                        seen_root = true;
                        root_attributes = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .map(|a| {
                                // Unescape on read; the writer re-escapes, which
                                // preserves the no-double-escape invariant that
                                // `get_attr` documents.
                                let raw = String::from_utf8_lossy(&a.value);
                                (
                                    String::from_utf8_lossy(a.key.as_ref()).to_string(),
                                    quick_xml::escape::unescape(&raw)
                                        .map(|c| c.into_owned())
                                        .unwrap_or_else(|_| raw.into_owned()),
                                )
                            })
                            .collect();
                    }
                    b"constant" if in_define => parse_constant(e, &mut defines),
                    b"quantity" if in_define => parse_quantity(e, &mut defines),
                    b"variable" if in_define => parse_variable(e, &mut defines),
                    b"expression" if in_define => {
                        let text = if open {
                            reader.read_text(e.name())?.into_owned()
                        } else {
                            String::new()
                        };
                        defines.expressions.push(Expression {
                            name: get_attr(e, "name").unwrap_or_default(),
                            value: collapse_spaces(&text),
                        });
                    }
                    b"position" if in_define => defines.positions.push(position_from(e)),
                    b"rotation" if in_define => defines.rotations.push(rotation_from(e)),
                    b"scale" if in_define => parse_scale(e, &mut defines),
                    b"isotope" if in_materials => {
                        read_isotope(&mut reader, e, open, &mut materials)?;
                    }
                    b"element" if in_materials => {
                        read_element(&mut reader, e, open, &mut materials)?;
                    }
                    b"material" if in_materials => {
                        read_material(&mut reader, e, open, &mut materials)?;
                    }
                    b"box" if in_solids => parse_box_solid(e, &mut solids),
                    b"tube" if in_solids => parse_tube_solid(e, &mut solids),
                    b"cone" if in_solids => parse_cone_solid(e, &mut solids),
                    b"sphere" if in_solids => parse_sphere_solid(e, &mut solids),
                    b"trd" if in_solids => parse_trd_solid(e, &mut solids),
                    b"orb" if in_solids => parse_orb_solid(e, &mut solids),
                    b"torus" if in_solids => parse_torus_solid(e, &mut solids),
                    b"trap" if in_solids => parse_trap_solid(e, &mut solids),
                    b"para" if in_solids => parse_para_solid(e, &mut solids),
                    b"cutTube" if in_solids => parse_cut_tube_solid(e, &mut solids),
                    b"ellipsoid" if in_solids => parse_ellipsoid_solid(e, &mut solids),
                    b"eltube" if in_solids => parse_eltube_solid(e, &mut solids),
                    b"tet" if in_solids => parse_tet_solid(e, &mut solids),
                    b"hype" if in_solids => parse_hype_solid(e, &mut solids),
                    b"elcone" if in_solids => parse_elcone_solid(e, &mut solids),
                    b"paraboloid" if in_solids => parse_paraboloid_solid(e, &mut solids),
                    b"arb8" if in_solids => parse_arb8_solid(e, &mut solids),
                    b"twistedtubs" if in_solids => parse_twisted_tubs_solid(e, &mut solids),
                    b"twistedbox" if in_solids => parse_twisted_box_solid(e, &mut solids),
                    b"twistedtrap" if in_solids => parse_twisted_trap_solid(e, &mut solids),
                    b"twistedtrd" if in_solids => parse_twisted_trd_solid(e, &mut solids),
                    b"reflectedSolid" if in_solids => parse_reflected_solid(e, &mut solids),
                    b"polycone" if in_solids => {
                        let solid = read_polycone_body(&mut reader, e, open)?;
                        solids.solids.push(Solid::Polycone(solid));
                    }
                    b"genericPolycone" if in_solids => {
                        let solid = read_generic_polycone_body(&mut reader, e, open)?;
                        solids.solids.push(Solid::GenericPolycone(solid));
                    }
                    b"xtru" if in_solids => {
                        let solid = read_xtru_body(&mut reader, e, open)?;
                        solids.solids.push(Solid::Xtru(solid));
                    }
                    b"tessellated" if in_solids => {
                        let solid = read_tessellated_body(&mut reader, e, open)?;
                        solids.solids.push(Solid::Tessellated(solid));
                    }
                    b"polyhedra" if in_solids => {
                        let solid = read_polyhedra_body(&mut reader, e, open)?;
                        solids.solids.push(Solid::Polyhedra(solid));
                    }
                    b"genericPolyhedra" if in_solids => {
                        let solid = read_generic_polyhedra_body(&mut reader, e, open)?;
                        solids.solids.push(Solid::GenericPolyhedra(solid));
                    }
                    b"multiUnion" if in_solids => {
                        let solid = read_multiunion_body(&mut reader, e, open)?;
                        solids.solids.push(Solid::MultiUnion(solid));
                    }
                    b"scaledSolid" if in_solids => {
                        let solid = read_scaled_solid_body(&mut reader, e, open)?;
                        solids.solids.push(Solid::Scaled(solid));
                    }
                    b"subtraction" | b"union" | b"intersection" if in_solids => {
                        let op = match tag {
                            b"union" => BooleanOp::Union,
                            b"intersection" => BooleanOp::Intersection,
                            _ => BooleanOp::Subtraction,
                        };
                        let solid = read_boolean_solid_body(&mut reader, e, open, op)?;
                        solids.solids.push(Solid::Boolean(solid));
                    }
                    b"volume" if section == Section::Structure => {
                        let volume =
                            read_volume_body(&mut reader, e, open, &mut skipped_unsupported)?;
                        structure.volumes.push(volume);
                    }
                    // Recognized-but-uninterpreted constructs: preserve verbatim so
                    // they survive a load -> save round-trip (and warn the user).
                    b"opticalsurface" | b"skinsurface" | b"bordersurface" | b"userinfo"
                    | b"loop" | b"assembly" | b"matrix" => {
                        let after = match section {
                            Section::Define | Section::MaterialsDefine => {
                                order.define_slots.last().map(|s| s.name.clone())
                            }
                            // The writer groups isotopes, elements, materials.
                            Section::Materials => materials
                                .materials
                                .last()
                                .map(|m| &m.name)
                                .or_else(|| materials.elements.last().map(|e| &e.name))
                                .or_else(|| materials.isotopes.last().map(|i| &i.name))
                                .cloned(),
                            Section::Solids => solids.solids.last().map(|s| s.name().to_string()),
                            Section::Structure => structure.volumes.last().map(|v| v.name.clone()),
                            _ => None,
                        };
                        let xml = capture_raw(&mut reader, e, open)?;
                        raw_unknown.push(RawElement {
                            tag: String::from_utf8_lossy(tag).to_string(),
                            section: Some(section_key(section)),
                            after,
                            xml,
                        });
                    }
                    b"setup" => {
                        let mut world_ref = get_attr(e, "world").unwrap_or_default();
                        for_each_child(&mut reader, e, open, |_, c, _| {
                            if c.local_name().as_ref() == b"world" {
                                world_ref = ref_attr(c);
                            }
                            Ok(())
                        })?;
                        setups.push(SetupSection {
                            name: get_attr(e, "name").unwrap_or_default(),
                            version: get_attr(e, "version").unwrap_or_else(|| "1.0".to_string()),
                            world_ref,
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                // Comments with nothing after them in this section anchor to its
                // end rather than being lost.
                if !pending_comments.is_empty()
                    && matches!(
                        local.as_ref(),
                        b"define" | b"materials" | b"solids" | b"structure" | b"gdml"
                    )
                {
                    let sect = if local.as_ref() == b"gdml" {
                        "root".to_string()
                    } else {
                        section_key(section)
                    };
                    for text in pending_comments.drain(..) {
                        order.anchors.push(CommentAnchor {
                            section: sect.clone(),
                            before: None,
                            text,
                        });
                    }
                }
                match local.as_ref() {
                    b"define" => {
                        if section == Section::MaterialsDefine {
                            section = Section::Materials;
                        } else if section == Section::Define {
                            section = Section::None;
                        }
                    }
                    b"materials" => section = Section::None,
                    b"solids" => section = Section::None,
                    b"structure" => section = Section::None,
                    _ => {}
                }
            }
            // Comments are content: the user wrote them, and export used to
            // delete every one (155 across 7 of the 10 shipped samples).
            // `from_utf8_lossy` without unescaping, deliberately — comment bodies
            // are not XML-escaped, so unescaping would corrupt a literal `&amp;`.
            Ok(Event::Comment(ref e)) => {
                let text = String::from_utf8_lossy(e).to_string();
                if seen_root {
                    pending_comments.push(text);
                } else {
                    order.prolog.push(text);
                }
            }
            // Preserve the DOCTYPE rather than deleting it. Entity references
            // are deliberately NOT expanded:
            //
            // - internal literal entities could be substituted, but doing so
            //   would bake the value into the exported file and destroy the
            //   declaration's purpose;
            // - external `SYSTEM` entities pull in another file, and resolving
            //   those server-side on an upload endpoint is textbook XXE. The
            //   sanctioned mechanism for modular geometry here is
            //   `<physvol><file/></physvol>` plus multi-file upload.
            //
            // So the declaration round-trips, and a file that actually uses
            // `&name;` gets a warning instead of silent misbehaviour.
            Ok(Event::DocType(ref e)) => {
                let text = String::from_utf8_lossy(e).to_string();
                if text.contains("SYSTEM") || text.contains("PUBLIC") {
                    skipped_unsupported.push(
                        "This file declares an EXTERNAL entity in its <!DOCTYPE>. Its contents \
                         are NOT loaded: geometry defined in the referenced file is missing from \
                         the view and from the export. Use \"Open File(s)\" with multiple files \
                         and <physvol><file/></physvol> instead."
                            .to_string(),
                    );
                } else if text.contains("<!ENTITY") {
                    skipped_unsupported.push(
                        "This file declares entities in its <!DOCTYPE>. The declaration is \
                         preserved, but entity references such as &name; are NOT expanded, so \
                         any dimension written that way evaluates as 0."
                            .to_string(),
                    );
                }
                order.doctype = Some(text);
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "XML parse error at pos {}: {}",
                    reader.buffer_position(),
                    e
                ))
            }
            _ => {}
        }
    }

    Ok(GdmlDocument {
        filename,
        defines,
        materials,
        solids,
        structure,
        // A file may carry several <setup> blocks. Overwriting meant the LAST
        // one won; Geant4's G4GDMLParser::Read asks for the one named "Default",
        // so a file with an alternative setup listed after the default rendered
        // the wrong world volume.
        setup: setups
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case("Default"))
            .or_else(|| setups.first())
            .cloned()
            .unwrap_or(SetupSection {
                name: "default".to_string(),
                version: "1.0".to_string(),
                world_ref: String::new(),
            }),
        setups,
        raw_unknown,
        order: {
            // Anything still pending sits after </gdml>.
            order.epilog.extend(pending_comments);
            order
        },
        root_attributes,
        materials_define,
        skipped_unsupported,
    })
}

/// Capture a recognized-but-uninterpreted element verbatim, including all
/// nested content, so it can be re-emitted unchanged on save.
fn capture_raw(reader: &mut Reader<&[u8]>, start: &BytesStart, open: bool) -> Result<String> {
    let mut w = Writer::new(Vec::new());
    if !open {
        w.write_event(Event::Empty(start.borrow()))?;
        return Ok(String::from_utf8(w.into_inner())?);
    }
    let tag_name = start.name().as_ref().to_vec();
    w.write_event(Event::Start(start.borrow()))?;
    let mut depth = 1usize;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                if e.name().as_ref() == tag_name.as_slice() {
                    depth += 1;
                }
                w.write_event(Event::Start(e))?;
            }
            Event::End(e) => {
                let matches_root = e.name().as_ref() == tag_name.as_slice();
                w.write_event(Event::End(e))?;
                if matches_root {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
            }
            Event::Eof => break,
            other => w.write_event(other)?,
        }
    }
    Ok(String::from_utf8(w.into_inner())?)
}

/// Call `f` for every element inside `parent` up to its end tag, with whether
/// that element is open (`<x>…</x>`) rather than self-closed (`<x/>`).
///
/// The one child loop the body readers share, so malformed XML inside any
/// element is an error instead of a silently truncated element. A self-closed
/// parent has no children and no end tag, so nothing is read for it. Nested
/// elements `f` does not consume itself are visited too, as flat children.
fn for_each_child(
    reader: &mut Reader<&[u8]>,
    parent: &BytesStart,
    open: bool,
    mut f: impl FnMut(&mut Reader<&[u8]>, &BytesStart, bool) -> Result<()>,
) -> Result<()> {
    if !open {
        return Ok(());
    }
    let end = parent.local_name().as_ref().to_vec();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => f(reader, &e, true)?,
            Ok(Event::Empty(e)) => f(reader, &e, false)?,
            Ok(Event::End(e)) if e.local_name().as_ref() == end.as_slice() => return Ok(()),
            Ok(Event::Eof) => return Ok(()),
            Ok(_) => {}
            Err(err) => bail!("XML error in <{}>: {err}", String::from_utf8_lossy(&end)),
        }
    }
}

// ─── Attribute helpers ───────────────────────────────────────────────────────

/// Which top-level GDML section the parser is currently inside.
///
/// Module-scoped rather than local to the parse loop so the comment-anchoring
/// helpers below can take it.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Section {
    None,
    Define,
    Materials,
    MaterialsDefine,
    Solids,
    Structure,
}

/// Record a `<define>` child: its position among its siblings, and whether it
/// came from the nested `<materials><define>` rather than the top-level block.
///
/// Order matters because `G4GDMLReadDefine::DefineRead` reads the children in
/// one forward pass and evaluates each `value` as it goes, so emitting them
/// regrouped by type can put a define ahead of one it references.
///
/// Called for every element at the top-level dispatch point; anything that is
/// not a define item inside a define block is ignored.
fn note_define(
    section: Section,
    order: &mut DocumentOrder,
    materials_slot: &mut Option<Vec<String>>,
    e: &BytesStart,
    tag: &[u8],
) {
    if !matches!(section, Section::Define | Section::MaterialsDefine) {
        return;
    }
    let kind = match tag {
        b"constant" => DefineKind::Constant,
        b"quantity" => DefineKind::Quantity,
        b"variable" => DefineKind::Variable,
        b"expression" => DefineKind::Expression,
        b"position" => DefineKind::Position,
        b"rotation" => DefineKind::Rotation,
        b"scale" => DefineKind::Scale,
        _ => return,
    };
    let Some(name) = get_attr(e, "name") else {
        return;
    };
    order.define_slots.push(DefineSlot {
        kind,
        name: name.clone(),
    });
    if section == Section::MaterialsDefine {
        if let Some(names) = materials_slot.as_mut() {
            names.push(name);
        }
    }
}

/// Name of the section a comment belongs to, for [`CommentAnchor::section`].
fn section_key(section: Section) -> String {
    match section {
        Section::Define | Section::MaterialsDefine => "define",
        Section::Materials => "materials",
        Section::Solids => "solids",
        Section::Structure => "structure",
        Section::None => "root",
    }
    .to_string()
}

/// Attach any comments seen since the last element to the element now starting.
///
/// Called from the single top-level dispatch point, which every named
/// define/material/solid/volume and every raw element passes through, so one
/// hook covers them all.
fn anchor_pending_comments(
    pending: &mut Vec<String>,
    order: &mut DocumentOrder,
    section: Section,
    e: &BytesStart,
    tag: &[u8],
) {
    if pending.is_empty() {
        return;
    }
    // A section element is itself the anchor, at root level -- that is where
    // the comments between </materials> and <solids> belong.
    let is_section = matches!(
        tag,
        b"define" | b"materials" | b"solids" | b"structure" | b"setup"
    );
    let (sect, before) = if is_section {
        (
            "root".to_string(),
            Some(String::from_utf8_lossy(tag).to_string()),
        )
    } else {
        match get_attr(e, "name") {
            Some(name) => (section_key(section), Some(name)),
            // Unnamed element: keep the comments pending so they attach to the
            // next named one rather than being silently dropped.
            None => return,
        }
    };

    for text in pending.drain(..) {
        order.anchors.push(CommentAnchor {
            section: sect.clone(),
            before: before.clone(),
            text,
        });
    }
}

fn get_attr(e: &BytesStart, name: &str) -> Option<String> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == name.as_bytes() {
            // quick_xml does NOT auto-unescape attribute values, so decode XML
            // entities here (e.g. "&amp;" -> "&"). The serializer re-escapes on
            // write, making the round-trip correct instead of double-escaping.
            // (GDML uploads are UTF-8; `unescape_value` is unavailable with the
            // `encoding` feature, so decode the bytes then unescape entities.)
            let raw = String::from_utf8_lossy(&attr.value);
            return Some(
                quick_xml::escape::unescape(&raw)
                    .map(|c| c.into_owned())
                    .unwrap_or_else(|_| raw.into_owned()),
            );
        }
    }
    None
}

fn get_attr_or(e: &BytesStart, name: &str, default: &str) -> String {
    get_attr(e, name).unwrap_or_else(|| default.to_string())
}

fn collapse_spaces(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_space {
                result.push(' ');
            }
            prev_space = true;
        } else {
            result.push(c);
            prev_space = false;
        }
    }
    result.trim().to_string()
}

// ─── Define parsers ──────────────────────────────────────────────────────────

fn parse_constant(e: &BytesStart, defines: &mut DefineSection) {
    defines.constants.push(Constant {
        name: get_attr(e, "name").unwrap_or_default(),
        value: get_attr(e, "value").unwrap_or_default(),
    });
}

fn parse_quantity(e: &BytesStart, defines: &mut DefineSection) {
    defines.quantities.push(Quantity {
        name: get_attr(e, "name").unwrap_or_default(),
        r#type: get_attr(e, "type"),
        value: get_attr(e, "value").unwrap_or_default(),
        unit: get_attr(e, "unit"),
    });
}

fn parse_variable(e: &BytesStart, defines: &mut DefineSection) {
    defines.variables.push(Variable {
        name: get_attr(e, "name").unwrap_or_default(),
        value: get_attr(e, "value").unwrap_or_default(),
    });
}

fn parse_scale(e: &BytesStart, defines: &mut DefineSection) {
    defines.scales.push(Scale {
        name: get_attr(e, "name").unwrap_or_default(),
        x: get_attr(e, "x"),
        y: get_attr(e, "y"),
        z: get_attr(e, "z"),
    });
}

/// A `<position>`, whether a define or inline in a placement.
fn position_from(e: &BytesStart) -> Position {
    Position {
        name: get_attr(e, "name").unwrap_or_default(),
        x: get_attr(e, "x"),
        y: get_attr(e, "y"),
        z: get_attr(e, "z"),
        unit: get_attr(e, "unit"),
    }
}

/// A `<rotation>`, whether a define or inline in a placement.
fn rotation_from(e: &BytesStart) -> Rotation {
    Rotation {
        name: get_attr(e, "name").unwrap_or_default(),
        x: get_attr(e, "x"),
        y: get_attr(e, "y"),
        z: get_attr(e, "z"),
        unit: get_attr(e, "unit"),
    }
}

/// The `ref` attribute of a reference such as `<solidref>` or `<positionref>`.
fn ref_attr(e: &BytesStart) -> String {
    get_attr(e, "ref").unwrap_or_default()
}

// ─── Material parsers ────────────────────────────────────────────────────────

fn read_element(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
    open: bool,
    materials: &mut MaterialSection,
) -> Result<()> {
    let mut element = Element {
        name: get_attr(e, "name").unwrap_or_default(),
        formula: get_attr(e, "formula"),
        z: get_attr(e, "Z"),
        atom_value: None,
        atom_unit: None,
        atom_type: None,
        fractions: Vec::new(),
    };
    for_each_child(reader, e, open, |_, c, _| {
        match c.local_name().as_ref() {
            b"atom" => {
                element.atom_value = get_attr(c, "value");
                element.atom_unit = get_attr(c, "unit");
                element.atom_type = get_attr(c, "type");
            }
            b"fraction" => element.fractions.push(Fraction {
                n: get_attr(c, "n").unwrap_or_default(),
                ref_name: ref_attr(c),
            }),
            _ => {}
        }
        Ok(())
    })?;
    materials.elements.push(element);
    Ok(())
}

fn read_isotope(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
    open: bool,
    materials: &mut MaterialSection,
) -> Result<()> {
    let mut isotope = Isotope {
        name: get_attr(e, "name").unwrap_or_default(),
        n: get_attr(e, "N"),
        z: get_attr(e, "Z"),
        atom_value: None,
        atom_unit: None,
        atom_type: None,
    };
    for_each_child(reader, e, open, |_, c, _| {
        if c.local_name().as_ref() == b"atom" {
            isotope.atom_value = get_attr(c, "value");
            isotope.atom_unit = get_attr(c, "unit");
            isotope.atom_type = get_attr(c, "type");
        }
        Ok(())
    })?;
    materials.isotopes.push(isotope);
    Ok(())
}

fn read_material(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
    open: bool,
    materials: &mut MaterialSection,
) -> Result<()> {
    let mut m = Material {
        name: get_attr(e, "name").unwrap_or_default(),
        formula: get_attr(e, "formula"),
        z: get_attr(e, "Z"),
        state: get_attr(e, "state"),
        density: None,
        density_ref: None,
        temperature: None,
        pressure: None,
        mee: None,
        atom_value: None,
        atom_unit: None,
        atom_type: None,
        properties: Vec::new(),
        rl: None,
        al: None,
        components: Vec::new(),
    };
    for_each_child(reader, e, open, |_, c, _| {
        let value = || PropertyValue {
            value: get_attr(c, "value").unwrap_or_default(),
            unit: get_attr(c, "unit"),
        };
        match c.local_name().as_ref() {
            b"D" => {
                m.density = Some(Density {
                    value: get_attr(c, "value").unwrap_or_default(),
                    unit: get_attr(c, "unit"),
                })
            }
            b"Dref" => m.density_ref = get_attr(c, "ref"),
            b"T" => m.temperature = Some(value()),
            b"P" => m.pressure = Some(value()),
            b"MEE" => m.mee = Some(value()),
            b"RL" => m.rl = Some(value()),
            b"AL" => m.al = Some(value()),
            b"property" => m.properties.push(MaterialProperty {
                name: get_attr(c, "name").unwrap_or_default(),
                ref_name: get_attr(c, "ref"),
                values: get_attr(c, "values"),
            }),
            b"atom" => {
                m.atom_value = get_attr(c, "value");
                m.atom_unit = get_attr(c, "unit");
                m.atom_type = get_attr(c, "type");
            }
            b"fraction" => m.components.push(MaterialComponent::Fraction {
                n: get_attr(c, "n").unwrap_or_default(),
                ref_name: ref_attr(c),
            }),
            b"composite" => m.components.push(MaterialComponent::Composite {
                n: get_attr(c, "n").unwrap_or_default(),
                ref_name: ref_attr(c),
            }),
            _ => {}
        }
        Ok(())
    })?;
    materials.materials.push(m);
    Ok(())
}

// ─── Solid parsers ───────────────────────────────────────────────────────────

fn parse_box_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::Box(BoxSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        x: get_attr_or(e, "x", "0"),
        y: get_attr_or(e, "y", "0"),
        z: get_attr_or(e, "z", "0"),
        lunit: get_attr(e, "lunit"),
    }));
}

fn parse_tube_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::Tube(TubeSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        rmin: get_attr(e, "rmin"),
        rmax: get_attr_or(e, "rmax", "0"),
        z: get_attr_or(e, "z", "0"),
        startphi: get_attr(e, "startphi"),
        deltaphi: get_attr(e, "deltaphi"),
        aunit: get_attr(e, "aunit"),
        lunit: get_attr(e, "lunit"),
    }));
}

fn parse_cone_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::Cone(ConeSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        rmin1: get_attr(e, "rmin1"),
        rmax1: get_attr_or(e, "rmax1", "0"),
        rmin2: get_attr(e, "rmin2"),
        rmax2: get_attr_or(e, "rmax2", "0"),
        z: get_attr_or(e, "z", "0"),
        startphi: get_attr(e, "startphi"),
        deltaphi: get_attr(e, "deltaphi"),
        aunit: get_attr(e, "aunit"),
        lunit: get_attr(e, "lunit"),
    }));
}

fn parse_sphere_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::Sphere(SphereSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        rmin: get_attr(e, "rmin"),
        rmax: get_attr_or(e, "rmax", "0"),
        startphi: get_attr(e, "startphi"),
        deltaphi: get_attr(e, "deltaphi"),
        starttheta: get_attr(e, "starttheta"),
        deltatheta: get_attr(e, "deltatheta"),
        aunit: get_attr(e, "aunit"),
        lunit: get_attr(e, "lunit"),
    }));
}

fn parse_trd_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::Trd(TrdSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        x1: get_attr_or(e, "x1", "0"),
        y1: get_attr_or(e, "y1", "0"),
        x2: get_attr_or(e, "x2", "0"),
        y2: get_attr_or(e, "y2", "0"),
        z: get_attr_or(e, "z", "0"),
        lunit: get_attr(e, "lunit"),
    }));
}

fn parse_cut_tube_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::CutTube(CutTubeSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        rmin: get_attr(e, "rmin"),
        rmax: get_attr_or(e, "rmax", "0"),
        z: get_attr_or(e, "z", "0"),
        startphi: get_attr(e, "startphi"),
        deltaphi: get_attr(e, "deltaphi"),
        low_x: get_attr(e, "lowX"),
        low_y: get_attr(e, "lowY"),
        low_z: get_attr(e, "lowZ"),
        high_x: get_attr(e, "highX"),
        high_y: get_attr(e, "highY"),
        high_z: get_attr(e, "highZ"),
        aunit: get_attr(e, "aunit"),
        lunit: get_attr(e, "lunit"),
    }));
}

fn parse_para_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::Para(ParaSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        x: get_attr_or(e, "x", "0"),
        y: get_attr_or(e, "y", "0"),
        z: get_attr_or(e, "z", "0"),
        alpha: get_attr(e, "alpha"),
        theta: get_attr(e, "theta"),
        phi: get_attr(e, "phi"),
        aunit: get_attr(e, "aunit"),
        lunit: get_attr(e, "lunit"),
    }));
}

fn parse_trap_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::Trap(TrapSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        z: get_attr_or(e, "z", "0"),
        theta: get_attr(e, "theta"),
        phi: get_attr(e, "phi"),
        y1: get_attr_or(e, "y1", "0"),
        x1: get_attr_or(e, "x1", "0"),
        x2: get_attr_or(e, "x2", "0"),
        alpha1: get_attr(e, "alpha1"),
        y2: get_attr_or(e, "y2", "0"),
        x3: get_attr_or(e, "x3", "0"),
        x4: get_attr_or(e, "x4", "0"),
        alpha2: get_attr(e, "alpha2"),
        aunit: get_attr(e, "aunit"),
        lunit: get_attr(e, "lunit"),
    }));
}

fn parse_torus_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::Torus(TorusSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        rmin: get_attr(e, "rmin"),
        rmax: get_attr_or(e, "rmax", "0"),
        rtor: get_attr_or(e, "rtor", "0"),
        startphi: get_attr(e, "startphi"),
        deltaphi: get_attr(e, "deltaphi"),
        aunit: get_attr(e, "aunit"),
        lunit: get_attr(e, "lunit"),
    }));
}

fn parse_orb_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::Orb(OrbSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        r: get_attr_or(e, "r", "0"),
        lunit: get_attr(e, "lunit"),
    }));
}

fn parse_paraboloid_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::Paraboloid(ParaboloidSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        rlo: get_attr_or(e, "rlo", "0"),
        rhi: get_attr_or(e, "rhi", "0"),
        dz: get_attr_or(e, "dz", "0"),
        lunit: get_attr(e, "lunit"),
    }));
}

fn parse_twisted_trd_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::TwistedTrd(TwistedTrdSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        phi_twist: get_attr_or(e, "PhiTwist", "0"),
        x1: get_attr_or(e, "x1", "0"),
        x2: get_attr_or(e, "x2", "0"),
        y1: get_attr_or(e, "y1", "0"),
        y2: get_attr_or(e, "y2", "0"),
        z: get_attr_or(e, "z", "0"),
        aunit: get_attr(e, "aunit"),
        lunit: get_attr(e, "lunit"),
    }));
}

fn parse_twisted_trap_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::TwistedTrap(TwistedTrapSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        phi_twist: get_attr_or(e, "PhiTwist", "0"),
        z: get_attr_or(e, "z", "0"),
        theta: get_attr_or(e, "Theta", "0"),
        phi: get_attr_or(e, "Phi", "0"),
        y1: get_attr_or(e, "y1", "0"),
        x1: get_attr_or(e, "x1", "0"),
        x2: get_attr_or(e, "x2", "0"),
        y2: get_attr_or(e, "y2", "0"),
        x3: get_attr_or(e, "x3", "0"),
        x4: get_attr_or(e, "x4", "0"),
        alph: get_attr_or(e, "Alph", "0"),
        aunit: get_attr(e, "aunit"),
        lunit: get_attr(e, "lunit"),
    }));
}

fn parse_twisted_box_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::TwistedBox(TwistedBoxSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        phi_twist: get_attr_or(e, "PhiTwist", "0"),
        x: get_attr_or(e, "x", "0"),
        y: get_attr_or(e, "y", "0"),
        z: get_attr_or(e, "z", "0"),
        aunit: get_attr(e, "aunit"),
        lunit: get_attr(e, "lunit"),
    }));
}

fn parse_twisted_tubs_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::TwistedTubs(TwistedTubsSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        twistedangle: get_attr_or(e, "twistedangle", "0"),
        endinnerrad: get_attr(e, "endinnerrad"),
        endouterrad: get_attr(e, "endouterrad"),
        zlen: get_attr(e, "zlen"),
        phi: get_attr(e, "phi"),
        midinnerrad: get_attr(e, "midinnerrad"),
        midouterrad: get_attr(e, "midouterrad"),
        negative_endz: get_attr(e, "negativeEndz"),
        positive_endz: get_attr(e, "positiveEndz"),
        nseg: get_attr(e, "nseg"),
        totphi: get_attr(e, "totphi"),
        aunit: get_attr(e, "aunit"),
        lunit: get_attr(e, "lunit"),
    }));
}

fn parse_arb8_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::Arb8(Arb8Solid {
        name: get_attr(e, "name").unwrap_or_default(),
        dz: get_attr_or(e, "dz", "0"),
        v1x: get_attr_or(e, "v1x", "0"),
        v1y: get_attr_or(e, "v1y", "0"),
        v2x: get_attr_or(e, "v2x", "0"),
        v2y: get_attr_or(e, "v2y", "0"),
        v3x: get_attr_or(e, "v3x", "0"),
        v3y: get_attr_or(e, "v3y", "0"),
        v4x: get_attr_or(e, "v4x", "0"),
        v4y: get_attr_or(e, "v4y", "0"),
        v5x: get_attr_or(e, "v5x", "0"),
        v5y: get_attr_or(e, "v5y", "0"),
        v6x: get_attr_or(e, "v6x", "0"),
        v6y: get_attr_or(e, "v6y", "0"),
        v7x: get_attr_or(e, "v7x", "0"),
        v7y: get_attr_or(e, "v7y", "0"),
        v8x: get_attr_or(e, "v8x", "0"),
        v8y: get_attr_or(e, "v8y", "0"),
        lunit: get_attr(e, "lunit"),
    }));
}

fn parse_elcone_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::Elcone(ElconeSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        dx: get_attr_or(e, "dx", "0"),
        dy: get_attr_or(e, "dy", "0"),
        zmax: get_attr_or(e, "zmax", "0"),
        zcut: get_attr_or(e, "zcut", "0"),
        lunit: get_attr(e, "lunit"),
    }));
}

fn parse_hype_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::Hype(HypeSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        rmin: get_attr(e, "rmin"),
        rmax: get_attr_or(e, "rmax", "0"),
        inst: get_attr(e, "inst"),
        outst: get_attr(e, "outst"),
        z: get_attr_or(e, "z", "0"),
        aunit: get_attr(e, "aunit"),
        lunit: get_attr(e, "lunit"),
    }));
}

fn parse_tet_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::Tet(TetSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        vertex1: get_attr(e, "vertex1").unwrap_or_default(),
        vertex2: get_attr(e, "vertex2").unwrap_or_default(),
        vertex3: get_attr(e, "vertex3").unwrap_or_default(),
        vertex4: get_attr(e, "vertex4").unwrap_or_default(),
        lunit: get_attr(e, "lunit"),
    }));
}

fn parse_eltube_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::Eltube(EltubeSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        dx: get_attr_or(e, "dx", "0"),
        dy: get_attr_or(e, "dy", "0"),
        dz: get_attr_or(e, "dz", "0"),
        lunit: get_attr(e, "lunit"),
    }));
}

fn parse_ellipsoid_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::Ellipsoid(EllipsoidSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        ax: get_attr_or(e, "ax", "0"),
        by: get_attr_or(e, "by", "0"),
        cz: get_attr_or(e, "cz", "0"),
        zcut1: get_attr(e, "zcut1"),
        zcut2: get_attr(e, "zcut2"),
        lunit: get_attr(e, "lunit"),
    }));
}

// ─── Polycone / polyhedra parsers ────────────────────────────────────────────

/// The `<zplane>` children of a polycone or polyhedra.
fn read_zplanes(reader: &mut Reader<&[u8]>, e: &BytesStart, open: bool) -> Result<Vec<ZPlane>> {
    let mut zplanes = Vec::new();
    for_each_child(reader, e, open, |_, c, _| {
        if c.local_name().as_ref() == b"zplane" {
            zplanes.push(ZPlane {
                rmin: get_attr(c, "rmin"),
                rmax: get_attr_or(c, "rmax", "0"),
                z: get_attr_or(c, "z", "0"),
            });
        }
        Ok(())
    })?;
    Ok(zplanes)
}

/// The `<rzpoint>` children of a generic polycone or polyhedra.
fn read_rzpoints(reader: &mut Reader<&[u8]>, e: &BytesStart, open: bool) -> Result<Vec<RZPoint>> {
    let mut rzpoints = Vec::new();
    for_each_child(reader, e, open, |_, c, _| {
        if c.local_name().as_ref() == b"rzpoint" {
            rzpoints.push(RZPoint {
                r: get_attr_or(c, "r", "0"),
                z: get_attr_or(c, "z", "0"),
            });
        }
        Ok(())
    })?;
    Ok(rzpoints)
}

fn read_polycone_body(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
    open: bool,
) -> Result<PolyconeSolid> {
    Ok(PolyconeSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        startphi: get_attr(e, "startphi"),
        deltaphi: get_attr(e, "deltaphi"),
        aunit: get_attr(e, "aunit"),
        lunit: get_attr(e, "lunit"),
        zplanes: read_zplanes(reader, e, open)?,
    })
}

fn read_generic_polycone_body(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
    open: bool,
) -> Result<GenericPolyconeSolid> {
    Ok(GenericPolyconeSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        startphi: get_attr(e, "startphi"),
        deltaphi: get_attr(e, "deltaphi"),
        aunit: get_attr(e, "aunit"),
        lunit: get_attr(e, "lunit"),
        rzpoints: read_rzpoints(reader, e, open)?,
    })
}

fn read_polyhedra_body(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
    open: bool,
) -> Result<PolyhedraSolid> {
    Ok(PolyhedraSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        startphi: get_attr(e, "startphi"),
        deltaphi: get_attr(e, "deltaphi"),
        numsides: get_attr(e, "numsides").unwrap_or_default(),
        aunit: get_attr(e, "aunit"),
        lunit: get_attr(e, "lunit"),
        zplanes: read_zplanes(reader, e, open)?,
    })
}

fn read_generic_polyhedra_body(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
    open: bool,
) -> Result<GenericPolyhedraSolid> {
    Ok(GenericPolyhedraSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        startphi: get_attr(e, "startphi"),
        deltaphi: get_attr(e, "deltaphi"),
        numsides: get_attr(e, "numsides").unwrap_or_default(),
        aunit: get_attr(e, "aunit"),
        lunit: get_attr(e, "lunit"),
        rzpoints: read_rzpoints(reader, e, open)?,
    })
}

// ─── Xtru parser ─────────────────────────────────────────────────────────────

fn read_xtru_body(reader: &mut Reader<&[u8]>, e: &BytesStart, open: bool) -> Result<XtruSolid> {
    let mut vertices = Vec::new();
    let mut sections = Vec::new();
    for_each_child(reader, e, open, |_, c, _| {
        match c.local_name().as_ref() {
            b"twoDimVertex" => vertices.push(TwoDimVertex {
                x: get_attr_or(c, "x", "0"),
                y: get_attr_or(c, "y", "0"),
            }),
            b"section" => sections.push(XtruSection {
                z_order: get_attr_or(c, "zOrder", "0"),
                z_position: get_attr_or(c, "zPosition", "0"),
                x_offset: get_attr_or(c, "xOffset", "0"),
                y_offset: get_attr_or(c, "yOffset", "0"),
                scaling_factor: get_attr_or(c, "scalingFactor", "1"),
            }),
            _ => {}
        }
        Ok(())
    })?;
    // Sections stay in document order: G4GDMLReadSolids::XtruRead ignores
    // zOrder (G4GDMLReadSolids.cc 684-690).
    Ok(XtruSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        lunit: get_attr(e, "lunit"),
        vertices,
        sections,
    })
}

// ─── Tessellated parser ──────────────────────────────────────────────────────

fn read_tessellated_body(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
    open: bool,
) -> Result<TessellatedSolid> {
    let mut facets = Vec::new();
    for_each_child(reader, e, open, |_, c, _| {
        match c.local_name().as_ref() {
            b"triangular" => facets.push(TessellatedFacet::Triangular {
                vertex1: get_attr_or(c, "vertex1", ""),
                vertex2: get_attr_or(c, "vertex2", ""),
                vertex3: get_attr_or(c, "vertex3", ""),
                r#type: get_attr(c, "type"),
                lunit: get_attr(c, "lunit"),
            }),
            b"quadrangular" => facets.push(TessellatedFacet::Quadrangular {
                vertex1: get_attr_or(c, "vertex1", ""),
                vertex2: get_attr_or(c, "vertex2", ""),
                vertex3: get_attr_or(c, "vertex3", ""),
                vertex4: get_attr_or(c, "vertex4", ""),
                r#type: get_attr(c, "type"),
                lunit: get_attr(c, "lunit"),
            }),
            _ => {}
        }
        Ok(())
    })?;

    Ok(TessellatedSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        lunit: get_attr(e, "lunit"),
        aunit: get_attr(e, "aunit"),
        facets,
    })
}

// ─── Structure parser ────────────────────────────────────────────────────────

fn read_volume_body(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
    open: bool,
    skipped: &mut Vec<String>,
) -> Result<Volume> {
    let mut vol = Volume {
        name: get_attr(e, "name").unwrap_or_default(),
        material_ref: String::new(),
        solid_ref: String::new(),
        physvols: Vec::new(),
        auxiliaries: Vec::new(),
        replica: None,
        body_comments: Vec::new(),
        loops: Vec::new(),
    };
    if !open {
        return Ok(vol);
    }
    // Its own loop rather than `for_each_child`, to keep the body's comments.
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let event = reader.read_event_into(&mut buf);
        let open = matches!(event, Ok(Event::Start(_)));
        match event {
            Ok(Event::Start(ref c) | Event::Empty(ref c)) => match c.local_name().as_ref() {
                b"materialref" => vol.material_ref = ref_attr(c),
                b"solidref" => vol.solid_ref = ref_attr(c),
                b"auxiliary" => vol.auxiliaries.push(read_auxiliary(reader, c, open)?),
                b"physvol" => {
                    let pv = read_physvol_body(reader, c, open, &vol.name, skipped)?;
                    vol.physvols.push(pv);
                }
                b"replicavol" => vol.replica = Some(read_replicavol_body(reader, c, open)?),
                // Capture the whole subtree. Falling through to the
                // catch-all meant the <loop> start tag was ignored and its
                // children were read as direct children of the volume, so
                // the wrapper vanished on save and N placements became one.
                b"loop" => {
                    let xml = capture_raw(reader, c, open)?;
                    vol.loops.push(RawElement {
                        tag: "loop".to_string(),
                        section: Some("structure".to_string()),
                        after: None,
                        xml,
                    });
                }
                // Unsupported placement constructs: skip the whole subtree so
                // their children can't leak into this volume, and warn.
                b"divisionvol" | b"paramvol" => {
                    skipped.push(format!(
                        "volume '{}': <{}> is not supported; its placements are not \
                         rendered and it will be MISSING from a save.",
                        vol.name,
                        String::from_utf8_lossy(c.local_name().as_ref())
                    ));
                    if open {
                        reader.read_to_end(c.to_end().name())?;
                    }
                }
                _ => {}
            },
            Ok(Event::End(ref c)) if c.local_name().as_ref() == b"volume" => return Ok(vol),
            Ok(Event::Comment(ref c)) => {
                vol.body_comments
                    .push(String::from_utf8_lossy(c).to_string());
            }
            Ok(Event::Eof) => return Ok(vol),
            Err(err) => bail!("XML error in volume: {err}"),
            _ => {}
        }
    }
}

/// Read an `<auxiliary>` and its nested `<auxiliary>` children.
///
/// The one volume child whose children are themselves `<auxiliary>`, hence
/// the recursion: the flat treatment re-parented a nested production cut onto
/// the volume and dropped its enclosing Region entirely.
fn read_auxiliary(reader: &mut Reader<&[u8]>, e: &BytesStart, open: bool) -> Result<Auxiliary> {
    let mut aux = Auxiliary {
        auxtype: get_attr(e, "auxtype").unwrap_or_default(),
        auxvalue: get_attr(e, "auxvalue").unwrap_or_default(),
        auxunit: get_attr(e, "auxunit"),
        children: Vec::new(),
    };
    for_each_child(reader, e, open, |reader, c, open| {
        if c.local_name().as_ref() == b"auxiliary" {
            aux.children.push(read_auxiliary(reader, c, open)?);
        } else if open {
            reader.read_to_end(c.to_end().name())?;
        }
        Ok(())
    })?;
    Ok(aux)
}

fn read_physvol_body(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
    open: bool,
    parent_vol: &str,
    skipped: &mut Vec<String>,
) -> Result<PhysVol> {
    let mut pv = PhysVol {
        name: get_attr(e, "name"),
        volume_ref: String::new(),
        copynumber: get_attr(e, "copynumber"),
        file_ref: None,
        position: None,
        rotation: None,
    };
    for_each_child(reader, e, open, |_, c, _| {
        match c.local_name().as_ref() {
            b"volumeref" => pv.volume_ref = ref_attr(c),
            b"file" => {
                pv.file_ref = Some(FileRef {
                    name: get_attr(c, "name").unwrap_or_default(),
                    volname: get_attr(c, "volname"),
                })
            }
            b"position" => pv.position = Some(PlacementPos::Inline(position_from(c))),
            b"positionref" => pv.position = Some(PlacementPos::Ref(ref_attr(c))),
            b"rotation" => pv.rotation = Some(PlacementRot::Inline(rotation_from(c))),
            b"rotationref" => pv.rotation = Some(PlacementRot::Ref(ref_attr(c))),
            b"scale" | b"scaleref" => skipped.push(format!(
                "volume '{}': physvol <scale> (reflection) is not supported; \
                 the placement is rendered unmirrored and the scale will be \
                 MISSING from a save.",
                parent_vol
            )),
            _ => {}
        }
        Ok(())
    })?;
    Ok(pv)
}

// ─── Boolean solid parser ────────────────────────────────────────────────────

fn parse_reflected_solid(e: &BytesStart, solids: &mut SolidSection) {
    solids.solids.push(Solid::Reflected(ReflectedSolidDef {
        name: get_attr(e, "name").unwrap_or_default(),
        solid_ref: get_attr(e, "solid").unwrap_or_default(),
        sx: get_attr_or(e, "sx", "1"),
        sy: get_attr_or(e, "sy", "1"),
        sz: get_attr_or(e, "sz", "1"),
        rx: get_attr_or(e, "rx", "0"),
        ry: get_attr_or(e, "ry", "0"),
        rz: get_attr_or(e, "rz", "0"),
        dx: get_attr_or(e, "dx", "0"),
        dy: get_attr_or(e, "dy", "0"),
        dz: get_attr_or(e, "dz", "0"),
        aunit: get_attr(e, "aunit"),
        lunit: get_attr(e, "lunit"),
    }));
}

fn read_multiunion_body(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
    open: bool,
) -> Result<MultiUnionSolid> {
    let mut nodes = Vec::new();
    for_each_child(reader, e, open, |reader, c, open| {
        if c.local_name().as_ref() == b"multiUnionNode" {
            nodes.push(read_multiunion_node(reader, c, open)?);
        }
        Ok(())
    })?;
    Ok(MultiUnionSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        nodes,
    })
}

fn read_multiunion_node(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
    open: bool,
) -> Result<MultiUnionNode> {
    let mut node = MultiUnionNode {
        name: get_attr(e, "name"),
        solid_ref: String::new(),
        position: None,
        rotation: None,
    };
    for_each_child(reader, e, open, |reader, c, open| {
        match c.local_name().as_ref() {
            // <solid ref="..."/>, or the name as text: <solid>name</solid>
            b"solid" | b"solidref" => {
                node.solid_ref = match get_attr(c, "ref") {
                    Some(r) => r,
                    None if open => reader.read_text(c.name())?.trim().to_string(),
                    None => String::new(),
                }
            }
            b"position" => node.position = Some(PlacementPos::Inline(position_from(c))),
            b"positionref" => node.position = Some(PlacementPos::Ref(ref_attr(c))),
            b"rotation" => node.rotation = Some(PlacementRot::Inline(rotation_from(c))),
            b"rotationref" => node.rotation = Some(PlacementRot::Ref(ref_attr(c))),
            _ => {}
        }
        Ok(())
    })?;
    Ok(node)
}

fn read_scaled_solid_body(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
    open: bool,
) -> Result<ScaledSolidDef> {
    let mut s = ScaledSolidDef {
        name: get_attr(e, "name").unwrap_or_default(),
        solid_ref: String::new(),
        scale_x: "1".to_string(),
        scale_y: "1".to_string(),
        scale_z: "1".to_string(),
        scale_ref: None,
        scale_name: None,
    };
    for_each_child(reader, e, open, |_, c, _| {
        match c.local_name().as_ref() {
            b"solidref" => s.solid_ref = ref_attr(c),
            b"scale" => {
                s.scale_x = get_attr_or(c, "x", "1");
                s.scale_y = get_attr_or(c, "y", "1");
                s.scale_z = get_attr_or(c, "z", "1");
                s.scale_name = get_attr(c, "name");
            }
            // Geant4 accepts a reference to a named <scale> here just as
            // it does an inline one.
            b"scaleref" => s.scale_ref = get_attr(c, "ref"),
            _ => {}
        }
        Ok(())
    })?;
    Ok(s)
}

fn read_boolean_solid_body(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
    open: bool,
    operation: BooleanOp,
) -> Result<BooleanSolid> {
    let mut b = BooleanSolid {
        name: get_attr(e, "name").unwrap_or_default(),
        operation,
        first_ref: String::new(),
        second_ref: String::new(),
        position: None,
        rotation: None,
        first_position: None,
        first_rotation: None,
    };
    for_each_child(reader, e, open, |_, c, _| {
        match c.local_name().as_ref() {
            b"first" => b.first_ref = ref_attr(c),
            b"second" => b.second_ref = ref_attr(c),
            b"position" => b.position = Some(PlacementPos::Inline(position_from(c))),
            b"positionref" => b.position = Some(PlacementPos::Ref(ref_attr(c))),
            b"rotation" => b.rotation = Some(PlacementRot::Inline(rotation_from(c))),
            b"rotationref" => b.rotation = Some(PlacementRot::Ref(ref_attr(c))),
            b"firstposition" => b.first_position = Some(PlacementPos::Inline(position_from(c))),
            b"firstpositionref" => b.first_position = Some(PlacementPos::Ref(ref_attr(c))),
            b"firstrotation" => b.first_rotation = Some(PlacementRot::Inline(rotation_from(c))),
            b"firstrotationref" => b.first_rotation = Some(PlacementRot::Ref(ref_attr(c))),
            _ => {}
        }
        Ok(())
    })?;
    Ok(b)
}

// ─── Replicavol parser ──────────────────────────────────────────────────────

fn read_replicavol_body(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
    open: bool,
) -> Result<ReplicaVol> {
    let mut r = ReplicaVol {
        volume_ref: String::new(),
        number: get_attr(e, "number").unwrap_or_else(|| "0".to_string()),
        direction: [None, None, None],
        curvilinear_axis: None,
        width: String::new(),
        width_unit: None,
        offset: String::new(),
        offset_unit: None,
    };
    for_each_child(reader, e, open, |reader, c, open| {
        match c.local_name().as_ref() {
            b"volumeref" => r.volume_ref = ref_attr(c),
            b"replicate_along_axis" => for_each_child(reader, c, open, |_, c, _| {
                match c.local_name().as_ref() {
                    b"direction" => {
                        r.direction = [get_attr(c, "x"), get_attr(c, "y"), get_attr(c, "z")];
                        r.curvilinear_axis = get_attr(c, "rho")
                            .map(|_| "rho".to_string())
                            .or_else(|| get_attr(c, "phi").map(|_| "phi".to_string()));
                    }
                    b"width" => {
                        r.width = get_attr(c, "value").unwrap_or_default();
                        r.width_unit = get_attr(c, "unit");
                    }
                    b"offset" => {
                        r.offset = get_attr(c, "value").unwrap_or_default();
                        r.offset_unit = get_attr(c, "unit");
                    }
                    _ => {}
                }
                Ok(())
            })?,
            _ => {}
        }
        Ok(())
    })?;
    Ok(r)
}
