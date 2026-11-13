//! STL reader (algorithm A in `doc/cad-import/PLAN.md`). STL carries no
//! units and no topology: a soup of triangles, binary or text.

use super::intersect::P;

/// A `solid` block of a text file, or the whole of a binary one.
pub struct Solid {
    pub name: String,
    pub triangles: Vec<[P; 3]>,
}

/// Parse an STL file, multiplying coordinates by `unit_mm`, the length of
/// one file unit in mm. Stored normals are ignored: the winding decides.
/// `fallback_name` names a binary file and unnamed text blocks.
pub fn parse(bytes: &[u8], unit_mm: f64, fallback_name: &str) -> Result<Vec<Solid>, String> {
    if bytes.is_empty() {
        return Err("the file is empty".to_string());
    }
    let mut solids = if is_binary(bytes) {
        vec![binary(bytes, fallback_name)]
    } else {
        text(bytes, fallback_name)?
    };
    for c in solids
        .iter_mut()
        .flat_map(|s| &mut s.triangles)
        .flatten()
        .flatten()
    {
        if !c.is_finite() {
            return Err("a vertex coordinate is not a finite number".to_string());
        }
        *c *= unit_mm;
    }
    solids.retain(|s| !s.triangles.is_empty());
    if solids.is_empty() {
        return Err("the file contains no facets".to_string());
    }
    Ok(solids)
}

/// A binary file is exactly 84 + 50 n bytes for the n in its header, even
/// when the 80-byte header starts with `solid`, as some exporters write.
fn is_binary(bytes: &[u8]) -> bool {
    bytes.len() >= 84 && {
        let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]);
        bytes.len() as u64 == 84 + 50 * n as u64
    }
}

/// Each 50-byte record: normal, three vertices (little-endian f32), and a
/// 2-byte attribute.
fn binary(bytes: &[u8], name: &str) -> Solid {
    let (records, _) = bytes[84..].as_chunks::<50>();
    let triangles = records
        .iter()
        .map(|r| {
            let f = |k: usize| {
                let o = 12 + 4 * k;
                f32::from_le_bytes([r[o], r[o + 1], r[o + 2], r[o + 3]]) as f64
            };
            [[f(0), f(1), f(2)], [f(3), f(4), f(5)], [f(6), f(7), f(8)]]
        })
        .collect();
    Solid {
        name: name.to_string(),
        triangles,
    }
}

/// The text form: `solid name`, then per facet `facet normal ..`,
/// `outer loop`, three `vertex x y z`, `endloop`, `endfacet`, and finally
/// `endsolid`. Keywords are case-insensitive; one statement per line.
fn text(bytes: &[u8], fallback_name: &str) -> Result<Vec<Solid>, String> {
    let text = String::from_utf8_lossy(bytes);
    if !text
        .trim_start()
        .get(..5)
        .is_some_and(|w| w.eq_ignore_ascii_case("solid"))
    {
        return Err(format!(
            "not an STL file: a text STL starts with 'solid', and a binary one is \
             84 + 50 n bytes long for its n facets (this file is {} bytes)",
            bytes.len()
        ));
    }
    let mut solids = Vec::new();
    let mut solid: Option<Solid> = None;
    let mut facet: Option<Vec<P>> = None;
    for (i, line) in text.lines().enumerate() {
        let at = |message: &str| format!("line {}: {message}", i + 1);
        let mut words = line.split_whitespace();
        let Some(keyword) = words.next() else {
            continue;
        };
        match keyword.to_ascii_lowercase().as_str() {
            "solid" if solid.is_none() => {
                let name = line.trim_start()[5..].trim();
                solid = Some(Solid {
                    name: if name.is_empty() { fallback_name } else { name }.to_string(),
                    triangles: Vec::new(),
                });
            }
            "facet" if solid.is_some() && facet.is_none() => facet = Some(Vec::with_capacity(3)),
            "outer" | "endloop" if facet.is_some() => {}
            "vertex" => {
                let vertices = facet
                    .as_mut()
                    .ok_or_else(|| at("'vertex' outside a facet"))?;
                let mut p = [0.0; 3];
                for c in &mut p {
                    *c = words
                        .next()
                        .and_then(|w| w.parse().ok())
                        .ok_or_else(|| at("a vertex needs three numbers"))?;
                }
                vertices.push(p);
            }
            "endfacet" if facet.is_some() => {
                let vertices = facet.take().unwrap_or_default();
                let [a, b, c] = vertices[..] else {
                    return Err(at(&format!(
                        "a facet with {} vertices; STL facets are triangles",
                        vertices.len()
                    )));
                };
                if let Some(s) = solid.as_mut() {
                    s.triangles.push([a, b, c]);
                }
            }
            "endsolid" if facet.is_none() => {
                solids.push(
                    solid
                        .take()
                        .ok_or_else(|| at("'endsolid' without 'solid'"))?,
                );
            }
            _ => return Err(at(&format!("unexpected '{keyword}'"))),
        }
    }
    if solid.is_some() {
        return Err("the file ends inside a solid (no 'endsolid'): it is truncated".to_string());
    }
    Ok(solids)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FACET: &str = "facet normal 0 0 1\n outer loop\n  vertex 0 0 0\n  vertex 1 0 0\n  vertex 0 1 0\n endloop\nendfacet\n";

    #[test]
    fn text_blocks_become_named_solids_scaled_to_mm() {
        let file = format!(
            "solid first part\n{FACET}endsolid first part\nSOLID\n{FACET}{FACET}ENDSOLID\n"
        );
        let solids = parse(file.as_bytes(), 25.4, "fallback").unwrap();
        assert_eq!(solids.len(), 2);
        assert_eq!(solids[0].name, "first part");
        assert_eq!(solids[1].name, "fallback");
        assert_eq!(solids[1].triangles.len(), 2);
        assert_eq!(solids[0].triangles[0][1], [25.4, 0.0, 0.0]);
    }

    #[test]
    fn crlf_and_exponents_are_accepted() {
        let file = "solid s\r\nfacet normal 0 0 1\r\nouter loop\r\nvertex 1e-3 0 0\r\nvertex 1.5E+1 0 0\r\nvertex 0 -2.5e0 0\r\nendloop\r\nendfacet\r\nendsolid s\r\n";
        let solids = parse(file.as_bytes(), 1.0, "f").unwrap();
        assert_eq!(
            solids[0].triangles[0],
            [[1e-3, 0.0, 0.0], [15.0, 0.0, 0.0], [0.0, -2.5, 0.0]]
        );
    }

    #[test]
    fn malformed_text_is_rejected_with_its_line() {
        let quad = "solid s\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 1 1 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid\n";
        assert_eq!(
            parse(quad.as_bytes(), 1.0, "f").err().unwrap(),
            "line 9: a facet with 4 vertices; STL facets are triangles"
        );
        let truncated = format!("solid s\n{FACET}");
        assert!(parse(truncated.as_bytes(), 1.0, "f")
            .err()
            .unwrap()
            .contains("truncated"));
        let nan = FACET.replace("vertex 1 0 0", "vertex nan 0 0");
        assert!(
            parse(format!("solid s\n{nan}endsolid\n").as_bytes(), 1.0, "f")
                .err()
                .unwrap()
                .contains("finite")
        );
        assert!(parse(b"", 1.0, "f").err().unwrap().contains("empty"));
        assert!(parse(b"PK\x03\x04 not a mesh", 1.0, "f")
            .err()
            .unwrap()
            .starts_with("not an STL file"));
        assert!(parse(b"solid s\nendsolid s\n", 1.0, "f")
            .err()
            .unwrap()
            .contains("no facets"));
    }

    fn binary_file(header: &[u8], triangles: &[[[f32; 3]; 3]]) -> Vec<u8> {
        let mut out = header.to_vec();
        out.resize(80, b' ');
        out.extend((triangles.len() as u32).to_le_bytes());
        for t in triangles {
            out.extend([0u8; 12]);
            for c in t.iter().flatten() {
                out.extend(c.to_le_bytes());
            }
            out.extend([0u8; 2]);
        }
        out
    }

    #[test]
    fn binary_is_recognized_by_its_length_even_with_a_solid_header() {
        let t = [[0.5f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let file = binary_file(b"solid exported by some tool", &[t, t]);
        let solids = parse(&file, 1.0, "part").unwrap();
        assert_eq!(solids[0].name, "part");
        assert_eq!(solids[0].triangles.len(), 2);
        assert_eq!(solids[0].triangles[0][0], [0.5, 0.0, 0.0]);
        // One byte short: neither binary nor valid text.
        assert!(parse(&file[..file.len() - 1], 1.0, "part").is_err());
    }
}
