//! `<loop>` expansion.
//!
//! GDML's `<loop>` repeats its body with a variable bound to each value in a
//! range. `G4GDMLRead::LoopRead` (`G4GDMLRead.cc:186`) is the whole
//! specification:
//!
//! ```text
//! _var = _from;                       // when `from` is given
//! if (_from < _to && _step <= 0) fatal "Infinite loop!"
//! if (_from > _to && _step >= 0) fatal "Infinite loop!"
//! while (_var <= _to) { eval.SetVariable(var, _var); read(body); _var += _step; }
//! ```
//!
//! so `to` is inclusive, the variable must already exist
//! (`if(!eval.IsVariable(var))` is fatal), and the body is re-read once per
//! iteration with the variable rebound.
//!
//! Names are disambiguated by brackets. Inside a loop every name goes through
//! `G4GDMLEvaluator::SolveBrackets`, which turns `Slice[i]` into
//! `Slice_<int(i) - 1>`: note the **one-based to zero-based** shift.
//!
//! Expansion happens on the XML, before parsing: an iteration is the body with
//! the loop variable replaced by its literal value, so the ordinary parser and
//! evaluator handle the result with no per-iteration state. Nesting comes for
//! free: an inner `<loop>` is expanded when the outer body is re-scanned.
//!
//! This never touches the document that gets exported. It feeds a separate
//! render document, so a save still writes the user's `<loop>` back verbatim.

use anyhow::{bail, Result};
use quick_xml::events::{BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::io::Cursor;

use crate::eval::engine::EvalEngine;

/// Bytes all iterations together may expand to, each costing its body plus
/// one. Shared by the whole document because nesting multiplies: four nested
/// loops of 100 are 10^8 copies of the innermost body. Geant4 has no cap; this
/// is a service that parses untrusted uploads.
const MAX_EXPANSION: usize = 32 << 20;
/// Bound on how deep loops may nest, so a crafted file cannot recurse forever.
const MAX_LOOP_DEPTH: u32 = 16;

/// Replace whole-identifier occurrences of `var` with `value`.
///
/// Word boundaries are the point: substituting `i` must not touch `imax`,
/// `mini` or the `i` inside `sin`. GDML identifiers are the usual
/// `[A-Za-z_][A-Za-z0-9_]*`.
fn substitute_identifier(expr: &str, var: &str, value: i64) -> String {
    if var.is_empty() || !expr.contains(var) {
        return expr.to_string();
    }
    let bytes = expr.as_bytes();
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut out = String::with_capacity(expr.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i..].starts_with(var.as_bytes()) {
            let before_ok = i == 0 || !is_ident(bytes[i - 1]);
            let after = i + var.len();
            let after_ok = after >= bytes.len() || !is_ident(bytes[after]);
            if before_ok && after_ok {
                // Parenthesised so a negative value cannot re-associate:
                // "2*i" with i = -3 must be 2*(-3), not 2*-3.
                out.push('(');
                out.push_str(&value.to_string());
                out.push(')');
                i = after;
                continue;
            }
        }
        // Copy one whole UTF-8 character.
        let ch_len = expr[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        out.push_str(&expr[i..i + ch_len]);
        i += ch_len;
    }
    out
}

/// The values a loop's variable takes, in order. A step past `i64::MAX` ends
/// the loop; the expansion budget bounds how many are used.
fn iterations(from: i64, to: i64, step: i64) -> Result<impl Iterator<Item = i64>> {
    // Both guards are Geant4's (G4GDMLRead.cc:257-267). The second is
    // unreachable in practice (with from > to the `while (_var <= _to)` never
    // runs), but it is what the reference rejects, so it is rejected here.
    if from < to && step <= 0 {
        bail!("<loop> from={from} to={to} step={step} would never terminate");
    }
    if from > to && step >= 0 {
        bail!("<loop> from={from} to={to} step={step} would never terminate");
    }
    if step == 0 {
        bail!("<loop> step is 0");
    }
    Ok(
        std::iter::successors(Some(from), move |v| v.checked_add(step))
            .take_while(move |v| *v <= to),
    )
}

/// Apply every enclosing loop's binding, outermost first.
fn apply_bindings(expr: &str, bindings: &[(String, i64)]) -> String {
    bindings.iter().fold(expr.to_string(), |acc, (var, val)| {
        substitute_identifier(&acc, var, *val)
    })
}

/// Rewrite one element for the enclosing loops' bindings.
fn rewrite_element(
    e: &BytesStart,
    bindings: &[(String, i64)],
    engine: &EvalEngine,
) -> Result<BytesStart<'static>> {
    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
    let mut out = BytesStart::new(name);
    for attr in e.attributes() {
        let attr = attr?;
        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
        let raw = String::from_utf8_lossy(&attr.value).to_string();
        // Attributes are escaped again by push_attribute; decode before rewriting.
        let decoded = quick_xml::escape::unescape(&raw)?;
        let substituted = apply_bindings(&decoded, bindings);
        // `name` and `ref` go through GenerateName in Geant4, which is where
        // SolveBrackets is applied; value attributes only need the variable.
        let final_value = if key == "name" || key == "ref" {
            engine.solve_brackets(&substituted)?
        } else {
            substituted
        };
        out.push_attribute((key.as_str(), final_value.as_str()));
    }
    Ok(out)
}

/// Expand every `<loop>` in `xml`, returning the rewritten document.
///
/// `engine` is used only to evaluate `from`/`to`/`step` and bracket indices, so
/// it must already hold the document's defines.
pub fn expand_loops(xml: &str, engine: &EvalEngine) -> Result<String> {
    let mut budget = MAX_EXPANSION;
    expand_fragment(xml, engine, &[], 0, &mut budget)
}

/// One pass over a fragment, expanding any loops it contains.
///
/// `bindings` carries every enclosing loop's variable, outermost first: a
/// nested loop's body still refers to the outer variables.
fn expand_fragment(
    xml: &str,
    engine: &EvalEngine,
    bindings: &[(String, i64)],
    depth: u32,
    budget: &mut usize,
) -> Result<String> {
    if depth > MAX_LOOP_DEPTH {
        bail!("<loop> nesting deeper than {MAX_LOOP_DEPTH}");
    }
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(Event::Start(ref e)) if e.local_name().as_ref() == b"loop" => {
                let attrs = |k: &str| -> Option<String> {
                    e.attributes()
                        .flatten()
                        .find(|a| a.key.local_name().as_ref() == k.as_bytes())
                        .map(|a| String::from_utf8_lossy(&a.value).to_string())
                };
                let body = read_subtree_inner(&mut reader, e.name().as_ref())?;

                let Some(loop_var) = attrs("for") else {
                    bail!("<loop> has no `for` attribute naming its variable");
                };
                let resolve = |expr: Option<String>, default: f64| -> Result<f64> {
                    match expr {
                        Some(s) => engine.eval_expr(&apply_bindings(&s, bindings)),
                        None => Ok(default),
                    }
                };
                let from = match attrs("from") {
                    Some(expr) => resolve(Some(expr), 0.0)?,
                    None => engine.eval_expr(&loop_var)?,
                }
                .round() as i64;
                let to = resolve(attrs("to"), 0.0)?.round() as i64;
                let step = resolve(attrs("step"), 1.0)?.round() as i64;

                for value in iterations(from, to, step)? {
                    let Some(left) = budget.checked_sub(1 + body.len()) else {
                        bail!("<loop> expands past the {} MiB budget", MAX_EXPANSION >> 20);
                    };
                    *budget = left;
                    let mut inner: Vec<(String, i64)> = bindings.to_vec();
                    inner.push((loop_var.clone(), value));
                    let once = expand_fragment(&body, engine, &inner, depth + 1, budget)?;
                    writer.get_mut().write_all(once.as_bytes())?;
                }
            }
            Ok(Event::Start(ref e)) => {
                let ev = if bindings.is_empty() {
                    e.clone().into_owned()
                } else {
                    rewrite_element(e, bindings, engine)?
                };
                writer.write_event(Event::Start(ev))?;
            }
            Ok(Event::Empty(ref e)) => {
                let ev = if bindings.is_empty() {
                    e.clone().into_owned()
                } else {
                    rewrite_element(e, bindings, engine)?
                };
                writer.write_event(Event::Empty(ev))?;
            }
            Ok(Event::Text(t)) => {
                // <expression> bodies carry arithmetic too.
                let raw = String::from_utf8_lossy(&t).to_string();
                let s = apply_bindings(&raw, bindings);
                writer.write_event(Event::Text(BytesText::from_escaped(&s)))?;
            }
            Ok(ev) => writer.write_event(ev)?,
            Err(e) => bail!("XML error while expanding <loop>: {e}"),
        }
    }

    use std::io::Write;
    Ok(String::from_utf8(writer.into_inner().into_inner())?)
}

/// Read to the matching end tag, returning the inner XML without the wrapper.
fn read_subtree_inner(reader: &mut Reader<&[u8]>, tag: &[u8]) -> Result<String> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let mut depth = 1usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                if e.name().as_ref() == tag {
                    depth += 1;
                }
                writer.write_event(Event::Start(e.clone().into_owned()))?;
            }
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == tag {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                writer.write_event(Event::End(e.clone().into_owned()))?;
            }
            Ok(Event::Eof) => bail!("unterminated <loop>"),
            Ok(ev) => writer.write_event(ev)?,
            Err(e) => bail!("XML error inside <loop>: {e}"),
        }
    }
    Ok(String::from_utf8(writer.into_inner().into_inner())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_substitution_respects_word_boundaries() {
        assert_eq!(substitute_identifier("i*10", "i", 3), "(3)*10");
        assert_eq!(substitute_identifier("imax", "i", 3), "imax");
        assert_eq!(substitute_identifier("mini", "i", 3), "mini");
        assert_eq!(substitute_identifier("sin(i)", "i", 3), "sin((3))");
        assert_eq!(substitute_identifier("i+i", "i", 3), "(3)+(3)");
        assert_eq!(substitute_identifier("x_i", "i", 3), "x_i");
        // Parenthesising matters for negatives.
        assert_eq!(substitute_identifier("2*i", "i", -3), "2*(-3)");
    }

    #[test]
    fn iteration_ranges_follow_the_reference() {
        let vals = |from, to, step| iterations(from, to, step).unwrap().collect::<Vec<_>>();
        assert_eq!(vals(1, 4, 1), vec![1, 2, 3, 4]); // `to` inclusive
        assert_eq!(vals(0, 6, 2), vec![0, 2, 4, 6]);
        assert_eq!(vals(0, 5, 2), vec![0, 2, 4]);
        assert_eq!(vals(3, 3, 1), vec![3]);
        // Both of Geant4's "Infinite loop!" guards.
        assert!(iterations(1, 4, 0).is_err());
        assert!(iterations(1, 4, -1).is_err());
        assert!(iterations(4, 1, 1).is_err());
    }
}

#[cfg(test)]
mod expand_tests {
    use super::*;

    fn engine() -> EvalEngine {
        EvalEngine::new()
    }

    #[test]
    fn a_loop_becomes_one_copy_per_iteration() {
        let xml = r#"<structure><volume name="W">
  <loop for="i" from="1" to="3" step="1">
    <physvol><volumeref ref="Inner"/><position name="p" z="i*10"/></physvol>
  </loop>
</volume></structure>"#;
        let out = expand_loops(xml, &engine()).unwrap();
        assert_eq!(out.matches("<physvol>").count(), 3, "{out}");
        assert!(out.contains(r#"z="(1)*10""#), "{out}");
        assert!(out.contains(r#"z="(2)*10""#), "{out}");
        assert!(out.contains(r#"z="(3)*10""#), "{out}");
        assert!(
            !out.contains("<loop"),
            "the loop element must be consumed:\n{out}"
        );
    }

    #[test]
    fn names_are_disambiguated_by_brackets() {
        // SolveBrackets: Slice[i] -> Slice_<i-1>
        let xml = r#"<solids><loop for="i" from="1" to="3" step="1">
  <box name="Slice[i]" x="1" y="1" z="i"/>
</loop></solids>"#;
        let out = expand_loops(xml, &engine()).unwrap();
        for want in ["Slice_0", "Slice_1", "Slice_2"] {
            assert!(out.contains(want), "missing {want}:\n{out}");
        }
    }

    #[test]
    fn nested_loops_expand_to_the_product() {
        let xml = r#"<structure><loop for="i" from="1" to="2" step="1">
  <loop for="j" from="1" to="3" step="1">
    <physvol name="p[i]"><position x="i" y="j"/></physvol>
  </loop>
</loop></structure>"#;
        let out = expand_loops(xml, &engine()).unwrap();
        assert_eq!(
            out.matches("<physvol").count(),
            6,
            "2 x 3 iterations:\n{out}"
        );
        assert!(out.contains(r#"x="(1)" y="(1)""#), "{out}");
        assert!(out.contains(r#"x="(2)" y="(3)""#), "{out}");
    }

    #[test]
    fn a_document_without_loops_is_unchanged_in_content() {
        let xml = r#"<gdml><solids><box name="B" x="1" y="2" z="3"/></solids></gdml>"#;
        let out = expand_loops(xml, &engine()).unwrap();
        assert!(
            out.contains(r#"name="B""#) && out.contains(r#"x="1""#),
            "{out}"
        );
        assert_eq!(out.matches("<box").count(), 1);
    }

    #[test]
    fn a_runaway_loop_is_refused_rather_than_hanging() {
        let xml =
            r#"<structure><loop for="i" from="1" to="4" step="-1"><physvol/></loop></structure>"#;
        assert!(expand_loops(xml, &engine()).is_err());
    }
}
