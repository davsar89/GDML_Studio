//! Placements of imported parts and their GDML form (algorithm G in
//! `doc/cad-import/PLAN.md`).

use super::intersect::P;

/// An active transform in mm, `p -> r p + t`, with `r` a rotation or a
/// rotation times a reflection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub r: [[f64; 3]; 3],
    pub t: P,
}

/// A physvol's placement as Geant4 reads it: `position` in mm, `angles_deg`
/// (x, y, z) in degrees, and `<scale x="-1"/>` when `reflected`.
#[derive(Debug, PartialEq)]
pub struct Placement {
    pub position: P,
    pub angles_deg: [f64; 3],
    pub reflected: bool,
}

impl Affine {
    pub const IDENTITY: Affine = Affine {
        r: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        t: [0.0; 3],
    };

    /// From a 3 x 4 row-major matrix whose last column is the translation.
    pub fn from_rows(m: &[f64; 12]) -> Affine {
        Affine {
            r: [0, 1, 2].map(|i| [m[4 * i], m[4 * i + 1], m[4 * i + 2]]),
            t: [m[3], m[7], m[11]],
        }
    }

    pub fn apply(&self, p: P) -> P {
        [0, 1, 2].map(|i| (0..3).map(|j| self.r[i][j] * p[j]).sum::<f64>() + self.t[i])
    }

    /// `inner` first, then `self`.
    pub fn after(&self, inner: &Affine) -> Affine {
        Affine {
            r: [0, 1, 2]
                .map(|i| [0, 1, 2].map(|j| (0..3).map(|k| self.r[i][k] * inner.r[k][j]).sum())),
            t: self.apply(inner.t),
        }
    }

    /// Whether `r` is orthonormal within 1e-9: no scale or shear, which a
    /// Geant4 placement cannot express (G4ReflectionFactory keeps only the
    /// sign of a scale).
    pub fn is_rigid(&self) -> bool {
        (0..3).all(|i| {
            (0..3).all(|j| {
                let dot: f64 = (0..3).map(|k| self.r[k][i] * self.r[k][j]).sum();
                let identity = if i == j { 1.0 } else { 0.0 };
                (dot - identity).abs() <= 1e-9
            })
        })
    }

    /// Geant4 places a physvol at `[R^-1, t] S` (G4GDMLReadStructure::PhysvolRead),
    /// where `R = Rz(z) Ry(y) Rx(x)` of its angles (G4GDMLReadDefine::GetRotationMatrix)
    /// and `S` its scale. A reflection is written as `S = diag(-1, 1, 1)`: with
    /// `Q = r S`, `[Q, t] S = [r, t]`, so `R = Q^T`.
    pub fn placement(&self) -> Placement {
        let mut q = self.r;
        let reflected = det(&q) < 0.0;
        if reflected {
            q.iter_mut().for_each(|row| row[0] = -row[0]);
        }
        Placement {
            position: self.t,
            angles_deg: angles_deg(&[0, 1, 2].map(|i| [0, 1, 2].map(|j| q[j][i]))), // R = Q^T
            reflected,
        }
    }

    /// A Boolean or multiUnion operand is placed at `[R, t]`, with `R` built
    /// from its angles as for a physvol (G4GDMLReadSolids::BooleanRead,
    /// MultiUnionNodeRead): the position and angles in degrees of a rigid
    /// `self`.
    pub fn operand(&self) -> (P, [f64; 3]) {
        (self.t, angles_deg(&self.r))
    }

    /// The inverse of a rigid transform.
    pub fn inverse(&self) -> Affine {
        let r = [0, 1, 2].map(|i| [0, 1, 2].map(|j| self.r[j][i]));
        let t = [0, 1, 2].map(|i| -(0..3).map(|j| r[i][j] * self.t[j]).sum::<f64>());
        Affine { r, t }
    }
}

/// The angles (x, y, z) in degrees with `r = Rz(z) Ry(y) Rx(x)`.
fn angles_deg(r: &[[f64; 3]; 3]) -> [f64; 3] {
    let (x, y, z) = if r[0][0].hypot(r[1][0]) < 1e-12 {
        // cos y = 0: only x - z (or x + z) is defined; take z = 0.
        ((-r[1][2]).atan2(r[1][1]), (-r[2][0]).atan2(0.0), 0.0)
    } else {
        (
            r[2][1].atan2(r[2][2]),
            (-r[2][0]).atan2(r[0][0].hypot(r[1][0])),
            r[1][0].atan2(r[0][0]),
        )
    };
    [x, y, z].map(f64::to_degrees)
}

fn det(m: &[[f64; 3]; 3]) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rx(a: f64) -> [[f64; 3]; 3] {
        let (s, c) = a.sin_cos();
        [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]]
    }
    fn ry(a: f64) -> [[f64; 3]; 3] {
        let (s, c) = a.sin_cos();
        [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]]
    }
    fn rz(a: f64) -> [[f64; 3]; 3] {
        let (s, c) = a.sin_cos();
        [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]
    }
    fn rot(r: [[f64; 3]; 3]) -> Affine {
        Affine { r, t: [0.0; 3] }
    }

    /// The transform Geant4 builds from the GDML text of `p`'s placement,
    /// with angles read back in degrees as `value * deg`.
    fn geant4(p: &Placement) -> Affine {
        let [x, y, z] = p.angles_deg.map(|a| {
            let text = format!("{a:?}");
            text.parse::<f64>().unwrap() * (std::f64::consts::PI / 180.0)
        });
        let r = rot(rz(z)).after(&rot(ry(y))).after(&rot(rx(x)));
        let inverse = [0, 1, 2].map(|i| [0, 1, 2].map(|j| r.r[j][i]));
        let s = if p.reflected { -1.0 } else { 1.0 };
        let scale = rot([[s, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        Affine {
            r: inverse,
            t: p.position,
        }
        .after(&scale)
    }

    #[test]
    fn placements_read_back_as_the_same_transform() {
        let mirror = rot([[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]]);
        let mut cases = vec![Affine::IDENTITY, mirror];
        for (x, y, z) in [
            (0.3, -1.1, 2.9),
            (-2.5, 0.7, -0.2),
            (1.0, std::f64::consts::FRAC_PI_2, 0.4),
            (0.6, -std::f64::consts::FRAC_PI_2, -1.3),
            (std::f64::consts::PI, 0.0, 0.0),
        ] {
            let r = rot(rz(z)).after(&rot(ry(y))).after(&rot(rx(x)));
            let r = Affine {
                t: [12.5, -3.0, 1e3],
                ..r
            };
            cases.extend([r, r.after(&mirror), mirror.after(&r)]);
        }
        for m in cases {
            assert!(m.is_rigid());
            let p = m.placement();
            assert_eq!(p.reflected, det(&m.r) < 0.0);
            let back = geant4(&p);
            for i in 0..3 {
                for j in 0..3 {
                    assert!((back.r[i][j] - m.r[i][j]).abs() < 1e-12, "{m:?} -> {p:?}");
                }
            }
            assert_eq!(back.t, m.t);
            if !p.reflected {
                // An operand's angles rebuild r itself, and inverse() undoes m.
                let (_, angles) = m.operand();
                let [x, y, z] =
                    angles.map(|a| format!("{a:?}").parse::<f64>().unwrap().to_radians());
                let r = rot(rz(z)).after(&rot(ry(y))).after(&rot(rx(x)));
                let one = m.inverse().after(&m);
                for i in 0..3 {
                    assert!(
                        (0..3).all(|j| (r.r[i][j] - m.r[i][j]).abs() < 1e-12),
                        "{m:?}"
                    );
                    assert!((0..3).all(|j| (one.r[i][j] - Affine::IDENTITY.r[i][j]).abs() < 1e-12));
                    assert!(one.t[i].abs() < 1e-9);
                }
            }
        }
    }

    #[test]
    fn rows_compose_and_scaling_is_not_rigid() {
        let a = Affine::from_rows(&[0.0, -1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 2.0, 0.0, 0.0, 1.0, 3.0]);
        assert_eq!(a.apply([1.0, 0.0, 0.0]), [1.0, 3.0, 3.0]);
        assert_eq!(a.after(&a).apply([1.0, 0.0, 0.0]), [-2.0, 3.0, 6.0]);
        let scaled = Affine {
            r: [[25.4, 0.0, 0.0], [0.0, 25.4, 0.0], [0.0, 0.0, 25.4]],
            t: [0.0; 3],
        };
        assert!(!scaled.is_rigid());
    }
}
