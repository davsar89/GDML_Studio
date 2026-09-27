use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::eval::engine::EvalEngine;
use crate::gdml::model::{GdmlDocument, RawElement};
use crate::mesh::types::TriangleMesh;

pub struct LoadedDocument {
    /// Exactly what was parsed from the file. This is what gets exported, so a
    /// save writes the user's `<loop>` elements back verbatim.
    pub document: GdmlDocument,
    /// The same document with every `<loop>` expanded, when it had any.
    ///
    /// Geometry is built from this so the preview shows all N placements; the
    /// source above is untouched. Only `structure` and `solids` are read from
    /// it. Material reference edits must update both structures through
    /// `change_material_refs`; density edits use the source material table.
    pub render: Option<GdmlDocument>,
    pub engine: EvalEngine,
    pub meshes: HashMap<String, TriangleMesh>,
    pub warnings: Vec<String>,
}

impl LoadedDocument {
    /// The document geometry is built from: the expanded one when loops were
    /// present, otherwise the source.
    pub fn geometry(&self) -> &GdmlDocument {
        self.render.as_ref().unwrap_or(&self.document)
    }

    /// Every element export writes back verbatim: the preserved top-level ones
    /// and the `<loop>`s inside volumes.
    fn preserved_xml_mut(&mut self) -> impl Iterator<Item = &mut RawElement> {
        let doc = &mut self.document;
        doc.raw_unknown
            .iter_mut()
            .chain(doc.structure.volumes.iter_mut().flat_map(|v| &mut v.loops))
    }

    /// Whether preserved XML names `material` in a `<materialref>`, e.g. a
    /// volume declared inside a `<loop>`.
    pub fn preserved_xml_uses_material(&mut self, material: &str) -> anyhow::Result<bool> {
        for raw in self.preserved_xml_mut() {
            // Renaming to itself changes nothing; it only reports a match.
            if rename_material_refs(&raw.xml, material, material)?.is_some() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Change an assignment (one source volume) or cascade a rename (all volumes).
    /// Rewrite preserved references before committing anything that can fail.
    pub fn change_material_refs(
        &mut self,
        volume: Option<&str>,
        old: &str,
        new: &str,
    ) -> anyhow::Result<()> {
        if volume.is_none() {
            let rewritten = self
                .preserved_xml_mut()
                .map(|raw| rename_material_refs(&raw.xml, old, new))
                .collect::<anyhow::Result<Vec<_>>>()?;
            for (raw, xml) in self.preserved_xml_mut().zip(rewritten) {
                if let Some(xml) = xml {
                    raw.xml = xml;
                }
            }
        }
        for vol in self.document.structure.volumes.iter_mut().chain(
            self.render
                .iter_mut()
                .flat_map(|doc| &mut doc.structure.volumes),
        ) {
            if volume.is_none_or(|name| vol.name == name) && vol.material_ref == old {
                vol.material_ref = new.to_string();
            }
        }
        Ok(())
    }
}

/// Replace only an exact materialref, never comments, substrings, or other refs.
/// `None` when no materialref in `xml` names `old`.
fn rename_material_refs(xml: &str, old: &str, new: &str) -> anyhow::Result<Option<String>> {
    use quick_xml::events::{BytesStart, Event};
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut writer = quick_xml::Writer::new(Vec::new());
    let mut found = false;
    loop {
        let event = reader.read_event()?;
        if matches!(event, Event::Eof) {
            break;
        }
        let mut rewrite = |e: &BytesStart| -> anyhow::Result<BytesStart<'static>> {
            if e.local_name().as_ref() != b"materialref" {
                return Ok(e.clone().into_owned());
            }
            let mut replacement = e.clone().into_owned();
            replacement.clear_attributes();
            for attr in e.attributes() {
                let attr = attr?;
                let value = attr.decode_and_unescape_value(reader.decoder())?;
                let value = if attr.key.as_ref() == b"ref" && value == old {
                    found = true;
                    new
                } else {
                    &value
                };
                replacement.push_attribute((std::str::from_utf8(attr.key.as_ref())?, value));
            }
            Ok(replacement)
        };
        writer.write_event(match event {
            Event::Start(e) => Event::Start(rewrite(&e)?),
            Event::Empty(e) => Event::Empty(rewrite(&e)?),
            e => e.into_owned(),
        })?;
    }
    Ok(found
        .then(|| String::from_utf8(writer.into_inner()))
        .transpose()?)
}

pub type SharedState = Arc<RwLock<AppState>>;

#[derive(Default)]
pub struct AppState {
    pub loaded: Option<LoadedDocument>,
}

pub fn create_shared_state() -> SharedState {
    Arc::new(RwLock::new(AppState::default()))
}
