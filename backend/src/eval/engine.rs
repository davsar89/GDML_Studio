use anyhow::{Context, Result};
use evalexpr::{Context as _, *};
use std::collections::HashMap;
use std::f64::consts::PI;

use super::dependency::{dependency_order, extract_identifiers};
use crate::gdml::model::{DefineSection, PlacementPos, PlacementRot, Position, Rotation};
use crate::gdml::units;

pub struct EvalEngine {
    /// Every variable an expression can name, seeded once with what
    /// G4GDMLEvaluator starts with and updated as defines are evaluated.
    context: HashMapContext,
    pub position_values: HashMap<String, [f64; 3]>,
    pub scale_values: HashMap<String, [f64; 3]>,
    pub rotation_values: HashMap<String, [f64; 3]>,
    /// Non-fatal evaluation warnings (e.g. an expression treated as 0), kept
    /// per thread. A call records and drains them on one thread, so requests
    /// building scenes from the loaded engine at once never take each other's.
    warnings: std::sync::Mutex<HashMap<std::thread::ThreadId, Vec<String>>>,
}

/// Names seeded below that G4GDMLEvaluator does not predefine, so a file may
/// define them: CLHEP's twopi and halfpi, the perCent family, unit plurals, and
/// symbols only G4UnitsTable or this viewer knows.
const NOT_GEANT4_NAMES: &[&str] = &[
    "twopi",
    "halfpi",
    "perCent",
    "perThousand",
    "perMillion",
    "radians",
    "degrees",
    "Ang",
    "fm",
    "inch",
    "in",
    "foot",
    "ft",
];

impl Default for EvalEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl EvalEngine {
    pub fn new() -> Self {
        let mut engine = Self {
            context: HashMapContext::new(),
            position_values: HashMap::new(),
            rotation_values: HashMap::new(),
            scale_values: HashMap::new(),
            warnings: Default::default(),
        };
        // setStdMath's constants and every CLHEP unit, so `2*cm` or
        // `1.5*g/cm3` is valid in any expression. CLHEP's twopi and halfpi are
        // kept as a convenience.
        let constants = [
            ("pi", PI),
            ("e", std::f64::consts::E),
            ("gamma", 0.577_215_664_901_532_9),
            ("twopi", 2.0 * PI),
            ("halfpi", PI / 2.0),
        ];
        for (name, value) in units::all().chain(constants) {
            engine.set(name, value);
        }
        engine
    }

    pub fn set(&mut self, name: &str, value: f64) {
        // Every variable is a Float, so set_value's type check cannot fail.
        let _ = self
            .context
            .set_value(name.to_string(), Value::Float(value));
    }

    pub fn get(&self, name: &str) -> Option<f64> {
        self.context.get_value(name)?.as_float().ok()
    }

    pub fn evaluate_all(&mut self, defines: &DefineSection) -> Result<()> {
        // Rebuild derived state from scratch for the current document.
        *self = Self::new();

        // Name, expression and, for a quantity, its unit.
        let entries: Vec<(&str, &str, Option<&str>)> = defines
            .constants
            .iter()
            .map(|c| (c.name.as_str(), c.value.as_str(), None))
            .chain(
                defines
                    .quantities
                    .iter()
                    .map(|q| (q.name.as_str(), q.value.as_str(), q.unit.as_deref())),
            )
            .chain(
                defines
                    .variables
                    .iter()
                    .map(|v| (v.name.as_str(), v.value.as_str(), None)),
            )
            .chain(
                defines
                    .expressions
                    .iter()
                    .map(|e| (e.name.as_str(), e.value.as_str(), None)),
            )
            .collect();
        // A name that is also a builtin unit or constant is still a dependency:
        // the define is the value used.
        let index: HashMap<&str, usize> =
            entries.iter().enumerate().map(|(i, e)| (e.0, i)).collect();
        let deps: Vec<Vec<usize>> = entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                extract_identifiers(e.1)
                    .iter()
                    .filter_map(|name| index.get(name.as_str()).copied())
                    .filter(|&j| j != i)
                    .collect()
            })
            .collect();
        let order = dependency_order(&deps, "define", |i| entries[i].0)
            .context("Failed to resolve define dependencies")?;

        // G4GDMLEvaluator refuses to redefine one of its own names ("Redefinition
        // of constant or variable"). Here the define wins; say so.
        for &(name, ..) in &entries {
            if self.get(name).is_some() && !NOT_GEANT4_NAMES.contains(&name) {
                self.record_warning(format!(
                    "Define \"{name}\" shadows a CLHEP unit/constant; Geant4 rejects this \
                     file. Here the define is used wherever \"{name}\" appears."
                ));
            }
        }

        for (name, expression, unit) in order.into_iter().map(|i| entries[i]) {
            // A define that cannot be evaluated is a warning, not a fatal error.
            // The identical expression appearing in a solid attribute already
            // degrades to 0 with a warning via `resolve_value`; propagating here
            // instead meant one bad <constant> returned HTTP 500 and the user saw
            // nothing at all rather than a geometry with one hole in it.
            let value = match self.eval_expr(expression) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        "Failed to evaluate define '{name}' = '{expression}': {e} -- using 0.0"
                    );
                    self.record_warning(format!(
                        "Could not evaluate define \"{name}\" = \"{}\"; treated as 0. \
                         Anything referencing it will be wrong.",
                        expression.trim()
                    ));
                    0.0
                }
            };
            // QuantityRead applies the declared unit independently of the type
            // hint. G4UnitsTable lists compound symbols like g/cm3 as units of
            // their own; evaluated, the symbol has the same value.
            let value = unit.map_or(value, |unit| value * self.resolve_value(unit));
            self.set(name, finite_value(value)?);
        }

        // A bad position or rotation is a warning too, not a rejected file. It
        // stays undefined, so whatever references it reports that and is skipped.
        for pos in &defines.positions {
            match self.eval_position(pos) {
                Ok(values) => {
                    self.position_values.insert(pos.name.clone(), values);
                }
                Err(e) => self.record_warning(format!(
                    "Could not evaluate position \"{}\": {e}. Anything using it is skipped.",
                    pos.name
                )),
            }
        }
        for rot in &defines.rotations {
            match self.eval_rotation(rot) {
                Ok(values) => {
                    self.rotation_values.insert(rot.name.clone(), values);
                }
                Err(e) => self.record_warning(format!(
                    "Could not evaluate rotation \"{}\": {e}. Anything using it is skipped.",
                    rot.name
                )),
            }
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
            return finite_value(v);
        }

        // Try direct variable lookup
        if let Some(v) = self.get(expr) {
            return finite_value(v);
        }

        // Rewrite GDML/CLHEP conventions for evalexpr:
        //  - bare math functions (sin, sqrt, …) live under the `math::` namespace
        //  - `**` is power in some GDML; evalexpr spells power as `^`
        let prepared = rewrite_math_functions(expr).replace("**", "^");
        let mut tree = build_operator_tree(&prepared)
            .map_err(|e| anyhow::anyhow!("Invalid expression '{expr}': {e}"))?;
        // GDML arithmetic is real-valued. Casting only the final result is too
        // late: evalexpr otherwise evaluates 1/2 as integer division first.
        float_literals(&mut tree);
        let value = tree
            .eval_number_with_context(&self.context)
            .map_err(|e| anyhow::anyhow!("Invalid expression '{expr}': {e}"))?;
        finite_value(value)
    }

    fn eval_position(&self, pos: &Position) -> Result<[f64; 3]> {
        let unit = pos.unit.as_deref().unwrap_or("mm");
        if units::length_factor(unit).is_none() {
            self.record_unit_warning(unit, "length");
        }
        let comp = |expr: &Option<String>| -> Result<f64> {
            finite_value(units::length_to_mm(
                expr.as_deref()
                    .map(|e| self.eval_expr(e))
                    .transpose()?
                    .unwrap_or(0.0),
                unit,
            ))
        };
        Ok([comp(&pos.x)?, comp(&pos.y)?, comp(&pos.z)?])
    }

    fn eval_rotation(&self, rot: &Rotation) -> Result<[f64; 3]> {
        let unit = rot.unit.as_deref().unwrap_or("rad");
        if units::angle_factor(unit).is_none() {
            self.record_unit_warning(unit, "angle");
        }
        let comp = |expr: &Option<String>| -> Result<f64> {
            finite_value(units::angle_to_rad(
                expr.as_deref()
                    .map(|e| self.eval_expr(e))
                    .transpose()?
                    .unwrap_or(0.0),
                unit,
            ))
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
        if let Ok(mut all) = self.warnings.lock() {
            let warnings = all.entry(std::thread::current().id()).or_default();
            if warnings.len() < 100 && !warnings.contains(&msg) {
                warnings.push(msg);
            }
        }
    }

    /// Drain the warnings this thread recorded.
    pub fn take_warnings(&self) -> Vec<String> {
        self.warnings
            .lock()
            .ok()
            .and_then(|mut all| all.remove(&std::thread::current().id()))
            .unwrap_or_default()
    }

    pub fn named_position(&self, name: &str) -> Result<[f64; 3]> {
        self.position_values
            .get(name)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("Undefined position '{name}'"))
    }

    pub fn resolve_position(&self, pos: &Option<PlacementPos>) -> Result<[f64; 3]> {
        match pos {
            Some(PlacementPos::Inline(p)) => self.eval_position(p),
            Some(PlacementPos::Ref(name)) => self.named_position(name),
            None => Ok([0.0; 3]),
        }
    }

    pub fn resolve_rotation(&self, rot: &Option<PlacementRot>) -> Result<[f64; 3]> {
        match rot {
            Some(PlacementRot::Inline(r)) => self.eval_rotation(r),
            Some(PlacementRot::Ref(name)) => self
                .rotation_values
                .get(name)
                .copied()
                .ok_or_else(|| anyhow::anyhow!("Undefined rotation '{name}'")),
            None => Ok([0.0; 3]),
        }
    }
}

fn finite_value(value: f64) -> Result<f64> {
    anyhow::ensure!(value.is_finite(), "Expression produced a non-finite value");
    Ok(value)
}

fn float_literals(node: &mut Node) {
    if let Operator::Const {
        value: Value::Int(v),
    } = node.operator()
    {
        *node.operator_mut() = Operator::Const {
            value: Value::Float(*v as f64),
        };
    }
    for child in node.children_mut() {
        float_literals(child);
    }
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
    fn geant4_11_time_and_millielectronvolt_units_resolve() {
        // Geant4 11's setSystemOfUnits: minute = 60*s, hour = 60*minute,
        // day = 24*hour, year = 365*day, millielectronvolt = 1e-3*eV.
        let engine = EvalEngine::new();
        for (expr, expected) in [
            ("minute/s", 60.0),
            ("hour/minute", 60.0),
            ("day/hour", 24.0),
            ("year/day", 365.0),
            ("millielectronvolt/eV", 1.0e-3),
        ] {
            let value = engine.eval_expr(expr).unwrap();
            assert!((value / expected - 1.0).abs() < 1e-12, "{expr} = {value}");
        }
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
        assert_eq!(e.eval_expr("360/8").unwrap(), 45.0); // integer operands
        assert_eq!(e.eval_expr("10-1").unwrap(), 9.0);
    }

    #[test]
    fn trailing_dot_literals_evaluate() {
        // C++-written GDML spells floats `360.`; evalexpr 13 parses them as is.
        let e = EvalEngine::new();
        for (expr, expected) in [
            ("360.*deg", 2.0 * PI),
            ("-1.*pi", -PI),
            ("2.*3", 6.0),
            ("1.e3", 1000.0),
            ("1.", 1.0),
        ] {
            assert!(
                (e.eval_expr(expr).unwrap() - expected).abs() < 1e-12,
                "{expr}"
            );
        }
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
    fn concurrent_callers_take_only_their_own_warnings() {
        // Scene requests evaluate against the one loaded engine at the same
        // time; each must report its own replica warnings, not another's.
        let engine = EvalEngine::new();
        let recorded = std::sync::Barrier::new(2);
        let call = |expr: &str| {
            engine.resolve_value(expr);
            recorded.wait();
            engine.take_warnings()
        };
        let (a, b) = std::thread::scope(|s| {
            let a = s.spawn(|| call("undefined_a"));
            let b = s.spawn(|| call("undefined_b"));
            (a.join().unwrap(), b.join().unwrap())
        });
        assert!(a.len() == 1 && a[0].contains("undefined_a"), "{a:?}");
        assert!(b.len() == 1 && b[0].contains("undefined_b"), "{b:?}");
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
