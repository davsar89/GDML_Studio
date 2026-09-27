use anyhow::Result;
use std::collections::BTreeSet;

/// Kahn's algorithm: an order in which every item comes after the items it
/// depends on (`deps[i]`). The lowest ready index goes first, so an order that
/// is already valid comes back unchanged. A cycle is an error naming the items
/// on or behind it.
pub fn dependency_order<'a>(
    deps: &[Vec<usize>],
    what: &str,
    name: impl Fn(usize) -> &'a str,
) -> Result<Vec<usize>> {
    let mut dependents = vec![Vec::new(); deps.len()];
    let mut remaining = vec![0usize; deps.len()];
    for (i, d) in deps.iter().enumerate() {
        let unique: BTreeSet<usize> = d.iter().copied().collect();
        remaining[i] = unique.len();
        for j in unique {
            dependents[j].push(i);
        }
    }
    let mut ready: BTreeSet<usize> = (0..deps.len()).filter(|&i| remaining[i] == 0).collect();
    let mut order = Vec::with_capacity(deps.len());
    while let Some(i) = ready.pop_first() {
        order.push(i);
        for &p in &dependents[i] {
            remaining[p] -= 1;
            if remaining[p] == 0 {
                ready.insert(p);
            }
        }
    }
    let stuck: Vec<&str> = (0..deps.len())
        .filter(|&i| remaining[i] > 0)
        .map(name)
        .collect();
    anyhow::ensure!(
        stuck.is_empty(),
        "Cyclic {what} references: {}",
        stuck.join(", ")
    );
    Ok(order)
}

/// Extract potential identifier references from an expression string.
/// Identifiers are sequences of [a-zA-Z_][a-zA-Z0-9_]* that aren't purely numeric.
pub fn extract_identifiers(expr: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = expr.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c.is_alphabetic() || c == '_' {
            current.push(c);
            i += 1;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                current.push(chars[i]);
                i += 1;
            }
            if !BUILTINS.contains(&current.as_str()) {
                ids.push(current.clone());
            }
            current.clear();
        } else if c.is_ascii_digit() || c == '.' {
            // A whole number literal: the exponent in `1e-3` does not name `e`.
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '.') {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    ids
}

/// Math functions and literals that evalexpr provides.
const BUILTINS: &[&str] = &[
    "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "sinh", "cosh", "tanh", "sqrt", "cbrt",
    "abs", "ln", "log", "log2", "log10", "exp", "exp2", "floor", "ceil", "round", "min", "max",
    "pow", "if", "true", "false",
];
