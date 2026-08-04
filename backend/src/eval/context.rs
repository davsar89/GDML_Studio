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
        // Length unit symbols. Geant4 evaluates every GDML expression through
        // G4GDMLEvaluator, which loads CLHEP's system of units, so `2*cm` is
        // valid in any <constant>/<variable>/<quantity> value. Without these the
        // identifier lookup fails and -- because evaluate_all propagates with `?`
        // -- the entire document returns HTTP 500 and nothing renders.
        // Base length unit is the millimetre, matching `units::length_to_mm`.
        for (name, mm) in [
            ("mm", 1.0),
            ("millimeter", 1.0),
            ("cm", 10.0),
            ("centimeter", 10.0),
            ("m", 1000.0),
            ("meter", 1000.0),
            ("km", 1.0e6),
            ("kilometer", 1.0e6),
            ("um", 1.0e-3),
            ("micrometer", 1.0e-3),
            ("nm", 1.0e-6),
            ("nanometer", 1.0e-6),
            ("Ang", 1.0e-7),
            ("angstrom", 1.0e-7),
            ("fm", 1.0e-12),
            ("fermi", 1.0e-12),
            ("pc", 3.0856775807e19),
            ("parsec", 3.0856775807e19),
        ] {
            ctx.values.insert(name.to_string(), mm);
        }
        ctx
    }

    pub fn set(&mut self, name: &str, value: f64) {
        self.values.insert(name.to_string(), value);
    }

    pub fn get(&self, name: &str) -> Option<f64> {
        self.values.get(name).copied()
    }
}
