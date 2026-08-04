//! Round-trip fidelity harness.
//!
//! GDML Studio can export the document it loaded, so anything the parser drops
//! is deleted from the user's file. `parser.rs` is the largest file in the
//! backend and had no unit tests, so this exists to make changes to it safe.
//!
//! Three levels, cheapest first:
//!
//! 1. **Idempotence** — `parse → serialize → parse → serialize` must be
//!    byte-identical. Catches non-determinism (HashMap iteration order leaking
//!    into output) and constructs that survive one trip but not two. It
//!    deliberately does *not* compare against the source: the writer legitimately
//!    re-indents and normalises attribute order.
//!
//! 2. **Normalised-XML equality** — one token per XML event, attributes sorted,
//!    whitespace and `<x/>` vs `<x></x>` erased, everything semantic kept. One
//!    assertion covers a whole class of drops at once.
//!
//! 3. **Corpus loss detection** — over every shipped sample, every element and
//!    attribute present in the source must still be present in the export. Weaker
//!    than level 2 (it tolerates additions and reordering) but it runs against
//!    real files with no hand-written expectations, so it catches drops in
//!    constructs nobody thought to write a fixture for.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use gdml_studio_backend::eval::engine::EvalEngine;
use gdml_studio_backend::gdml::materials::serialize_gdml;
use gdml_studio_backend::gdml::model::Solid;
use gdml_studio_backend::gdml::parser::parse_gdml_from_bytes;

use gdml_studio_backend::mesh::tessellator::tessellate_all_solids;

use quick_xml::events::Event;
use quick_xml::Reader;

fn sample_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sample_data")
}

fn sample_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(sample_dir())
        .expect("sample_data missing")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "gdml"))
        .collect();
    // Glob rather than a hardcoded list, so new samples are covered
    // automatically; sort so failures are reported deterministically.
    files.sort();
    assert!(!files.is_empty(), "no sample .gdml files found");
    files
}

fn round_trip(src: &[u8], name: &str) -> String {
    let doc = parse_gdml_from_bytes(src, name.to_string())
        .unwrap_or_else(|e| panic!("{name}: parse failed: {e}"));
    serialize_gdml(&doc).unwrap_or_else(|e| panic!("{name}: serialize failed: {e}"))
}

/// One token per XML event, with attributes sorted by name.
///
/// `Empty` and `Start` collapse to the same token, so `<x/>` and `<x></x>`
/// compare equal. Whitespace-only text is dropped.
fn canonicalize(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut out = Vec::new();

    loop {
        match reader.read_event() {
            Ok(ref ev @ (Event::Start(ref e) | Event::Empty(ref e))) => {
                let is_empty = matches!(ev, Event::Empty(_));
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                let mut attrs: Vec<String> = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .map(|a| {
                        let key = String::from_utf8_lossy(a.key.local_name().as_ref()).to_string();
                        let val = String::from_utf8_lossy(&a.value).to_string();
                        format!("{key}={val}")
                    })
                    .collect();
                attrs.sort();
                out.push(format!("E:{name}|{}", attrs.join("|")));
                // Emit the closing token for a self-closing element too, so
                // `<x/>` and `<x></x>` really do compare equal.
                if is_empty {
                    out.push(format!("/{name}"));
                }
            }
            Ok(Event::End(e)) => {
                out.push(format!(
                    "/{}",
                    String::from_utf8_lossy(e.local_name().as_ref())
                ));
            }
            Ok(Event::Comment(e)) => {
                out.push(format!("C:{}", String::from_utf8_lossy(&e).trim()));
            }
            Ok(Event::DocType(e)) => {
                out.push(format!("D:{}", String::from_utf8_lossy(&e).trim()));
            }
            Ok(Event::Text(e)) => {
                // Collapse internal whitespace: an <expression> body may be
                // wrapped across lines in the source and emitted on one line,
                // which is a formatting change, not a loss.
                let raw = String::from_utf8_lossy(&e);
                let t = raw.split_whitespace().collect::<Vec<_>>().join(" ");
                if !t.is_empty() {
                    out.push(format!("T:{t}"));
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => panic!("canonicalize: XML error: {e}"),
        }
    }
    out
}

/// Multiset of canonical tokens, for subset comparison.
fn token_counts(xml: &str) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for tok in canonicalize(xml) {
        *counts.entry(tok).or_insert(0) += 1;
    }
    counts
}

/// Assert every token in `src` survives into `out`.
///
/// A subset check rather than equality, because the writer legitimately adds the
/// `<gdml>` schema attributes (it hardcodes them, which is itself a known gap —
/// see the corpus test's exemption list).
fn assert_tokens_preserved(src: &str, out: &str) {
    let before = token_counts(src);
    let after = token_counts(out);
    let mut missing = Vec::new();
    for (tok, n) in &before {
        if tok.starts_with("E:gdml") {
            continue;
        }
        let got = after.get(tok).copied().unwrap_or(0);
        if got < *n {
            missing.push(format!("{tok}  ({n} before, {got} after)"));
        }
    }
    assert!(
        missing.is_empty(),
        "round trip dropped:\n  {}\n\n--- export ---\n{out}",
        missing.join("\n  ")
    );
}

/// Wrap a fragment in the smallest valid GDML document.
fn doc_with(solids: &str, structure: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<gdml>
  <define/>
  <materials>
    <material name="Vacuum" state="gas"><D value="1e-25"/><atom value="1.008"/></material>
  </materials>
  <solids>
    <box name="WorldBox" x="1000" y="1000" z="1000" lunit="mm"/>
{solids}
  </solids>
  <structure>
{structure}
    <volume name="World">
      <materialref ref="Vacuum"/>
      <solidref ref="WorldBox"/>
    </volume>
  </structure>
  <setup name="Default" version="1.0"><world ref="World"/></setup>
</gdml>"#
    )
}

#[test]
fn physvol_copynumber_survives() {
    let src = doc_with(
        "",
        r#"    <volume name="Inner">
      <materialref ref="Vacuum"/>
      <solidref ref="WorldBox"/>
    </volume>
    <volume name="Mother">
      <materialref ref="Vacuum"/>
      <solidref ref="WorldBox"/>
      <physvol name="p1" copynumber="17"><volumeref ref="Inner"/></physvol>
    </volume>"#,
    );
    let out = round_trip(src.as_bytes(), "copynumber.gdml");
    assert!(
        out.contains(r#"copynumber="17""#),
        "copynumber was dropped:\n{out}"
    );
    assert_tokens_preserved(&src, &out);
}

#[test]
fn nested_auxiliary_is_not_reparented() {
    // A production cut inside a Region must stay inside it. Handling only the
    // self-closing form dropped the Region and promoted the cut to a sibling.
    let src = doc_with(
        "",
        r#"    <volume name="Mother">
      <materialref ref="Vacuum"/>
      <solidref ref="WorldBox"/>
      <auxiliary auxtype="Region" auxvalue="R1" auxunit="mm">
        <auxiliary auxtype="gamcut" auxvalue="0.1"/>
      </auxiliary>
    </volume>"#,
    );

    let doc = parse_gdml_from_bytes(src.as_bytes(), "aux.gdml".to_string()).unwrap();
    let mother = doc
        .structure
        .volumes
        .iter()
        .find(|v| v.name == "Mother")
        .expect("Mother volume");

    assert_eq!(mother.auxiliaries.len(), 1, "Region should not be dropped");
    let region = &mother.auxiliaries[0];
    assert_eq!(region.auxtype, "Region");
    assert_eq!(region.auxunit.as_deref(), Some("mm"));
    assert_eq!(region.children.len(), 1, "gamcut should stay nested");
    assert_eq!(region.children[0].auxtype, "gamcut");

    assert_tokens_preserved(&src, &serialize_gdml(&doc).unwrap());
}

#[test]
fn comments_survive_at_every_depth() {
    let src = r#"<?xml version="1.0" encoding="UTF-8"?>
<!-- prolog comment -->
<gdml>
  <!-- before define -->
  <define>
    <!-- before a constant -->
    <constant name="a" value="1"/>
    <!-- trailing inside define -->
  </define>
  <!-- between sections -->
  <materials>
    <material name="Vacuum" state="gas"><D value="1e-25"/><atom value="1.008"/></material>
  </materials>
  <solids>
    <!-- before a solid -->
    <box name="WorldBox" x="1000" y="1000" z="1000" lunit="mm"/>
  </solids>
  <structure>
    <volume name="World">
      <!-- inside a volume body -->
      <materialref ref="Vacuum"/>
      <solidref ref="WorldBox"/>
    </volume>
  </structure>
  <!-- before setup -->
  <setup name="Default" version="1.0"><world ref="World"/></setup>
</gdml>
<!-- epilog comment -->"#;

    let out = round_trip(src.as_bytes(), "comments.gdml");
    for expected in [
        "prolog comment",
        "before define",
        "before a constant",
        "trailing inside define",
        "between sections",
        "before a solid",
        "inside a volume body",
        "before setup",
        "epilog comment",
    ] {
        assert!(out.contains(expected), "lost comment {expected:?}:\n{out}");
    }
    assert_tokens_preserved(src, &out);
}

#[test]
fn comment_bodies_are_not_escaped() {
    // Comment content is not XML-escaped, so it must not be unescaped on read
    // nor escaped on write -- `BytesText::new` would turn `<` into `&lt;`.
    // Commented-out markup is common in real files: fermi_simple_elements_
    // satellite.gdml has 11 commented-out <auxiliary/> elements.
    let src = doc_with(
        "",
        r#"    <volume name="Mother">
      <!-- <auxiliary auxtype="Hierarchy" auxvalue="a &amp; b"/> -->
      <materialref ref="Vacuum"/>
      <solidref ref="WorldBox"/>
    </volume>"#,
    );
    let out = round_trip(src.as_bytes(), "escape.gdml");
    assert!(
        out.contains(r#"<!-- <auxiliary auxtype="Hierarchy" auxvalue="a &amp; b"/> -->"#),
        "comment body was mangled:\n{out}"
    );
}

#[test]
fn non_self_closed_leaf_children_are_read() {
    // Seven body readers matched Event::Empty only, so `<zplane ...></zplane>`
    // — legal XML that a DOM serialiser will happily emit — was silently
    // skipped, leaving e.g. a polycone with zero z-planes and no warning.
    // These children are schema leaves, so the merged Empty|Start arm is safe:
    // the reader's End arm is name-guarded on the parent, so the child's own
    // closing tag is ignored.
    let cases: &[(&str, &str, usize)] = &[
        (
            "polycone",
            r#"<polycone name="s" startphi="0" deltaphi="360" aunit="deg" lunit="mm"><zplane z="0" rmin="0" rmax="5"></zplane><zplane z="10" rmin="0" rmax="5"></zplane></polycone>"#,
            2,
        ),
        (
            "genericPolycone",
            r#"<genericPolycone name="s" startphi="0" deltaphi="360" aunit="deg" lunit="mm"><rzpoint r="0" z="0"></rzpoint><rzpoint r="5" z="5"></rzpoint><rzpoint r="0" z="10"></rzpoint></genericPolycone>"#,
            3,
        ),
        (
            "polyhedra",
            r#"<polyhedra name="s" startphi="0" deltaphi="360" numsides="6" aunit="deg" lunit="mm"><zplane z="0" rmin="0" rmax="5"></zplane><zplane z="10" rmin="0" rmax="5"></zplane></polyhedra>"#,
            2,
        ),
        (
            "genericPolyhedra",
            r#"<genericPolyhedra name="s" startphi="0" deltaphi="360" numsides="6" aunit="deg" lunit="mm"><rzpoint r="0" z="0"></rzpoint><rzpoint r="5" z="5"></rzpoint><rzpoint r="0" z="10"></rzpoint></genericPolyhedra>"#,
            3,
        ),
        (
            "xtru",
            r#"<xtru name="s" lunit="mm"><twoDimVertex x="0" y="0"></twoDimVertex><twoDimVertex x="5" y="0"></twoDimVertex><twoDimVertex x="0" y="5"></twoDimVertex><section zOrder="0" zPosition="0" xOffset="0" yOffset="0" scalingFactor="1"></section><section zOrder="1" zPosition="10" xOffset="0" yOffset="0" scalingFactor="1"></section></xtru>"#,
            5,
        ),
        (
            "tessellated",
            r#"<tessellated name="s"><triangular vertex1="v1" vertex2="v2" vertex3="v3" type="ABSOLUTE"></triangular></tessellated>"#,
            1,
        ),
        (
            "scaledSolid",
            r#"<scaledSolid name="s"><solidref ref="WorldBox"></solidref><scale name="sc" x="2" y="2" z="2"></scale></scaledSolid>"#,
            2,
        ),
    ];

    for (kind, solid_xml, expected_children) in cases {
        let src = doc_with(&format!("    {solid_xml}"), "");
        let doc = parse_gdml_from_bytes(src.as_bytes(), "leaf.gdml".to_string())
            .unwrap_or_else(|e| panic!("{kind}: parse failed: {e}"));

        let solid = doc
            .solids
            .solids
            .iter()
            .find(|s| s.name() == "s")
            .unwrap_or_else(|| panic!("{kind}: solid not parsed at all"));

        let got = match solid {
            Solid::Polycone(p) => p.zplanes.len(),
            Solid::GenericPolycone(p) => p.rzpoints.len(),
            Solid::Polyhedra(p) => p.zplanes.len(),
            Solid::GenericPolyhedra(p) => p.rzpoints.len(),
            Solid::Xtru(x) => x.vertices.len() + x.sections.len(),
            Solid::Tessellated(t) => t.facets.len(),
            // solidref + scale: the ref must be non-empty and the scale applied.
            Solid::Scaled(s) => {
                assert_eq!(s.solid_ref, "WorldBox", "{kind}: solidref lost");
                assert_eq!(s.scale_x, "2", "{kind}: scale lost");
                2
            }
            other => panic!("{kind}: unexpected variant {other:?}"),
        };

        assert_eq!(
            got, *expected_children,
            "{kind}: non-self-closed children were skipped"
        );
    }
}

#[test]
fn loop_is_re_emitted_in_its_own_section() {
    // <loop> is legal in <define>, <solids> and <structure>. The writer inferred
    // the section from the tag name alone and always chose <define>, so a loop
    // in solids or structure came back nested inside <define> — a file Geant4
    // will not load. The user-facing warning meanwhile claimed the element was
    // "preserved on save".
    let src = doc_with(
        r#"    <loop for="i" to="3" step="1"><box name="looped" x="1" y="1" z="1"/></loop>"#,
        r#"    <loop for="j" to="2" step="1"><volume name="LV"><materialref ref="Vacuum"/><solidref ref="WorldBox"/></volume></loop>"#,
    );
    let out = round_trip(src.as_bytes(), "loop.gdml");

    let in_section = |section: &str, needle: &str| -> bool {
        let open = format!("<{section}>");
        let close = format!("</{section}>");
        match (out.find(&open), out.find(&close)) {
            (Some(a), Some(b)) if a < b => out[a..b].contains(needle),
            _ => false,
        }
    };

    assert!(
        in_section("solids", "<loop"),
        "loop from <solids> not written back there:\n{out}"
    );
    assert!(
        in_section("structure", "<loop"),
        "loop from <structure> not written back there:\n{out}"
    );
    assert!(
        !in_section("define", "<loop"),
        "loop wrongly written into <define>, producing invalid GDML:\n{out}"
    );
}

#[test]
fn optical_material_properties_survive() {
    // <property> is how an optical material binds a named <matrix>. Dropping
    // these removed every optical property from the file while leaving the
    // matrices they referenced behind — a silently non-functional export.
    let src = r#"<?xml version="1.0" encoding="UTF-8"?>
<gdml>
  <define/>
  <materials>
    <isotope name="H1" N="1" Z="1"><atom value="1.008" unit="g/mole" type="A"/></isotope>
    <element name="Hyd"><atom value="1.008" unit="g/mole"/></element>
    <material name="Glass" state="solid">
      <D value="2.5"/>
      <MEE value="85.7" unit="eV"/>
      <RL value="12.3" unit="cm"/>
      <AL value="45.6" unit="cm"/>
      <property name="RINDEX" ref="rindexMatrix"/>
      <property name="ABSLENGTH" ref="abslenMatrix"/>
      <atom value="20.0" unit="g/mole"/>
    </material>
  </materials>
  <solids>
    <box name="WorldBox" x="10" y="10" z="10" lunit="mm"/>
  </solids>
  <structure>
    <volume name="World"><materialref ref="Glass"/><solidref ref="WorldBox"/></volume>
  </structure>
  <setup name="Default" version="1.0"><world ref="World"/></setup>
</gdml>"#;

    let doc = parse_gdml_from_bytes(src.as_bytes(), "optical.gdml".to_string()).unwrap();
    let glass = doc
        .materials
        .materials
        .iter()
        .find(|m| m.name == "Glass")
        .expect("Glass");
    assert_eq!(glass.properties.len(), 2, "optical properties lost");
    assert_eq!(glass.properties[0].name, "RINDEX");
    assert_eq!(
        glass.properties[0].ref_name.as_deref(),
        Some("rindexMatrix")
    );
    assert!(glass.rl.is_some(), "RL lost");
    assert!(glass.al.is_some(), "AL lost");
    assert_eq!(glass.atom_unit.as_deref(), Some("g/mole"), "atom unit lost");

    let iso = &doc.materials.isotopes[0];
    assert_eq!(iso.atom_unit.as_deref(), Some("g/mole"));
    assert_eq!(iso.atom_type.as_deref(), Some("A"));
    assert_eq!(
        doc.materials.elements[0].atom_unit.as_deref(),
        Some("g/mole")
    );

    assert_tokens_preserved(src, &serialize_gdml(&doc).unwrap());
}

#[test]
fn named_default_setup_wins_over_the_last_one() {
    // G4GDMLParser::Read asks for the setup named "Default". Overwriting on each
    // <setup> meant the last one won, so a file listing an alternative setup
    // after the default rendered the wrong world volume — and the alternative
    // was deleted on save either way.
    let src = r#"<?xml version="1.0" encoding="UTF-8"?>
<gdml>
  <define/>
  <materials>
    <material name="Vacuum" state="gas"><D value="1e-25"/><atom value="1.008"/></material>
  </materials>
  <solids>
    <box name="WorldBox" x="1000" y="1000" z="1000" lunit="mm"/>
  </solids>
  <structure>
    <volume name="Alt"><materialref ref="Vacuum"/><solidref ref="WorldBox"/></volume>
    <volume name="World"><materialref ref="Vacuum"/><solidref ref="WorldBox"/></volume>
  </structure>
  <setup name="Default" version="1.0"><world ref="World"/></setup>
  <setup name="Alternative" version="1.0"><world ref="Alt"/></setup>
</gdml>"#;

    let doc = parse_gdml_from_bytes(src.as_bytes(), "setups.gdml".to_string()).unwrap();
    assert_eq!(doc.setup.name, "Default", "wrong setup selected");
    assert_eq!(doc.setup.world_ref, "World", "wrong world volume");
    assert_eq!(doc.setups.len(), 2, "the alternative setup was dropped");

    let out = serialize_gdml(&doc).unwrap();
    assert!(
        out.contains(r#"name="Alternative""#),
        "alt setup lost:\n{out}"
    );
    assert_tokens_preserved(src, &out);
}

#[test]
fn scaleref_is_resolved_and_not_rewritten() {
    // Geant4 accepts <scaleref> in a <scaledSolid> as readily as an inline
    // <scale>. Only the inline form was handled, so a scaleref fell through to
    // the (1,1,1) defaults — an unscaled render — and was then rewritten on
    // export as a fabricated inline scale, corrupting the file too.
    let src = r#"<?xml version="1.0" encoding="UTF-8"?>
<gdml>
  <define>
    <scale name="doubler" x="2" y="2" z="2"/>
  </define>
  <materials>
    <material name="Vacuum" state="gas"><D value="1e-25"/><atom value="1.008"/></material>
  </materials>
  <solids>
    <box name="WorldBox" x="10" y="10" z="10" lunit="mm"/>
    <scaledSolid name="Big"><solidref ref="WorldBox"/><scaleref ref="doubler"/></scaledSolid>
  </solids>
  <structure>
    <volume name="World"><materialref ref="Vacuum"/><solidref ref="Big"/></volume>
  </structure>
  <setup name="Default" version="1.0"><world ref="World"/></setup>
</gdml>"#;

    let doc = parse_gdml_from_bytes(src.as_bytes(), "scaleref.gdml".to_string()).unwrap();
    assert_eq!(doc.defines.scales.len(), 1, "<scale> define not parsed");

    // The reference must actually scale the mesh: a 10mm box doubled is 20mm.
    let mut engine = EvalEngine::new();
    engine.evaluate_all(&doc.defines).unwrap();
    let (meshes, _) = tessellate_all_solids(&doc.solids, &engine, 32).unwrap();
    let big = meshes.get("Big").expect("scaled solid not tessellated");
    let extent = big
        .positions
        .chunks_exact(3)
        .map(|v| v[0].abs())
        .fold(0.0f32, f32::max);
    assert!(
        (extent - 10.0).abs() < 1e-3,
        "scaleref not applied: half-extent {extent}, expected 10 (a 10mm box scaled 2x)"
    );

    let out = serialize_gdml(&doc).unwrap();
    assert!(
        out.contains(r#"<scaleref ref="doubler"/>"#),
        "scaleref rewritten as an inline scale:\n{out}"
    );
    assert_tokens_preserved(src, &out);
}

#[test]
fn doctype_survives_and_entities_are_reported() {
    // The declaration must round-trip -- losing it permanently breaks any file
    // that uses entity-based includes. References are deliberately not expanded
    // (see the parser), so the user is told rather than left with silent zeros.
    let src = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE gdml [<!ENTITY size "10">]>
<gdml>
  <define/>
  <materials>
    <material name="Vacuum" state="gas"><D value="1e-25"/><atom value="1.008"/></material>
  </materials>
  <solids>
    <box name="WorldBox" x="10" y="10" z="10" lunit="mm"/>
  </solids>
  <structure>
    <volume name="World"><materialref ref="Vacuum"/><solidref ref="WorldBox"/></volume>
  </structure>
  <setup name="Default" version="1.0"><world ref="World"/></setup>
</gdml>"#;

    let doc = parse_gdml_from_bytes(src.as_bytes(), "dt.gdml".to_string()).unwrap();
    assert!(doc.order.doctype.is_some(), "DOCTYPE dropped");
    assert!(
        doc.skipped_unsupported
            .iter()
            .any(|w| w.contains("entities")),
        "no warning about unexpanded entities: {:?}",
        doc.skipped_unsupported
    );

    let out = serialize_gdml(&doc).unwrap();
    assert!(
        out.contains("<!DOCTYPE gdml"),
        "DOCTYPE not written:\n{out}"
    );
    assert!(
        out.contains(r#"<!ENTITY size "10">"#),
        "subset lost:\n{out}"
    );
}

#[test]
fn external_entity_is_refused_loudly() {
    // Resolving a SYSTEM entity server-side on an upload endpoint is XXE. The
    // file still loads -- it is mostly renderable -- but the omission is stated.
    let src = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE gdml [<!ENTITY sub SYSTEM "sub.gdml">]>
<gdml>
  <define/>
  <materials>
    <material name="Vacuum" state="gas"><D value="1e-25"/><atom value="1.008"/></material>
  </materials>
  <solids><box name="WorldBox" x="10" y="10" z="10" lunit="mm"/></solids>
  <structure>
    <volume name="World"><materialref ref="Vacuum"/><solidref ref="WorldBox"/></volume>
  </structure>
  <setup name="Default" version="1.0"><world ref="World"/></setup>
</gdml>"#;

    let doc = parse_gdml_from_bytes(src.as_bytes(), "xxe.gdml".to_string()).unwrap();
    assert!(
        doc.skipped_unsupported
            .iter()
            .any(|w| w.contains("EXTERNAL entity")),
        "external entity not reported: {:?}",
        doc.skipped_unsupported
    );
}

#[test]
fn export_is_idempotent_across_the_corpus() {
    for path in sample_files() {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let src = std::fs::read(&path).unwrap();

        let once = round_trip(&src, &name);
        let twice = round_trip(once.as_bytes(), &name);

        assert_eq!(
            once, twice,
            "{name}: export is not idempotent — a second round trip changed the bytes. \
             Usually non-deterministic ordering, or a construct that survives one trip \
             but not two."
        );
    }
}

#[test]
fn export_drops_nothing_from_the_corpus() {
    // The net that catches regressions in constructs with no dedicated fixture.
    // Known gaps are listed explicitly so the test fails when one is fixed and
    // the exemption becomes stale, rather than silently passing forever.
    // No exemptions. Every construct any sample uses now survives export.
    const KNOWN_DROPPED: &[&str] = &[];
    // Everything previously exempted here — loop, physvol, auxiliary, atom,
    // multiUnionNode, gdml, setup, scaledSolid, scale, solidref, and finally
    // the nested <materials><define> — is now either fixed or provably
    // unexercised by any sample. Constructs still unmodelled (atom's unit/type,
    // a second <setup>, <scaleref>, material <property>/<RL>/<AL>) will fail
    // this test the moment a sample uses one, which is the intent.

    let mut failures = Vec::new();

    for path in sample_files() {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let src = std::fs::read(&path).unwrap();
        let src_text = String::from_utf8_lossy(&src).to_string();
        let exported = round_trip(&src, &name);

        let before = token_counts(&src_text);
        let after = token_counts(&exported);

        for (tok, n_before) in &before {
            if KNOWN_DROPPED.iter().any(|p| tok.starts_with(p)) {
                continue;
            }
            let n_after = after.get(tok).copied().unwrap_or(0);
            if n_after < *n_before {
                failures.push(format!(
                    "{name}: {tok}  ({n_before} in source, {n_after} in export)"
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "export dropped content that was in the source:\n  {}",
        failures.join("\n  ")
    );
}

#[test]
fn nested_materials_define_stays_in_materials() {
    // GDML allows a <define> inside <materials>; BgoDetModel and
    // NaiDetModelWithMLI both use one for universe_mean_density. Defines are
    // global wherever they sit, so everything is parsed into one list -- but
    // folding them all into the top-level block on save rewrote the file's
    // shape, which was the corpus detector's last standing exemption.
    let src = r#"<?xml version="1.0" encoding="UTF-8"?>
<gdml>
  <define>
    <constant name="top_level" value="1"/>
  </define>
  <materials>
    <define>
      <quantity type="density" name="universe_mean_density" value="1.e-25" unit="g/cm3"/>
      <constant name="nested_const" value="7"/>
    </define>
    <material name="Vacuum" state="gas"><D value="1e-25"/><atom value="1.008"/></material>
  </materials>
  <solids>
    <box name="WorldBox" x="10" y="10" z="10" lunit="mm"/>
  </solids>
  <structure>
    <volume name="World"><materialref ref="Vacuum"/><solidref ref="WorldBox"/></volume>
  </structure>
  <setup name="Default" version="1.0"><world ref="World"/></setup>
</gdml>"#;

    let doc = parse_gdml_from_bytes(src.as_bytes(), "t.gdml".to_string()).unwrap();

    // A <constant> in the nested block used to be dropped outright: the parser
    // only accepted <quantity> there, and every other arm required the
    // top-level section.
    assert!(
        doc.defines
            .constants
            .iter()
            .any(|c| c.name == "nested_const"),
        "a <constant> inside <materials><define> was dropped"
    );
    assert_eq!(
        doc.materials_define.as_deref(),
        Some(
            &[
                "universe_mean_density".to_string(),
                "nested_const".to_string()
            ][..]
        ),
        "nested define membership not recorded"
    );

    let xml = serialize_gdml(&doc).unwrap();
    let materials_block = xml
        .split("<materials")
        .nth(1)
        .and_then(|s| s.split("</materials>").next())
        .expect("no <materials> block in export");

    assert!(
        materials_block.contains("universe_mean_density"),
        "nested quantity was hoisted out of <materials>:\n{xml}"
    );
    assert!(
        materials_block.contains("nested_const"),
        "nested constant was hoisted out of <materials>:\n{xml}"
    );
    assert!(
        !materials_block.contains("top_level"),
        "a top-level define was pulled into <materials>:\n{xml}"
    );
    // Exactly one of each: nothing duplicated across the two blocks.
    assert_eq!(
        xml.matches("universe_mean_density").count(),
        1,
        "nested quantity emitted twice:\n{xml}"
    );
    assert_eq!(
        xml.matches("top_level").count(),
        1,
        "top-level define emitted twice:\n{xml}"
    );

    // And it survives a second trip. Compared as a set: the writer emits each
    // typed collection in turn (all constants, then all quantities, ...), so a
    // block mixing kinds comes back permuted. That reordering is pre-existing
    // and applies to the top-level <define> just the same — `evaluate_all`
    // topologically sorts, so it is harmless here, and it is not something this
    // change introduced.
    let doc2 = parse_gdml_from_bytes(xml.as_bytes(), "t2.gdml".to_string()).unwrap();
    let mut a = doc.materials_define.clone().unwrap();
    let mut b = doc2.materials_define.clone().unwrap();
    a.sort();
    b.sort();
    assert_eq!(a, b, "nested define membership changed on the second trip");
}

#[test]
fn a_document_without_a_nested_define_does_not_grow_one() {
    let src = doc_with("", "");
    let doc = parse_gdml_from_bytes(src.as_bytes(), "t.gdml".to_string()).unwrap();
    assert_eq!(doc.materials_define, None);

    let xml = serialize_gdml(&doc).unwrap();
    let materials_block = xml
        .split("<materials")
        .nth(1)
        .and_then(|s| s.split("</materials>").next())
        .expect("no <materials> block in export");
    assert!(
        !materials_block.contains("<define"),
        "export invented a <define> inside <materials>:\n{xml}"
    );
}

#[test]
fn defines_are_emitted_in_source_order() {
    // G4GDMLReadDefine::DefineRead walks the <define> children once, in document
    // order, and evaluates each `value` inline as it reads it
    // (G4GDMLReadDefine.cc:601 and :203). Emitting the collections one type at a
    // time -- every constant, then every quantity, ... -- can therefore put a
    // define ahead of one it references and produce a file Geant4 refuses to
    // load, which this project's own evaluator never notices because
    // `evaluate_all` sorts topologically.
    let src = r#"<?xml version="1.0"?>
<gdml>
  <define>
    <variable name="v" value="5"/>
    <constant name="c" value="v*2"/>
    <position name="p" x="c" y="0" z="0"/>
    <quantity name="q" type="length" value="v" unit="mm"/>
  </define>
  <materials>
    <material name="M" Z="1"><D value="1"/><atom value="1"/></material>
  </materials>
  <solids><box name="B" x="1" y="1" z="1"/></solids>
  <structure>
    <volume name="W"><materialref ref="M"/><solidref ref="B"/></volume>
  </structure>
  <setup name="Default" version="1.0"><world ref="W"/></setup>
</gdml>"#;

    let doc = parse_gdml_from_bytes(src.as_bytes(), "t.gdml".to_string()).unwrap();
    let xml = serialize_gdml(&doc).unwrap();
    let block = xml
        .split("<define>")
        .nth(1)
        .and_then(|s| s.split("</define>").next())
        .expect("no <define> block");

    let at = |needle: &str| {
        block
            .find(needle)
            .unwrap_or_else(|| panic!("{needle} missing:\n{block}"))
    };
    assert!(
        at(r#"name="v""#) < at(r#"name="c""#),
        "constant c emitted before the variable v it references:\n{block}"
    );
    assert!(
        at(r#"name="c""#) < at(r#"name="p""#),
        "position p emitted before the constant c it references:\n{block}"
    );
    assert!(
        at(r#"name="v""#) < at(r#"name="q""#),
        "quantity q emitted before the variable v it references:\n{block}"
    );

    // Idempotent: the second trip must not shuffle it back.
    let doc2 = parse_gdml_from_bytes(xml.as_bytes(), "t2.gdml".to_string()).unwrap();
    assert_eq!(
        serialize_gdml(&doc2).unwrap(),
        xml,
        "define order is not stable across a second round trip"
    );
}

#[test]
fn a_hand_built_document_still_serialises() {
    // No manifest: the grouped fallback has to kick in rather than emitting a
    // partial block or panicking.
    use gdml_studio_backend::gdml::model::{Constant, GdmlDocument, Variable};

    let src = doc_with("", "");
    let mut doc = parse_gdml_from_bytes(src.as_bytes(), "t.gdml".to_string()).unwrap();
    doc.defines.constants.push(Constant {
        name: "added_c".to_string(),
        value: "1".to_string(),
    });
    doc.defines.variables.push(Variable {
        name: "added_v".to_string(),
        value: "2".to_string(),
    });
    // The manifest no longer matches the collections.
    assert_ne!(doc.order.define_slots.len(), 2);

    let xml = serialize_gdml(&doc).unwrap();
    assert!(
        xml.contains(r#"name="added_c""#),
        "constant dropped:\n{xml}"
    );
    assert!(
        xml.contains(r#"name="added_v""#),
        "variable dropped:\n{xml}"
    );

    let _: GdmlDocument = parse_gdml_from_bytes(xml.as_bytes(), "t2.gdml".to_string()).unwrap();
}
