use anyhow::{Context, Result};
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
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
    let mut setup = None;

    #[derive(Debug, PartialEq)]
    enum Section {
        None,
        Define,
        Materials,
        MaterialsDefine,
        Solids,
        Structure,
    }

    let mut section = Section::None;
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                let tag = local.as_ref();
                match tag {
                    b"define" => {
                        if section == Section::Materials {
                            section = Section::MaterialsDefine;
                        } else {
                            section = Section::Define;
                        }
                    }
                    b"materials" => section = Section::Materials,
                    b"solids" => section = Section::Solids,
                    b"structure" => section = Section::Structure,
                    b"constant" if section == Section::Define => {
                        parse_constant(e, &mut defines);
                    }
                    b"quantity" if section == Section::Define || section == Section::MaterialsDefine => {
                        parse_quantity(e, &mut defines);
                    }
                    b"variable" if section == Section::Define => {
                        parse_variable(e, &mut defines);
                    }
                    b"expression" if section == Section::Define => {
                        let name = get_attr(e, "name").unwrap_or_default();
                        let text = reader.read_text(e.name()).unwrap_or_default().to_string();
                        let text = collapse_spaces(&text);
                        defines.expressions.push(Expression { name, value: text });
                    }
                    b"position" if section == Section::Define => {
                        parse_position(e, &mut defines);
                    }
                    b"rotation" if section == Section::Define => {
                        parse_rotation(e, &mut defines);
                    }
                    b"element" if section == Section::Materials => {
                        let attrs = extract_element_attrs(e);
                        read_element_body(&mut reader, attrs, &mut materials)?;
                    }
                    b"material" if section == Section::Materials => {
                        let attrs = extract_material_attrs(e);
                        read_material_body(&mut reader, attrs, &mut materials)?;
                    }
                    b"box" if section == Section::Solids => {
                        parse_box_solid(e, &mut solids);
                    }
                    b"tube" if section == Section::Solids => {
                        parse_tube_solid(e, &mut solids);
                    }
                    b"cone" if section == Section::Solids => {
                        parse_cone_solid(e, &mut solids);
                    }
                    b"sphere" if section == Section::Solids => {
                        parse_sphere_solid(e, &mut solids);
                    }
                    b"volume" if section == Section::Structure => {
                        let vol_name = get_attr(e, "name").unwrap_or_default();
                        read_volume_body(&mut reader, vol_name, &mut structure)?;
                    }
                    b"setup" => {
                        let name = get_attr(e, "name").unwrap_or_default();
                        let version = get_attr(e, "version").unwrap_or_else(|| "1.0".to_string());
                        let world_ref = read_setup_body(&mut reader)?;
                        setup = Some(SetupSection { name, version, world_ref });
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                let tag = local.as_ref();
                match tag {
                    b"constant" if section == Section::Define => {
                        parse_constant(e, &mut defines);
                    }
                    b"quantity" if section == Section::Define || section == Section::MaterialsDefine => {
                        parse_quantity(e, &mut defines);
                    }
                    b"variable" if section == Section::Define => {
                        parse_variable(e, &mut defines);
                    }
                    b"position" if section == Section::Define => {
                        parse_position(e, &mut defines);
                    }
                    b"rotation" if section == Section::Define => {
                        parse_rotation(e, &mut defines);
                    }
                    b"element" if section == Section::Materials => {
                        let name = get_attr(e, "name").unwrap_or_default();
                        let formula = get_attr(e, "formula");
                        let z = get_attr(e, "Z");
                        materials.elements.push(Element { name, formula, z, atom_value: None });
                    }
                    b"box" if section == Section::Solids => {
                        parse_box_solid(e, &mut solids);
                    }
                    b"tube" if section == Section::Solids => {
                        parse_tube_solid(e, &mut solids);
                    }
                    b"cone" if section == Section::Solids => {
                        parse_cone_solid(e, &mut solids);
                    }
                    b"sphere" if section == Section::Solids => {
                        parse_sphere_solid(e, &mut solids);
                    }
                    b"setup" => {
                        let name = get_attr(e, "name").unwrap_or_default();
                        let version = get_attr(e, "version").unwrap_or_else(|| "1.0".to_string());
                        let world_ref = get_attr(e, "world").unwrap_or_default();
                        setup = Some(SetupSection { name, version, world_ref });
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
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
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML parse error at pos {}: {}", reader.buffer_position(), e)),
            _ => {}
        }
    }

    Ok(GdmlDocument {
        filename,
        defines,
        materials,
        solids,
        structure,
        setup: setup.unwrap_or(SetupSection {
            name: "default".to_string(),
            version: "1.0".to_string(),
            world_ref: String::new(),
        }),
    })
}

// ─── Attribute helpers ───────────────────────────────────────────────────────

fn get_attr(e: &BytesStart, name: &str) -> Option<String> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == name.as_bytes() {
            return Some(String::from_utf8_lossy(&attr.value).to_string());
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

fn parse_position(e: &BytesStart, defines: &mut DefineSection) {
    defines.positions.push(Position {
        name: get_attr(e, "name").unwrap_or_default(),
        x: get_attr(e, "x"),
        y: get_attr(e, "y"),
        z: get_attr(e, "z"),
        unit: get_attr(e, "unit"),
    });
}

fn parse_rotation(e: &BytesStart, defines: &mut DefineSection) {
    defines.rotations.push(Rotation {
        name: get_attr(e, "name").unwrap_or_default(),
        x: get_attr(e, "x"),
        y: get_attr(e, "y"),
        z: get_attr(e, "z"),
        unit: get_attr(e, "unit"),
    });
}

// ─── Material parsers (extract attrs, then read body with own buffer) ────────

struct ElementAttrs {
    name: String,
    formula: Option<String>,
    z: Option<String>,
}

fn extract_element_attrs(e: &BytesStart) -> ElementAttrs {
    ElementAttrs {
        name: get_attr(e, "name").unwrap_or_default(),
        formula: get_attr(e, "formula"),
        z: get_attr(e, "Z"),
    }
}

fn read_element_body(
    reader: &mut Reader<&[u8]>,
    attrs: ElementAttrs,
    materials: &mut MaterialSection,
) -> Result<()> {
    let mut atom_value = None;
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref inner)) => {
                if inner.local_name().as_ref() == b"atom" {
                    atom_value = get_attr(inner, "value");
                }
            }
            Ok(Event::End(ref inner)) => {
                if inner.local_name().as_ref() == b"element" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML error in element: {}", e)),
            _ => {}
        }
    }

    materials.elements.push(Element {
        name: attrs.name,
        formula: attrs.formula,
        z: attrs.z,
        atom_value,
    });
    Ok(())
}

struct MaterialAttrs {
    name: String,
    formula: Option<String>,
    z: Option<String>,
}

fn extract_material_attrs(e: &BytesStart) -> MaterialAttrs {
    MaterialAttrs {
        name: get_attr(e, "name").unwrap_or_default(),
        formula: get_attr(e, "formula"),
        z: get_attr(e, "Z"),
    }
}

fn read_material_body(
    reader: &mut Reader<&[u8]>,
    attrs: MaterialAttrs,
    materials: &mut MaterialSection,
) -> Result<()> {
    let mut density = None;
    let mut density_ref = None;
    let mut temperature = None;
    let mut pressure = None;
    let mut atom_value = None;
    let mut components = Vec::new();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref inner)) | Ok(Event::Start(ref inner)) => {
                let tag = inner.local_name();
                match tag.as_ref() {
                    b"D" => {
                        density = Some(Density {
                            value: get_attr(inner, "value").unwrap_or_default(),
                            unit: get_attr(inner, "unit"),
                        });
                    }
                    b"Dref" => {
                        density_ref = get_attr(inner, "ref");
                    }
                    b"T" => {
                        temperature = Some(PropertyValue {
                            value: get_attr(inner, "value").unwrap_or_default(),
                            unit: get_attr(inner, "unit"),
                        });
                    }
                    b"P" => {
                        pressure = Some(PropertyValue {
                            value: get_attr(inner, "value").unwrap_or_default(),
                            unit: get_attr(inner, "unit"),
                        });
                    }
                    b"atom" => {
                        atom_value = get_attr(inner, "value");
                    }
                    b"fraction" => {
                        components.push(MaterialComponent::Fraction {
                            n: get_attr(inner, "n").unwrap_or_default(),
                            ref_name: get_attr(inner, "ref").unwrap_or_default(),
                        });
                    }
                    b"composite" => {
                        components.push(MaterialComponent::Composite {
                            n: get_attr(inner, "n").unwrap_or_default(),
                            ref_name: get_attr(inner, "ref").unwrap_or_default(),
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref inner)) => {
                if inner.local_name().as_ref() == b"material" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML error in material: {}", e)),
            _ => {}
        }
    }

    materials.materials.push(Material {
        name: attrs.name,
        formula: attrs.formula,
        z: attrs.z,
        density,
        density_ref,
        temperature,
        pressure,
        atom_value,
        components,
    });
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

// ─── Structure parser ────────────────────────────────────────────────────────

fn read_volume_body(
    reader: &mut Reader<&[u8]>,
    vol_name: String,
    structure: &mut StructureSection,
) -> Result<()> {
    let mut material_ref = String::new();
    let mut solid_ref = String::new();
    let mut physvols = Vec::new();
    let mut auxiliaries = Vec::new();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref inner)) => {
                let tag = inner.local_name();
                match tag.as_ref() {
                    b"materialref" => {
                        material_ref = get_attr(inner, "ref").unwrap_or_default();
                    }
                    b"solidref" => {
                        solid_ref = get_attr(inner, "ref").unwrap_or_default();
                    }
                    b"auxiliary" => {
                        auxiliaries.push(Auxiliary {
                            auxtype: get_attr(inner, "auxtype").unwrap_or_default(),
                            auxvalue: get_attr(inner, "auxvalue").unwrap_or_default(),
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::Start(ref inner)) => {
                let tag = inner.local_name();
                match tag.as_ref() {
                    b"materialref" => {
                        material_ref = get_attr(inner, "ref").unwrap_or_default();
                        // consume closing tag
                        reader.read_to_end(inner.to_end().name())?;
                    }
                    b"solidref" => {
                        solid_ref = get_attr(inner, "ref").unwrap_or_default();
                        reader.read_to_end(inner.to_end().name())?;
                    }
                    b"physvol" => {
                        let pv_name = get_attr(inner, "name");
                        let pv = read_physvol_body(reader, pv_name)?;
                        physvols.push(pv);
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref inner)) => {
                if inner.local_name().as_ref() == b"volume" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML error in volume: {}", e)),
            _ => {}
        }
    }

    structure.volumes.push(Volume {
        name: vol_name,
        material_ref,
        solid_ref,
        physvols,
        auxiliaries,
    });
    Ok(())
}

fn read_physvol_body(
    reader: &mut Reader<&[u8]>,
    name: Option<String>,
) -> Result<PhysVol> {
    let mut volume_ref = String::new();
    let mut position = None;
    let mut rotation = None;
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref inner)) => {
                let tag = inner.local_name();
                match tag.as_ref() {
                    b"volumeref" => {
                        volume_ref = get_attr(inner, "ref").unwrap_or_default();
                    }
                    b"position" => {
                        position = Some(PlacementPos::Inline(Position {
                            name: get_attr(inner, "name").unwrap_or_default(),
                            x: get_attr(inner, "x"),
                            y: get_attr(inner, "y"),
                            z: get_attr(inner, "z"),
                            unit: get_attr(inner, "unit"),
                        }));
                    }
                    b"positionref" => {
                        position = Some(PlacementPos::Ref(
                            get_attr(inner, "ref").unwrap_or_default(),
                        ));
                    }
                    b"rotation" => {
                        rotation = Some(PlacementRot::Inline(Rotation {
                            name: get_attr(inner, "name").unwrap_or_default(),
                            x: get_attr(inner, "x"),
                            y: get_attr(inner, "y"),
                            z: get_attr(inner, "z"),
                            unit: get_attr(inner, "unit"),
                        }));
                    }
                    b"rotationref" => {
                        rotation = Some(PlacementRot::Ref(
                            get_attr(inner, "ref").unwrap_or_default(),
                        ));
                    }
                    _ => {}
                }
            }
            Ok(Event::Start(ref inner)) => {
                let tag = inner.local_name();
                match tag.as_ref() {
                    b"volumeref" => {
                        volume_ref = get_attr(inner, "ref").unwrap_or_default();
                        reader.read_to_end(inner.to_end().name())?;
                    }
                    b"position" => {
                        position = Some(PlacementPos::Inline(Position {
                            name: get_attr(inner, "name").unwrap_or_default(),
                            x: get_attr(inner, "x"),
                            y: get_attr(inner, "y"),
                            z: get_attr(inner, "z"),
                            unit: get_attr(inner, "unit"),
                        }));
                        reader.read_to_end(inner.to_end().name())?;
                    }
                    b"positionref" => {
                        position = Some(PlacementPos::Ref(
                            get_attr(inner, "ref").unwrap_or_default(),
                        ));
                        reader.read_to_end(inner.to_end().name())?;
                    }
                    b"rotation" => {
                        rotation = Some(PlacementRot::Inline(Rotation {
                            name: get_attr(inner, "name").unwrap_or_default(),
                            x: get_attr(inner, "x"),
                            y: get_attr(inner, "y"),
                            z: get_attr(inner, "z"),
                            unit: get_attr(inner, "unit"),
                        }));
                        reader.read_to_end(inner.to_end().name())?;
                    }
                    b"rotationref" => {
                        rotation = Some(PlacementRot::Ref(
                            get_attr(inner, "ref").unwrap_or_default(),
                        ));
                        reader.read_to_end(inner.to_end().name())?;
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref inner)) => {
                if inner.local_name().as_ref() == b"physvol" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML error in physvol: {}", e)),
            _ => {}
        }
    }

    Ok(PhysVol {
        name,
        volume_ref,
        position,
        rotation,
    })
}

// ─── Setup parser ────────────────────────────────────────────────────────────

fn read_setup_body(reader: &mut Reader<&[u8]>) -> Result<String> {
    let mut world_ref = String::new();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref inner)) => {
                if inner.local_name().as_ref() == b"world" {
                    world_ref = get_attr(inner, "ref").unwrap_or_default();
                }
            }
            Ok(Event::Start(ref inner)) => {
                if inner.local_name().as_ref() == b"world" {
                    world_ref = get_attr(inner, "ref").unwrap_or_default();
                    reader.read_to_end(inner.to_end().name())?;
                }
            }
            Ok(Event::End(ref inner)) => {
                if inner.local_name().as_ref() == b"setup" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML error in setup: {}", e)),
            _ => {}
        }
    }

    Ok(world_ref)
}
