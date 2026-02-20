use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct EvalContext {
    pub values: HashMap<String, f64>,
}

impl EvalContext {
    pub fn new() -> Self {
        let mut ctx = Self {
            values: HashMap::new(),
        };
        ctx.values.insert("pi".to_string(), std::f64::consts::PI);
        ctx.values
            .insert("PI".to_string(), std::f64::consts::PI);
        ctx.values
            .insert("e".to_string(), std::f64::consts::E);
        ctx.values
            .insert("TWOPI".to_string(), 2.0 * std::f64::consts::PI);
        ctx
    }

    pub fn set(&mut self, name: &str, value: f64) {
        self.values.insert(name.to_string(), value);
    }

    pub fn get(&self, name: &str) -> Option<f64> {
        self.values.get(name).copied()
    }
}
