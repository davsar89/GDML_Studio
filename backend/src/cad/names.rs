//! GDML names for what an import generates.

use std::collections::HashSet;

/// `label` as a GDML name: ASCII letters, digits and `_`, not starting with a
/// digit, at most 64 characters, and never containing `0x`, where Geant4's
/// reader cuts a name off (G4GDMLRead::StripName, G4GDMLRead.cc:107).
pub fn sanitize(label: &str) -> String {
    let name: String = label
        .chars()
        .take(64)
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let name = if name.chars().all(|c| c == '_') {
        "part".to_string()
    } else if name.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{name}")
    } else {
        name
    };
    name.replace("0x", "0_x")
}

/// The names handed out so far. One set for solids, volumes and placements
/// keeps every generated name unique within each Geant4 store too.
#[derive(Default)]
pub struct Names(HashSet<String>);

impl Names {
    pub fn reserve(&mut self, name: &str) {
        self.0.insert(name.to_string());
    }

    /// A base name from `label` for which `base + s` is free for every suffix
    /// `s`, claiming those names. Repeats get `_2`, `_3`, ... in call order.
    pub fn claim(&mut self, label: &str, suffixes: &[impl AsRef<str>]) -> String {
        let stem = sanitize(label);
        let base = (1..)
            .map(|n| match n {
                1 => stem.clone(),
                n => format!("{stem}_{n}"),
            })
            .find(|base| {
                suffixes
                    .iter()
                    .all(|s| !self.0.contains(&format!("{base}{}", s.as_ref())))
            })
            .unwrap_or_default();
        for s in suffixes {
            self.reserve(&format!("{base}{}", s.as_ref()));
        }
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_become_valid_unique_names() {
        assert_eq!(sanitize("Bracket (left) #2"), "Bracket__left___2");
        assert_eq!(sanitize("3D part"), "_3D_part");
        assert_eq!(sanitize("plate0x1f"), "plate0_x1f");
        assert_eq!(sanitize("Ölmühle"), "_lm_hle");
        assert_eq!(sanitize("世界"), "part");
        let mut names = Names::default();
        names.reserve("World");
        let suffixes = ["", "_lv", "_pv"];
        assert_eq!(names.claim("World", &suffixes), "World_2");
        assert_eq!(names.claim("a", &suffixes), "a");
        assert_eq!(names.claim("a", &suffixes), "a_2");
        // "a_lv" is taken by the first "a", so this label moves on too.
        assert_eq!(names.claim("a_lv", &suffixes), "a_lv_2");
    }
}
