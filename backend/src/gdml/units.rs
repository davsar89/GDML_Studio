pub fn length_to_mm(value: f64, unit: &str) -> f64 {
    match unit {
        "mm" => value,
        "cm" => value * 10.0,
        "m" => value * 1000.0,
        "um" | "micrometer" => value * 0.001,
        "nm" | "nanometer" => value * 0.000001,
        "km" => value * 1_000_000.0,
        "in" | "inch" => value * 25.4,
        "ft" | "foot" => value * 304.8,
        _ => value, // default mm
    }
}

pub fn angle_to_rad(value: f64, unit: &str) -> f64 {
    match unit {
        "deg" | "degree" => value * std::f64::consts::PI / 180.0,
        "rad" | "radian" => value,
        "mrad" => value * 0.001,
        _ => value, // default rad
    }
}

pub fn default_length_unit() -> &'static str {
    "mm"
}

pub fn default_angle_unit() -> &'static str {
    "rad"
}
