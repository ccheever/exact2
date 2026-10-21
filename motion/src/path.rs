//! A path's `d` as an animatable value (LLP 1055.000 D15).
//!
//! @ref CSS Shapes 1 §"Interpolating `path()`"; SVG 2 §9.3.3 (path data
//! normalised to absolute commands)
//!
//! A path is a list of commands, each absolute: `M`, `L`, `H`, `V`, `C`,
//! `S`, `Q`, `T`, `A` and `Z` with their numbers in user units. Two paths
//! interpolate when they have the same commands, one for one, after that
//! normalisation, as Chrome decides it: each number moves linearly, and an
//! arc's two flags take the first path's until half way, then the second's.
//! Any other pair is not interpolable, so a `transition` starts nothing and
//! the path changes at once (CSS Transitions 1 §3), as Chrome shows it.
//!
//! The engine never parses or draws a path: the kernel normalises the
//! authored `d` into a [`PathValue`], and a host draws the presented one back
//! through the kernel's own parser ([`PathValue::to_d`]).

use std::fmt::Write as _;

/// One absolute path command.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathCommand {
    /// The command's letter, upper case (absolute).
    pub verb: u8,
    /// Its numbers, the first [`arity`] of them; an arc's flags are 0 or 1.
    pub args: [f64; 7],
}

/// How many numbers a command takes; `None` for a letter that is not one.
pub fn arity(verb: u8) -> Option<usize> {
    Some(match verb {
        b'M' | b'L' | b'T' => 2,
        b'H' | b'V' => 1,
        b'C' => 6,
        b'S' | b'Q' => 4,
        b'A' => 7,
        b'Z' => 0,
        _ => return None,
    })
}

/// A path's data, every command absolute.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PathValue(pub Vec<PathCommand>);

impl PathValue {
    /// Whether the two paths interpolate: the same commands, one for one.
    pub fn interpolable(&self, other: &PathValue) -> bool {
        self.0.len() == other.0.len() && self.0.iter().zip(&other.0).all(|(a, b)| a.verb == b.verb)
    }

    /// The path at `progress` from `self` to `to`, which must be
    /// [`PathValue::interpolable`] with it: each number linearly (an easing
    /// that overshoots extrapolates), an arc's flags discretely at one half.
    pub fn lerp(&self, to: &PathValue, progress: f64) -> PathValue {
        debug_assert!(self.interpolable(to));
        PathValue(
            self.0
                .iter()
                .zip(&to.0)
                .map(|(a, b)| {
                    let mut args = [0.0; 7];
                    for (i, arg) in args.iter_mut().enumerate() {
                        let flag = a.verb == b'A' && (i == 3 || i == 4);
                        *arg = if flag {
                            if progress < 0.5 {
                                a.args[i]
                            } else {
                                b.args[i]
                            }
                        } else {
                            a.args[i] + (b.args[i] - a.args[i]) * progress
                        };
                    }
                    PathCommand { verb: a.verb, args }
                })
                .collect(),
        )
    }

    /// Whether every number is finite.
    pub fn is_finite(&self) -> bool {
        self.0.iter().all(|c| c.args.iter().all(|a| a.is_finite()))
    }

    /// The path as absolute path data, which draws it.
    pub fn to_d(&self) -> String {
        let mut d = String::new();
        for c in &self.0 {
            d.push(c.verb as char);
            let n = arity(c.verb).unwrap_or(0);
            for (i, a) in c.args[..n].iter().enumerate() {
                if i > 0 {
                    d.push(' ');
                }
                let flag = c.verb == b'A' && (i == 3 || i == 4);
                if flag {
                    d.push(if *a != 0.0 { '1' } else { '0' });
                } else {
                    // Never `-0` or an exponent: path data's numbers.
                    let _ = write!(d, "{}", if *a == 0.0 { 0.0 } else { *a });
                }
            }
        }
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(verb: u8, args: &[f64]) -> PathCommand {
        let mut a = [0.0; 7];
        a[..args.len()].copy_from_slice(args);
        PathCommand { verb, args: a }
    }

    #[test]
    fn matched_paths_move_number_by_number_and_arc_flags_flip_half_way() {
        let down = PathValue(vec![
            cmd(b'M', &[6.0, 9.0]),
            cmd(b'L', &[12.0, 15.0]),
            cmd(b'L', &[18.0, 9.0]),
        ]);
        let up = PathValue(vec![
            cmd(b'M', &[6.0, 15.0]),
            cmd(b'L', &[12.0, 9.0]),
            cmd(b'L', &[18.0, 15.0]),
        ]);
        assert!(down.interpolable(&up));
        assert_eq!(down.lerp(&up, 0.5).to_d(), "M6 12L12 12L18 12");
        let curve = PathValue(vec![
            cmd(b'M', &[6.0, 9.0]),
            cmd(b'Q', &[12.0, 2.0, 18.0, 9.0]),
        ]);
        assert!(!down.interpolable(&curve), "M L L against M Q");

        let a = PathValue(vec![
            cmd(b'M', &[0.0, 0.0]),
            cmd(b'A', &[5.0, 5.0, 0.0, 0.0, 1.0, 10.0, 0.0]),
        ]);
        let b = PathValue(vec![
            cmd(b'M', &[0.0, 0.0]),
            cmd(b'A', &[9.0, 5.0, 0.0, 1.0, 0.0, 10.0, 0.0]),
        ]);
        assert_eq!(a.lerp(&b, 0.25).to_d(), "M0 0A6 5 0 0 1 10 0");
        assert_eq!(a.lerp(&b, 0.5).to_d(), "M0 0A7 5 0 1 0 10 0");
    }
}
