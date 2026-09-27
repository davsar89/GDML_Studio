use std::collections::HashMap;
use std::f64::consts::PI;

use crate::gdml::units;

#[derive(Debug, Clone, Default)]
pub struct EvalContext {
    pub values: HashMap<String, f64>,
}

impl EvalContext {
    pub fn new() -> Self {
        // What G4GDMLEvaluator starts with: setStdMath's constants and every
        // CLHEP unit, so `2*cm` or `1.5*g/cm3` is valid in any expression.
        // CLHEP's twopi and halfpi are kept as a convenience.
        let constants = [
            ("pi", PI),
            ("e", std::f64::consts::E),
            ("gamma", 0.577_215_664_901_532_9),
            ("twopi", 2.0 * PI),
            ("halfpi", PI / 2.0),
        ];
        Self {
            values: units::all()
                .chain(constants)
                .map(|(name, value)| (name.to_string(), value))
                .collect(),
        }
    }

    pub fn set(&mut self, name: &str, value: f64) {
        self.values.insert(name.to_string(), value);
    }

    pub fn get(&self, name: &str) -> Option<f64> {
        self.values.get(name).copied()
    }
}
