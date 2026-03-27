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
                    b"quantity"
                        if section == Section::Define || section == Section::MaterialsDefine =>
                    {
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
                    b"trd" if section == Section::Solids => {
                        parse_trd_solid(e, &mut solids);
                    }
                    b"orb" if section == Section::Solids => {
                        parse_orb_solid(e, &mut solids);
                    }
                    b"torus" if section == Section::Solids => {
                        parse_torus_solid(e, &mut solids);
                    }
                    b"trap" if section == Section::Solids => {
                        parse_trap_solid(e, &mut solids);
                    }
                    b"para" if section == Section::Solids => {
                        parse_para_solid(e, &mut solids);
                    }
                    b"cutTube" if section == Section::Solids => {
                        parse_cut_tube_solid(e, &mut solids);
                    }
                    b"ellipsoid" if section == Section::Solids => {
                        parse_ellipsoid_solid(e, &mut solids);
                    }
                    b"eltube" if section == Section::Solids => {
                        parse_eltube_solid(e, &mut solids);
                    }
                    b"tet" if section == Section::Solids => {
                        parse_tet_solid(e, &mut solids);
                    }
                    b"hype" if section == Section::Solids => {
                        parse_hype_solid(e, &mut solids);
                    }
                    b"elcone" if section == Section::Solids => {
                        parse_elcone_solid(e, &mut solids);
                    }
                    b"paraboloid" if section == Section::Solids => {
                        parse_paraboloid_solid(e, &mut solids);
                    }
                    b"arb8" if section == Section::Solids => {
                        parse_arb8_solid(e, &mut solids);
                    }
                    b"twistedtubs" if section == Section::Solids => {
                        parse_twisted_tubs_solid(e, &mut solids);
                    }
                    b"twistedbox" if section == Section::Solids => {
                        parse_twisted_box_solid(e, &mut solids);
                    }
                    b"twistedtrap" if section == Section::Solids => {
                        parse_twisted_trap_solid(e, &mut solids);
                    }
                    b"twistedtrd" if section == Section::Solids => {
                        parse_twisted_trd_solid(e, &mut solids);
                    }
                    b"polycone" if section == Section::Solids => {
                        let attrs = extract_polycone_attrs(e);
                        let solid = read_polycone_body(&mut reader, attrs)?;
                        solids.solids.push(Solid::Polycone(solid));
                    }
                    b"genericPolycone" if section == Section::Solids => {
                        let solid = read_generic_polycone_body(&mut reader, e)?;
                        solids.solids.push(Solid::GenericPolycone(solid));
                    }
                    b"xtru" if section == Section::Solids => {
                        let attrs = extract_xtru_attrs(e);
                        let solid = read_xtru_body(&mut reader, attrs)?;
                        solids.solids.push(Solid::Xtru(solid));
                    }
                    b"tessellated" if section == Section::Solids => {
                        let name = get_attr(e, "name").unwrap_or_default();
                        let solid = read_tessellated_body(&mut reader, name)?;
                        solids.solids.push(Solid::Tessellated(solid));
                    }
                    b"polyhedra" if section == Section::Solids => {
                        let solid = read_polyhedra_body(&mut reader, e)?;
                        solids.solids.push(Solid::Polyhedra(solid));
                    }
                    b"genericPolyhedra" if section == Section::Solids => {
                        let solid = read_generic_polyhedra_body(&mut reader, e)?;
                        solids.solids.push(Solid::GenericPolyhedra(solid));
                    }
                    b"reflectedSolid" if section == Section::Solids => {
                        parse_reflected_solid(e, &mut solids);
                    }
                    b"multiUnion" if section == Section::Solids => {
                        let name = get_attr(e, "name").unwrap_or_default();
                        let mu = read_multiunion_body(&mut reader, name)?;
                        solids.solids.push(Solid::MultiUnion(mu));
                    }
                    b"scaledSolid" if section == Section::Solids => {
                        let name = get_attr(e, "name").unwrap_or_default();
                        let ss = read_scaled_solid_body(&mut reader, name)?;
                        solids.solids.push(Solid::Scaled(ss));
                    }
                    b"subtraction" if section == Section::Solids => {
                        let name = get_attr(e, "name").unwrap_or_default();
                        let bs =
                            read_boolean_solid_body(&mut reader, name, BooleanOp::Subtraction)?;
                        solids.solids.push(Solid::Boolean(bs));
                    }
                    b"union" if section == Section::Solids => {
                        let name = get_attr(e, "name").unwrap_or_default();
                        let bs = read_boolean_solid_body(&mut reader, name, BooleanOp::Union)?;
                        solids.solids.push(Solid::Boolean(bs));
                    }
                    b"intersection" if section == Section::Solids => {
                        let name = get_attr(e, "name").unwrap_or_default();
                        let bs =
                            read_boolean_solid_body(&mut reader, name, BooleanOp::Intersection)?;
                        solids.solids.push(Solid::Boolean(bs));
                    }
                    b"volume" if section == Section::Structure => {
                        let vol_name = get_attr(e, "name").unwrap_or_default();
                        read_volume_body(&mut reader, vol_name, &mut structure)?;
                    }
                    b"setup" => {
                        let name = get_attr(e, "name").unwrap_or_default();
                        let version = get_attr(e, "version").unwrap_or_else(|| "1.0".to_string());
                        let world_ref = read_setup_body(&mut reader)?;
                        setup = Some(SetupSection {
                            name,
                            version,
                            world_ref,
                        });
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
                    b"quantity"
                        if section == Section::Define || section == Section::MaterialsDefine =>
                    {
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
                        materials.elements.push(Element {
                            name,
                            formula,
                            z,
                            atom_value: None,
                        });
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
                    b"trd" if section == Section::Solids => {
                        parse_trd_solid(e, &mut solids);
                    }
                    b"orb" if section == Section::Solids => {
                        parse_orb_solid(e, &mut solids);
                    }
                    b"torus" if section == Section::Solids => {
                        parse_torus_solid(e, &mut solids);
                    }
                    b"trap" if section == Section::Solids => {
                        parse_trap_solid(e, &mut solids);
                    }
                    b"para" if section == Section::Solids => {
                        parse_para_solid(e, &mut solids);
                    }
                    b"cutTube" if section == Section::Solids => {
                        parse_cut_tube_solid(e, &mut solids);
                    }
                    b"ellipsoid" if section == Section::Solids => {
                        parse_ellipsoid_solid(e, &mut solids);
                    }
                    b"eltube" if section == Section::Solids => {
                        parse_eltube_solid(e, &mut solids);
                    }
                    b"tet" if section == Section::Solids => {
                        parse_tet_solid(e, &mut solids);
                    }
                    b"hype" if section == Section::Solids => {
                        parse_hype_solid(e, &mut solids);
                    }
                    b"elcone" if section == Section::Solids => {
                        parse_elcone_solid(e, &mut solids);
                    }
                    b"paraboloid" if section == Section::Solids => {
                        parse_paraboloid_solid(e, &mut solids);
                    }
                    b"arb8" if section == Section::Solids => {
                        parse_arb8_solid(e, &mut solids);
                    }
                    b"twistedtubs" if section == Section::Solids => {
                        parse_twisted_tubs_solid(e, &mut solids);
                    }
                    b"twistedbox" if section == Section::Solids => {
                        parse_twisted_box_solid(e, &mut solids);
                    }
                    b"twistedtrap" if section == Section::Solids => {
                        parse_twisted_trap_solid(e, &mut solids);
                    }
                    b"twistedtrd" if section == Section::Solids => {
                        parse_twisted_trd_solid(e, &mut solids);
                    }
                    b"reflectedSolid" if section == Section::Solids => {
                        parse_reflected_solid(e, &mut solids);
                    }
                    b"setup" => {
                        let name = get_attr(e, "name").unwrap_or_default();
                        let version = get_attr(e, "version").unwrap_or_else(|| "1.0".to_string());
                        let world_ref = get_attr(e, "world").unwrap_or_default();
                        setup = Some(SetupSection {
                            name,
                            version,
                            world_ref,
                        });
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
        endouterrad: get_attr_or(e, "endouterrad", "0"),
        zlen: get_attr_or(e, "zlen", "0"),
        phi: get_attr(e, "phi"),
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

// ─── Polycone parser ─────────────────────────────────────────────────────────

struct PolyconeAttrs {
    name: String,
    startphi: Option<String>,
    deltaphi: Option<String>,
    aunit: Option<String>,
    lunit: Option<String>,
}

fn extract_polycone_attrs(e: &BytesStart) -> PolyconeAttrs {
    PolyconeAttrs {
        name: get_attr(e, "name").unwrap_or_default(),
        startphi: get_attr(e, "startphi"),
        deltaphi: get_attr(e, "deltaphi"),
        aunit: get_attr(e, "aunit"),
        lunit: get_attr(e, "lunit"),
    }
}

fn read_polycone_body(
    reader: &mut Reader<&[u8]>,
    attrs: PolyconeAttrs,
) -> Result<PolyconeSolid> {
    let mut zplanes = Vec::new();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref inner)) => {
                if inner.local_name().as_ref() == b"zplane" {
                    zplanes.push(ZPlane {
                        rmin: get_attr(inner, "rmin"),
                        rmax: get_attr_or(inner, "rmax", "0"),
                        z: get_attr_or(inner, "z", "0"),
                    });
                }
            }
            Ok(Event::End(ref inner)) => {
                if inner.local_name().as_ref() == b"polycone" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML error in polycone: {}", e)),
            _ => {}
        }
    }

    Ok(PolyconeSolid {
        name: attrs.name,
        startphi: attrs.startphi,
        deltaphi: attrs.deltaphi,
        aunit: attrs.aunit,
        lunit: attrs.lunit,
        zplanes,
    })
}

fn read_generic_polycone_body(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
) -> Result<GenericPolyconeSolid> {
    let name = get_attr(e, "name").unwrap_or_default();
    let startphi = get_attr(e, "startphi");
    let deltaphi = get_attr(e, "deltaphi");
    let aunit = get_attr(e, "aunit");
    let lunit = get_attr(e, "lunit");

    let mut rzpoints = Vec::new();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref inner)) => {
                if inner.local_name().as_ref() == b"rzpoint" {
                    rzpoints.push(RZPoint {
                        r: get_attr_or(inner, "r", "0"),
                        z: get_attr_or(inner, "z", "0"),
                    });
                }
            }
            Ok(Event::End(ref inner)) => {
                if inner.local_name().as_ref() == b"genericPolycone" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML error in genericPolycone: {}", e)),
            _ => {}
        }
    }

    Ok(GenericPolyconeSolid {
        name,
        startphi,
        deltaphi,
        aunit,
        lunit,
        rzpoints,
    })
}

// ─── Polyhedra parser ────────────────────────────────────────────────────────

fn read_polyhedra_body(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
) -> Result<PolyhedraSolid> {
    let name = get_attr(e, "name").unwrap_or_default();
    let numsides = get_attr_or(e, "numsides", "6");
    let startphi = get_attr(e, "startphi");
    let deltaphi = get_attr(e, "deltaphi");
    let aunit = get_attr(e, "aunit");
    let lunit = get_attr(e, "lunit");

    let mut zplanes = Vec::new();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref inner)) => {
                if inner.local_name().as_ref() == b"zplane" {
                    zplanes.push(ZPlane {
                        rmin: get_attr(inner, "rmin"),
                        rmax: get_attr_or(inner, "rmax", "0"),
                        z: get_attr_or(inner, "z", "0"),
                    });
                }
            }
            Ok(Event::End(ref inner)) => {
                if inner.local_name().as_ref() == b"polyhedra" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML error in polyhedra: {}", e)),
            _ => {}
        }
    }

    Ok(PolyhedraSolid {
        name,
        startphi,
        deltaphi,
        numsides,
        aunit,
        lunit,
        zplanes,
    })
}

fn read_generic_polyhedra_body(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
) -> Result<GenericPolyhedraSolid> {
    let name = get_attr(e, "name").unwrap_or_default();
    let numsides = get_attr_or(e, "numsides", "6");
    let startphi = get_attr(e, "startphi");
    let deltaphi = get_attr(e, "deltaphi");
    let aunit = get_attr(e, "aunit");
    let lunit = get_attr(e, "lunit");

    let mut rzpoints = Vec::new();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref inner)) => {
                if inner.local_name().as_ref() == b"rzpoint" {
                    rzpoints.push(RZPoint {
                        r: get_attr_or(inner, "r", "0"),
                        z: get_attr_or(inner, "z", "0"),
                    });
                }
            }
            Ok(Event::End(ref inner)) => {
                if inner.local_name().as_ref() == b"genericPolyhedra" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML error in genericPolyhedra: {}", e)),
            _ => {}
        }
    }

    Ok(GenericPolyhedraSolid {
        name,
        startphi,
        deltaphi,
        numsides,
        aunit,
        lunit,
        rzpoints,
    })
}

// ─── Xtru parser ─────────────────────────────────────────────────────────────

struct XtruAttrs {
    name: String,
    lunit: Option<String>,
}

fn extract_xtru_attrs(e: &BytesStart) -> XtruAttrs {
    XtruAttrs {
        name: get_attr(e, "name").unwrap_or_default(),
        lunit: get_attr(e, "lunit"),
    }
}

fn read_xtru_body(reader: &mut Reader<&[u8]>, attrs: XtruAttrs) -> Result<XtruSolid> {
    let mut vertices = Vec::new();
    let mut sections = Vec::new();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref inner)) => {
                let tag = inner.local_name();
                match tag.as_ref() {
                    b"twoDimVertex" => {
                        vertices.push(TwoDimVertex {
                            x: get_attr_or(inner, "x", "0"),
                            y: get_attr_or(inner, "y", "0"),
                        });
                    }
                    b"section" => {
                        sections.push(XtruSection {
                            z_order: get_attr_or(inner, "zOrder", "0"),
                            z_position: get_attr_or(inner, "zPosition", "0"),
                            x_offset: get_attr_or(inner, "xOffset", "0"),
                            y_offset: get_attr_or(inner, "yOffset", "0"),
                            scaling_factor: get_attr_or(inner, "scalingFactor", "1"),
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref inner)) => {
                if inner.local_name().as_ref() == b"xtru" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML error in xtru: {}", e)),
            _ => {}
        }
    }

    // Sort sections by zOrder for correct ordering
    sections.sort_by_key(|s| s.z_order.parse::<i64>().unwrap_or(0));

    Ok(XtruSolid {
        name: attrs.name,
        lunit: attrs.lunit,
        vertices,
        sections,
    })
}

// ─── Tessellated parser ──────────────────────────────────────────────────────

fn read_tessellated_body(
    reader: &mut Reader<&[u8]>,
    name: String,
) -> Result<TessellatedSolid> {
    let mut facets = Vec::new();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref inner)) => {
                let tag = inner.local_name();
                match tag.as_ref() {
                    b"triangular" => {
                        facets.push(TessellatedFacet::Triangular {
                            vertex1: get_attr_or(inner, "vertex1", ""),
                            vertex2: get_attr_or(inner, "vertex2", ""),
                            vertex3: get_attr_or(inner, "vertex3", ""),
                            r#type: get_attr(inner, "type"),
                        });
                    }
                    b"quadrangular" => {
                        facets.push(TessellatedFacet::Quadrangular {
                            vertex1: get_attr_or(inner, "vertex1", ""),
                            vertex2: get_attr_or(inner, "vertex2", ""),
                            vertex3: get_attr_or(inner, "vertex3", ""),
                            vertex4: get_attr_or(inner, "vertex4", ""),
                            r#type: get_attr(inner, "type"),
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref inner)) => {
                if inner.local_name().as_ref() == b"tessellated" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML error in tessellated: {}", e)),
            _ => {}
        }
    }

    Ok(TessellatedSolid { name, facets })
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
    let mut replica = None;
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
                    b"replicavol" => {
                        let number = get_attr(inner, "number").unwrap_or_else(|| "0".to_string());
                        replica = Some(read_replicavol_body(reader, number)?);
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
        replica,
    });
    Ok(())
}

fn read_physvol_body(reader: &mut Reader<&[u8]>, name: Option<String>) -> Result<PhysVol> {
    let mut volume_ref = String::new();
    let mut file_ref = None;
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
                    b"file" => {
                        let fname = get_attr(inner, "name").unwrap_or_default();
                        let volname = get_attr(inner, "volname");
                        file_ref = Some(FileRef {
                            name: fname,
                            volname,
                        });
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
                    b"file" => {
                        let fname = get_attr(inner, "name").unwrap_or_default();
                        let volname = get_attr(inner, "volname");
                        file_ref = Some(FileRef {
                            name: fname,
                            volname,
                        });
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
        file_ref,
        position,
        rotation,
    })
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
    name: String,
) -> Result<MultiUnionSolid> {
    let mut nodes = Vec::new();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref inner)) => {
                if inner.local_name().as_ref() == b"multiUnionNode" {
                    let node = read_multiunion_node(reader)?;
                    nodes.push(node);
                }
            }
            Ok(Event::End(ref e)) if e.local_name().as_ref() == b"multiUnion" => break,
            Ok(Event::Eof) => break,
            _ => {}
        }
    }

    Ok(MultiUnionSolid { name, nodes })
}

fn read_multiunion_node(reader: &mut Reader<&[u8]>) -> Result<MultiUnionNode> {
    let mut solid_ref = String::new();
    let mut position = None;
    let mut rotation = None;
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref inner)) => {
                let tag = inner.local_name();
                match tag.as_ref() {
                    b"solid" => {
                        // <solid ref="...">...</solid> or <solid>ref_name</solid>
                        if let Some(r) = get_attr(inner, "ref") {
                            solid_ref = r;
                            // consume until </solid>
                            let _ = reader.read_to_end(inner.to_end().name());
                        } else {
                            solid_ref = reader
                                .read_text(inner.to_end().name())
                                .unwrap_or_default()
                                .trim()
                                .to_string();
                        }
                    }
                    b"position" => {
                        position = Some(PlacementPos::Inline(Position {
                            name: get_attr(inner, "name").unwrap_or_default(),
                            x: get_attr(inner, "x"),
                            y: get_attr(inner, "y"),
                            z: get_attr(inner, "z"),
                            unit: get_attr(inner, "unit"),
                        }));
                        let _ = reader.read_to_end(inner.to_end().name());
                    }
                    b"rotation" => {
                        rotation = Some(PlacementRot::Inline(Rotation {
                            name: get_attr(inner, "name").unwrap_or_default(),
                            x: get_attr(inner, "x"),
                            y: get_attr(inner, "y"),
                            z: get_attr(inner, "z"),
                            unit: get_attr(inner, "unit"),
                        }));
                        let _ = reader.read_to_end(inner.to_end().name());
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref inner)) => {
                let tag = inner.local_name();
                match tag.as_ref() {
                    b"solid" => {
                        solid_ref = get_attr(inner, "ref").unwrap_or_default();
                    }
                    b"solidref" => {
                        solid_ref = get_attr(inner, "ref").unwrap_or_default();
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
            Ok(Event::End(ref e)) if e.local_name().as_ref() == b"multiUnionNode" => break,
            Ok(Event::Eof) => break,
            _ => {}
        }
    }

    Ok(MultiUnionNode {
        solid_ref,
        position,
        rotation,
    })
}

fn read_scaled_solid_body(
    reader: &mut Reader<&[u8]>,
    name: String,
) -> Result<ScaledSolidDef> {
    let mut solid_ref = String::new();
    let mut scale_x = "1".to_string();
    let mut scale_y = "1".to_string();
    let mut scale_z = "1".to_string();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref inner)) => {
                let tag = inner.local_name();
                match tag.as_ref() {
                    b"solidref" => {
                        solid_ref = get_attr(inner, "ref").unwrap_or_default();
                    }
                    b"scale" => {
                        scale_x = get_attr_or(inner, "x", "1");
                        scale_y = get_attr_or(inner, "y", "1");
                        scale_z = get_attr_or(inner, "z", "1");
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if e.local_name().as_ref() == b"scaledSolid" => break,
            Ok(Event::Eof) => break,
            _ => {}
        }
    }

    Ok(ScaledSolidDef {
        name,
        solid_ref,
        scale_x,
        scale_y,
        scale_z,
    })
}

fn read_boolean_solid_body(
    reader: &mut Reader<&[u8]>,
    name: String,
    operation: BooleanOp,
) -> Result<BooleanSolid> {
    let mut first_ref = String::new();
    let mut second_ref = String::new();
    let mut position = None;
    let mut rotation = None;
    let mut first_position = None;
    let mut first_rotation = None;
    let mut buf = Vec::new();

    let end_tag: &[u8] = match operation {
        BooleanOp::Subtraction => b"subtraction",
        BooleanOp::Union => b"union",
        BooleanOp::Intersection => b"intersection",
    };

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref inner)) => {
                let tag = inner.local_name();
                match tag.as_ref() {
                    b"first" => {
                        first_ref = get_attr(inner, "ref").unwrap_or_default();
                    }
                    b"second" => {
                        second_ref = get_attr(inner, "ref").unwrap_or_default();
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
                    b"firstposition" => {
                        first_position = Some(PlacementPos::Inline(Position {
                            name: get_attr(inner, "name").unwrap_or_default(),
                            x: get_attr(inner, "x"),
                            y: get_attr(inner, "y"),
                            z: get_attr(inner, "z"),
                            unit: get_attr(inner, "unit"),
                        }));
                    }
                    b"firstpositionref" => {
                        first_position = Some(PlacementPos::Ref(
                            get_attr(inner, "ref").unwrap_or_default(),
                        ));
                    }
                    b"firstrotation" => {
                        first_rotation = Some(PlacementRot::Inline(Rotation {
                            name: get_attr(inner, "name").unwrap_or_default(),
                            x: get_attr(inner, "x"),
                            y: get_attr(inner, "y"),
                            z: get_attr(inner, "z"),
                            unit: get_attr(inner, "unit"),
                        }));
                    }
                    b"firstrotationref" => {
                        first_rotation = Some(PlacementRot::Ref(
                            get_attr(inner, "ref").unwrap_or_default(),
                        ));
                    }
                    _ => {}
                }
            }
            Ok(Event::Start(ref inner)) => {
                let tag = inner.local_name();
                match tag.as_ref() {
                    b"first" => {
                        first_ref = get_attr(inner, "ref").unwrap_or_default();
                        reader.read_to_end(inner.to_end().name())?;
                    }
                    b"second" => {
                        second_ref = get_attr(inner, "ref").unwrap_or_default();
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
                    b"firstposition" => {
                        first_position = Some(PlacementPos::Inline(Position {
                            name: get_attr(inner, "name").unwrap_or_default(),
                            x: get_attr(inner, "x"),
                            y: get_attr(inner, "y"),
                            z: get_attr(inner, "z"),
                            unit: get_attr(inner, "unit"),
                        }));
                        reader.read_to_end(inner.to_end().name())?;
                    }
                    b"firstpositionref" => {
                        first_position = Some(PlacementPos::Ref(
                            get_attr(inner, "ref").unwrap_or_default(),
                        ));
                        reader.read_to_end(inner.to_end().name())?;
                    }
                    b"firstrotation" => {
                        first_rotation = Some(PlacementRot::Inline(Rotation {
                            name: get_attr(inner, "name").unwrap_or_default(),
                            x: get_attr(inner, "x"),
                            y: get_attr(inner, "y"),
                            z: get_attr(inner, "z"),
                            unit: get_attr(inner, "unit"),
                        }));
                        reader.read_to_end(inner.to_end().name())?;
                    }
                    b"firstrotationref" => {
                        first_rotation = Some(PlacementRot::Ref(
                            get_attr(inner, "ref").unwrap_or_default(),
                        ));
                        reader.read_to_end(inner.to_end().name())?;
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref inner)) => {
                if inner.local_name().as_ref() == end_tag {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML error in boolean solid: {}", e)),
            _ => {}
        }
    }

    Ok(BooleanSolid {
        name,
        operation,
        first_ref,
        second_ref,
        position,
        rotation,
        first_position,
        first_rotation,
    })
}

// ─── Replicavol parser ──────────────────────────────────────────────────────

fn read_replicavol_body(reader: &mut Reader<&[u8]>, number: String) -> Result<ReplicaVol> {
    let mut volume_ref = String::new();
    let mut direction = [None, None, None];
    let mut width = String::new();
    let mut width_unit = None;
    let mut offset = String::new();
    let mut offset_unit = None;
    let mut buf = Vec::new();
    let mut in_replicate = false;

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref inner)) => {
                let tag = inner.local_name();
                match tag.as_ref() {
                    b"volumeref" => {
                        volume_ref = get_attr(inner, "ref").unwrap_or_default();
                    }
                    b"direction" if in_replicate => {
                        direction[0] = get_attr(inner, "x");
                        direction[1] = get_attr(inner, "y");
                        direction[2] = get_attr(inner, "z");
                    }
                    b"width" if in_replicate => {
                        width = get_attr(inner, "value").unwrap_or_default();
                        width_unit = get_attr(inner, "unit");
                    }
                    b"offset" if in_replicate => {
                        offset = get_attr(inner, "value").unwrap_or_default();
                        offset_unit = get_attr(inner, "unit");
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
                    b"replicate_along_axis" => {
                        in_replicate = true;
                    }
                    b"direction" if in_replicate => {
                        direction[0] = get_attr(inner, "x");
                        direction[1] = get_attr(inner, "y");
                        direction[2] = get_attr(inner, "z");
                        reader.read_to_end(inner.to_end().name())?;
                    }
                    b"width" if in_replicate => {
                        width = get_attr(inner, "value").unwrap_or_default();
                        width_unit = get_attr(inner, "unit");
                        reader.read_to_end(inner.to_end().name())?;
                    }
                    b"offset" if in_replicate => {
                        offset = get_attr(inner, "value").unwrap_or_default();
                        offset_unit = get_attr(inner, "unit");
                        reader.read_to_end(inner.to_end().name())?;
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref inner)) => {
                let tag = inner.local_name();
                match tag.as_ref() {
                    b"replicate_along_axis" => {
                        in_replicate = false;
                    }
                    b"replicavol" => break,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML error in replicavol: {}", e)),
            _ => {}
        }
    }

    Ok(ReplicaVol {
        volume_ref,
        number,
        direction,
        width,
        width_unit,
        offset,
        offset_unit,
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
