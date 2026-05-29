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
        // Angle constants (GDML/CLHEP conventions). Lower-case `twopi`/`halfpi`
        // are the spellings used in real GDML; the upper-case aliases are kept
        // for backwards compatibility.
        ctx.values.insert("pi".to_string(), std::f64::consts::PI);
        ctx.values.insert("PI".to_string(), std::f64::consts::PI);
        ctx.values.insert("e".to_string(), std::f64::consts::E);
        ctx.values
            .insert("twopi".to_string(), 2.0 * std::f64::consts::PI);
        ctx.values
            .insert("TWOPI".to_string(), 2.0 * std::f64::consts::PI);
        ctx.values
            .insert("halfpi".to_string(), std::f64::consts::FRAC_PI_2);
        ctx.values
            .insert("HALFPI".to_string(), std::f64::consts::FRAC_PI_2);
        // Angle unit symbols usable directly inside expressions (e.g. "90*deg").
        // Base angle unit is the radian, matching `units::angle_to_rad`.
        ctx.values
            .insert("deg".to_string(), std::f64::consts::PI / 180.0);
        ctx.values
            .insert("degree".to_string(), std::f64::consts::PI / 180.0);
        ctx.values.insert("rad".to_string(), 1.0);
        ctx.values.insert("radian".to_string(), 1.0);
        ctx.values.insert("mrad".to_string(), 0.001);
        ctx
    }

    pub fn set(&mut self, name: &str, value: f64) {
        self.values.insert(name.to_string(), value);
    }

    pub fn get(&self, name: &str) -> Option<f64> {
        self.values.get(name).copied()
    }
}
