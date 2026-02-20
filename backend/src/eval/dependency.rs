use std::collections::{HashMap, HashSet, VecDeque};
use anyhow::{Result, anyhow};

#[derive(Debug, Clone)]
pub struct DefineEntry {
    pub name: String,
    pub expression: String,
}

pub fn topological_sort(entries: &[DefineEntry], known: &HashSet<String>) -> Result<Vec<usize>> {
    let name_to_idx: HashMap<&str, usize> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.name.as_str(), i))
        .collect();

    let n = entries.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut in_degree: Vec<usize> = vec![0; n];

    for (i, entry) in entries.iter().enumerate() {
        let refs = extract_identifiers(&entry.expression);
        for ref_name in &refs {
            if known.contains(ref_name.as_str()) {
                continue; // built-in, no dependency
            }
            if let Some(&j) = name_to_idx.get(ref_name.as_str()) {
                if j != i {
                    adj[j].push(i);
                    in_degree[i] += 1;
                }
            }
            // If not found, it might be a number or we'll handle the error at eval time
        }
    }

    // Kahn's algorithm
    let mut queue: VecDeque<usize> = VecDeque::new();
    for i in 0..n {
        if in_degree[i] == 0 {
            queue.push_back(i);
        }
    }

    let mut order = Vec::with_capacity(n);
    while let Some(u) = queue.pop_front() {
        order.push(u);
        for &v in &adj[u] {
            in_degree[v] -= 1;
            if in_degree[v] == 0 {
                queue.push_back(v);
            }
        }
    }

    if order.len() != n {
        // Find cycle participants for error message
        let in_cycle: Vec<&str> = (0..n)
            .filter(|i| in_degree[*i] > 0)
            .map(|i| entries[i].name.as_str())
            .collect();
        return Err(anyhow!(
            "Cyclic dependency detected among defines: {:?}",
            in_cycle
        ));
    }

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
            // Filter out common math functions that evalexpr provides
            if !is_builtin_function(&current) {
                ids.push(current.clone());
            }
            current.clear();
        } else {
            i += 1;
        }
    }

    ids
}

fn is_builtin_function(name: &str) -> bool {
    matches!(
        name,
        "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "atan2"
            | "sinh" | "cosh" | "tanh"
            | "sqrt" | "cbrt" | "abs"
            | "ln" | "log" | "log2" | "log10"
            | "exp" | "exp2"
            | "floor" | "ceil" | "round"
            | "min" | "max"
            | "pow"
            | "if" | "true" | "false"
    )
}
