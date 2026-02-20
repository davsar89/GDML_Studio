use anyhow::{Context, Result};
use evalexpr::*;
use std::collections::{HashMap, HashSet};

use super::context::EvalContext;
use super::dependency::{topological_sort, DefineEntry};
use crate::gdml::model::{DefineSection, Position, Rotation};
use crate::gdml::units;

pub struct EvalEngine {
    pub context: EvalContext,
    pub position_values: HashMap<String, [f64; 3]>,
    pub rotation_values: HashMap<String, [f64; 3]>,
}

impl EvalEngine {
    pub fn new() -> Self {
        Self {
            context: EvalContext::new(),
            position_values: HashMap::new(),
            rotation_values: HashMap::new(),
        }
    }

    pub fn evaluate_all(&mut self, defines: &DefineSection) -> Result<()> {
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
        let order = topological_sort(&entries, &known)
            .context("Failed to resolve define dependencies")?;

        // Build unit map for quantities
        let quantity_units: HashMap<String, String> = defines
            .quantities
            .iter()
            .filter_map(|q| q.unit.as_ref().map(|u| (q.name.clone(), u.clone())))
            .collect();

        // Evaluate in order
        for idx in order {
            let entry = &entries[idx];
            let value = self
                .eval_expr(&entry.expression)
                .with_context(|| format!("Failed to evaluate '{}' = '{}'", entry.name, entry.expression))?;

            // Apply unit conversion for quantities
            let final_value = if let Some(unit) = quantity_units.get(&entry.name) {
                let qty = defines.quantities.iter().find(|q| q.name == entry.name);
                if let Some(q) = qty {
                    match q.r#type.as_deref() {
                        Some("length") => units::length_to_mm(value, unit),
                        Some("density") => value, // keep as-is
                        _ => value,
                    }
                } else {
                    value
                }
            } else {
                value
            };

            self.context.set(&entry.name, final_value);
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

        // evalexpr uses ^ for power instead of ** - GDML doesn't use ** typically
        // evalexpr uses "math::pi" etc but we've set pi as a variable

        let result = eval_float_with_context(expr, &eval_context);
        match result {
            Ok(v) => Ok(v),
            Err(_) => {
                // Try with some fixups for GDML patterns
                let fixed = fix_gdml_expr(expr);
                eval_float_with_context(&fixed, &eval_context)
                    .map_err(|e| anyhow::anyhow!("evalexpr error for '{}': {}", expr, e))
            }
        }
    }

    fn eval_position(&self, pos: &Position) -> Result<[f64; 3]> {
        let unit = pos.unit.as_deref().unwrap_or("mm");
        let x = pos
            .x
            .as_ref()
            .map(|v| self.eval_expr(v))
            .transpose()?
            .unwrap_or(0.0);
        let y = pos
            .y
            .as_ref()
            .map(|v| self.eval_expr(v))
            .transpose()?
            .unwrap_or(0.0);
        let z = pos
            .z
            .as_ref()
            .map(|v| self.eval_expr(v))
            .transpose()?
            .unwrap_or(0.0);

        Ok([
            units::length_to_mm(x, unit),
            units::length_to_mm(y, unit),
            units::length_to_mm(z, unit),
        ])
    }

    fn eval_rotation(&self, rot: &Rotation) -> Result<[f64; 3]> {
        let unit = rot.unit.as_deref().unwrap_or("rad");
        let x = rot
            .x
            .as_ref()
            .map(|v| self.eval_expr(v))
            .transpose()?
            .unwrap_or(0.0);
        let y = rot
            .y
            .as_ref()
            .map(|v| self.eval_expr(v))
            .transpose()?
            .unwrap_or(0.0);
        let z = rot
            .z
            .as_ref()
            .map(|v| self.eval_expr(v))
            .transpose()?
            .unwrap_or(0.0);

        Ok([
            units::angle_to_rad(x, unit),
            units::angle_to_rad(y, unit),
            units::angle_to_rad(z, unit),
        ])
    }

    pub fn resolve_value(&self, expr_str: &str) -> f64 {
        match self.eval_expr(expr_str) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("Failed to evaluate '{}': {} -- using 0.0", expr_str, e);
                0.0
            }
        }
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
fn fix_gdml_expr(expr: &str) -> String {
    let bytes = expr.as_bytes();
    let mut result = String::with_capacity(expr.len() + 8);
    let mut i = 0;
    while i < bytes.len() {
        result.push(bytes[i] as char);
        if bytes[i].is_ascii_digit()
            && i + 2 < bytes.len()
            && bytes[i + 1] == b'.'
            && bytes[i + 2] == b'*'
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
