use anyhow::{Context, Result};
use evalexpr::*;
use std::collections::{HashMap, HashSet};

use super::context::EvalContext;
use super::dependency::{extract_identifiers, topological_sort, DefineEntry};
use crate::gdml::model::{DefineSection, Position, Rotation};
use crate::gdml::units;

pub struct EvalEngine {
    pub context: EvalContext,
    pub position_values: HashMap<String, [f64; 3]>,
    pub scale_values: HashMap<String, [f64; 3]>,
    pub rotation_values: HashMap<String, [f64; 3]>,
    pub length_symbols: HashSet<String>,
    pub angle_symbols: HashSet<String>,
    /// Non-fatal evaluation warnings collected during value resolution
    /// (e.g. expressions that failed and were treated as 0). Behind a `Mutex`
    /// rather than a `RefCell` so `EvalEngine` stays `Sync` (it lives inside
    /// `AppState` behind a `tokio::RwLock`).
    warnings: std::sync::Mutex<Vec<String>>,
}

impl Default for EvalEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl EvalEngine {
    pub fn new() -> Self {
        Self {
            context: EvalContext::new(),
            position_values: HashMap::new(),
            rotation_values: HashMap::new(),
            scale_values: HashMap::new(),
            length_symbols: HashSet::new(),
            angle_symbols: HashSet::new(),
            warnings: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn evaluate_all(&mut self, defines: &DefineSection) -> Result<()> {
        // Rebuild derived state from scratch for the current document.
        self.context = EvalContext::new();
        self.position_values.clear();
        self.scale_values.clear();
        self.rotation_values.clear();
        self.length_symbols.clear();
        self.angle_symbols.clear();
        if let Ok(mut w) = self.warnings.lock() {
            w.clear();
        }

        // Collect all define entries for dependency analysis
        let mut entries: Vec<DefineEntry> = Vec::new();

        for c in &defines.constants {
            entries.push(DefineEntry {
                name: c.name.clone(),
                expression: c.value.clone(),
            });
        }
        for q in &defines.quantities {
            entries.push(DefineEntry {
                name: q.name.clone(),
                expression: q.value.clone(),
            });
        }
        for v in &defines.variables {
            entries.push(DefineEntry {
                name: v.name.clone(),
                expression: v.value.clone(),
            });
        }
        for e in &defines.expressions {
            entries.push(DefineEntry {
                name: e.name.clone(),
                expression: e.value.clone(),
            });
        }

        // Build known set (builtins)
        let known: HashSet<String> = self.context.values.keys().cloned().collect();

        // Topological sort
        let order =
            topological_sort(&entries, &known).context("Failed to resolve define dependencies")?;

        // Build unit map for quantities
        let quantity_units: HashMap<String, String> = defines
            .quantities
            .iter()
            .filter_map(|q| q.unit.as_ref().map(|u| (q.name.clone(), u.clone())))
            .collect();
        // `type` is optional and Geant4's QuantityRead never reads it — the unit
        // is applied unconditionally. So a quantity's kind comes from its unit
        // when the type is absent or unrecognised; gating purely on `type` left
        // `<quantity name="w" value="5" unit="cm"/>` unconverted, giving 5 mm
        // where Geant4 gives 50.
        let quantity_kind = |q: &crate::gdml::model::Quantity| -> Option<units::UnitKind> {
            match q.r#type.as_deref() {
                Some("length") => Some(units::UnitKind::Length),
                Some("angle") => Some(units::UnitKind::Angle),
                Some("density") => None,
                _ => q.unit.as_deref().and_then(units::unit_kind),
            }
        };

        let length_quantity_names: HashSet<&str> = defines
            .quantities
            .iter()
            .filter(|q| quantity_kind(q) == Some(units::UnitKind::Length))
            .map(|q| q.name.as_str())
            .collect();
        let angle_quantity_names: HashSet<&str> = defines
            .quantities
            .iter()
            .filter(|q| quantity_kind(q) == Some(units::UnitKind::Angle))
            .map(|q| q.name.as_str())
            .collect();

        // Evaluate in order
        for idx in order {
            let entry = &entries[idx];
            let refs = extract_identifiers(&entry.expression);
            let is_length_symbol = length_quantity_names.contains(entry.name.as_str())
                || refs.iter().any(|name| self.length_symbols.contains(name));
            let is_angle_symbol = angle_quantity_names.contains(entry.name.as_str())
                || refs.iter().any(|name| self.angle_symbols.contains(name));

            // A define that cannot be evaluated is a warning, not a fatal error.
            // The identical expression appearing in a solid attribute already
            // degrades to 0 with a warning via `resolve_value`; propagating here
            // instead meant one bad <constant> returned HTTP 500 and the user saw
            // nothing at all rather than a geometry with one hole in it.
            let value = match self.eval_expr(&entry.expression) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        "Failed to evaluate define '{}' = '{}': {} -- using 0.0",
                        entry.name,
                        entry.expression,
                        e
                    );
                    self.record_warning(format!(
                        "Could not evaluate define \"{}\" = \"{}\"; treated as 0. \
                         Anything referencing it will be wrong.",
                        entry.name,
                        entry.expression.trim()
                    ));
                    0.0
                }
            };

            // Apply unit conversion for quantities
            let final_value = if let Some(unit) = quantity_units.get(&entry.name) {
                let qty = defines.quantities.iter().find(|q| q.name == entry.name);
                if let Some(q) = qty {
                    match quantity_kind(q) {
                        Some(units::UnitKind::Length) => units::length_to_mm(value, unit),
                        Some(units::UnitKind::Angle) => units::angle_to_rad(value, unit),
                        // type="density" and anything with an unrecognised unit
                        // are carried through as written.
                        None => value,
                    }
                } else {
                    value
                }
            } else {
                value
            };

            self.context.set(&entry.name, final_value);
            if is_length_symbol {
                self.length_symbols.insert(entry.name.clone());
            }
            if is_angle_symbol {
                self.angle_symbols.insert(entry.name.clone());
            }
        }

        // Evaluate positions
        for pos in &defines.positions {
            let values = self.eval_position(pos)?;
            self.position_values.insert(pos.name.clone(), values);
        }

        // Evaluate rotations
        for rot in &defines.rotations {
            let values = self.eval_rotation(rot)?;
            self.rotation_values.insert(rot.name.clone(), values);
        }

        // Evaluate named <scale> defines. Dimensionless, so no unit handling.
        for sc in &defines.scales {
            let comp = |e: &Option<String>| -> f64 {
                e.as_deref().map(|x| self.resolve_value(x)).unwrap_or(1.0)
            };
            let values = [comp(&sc.x), comp(&sc.y), comp(&sc.z)];
            self.scale_values.insert(sc.name.clone(), values);
        }

        Ok(())
    }

    pub fn eval_expr(&self, expr: &str) -> Result<f64> {
        let expr = expr.trim();
        if expr.is_empty() {
            return Ok(0.0);
        }

        // Try direct numeric parse first
        if let Ok(v) = expr.parse::<f64>() {
            return Ok(v);
        }

        // Try direct variable lookup
        if let Some(v) = self.context.get(expr) {
            return Ok(v);
        }

        // Build evalexpr context with all known values
        let mut eval_context: HashMapContext = HashMapContext::new();
        for (name, value) in &self.context.values {
            eval_context
                .set_value(name.clone(), Value::Float(*value))
                .ok();
        }

        // Rewrite GDML/CLHEP conventions for evalexpr:
        //  - bare math functions (sin, sqrt, …) live under the `math::` namespace
        //  - `**` is power in some GDML; evalexpr spells power as `^`
        // We use `eval_number_with_context` (not `eval_float_*`) so integer-valued
        // results (e.g. "2*3", "360/8") are coerced to f64 instead of erroring.
        let prepared = rewrite_math_functions(expr).replace("**", "^");
        let result = eval_number_with_context(&prepared, &eval_context);
        match result {
            Ok(v) => Ok(v),
            Err(_) => {
                // Try with some fixups for GDML patterns (e.g. "360.*deg" -> "360.0*deg")
                let fixed = fix_gdml_expr(&prepared);
                eval_number_with_context(&fixed, &eval_context)
                    .map_err(|e| anyhow::anyhow!("evalexpr error for '{}': {}", expr, e))
            }
        }
    }

    fn eval_position(&self, pos: &Position) -> Result<[f64; 3]> {
        let unit = pos.unit.as_deref().unwrap_or("mm");
        // Components whose expression references an already-converted length
        // quantity are in mm already; converting again would double-apply the unit.
        let comp = |expr: &Option<String>| -> Result<f64> {
            match expr {
                Some(e) => {
                    let v = self.eval_expr(e)?;
                    Ok(if self.expression_uses_length_symbols(e) {
                        v
                    } else {
                        units::length_to_mm(v, unit)
                    })
                }
                None => Ok(0.0),
            }
        };
        Ok([comp(&pos.x)?, comp(&pos.y)?, comp(&pos.z)?])
    }

    fn eval_rotation(&self, rot: &Rotation) -> Result<[f64; 3]> {
        let unit = rot.unit.as_deref().unwrap_or("rad");
        // Same guard as eval_position, for angle quantities already in radians.
        let comp = |expr: &Option<String>| -> Result<f64> {
            match expr {
                Some(e) => {
                    let v = self.eval_expr(e)?;
                    Ok(if self.expression_uses_angle_symbols(e) {
                        v
                    } else {
                        units::angle_to_rad(v, unit)
                    })
                }
                None => Ok(0.0),
            }
        };
        Ok([comp(&rot.x)?, comp(&rot.y)?, comp(&rot.z)?])
    }

    pub fn resolve_value(&self, expr_str: &str) -> f64 {
        match self.eval_expr(expr_str) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("Failed to evaluate '{}': {} -- using 0.0", expr_str, e);
                self.record_warning(format!(
                    "Could not evaluate expression \"{}\"; treated as 0.",
                    expr_str.trim()
                ));
                0.0
            }
        }
    }

    /// Record a non-fatal evaluation warning (deduplicated, capped to avoid flooding).
    /// Record a warning from outside the evaluator (deduplicated and capped
    /// alongside evaluation warnings, and surfaced through the same channel).
    pub fn record_warning_public(&self, msg: String) {
        self.record_warning(msg);
    }

    /// Report a `lunit`/`aunit` string that is not in Geant4's unit table.
    ///
    /// Geant4 refuses to load such a file; this viewer falls back to the base
    /// unit, which silently renders wrong geometry, so the fallback has to be
    /// visible.
    pub fn record_unit_warning(&self, unit: &str, kind: &str) {
        self.record_warning(format!(
            "Unknown {kind} unit \"{unit}\"; treated as {}. Geant4 would reject \
             this file — check the spelling (note `micron` and `dm` are not GDML \
             units; use `um` and `cm`).",
            if kind == "length" { "mm" } else { "rad" }
        ));
    }

    fn record_warning(&self, msg: String) {
        if let Ok(mut warnings) = self.warnings.lock() {
            if warnings.len() < 100 && !warnings.contains(&msg) {
                warnings.push(msg);
            }
        }
    }

    /// Drain accumulated non-fatal evaluation warnings. Returns and clears the buffer.
    pub fn take_warnings(&self) -> Vec<String> {
        self.warnings
            .lock()
            .map(|mut w| std::mem::take(&mut *w))
            .unwrap_or_default()
    }

    pub fn expression_uses_length_symbols(&self, expr: &str) -> bool {
        let trimmed = expr.trim();
        if self.length_symbols.contains(trimmed) {
            return true;
        }
        extract_identifiers(trimmed)
            .iter()
            .any(|name| self.length_symbols.contains(name))
    }

    pub fn expression_uses_angle_symbols(&self, expr: &str) -> bool {
        let trimmed = expr.trim();
        if self.angle_symbols.contains(trimmed) {
            return true;
        }
        extract_identifiers(trimmed)
            .iter()
            .any(|name| self.angle_symbols.contains(name))
    }

    pub fn resolve_position(&self, pos: &PlacementPosRef) -> [f64; 3] {
        match pos {
            PlacementPosRef::Values(v) => *v,
            PlacementPosRef::Name(name) => {
                self.position_values.get(name).copied().unwrap_or([0.0; 3])
            }
        }
    }

    pub fn resolve_rotation(&self, rot: &PlacementRotRef) -> [f64; 3] {
        match rot {
            PlacementRotRef::Values(v) => *v,
            PlacementRotRef::Name(name) => {
                self.rotation_values.get(name).copied().unwrap_or([0.0; 3])
            }
        }
    }
}

pub enum PlacementPosRef {
    Values([f64; 3]),
    Name(String),
}

pub enum PlacementRotRef {
    Values([f64; 3]),
    Name(String),
}

/// Fix common GDML expression patterns for evalexpr.
/// Handles any `<digit>.*` pattern (e.g. "360.*deg", "-1.*pi") by inserting ".0*".
/// Iterates over chars (not bytes) so multi-byte UTF-8 input passes through intact.
fn fix_gdml_expr(expr: &str) -> String {
    let chars: Vec<char> = expr.chars().collect();
    let mut result = String::with_capacity(expr.len() + 8);
    let mut i = 0;
    while i < chars.len() {
        result.push(chars[i]);
        if chars[i].is_ascii_digit()
            && i + 2 < chars.len()
            && chars[i + 1] == '.'
            && chars[i + 2] == '*'
        {
            // Convert "N.*" to "N.0*"
            result.push_str(".0*");
            i += 3;
        } else {
            i += 1;
        }
    }
    result
}

/// Map a bare GDML/CLHEP math-function name to evalexpr's `math::` namespace.
/// Returns `None` for names that are NOT namespaced in evalexpr (`floor`, `ceil`,
/// `round`, `min`, `max`) or that aren't functions. Note GDML `log` is the natural
/// logarithm (CLHEP), which is `math::ln` in evalexpr (`math::log` there is 2-arg).
fn map_math_function(name: &str) -> Option<&'static str> {
    Some(match name {
        "sin" => "math::sin",
        "cos" => "math::cos",
        "tan" => "math::tan",
        "asin" => "math::asin",
        "acos" => "math::acos",
        "atan" => "math::atan",
        "atan2" => "math::atan2",
        "sinh" => "math::sinh",
        "cosh" => "math::cosh",
        "tanh" => "math::tanh",
        "exp" => "math::exp",
        "exp2" => "math::exp2",
        "log" | "ln" => "math::ln",
        "log2" => "math::log2",
        "log10" => "math::log10",
        "sqrt" => "math::sqrt",
        "cbrt" => "math::cbrt",
        "abs" => "math::abs",
        "pow" => "math::pow",
        "hypot" => "math::hypot",
        _ => return None,
    })
}

/// Rewrite bare math-function calls (`sin(...)`, `sqrt(...)`) to evalexpr's
/// `math::` namespace. Only identifiers immediately followed by `(` are treated as
/// calls, and identifiers already prefixed with `::` are left untouched, so this is
/// idempotent and never touches variable names that merely look like a function.
fn rewrite_math_functions(expr: &str) -> String {
    let chars: Vec<char> = expr.chars().collect();
    let mut out = String::with_capacity(expr.len() + 16);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_alphabetic() || c == '_' {
            let start = i;
            let mut ident = String::new();
            ident.push(c);
            i += 1;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                ident.push(chars[i]);
                i += 1;
            }
            // Is the next non-space character a '(' (i.e. a function call)?
            let mut j = i;
            while j < chars.len() && chars[j] == ' ' {
                j += 1;
            }
            let is_call = j < chars.len() && chars[j] == '(';
            // Already namespaced (preceded by "::")? Then leave as-is.
            let already_ns = start >= 2 && chars[start - 1] == ':' && chars[start - 2] == ':';
            if is_call && !already_ns {
                if let Some(mapped) = map_math_function(&ident) {
                    out.push_str(mapped);
                    continue;
                }
            }
            out.push_str(&ident);
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gdml::model::{Constant, DefineSection, Quantity};

    fn quantity(name: &str, value: &str, unit: Option<&str>, ty: Option<&str>) -> Quantity {
        Quantity {
            name: name.to_string(),
            r#type: ty.map(str::to_string),
            value: value.to_string(),
            unit: unit.map(str::to_string),
        }
    }

    fn constant(name: &str, value: &str) -> Constant {
        Constant {
            name: name.to_string(),
            value: value.to_string(),
        }
    }

    #[test]
    fn clhep_length_symbols_are_usable_in_expressions() {
        // Geant4 evaluates GDML expressions through an evaluator preloaded with
        // CLHEP's units, so `2*cm` is valid GDML. Without these bindings the
        // lookup failed and the whole document 500'd.
        let engine = EvalEngine::new();
        assert_eq!(engine.eval_expr("2*cm").unwrap(), 20.0);
        assert_eq!(engine.eval_expr("5*mm").unwrap(), 5.0);
        assert_eq!(engine.eval_expr("1*m").unwrap(), 1000.0);
        assert_eq!(engine.eval_expr("3*um").unwrap(), 3.0e-3);
    }

    #[test]
    fn untyped_quantity_still_applies_its_unit() {
        // G4GDMLReadDefine::QuantityRead never reads `type`; it multiplies by
        // the unit unconditionally. Gating on type left this at 5 instead of 50.
        let mut defines = DefineSection::default();
        defines
            .quantities
            .push(quantity("untyped", "5", Some("cm"), None));
        defines
            .quantities
            .push(quantity("typed", "5", Some("cm"), Some("length")));
        defines
            .quantities
            .push(quantity("degrees", "180", Some("deg"), None));

        let mut engine = EvalEngine::new();
        engine.evaluate_all(&defines).unwrap();

        assert_eq!(engine.eval_expr("untyped").unwrap(), 50.0);
        assert_eq!(engine.eval_expr("typed").unwrap(), 50.0);
        assert!((engine.eval_expr("degrees").unwrap() - std::f64::consts::PI).abs() < 1e-12);
    }

    #[test]
    fn unevaluable_define_warns_instead_of_failing_the_document() {
        // One bad expression used to abort the whole load with a 500. The same
        // expression in a solid attribute has always degraded to 0 with a
        // warning; defines now match.
        let mut defines = DefineSection::default();
        defines.constants.push(constant("good", "10"));
        defines
            .constants
            .push(constant("bad", "nonexistent_symbol*2"));

        let mut engine = EvalEngine::new();
        engine
            .evaluate_all(&defines)
            .expect("one bad define must not fail the document");

        assert_eq!(engine.eval_expr("good").unwrap(), 10.0);
        assert_eq!(engine.eval_expr("bad").unwrap(), 0.0);

        let warnings = engine.take_warnings();
        assert!(
            warnings.iter().any(|w| w.contains("bad")),
            "expected a warning naming the bad define, got {:?}",
            warnings
        );
    }

    #[test]
    fn integer_result_expressions_evaluate() {
        let e = EvalEngine::new();
        assert_eq!(e.eval_expr("2*3").unwrap(), 6.0);
        assert_eq!(e.eval_expr("360/8").unwrap(), 45.0); // integer division
        assert_eq!(e.eval_expr("10-1").unwrap(), 9.0);
    }

    #[test]
    fn mixed_int_float_arithmetic_evaluates() {
        let e = EvalEngine::new();
        let v = e.eval_expr("2*pi").unwrap();
        assert!((v - 2.0 * std::f64::consts::PI).abs() < 1e-9);
    }

    #[test]
    fn bare_math_functions_evaluate() {
        let e = EvalEngine::new();
        assert!((e.eval_expr("sqrt(2)*2").unwrap() - 2.0 * std::f64::consts::SQRT_2).abs() < 1e-9);
        assert!(e.eval_expr("sin(0)").unwrap().abs() < 1e-12);
        assert!((e.eval_expr("cos(0)").unwrap() - 1.0).abs() < 1e-12);
        assert!((e.eval_expr("abs(0-5)").unwrap() - 5.0).abs() < 1e-12);
    }

    #[test]
    fn predefined_constants_resolve() {
        let e = EvalEngine::new();
        assert!((e.eval_expr("twopi/4").unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        assert!((e.eval_expr("halfpi").unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        assert!((e.eval_expr("90*deg").unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn undefined_symbol_records_warning_and_returns_zero() {
        let e = EvalEngine::new();
        assert_eq!(e.resolve_value("nonexistent_symbol_xyz * 2"), 0.0);
        let w = e.take_warnings();
        assert!(!w.is_empty(), "expected a warning for an undefined symbol");
        // buffer should be drained
        assert!(e.take_warnings().is_empty());
    }

    #[test]
    fn rewrite_is_idempotent_and_targeted() {
        assert_eq!(
            rewrite_math_functions("sin(x)+cos(y)"),
            "math::sin(x)+math::cos(y)"
        );
        assert_eq!(rewrite_math_functions("math::sin(x)"), "math::sin(x)");
        // bare evalexpr builtins must NOT be namespaced
        assert_eq!(rewrite_math_functions("max(1,2)"), "max(1,2)");
        // a variable that merely looks like a function name is left alone
        assert_eq!(rewrite_math_functions("sinphi*2"), "sinphi*2");
    }
}
