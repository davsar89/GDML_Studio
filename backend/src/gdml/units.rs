//! GDML unit conversion.
//!
//! The internal system matches Geant4's for the quantities that reach geometry:
//! millimetres for length, radians for angle. Geant4 resolves a `lunit`/`aunit`
//! string through `G4UnitDefinition::GetValueOf` and raises a
//! `FatalException` when the category does not match, so a file naming a unit
//! that is not in the table below would not load there at all.
//!
//! Note that `micron` and `dm` are *not* legal GDML length units despite
//! appearing in some documentation — Geant4's length table spells the micrometre
//! `um`/`micrometer`, and has no decimetre.

/// Which quantity a unit symbol measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitKind {
    Length,
    Angle,
}

/// Multiplier taking `unit` into millimetres, or `None` if it is not a length
/// unit Geant4 recognises.
pub fn length_factor(unit: &str) -> Option<f64> {
    Some(match unit {
        "mm" | "millimeter" => 1.0,
        "cm" | "centimeter" => 10.0,
        "m" | "meter" => 1000.0,
        "km" | "kilometer" => 1_000_000.0,
        "um" | "micrometer" => 1.0e-3,
        "nm" | "nanometer" => 1.0e-6,
        "Ang" | "angstrom" => 1.0e-7,
        "fm" | "fermi" => 1.0e-12,
        "pc" | "parsec" => 3.0856775807e19,
        // Not Geant4 units, accepted as a convenience for hand-written files.
        "in" | "inch" => 25.4,
        "ft" | "foot" => 304.8,
        _ => return None,
    })
}

/// Multiplier taking `unit` into radians, or `None` if it is not an angle unit.
pub fn angle_factor(unit: &str) -> Option<f64> {
    Some(match unit {
        "rad" | "radian" | "radians" => 1.0,
        "deg" | "degree" | "degrees" => std::f64::consts::PI / 180.0,
        "mrad" | "milliradian" => 1.0e-3,
        _ => return None,
    })
}

/// Classify a unit symbol, or `None` if it is unrecognised.
pub fn unit_kind(unit: &str) -> Option<UnitKind> {
    if length_factor(unit).is_some() {
        Some(UnitKind::Length)
    } else if angle_factor(unit).is_some() {
        Some(UnitKind::Angle)
    } else {
        None
    }
}

/// Convert to millimetres. Unrecognised units pass through unchanged; callers
/// that can surface a diagnostic should check [`length_factor`] first, since a
/// silently unconverted value renders as wrong geometry with no other signal.
pub fn length_to_mm(value: f64, unit: &str) -> f64 {
    value * length_factor(unit).unwrap_or(1.0)
}

/// Convert to radians. Unrecognised units pass through unchanged.
pub fn angle_to_rad(value: f64, unit: &str) -> f64 {
    value * angle_factor(unit).unwrap_or(1.0)
}

/// Apply a unit without being told which kind it is, matching
/// `G4GDMLReadDefine::QuantityRead`, which multiplies by
/// `G4UnitDefinition::GetValueOf(unit)` without ever consulting the `type`
/// attribute.
pub fn apply_unit(value: f64, unit: &str) -> f64 {
    match unit_kind(unit) {
        Some(UnitKind::Length) => length_to_mm(value, unit),
        Some(UnitKind::Angle) => angle_to_rad(value, unit),
        None => value,
    }
}

pub fn default_length_unit() -> &'static str {
    "mm"
}

pub fn default_angle_unit() -> &'static str {
    "rad"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_conversions_match_geant4() {
        assert_eq!(length_to_mm(1.0, "cm"), 10.0);
        assert_eq!(length_to_mm(1.0, "m"), 1000.0);
        assert_eq!(length_to_mm(1000.0, "um"), 1.0);
        assert_eq!(length_to_mm(1.0, "km"), 1.0e6);
        assert_eq!(length_to_mm(1.0, "angstrom"), 1.0e-7);
        assert_eq!(length_to_mm(1.0, "fermi"), 1.0e-12);
    }

    #[test]
    fn angle_conversions_match_geant4() {
        assert!((angle_to_rad(180.0, "deg") - std::f64::consts::PI).abs() < 1e-12);
        assert_eq!(angle_to_rad(1.0, "rad"), 1.0);
        assert_eq!(angle_to_rad(1000.0, "mrad"), 1.0);
    }

    #[test]
    fn unrecognised_units_are_reported_not_guessed() {
        // `micron` and `dm` look plausible but are not Geant4 length units;
        // treating them as mm would silently render wrong geometry.
        assert!(length_factor("micron").is_none());
        assert!(length_factor("dm").is_none());
        assert!(unit_kind("furlong").is_none());
        // The lenient converters still pass the value through unchanged.
        assert_eq!(length_to_mm(5.0, "furlong"), 5.0);
    }

    #[test]
    fn apply_unit_dispatches_without_a_type_hint() {
        assert_eq!(apply_unit(5.0, "cm"), 50.0);
        assert!((apply_unit(180.0, "deg") - std::f64::consts::PI).abs() < 1e-12);
        assert_eq!(apply_unit(5.0, "g/cm3"), 5.0);
    }
}
