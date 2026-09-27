//! CLHEP units, one table for both places Geant4 reads a unit:
//!
//! - a `lunit`/`aunit`/`unit` attribute goes through
//!   `G4UnitDefinition::GetValueOf` (`G4UnitsTable.cc`), and a `lunit` outside
//!   its "Length" category, or an `aunit` outside "Angle", is a
//!   `FatalException`;
//! - a symbol inside an expression is a variable of `G4GDMLEvaluator`, which
//!   loads CLHEP's `setStdMath` and `setSystemOfUnits`.
//!
//! Values are CLHEP internal units (`SystemOfUnits.h`): millimetre, nanosecond,
//! MeV, positron charge, kelvin, mole, candela and radian are 1. So lengths come
//! out in mm, angles in rad, and a density in MeV·ns²/mm⁵ (g/cm3 = 6.2415e18).
//!
//! Note that `micron` and `dm` are *not* legal GDML length units despite
//! appearing in some documentation — Geant4's length table spells the micrometre
//! `um`/`micrometer`, and has no decimetre. (`micron` works inside expressions.)

use std::f64::consts::PI;

/// Groups of space-separated aliases with their value in internal units.
type Units = &'static [(&'static str, f64)];

// SI base units as G4GDMLEvaluator hands them to setSystemOfUnits, and the
// derived units it builds from them.
const M: f64 = 1.0e3;
const S: f64 = 1.0e9;
const E_SI: f64 = 1.602176634e-19; // positron charge in coulomb
const J: f64 = 1.0e-6 / E_SI; // joule = electronvolt / e_SI
const KG: f64 = J * S * S / (M * M);
const C: f64 = 1.0 / E_SI; // coulomb = eplus / e_SI
const A: f64 = C / S;
const V: f64 = J / C;
const N: f64 = J / M;
const PA: f64 = N / (M * M);
const BAR: f64 = 1.0e5 * PA;
const W: f64 = J / S;
const F: f64 = C / V;
const T: f64 = V * S / (M * M);
const GS: f64 = 1.0e-4 * T;
const WB: f64 = V * S;
const BQ: f64 = 1.0 / S;
const CI: f64 = 3.7e10 * BQ;
const GY: f64 = J / KG;
const G: f64 = 1.0e-3 * KG;
const L: f64 = 1.0e-3 * M * M * M;
const BARN: f64 = 1.0e-28 * M * M;
const EV: f64 = 1.0e-6;
const CM: f64 = 1.0e-2 * M;
const KM: f64 = 1.0e3 * M;

/// Legal `lunit` values: `G4UnitsTable.cc` "Length".
const LENGTH: Units = &[
    ("parsec pc", 3.0856775807e16 * M),
    ("kilometer km", KM),
    ("meter m", M),
    ("centimeter cm", CM),
    ("millimeter mm", 1.0),
    ("micrometer um", 1.0e-6 * M),
    ("nanometer nm", 1.0e-9 * M),
    ("angstrom Ang", 1.0e-10 * M),
    ("fermi fm", 1.0e-15 * M),
    // Not Geant4 units, accepted as a convenience for hand-written files.
    ("inch in", 25.4),
    ("foot ft", 304.8),
];

/// Legal `aunit` values: `G4UnitsTable.cc` "Angle" (plus the plurals).
const ANGLE: Units = &[
    ("radian rad radians", 1.0),
    ("milliradian mrad", 1.0e-3),
    ("degree deg degrees", PI / 180.0),
];

/// Every other unit `setSystemOfUnits.cc` gives the expression evaluator, plus
/// CLHEP's perCent family.
const OTHER: Units = &[
    ("metre", M),
    ("decimeter", 0.1 * M),
    ("micron", 1.0e-6 * M),
    ("m2", M * M),
    ("mm2", 1.0),
    ("cm2", CM * CM),
    ("km2", KM * KM),
    ("barn", BARN),
    ("millibarn mbarn", 1.0e-3 * BARN),
    ("microbarn", 1.0e-6 * BARN),
    ("nanobarn", 1.0e-9 * BARN),
    ("picobarn", 1.0e-12 * BARN),
    ("m3", M * M * M),
    ("mm3", 1.0),
    ("cm3 cc", CM * CM * CM),
    ("km3", KM * KM * KM),
    ("liter litre L", L),
    ("centiliter cL", 1.0e-2 * L),
    ("milliliter mL", 1.0e-3 * L),
    ("diopter dioptre dpt", 1.0 / M),
    ("steradian sr", 1.0),
    ("kilogram kg", KG),
    ("gram g", G),
    ("milligram mg", 1.0e-3 * G),
    ("second s", S),
    ("millisecond ms", 1.0e-3 * S),
    ("microsecond us", 1.0e-6 * S),
    ("nanosecond ns", 1.0e-9 * S),
    ("picosecond ps", 1.0e-12 * S),
    // Geant4 11 adds these four and millielectronvolt; the vendored 10.7 lacks them.
    ("minute", 60.0 * S),
    ("hour", 3600.0 * S),
    ("day", 86400.0 * S),
    ("year", 365.0 * 86400.0 * S),
    ("hertz Hz", 1.0 / S),
    ("kilohertz kHz", 1.0e3 / S),
    ("megahertz MHz", 1.0e6 / S),
    ("kelvin K", 1.0),
    ("mole mol", 1.0),
    ("candela cd", 1.0),
    ("lumen lm", 1.0),
    ("lux lx", 1.0 / (M * M)),
    ("newton N", N),
    ("kilonewton kN", 1.0e3 * N),
    ("pascal Pa", PA),
    ("atmosphere atm", 101325.0 * PA),
    ("bar", BAR),
    ("kilobar kbar", 1.0e3 * BAR),
    ("millibar mbar", 1.0e-3 * BAR),
    ("joule J", J),
    ("kilojoule kJ", 1.0e3 * J),
    ("megajoule MJ", 1.0e6 * J),
    ("gigajoule GJ", 1.0e9 * J),
    ("millielectronvolt", 1.0e-3 * EV),
    ("electronvolt eV", EV),
    ("kiloelectronvolt keV", 1.0e3 * EV),
    ("megaelectronvolt MeV", 1.0e6 * EV),
    ("gigaelectronvolt GeV", 1.0e9 * EV),
    ("teraelectronvolt TeV", 1.0e12 * EV),
    ("petaelectronvolt PeV", 1.0e15 * EV),
    ("watt W", W),
    ("kilowatt kW", 1.0e3 * W),
    ("megawatt MW", 1.0e6 * W),
    ("gigawatt GW", 1.0e9 * W),
    ("ampere amp A", A),
    ("milliampere mA", 1.0e-3 * A),
    ("microampere", 1.0e-6 * A),
    ("nanoampere", 1.0e-9 * A),
    ("coulomb C", C),
    ("volt V", V),
    ("kilovolt kV", 1.0e3 * V),
    ("megavolt MV", 1.0e6 * V),
    ("ohm", V / A),
    ("siemens S", A / V),
    ("farad F", F),
    ("millifarad mF", 1.0e-3 * F),
    ("microfarad uF", 1.0e-6 * F),
    ("nanofarad nF", 1.0e-9 * F),
    ("picofarad pF", 1.0e-12 * F),
    ("tesla T", T),
    ("gauss Gs", GS),
    ("kilogauss kGs", 1.0e3 * GS),
    ("weber Wb", WB),
    ("henry H", WB / A),
    ("becquerel Bq", BQ),
    ("kilobecquerel kBq", 1.0e3 * BQ),
    ("megabecquerel MBq", 1.0e6 * BQ),
    ("gigabecquerel GBq", 1.0e9 * BQ),
    ("curie Ci", CI),
    ("millicurie mCi", 1.0e-3 * CI),
    ("microcurie uCi", 1.0e-6 * CI),
    ("gray Gy", GY),
    ("kilogray", 1.0e3 * GY),
    ("milligray", 1.0e-3 * GY),
    ("microgray", 1.0e-6 * GY),
    ("sievert Sv", GY),
    ("perCent", 0.01),
    ("perThousand", 0.001),
    ("perMillion", 1.0e-6),
];

fn lookup(groups: &[Units], unit: &str) -> Option<f64> {
    groups
        .iter()
        .flat_map(|group| group.iter())
        .find(|(names, _)| names.split(' ').any(|n| n == unit))
        .map(|&(_, value)| value)
}

/// Every unit symbol with its value, for seeding the expression evaluator.
pub fn all() -> impl Iterator<Item = (&'static str, f64)> {
    [LENGTH, ANGLE, OTHER]
        .into_iter()
        .flatten()
        .flat_map(|&(names, value)| names.split(' ').map(move |n| (n, value)))
}

/// Multiplier taking `unit` into millimetres, or `None` if it is not a length
/// unit Geant4 recognises.
pub fn length_factor(unit: &str) -> Option<f64> {
    lookup(&[LENGTH], unit)
}

/// Multiplier taking `unit` into radians, or `None` if it is not an angle unit.
pub fn angle_factor(unit: &str) -> Option<f64> {
    lookup(&[ANGLE], unit)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_conversions_match_geant4() {
        assert_eq!(length_to_mm(1.0, "cm"), 10.0);
        assert_eq!(length_to_mm(1.0, "m"), 1000.0);
        assert_eq!(length_to_mm(1000.0, "um"), 1.0);
        assert_eq!(length_to_mm(1.0, "km"), 1.0e6);
        // CLHEP builds these as 1e-10*meter and 1e-15*meter, one ulp off.
        assert!((length_to_mm(1.0, "angstrom") / 1.0e-7 - 1.0).abs() < 1e-15);
        assert!((length_to_mm(1.0, "fermi") / 1.0e-12 - 1.0).abs() < 1e-15);
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
        // The lenient converters still pass the value through unchanged.
        assert_eq!(length_to_mm(5.0, "furlong"), 5.0);
    }
}
