//! Beacons' data: the motion grant (LLP 1133.013 §10), and `tiltAngles`,
//! the readout beside the tilt panel's level.

use exact_runner::{DataError, DataSource, Value};

#[derive(Default)]
pub struct BeaconsData;

impl DataSource for BeaconsData {
    fn grants(&self) -> &str {
        "device.motion purpose.motion"
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "tiltAngles" => {
                let n: Vec<f64> = args.iter().map(|a| a.as_number().unwrap_or(0.0)).collect();
                let [x, y, z, lx, ly, lz] = n[..] else {
                    return Err(DataError::BadArguments(
                        "tiltAngles(x, y, z, levelX, levelY, levelZ)".into(),
                    ));
                };
                Ok(Value::str(&label([x, y, z], [lx, ly, lz])))
            }
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
}

/// Degrees of roll and pitch of gravity `g` from the level `n`, as the
/// engine's tilt stick reads them (LLP 1133.013 D6): roll positive to the
/// right, pitch positive forward, each rounded to a whole degree.
pub fn angles(g: [f64; 3], n: [f64; 3]) -> (f64, f64) {
    let along = |v: [f64; 3], i: usize| {
        let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if length > 0.0 {
            (v[i] / length).clamp(-1.0, 1.0).asin().to_degrees()
        } else {
            0.0
        }
    };
    let roll = -(along(g, 0) - along(n, 0));
    let pitch = along(g, 1) - along(n, 1);
    (roll.round() + 0.0, pitch.round() + 0.0)
}

/// "12° right · 30° forward"; "0° · 0°" level.
pub fn label(g: [f64; 3], n: [f64; 3]) -> String {
    let (roll, pitch) = angles(g, n);
    let side = |d: f64, plus: &str, minus: &str| match d {
        d if d > 0.0 => format!("{d}° {plus}"),
        d if d < 0.0 => format!("{}° {minus}", -d),
        _ => "0°".to_owned(),
    };
    format!("{} · {}", side(roll, "right", "left"), side(pitch, "forward", "back"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEVEL: [f64; 3] = [0.0, 0.0, 9.80665];

    #[test]
    fn the_readout_is_the_tilt_from_the_level() {
        assert_eq!(label(LEVEL, LEVEL), "0° · 0°");
        assert_eq!(label([0.0, 4.9, 8.49], LEVEL), "0° · 30° forward");
        assert_eq!(label([3.0, 4.9, 7.9], [0.0, 4.9, 8.49]), "18° left · 0°");
        assert_eq!(label([-3.0, -4.9, 7.9], LEVEL), "18° right · 30° back");
    }
}
