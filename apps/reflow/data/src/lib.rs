//! Reflow's data source: Pretext's split — measure once, then arithmetic —
//! below exact2's data seam. Advances come from the app's own font files at
//! build time (`build.rs`), lines are broken by the same `exact-textflow`
//! walker every host flows text with, and what the kernel receives is text
//! already cut to the columns and heights the arithmetic found.
#![deny(missing_docs)]

use exact_plan::Value;
use exact_runner::{DataError, DataSource};

mod ascii;
mod balls;
mod cards;
mod columns;
mod dragon;
pub mod font;
mod prose;
pub mod typeset;

pub use cards::{BODY as CARD_BODY, TITLE as CARD_TITLE};
pub use columns::BODY as ARTICLE_BODY;

/// The source; the wall's placement is memoized across scrolls.
#[derive(Default)]
pub struct Reflow {
    wall: Option<cards::Wall>,
}

fn number(args: &[Value], i: usize, bad: &dyn Fn() -> DataError) -> Result<f64, DataError> {
    args.get(i)
        .and_then(Value::as_number)
        .filter(|v| v.is_finite())
        .ok_or_else(bad)
}

impl DataSource for Reflow {
    fn app_id(&self) -> &str {
        "com.exact.reflow"
    }
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        let signature = match source {
            "balls" => "balls(elapsedMs, width, height)",
            "dragon" => "dragon(width, dx, dy, boxHeight)",
            "masonry" => "masonry(width, seed)",
            "wall" => "wall(width, scrollTop, viewportHeight, seed)",
            "magazine" => "magazine(width)",
            "spread" => "spread(width)",
            "ascii" => "ascii(elapsedMs, measured, width)",
            _ => return Err(DataError::UnknownSource(source.into())),
        };
        let bad = || DataError::BadArguments(signature.into());
        let n = |i: usize| number(args, i, &bad);
        let expected = signature.matches(',').count() + 1;
        if args.len() != expected {
            return Err(bad());
        }
        Ok(match source {
            "balls" => balls::balls(n(0)?, n(1)?, n(2)?),
            "dragon" => dragon::dragon(n(0)? as f32, n(1)? as f32, n(2)? as f32, n(3)? as f32),
            "masonry" => cards::masonry(n(0)? as f32, n(1)? as u64),
            "wall" => cards::wall(
                &mut self.wall,
                n(0)? as f32,
                n(1)? as f32,
                n(2)? as f32,
                n(3)? as u64,
            ),
            "magazine" => columns::magazine(n(0)? as f32),
            "spread" => columns::spread(n(0)? as f32),
            "ascii" => {
                let measured = args[1].as_bool().ok_or_else(bad)?;
                ascii::ascii(n(0)? as f32, measured, n(2)? as f32)
            }
            _ => unreachable!(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_source_answers_and_refuses_bad_arguments() {
        let mut data = Reflow::default();
        let n = Value::Number;
        assert!(data.query("balls", &[n(0.0), n(600.0), n(400.0)]).is_ok());
        assert!(data
            .query("dragon", &[n(600.0), n(0.0), n(0.0), n(700.0)])
            .is_ok());
        assert!(data.query("masonry", &[n(900.0), n(1.0)]).is_ok());
        assert!(data
            .query("wall", &[n(900.0), n(0.0), n(800.0), n(1.0)])
            .is_ok());
        assert!(data.query("magazine", &[n(900.0)]).is_ok());
        assert!(data.query("spread", &[n(900.0)]).is_ok());
        assert!(data
            .query("ascii", &[n(0.0), Value::Bool(true), n(400.0)])
            .is_ok());
        assert!(data.query("ascii", &[n(0.0), n(1.0), n(400.0)]).is_err());
        assert!(data
            .query("balls", &[n(f64::NAN), n(600.0), n(400.0)])
            .is_err());
        assert!(data.query("nothing", &[]).is_err());
        // Same inputs, same answer: no hidden history.
        let a = data
            .query("balls", &[n(5000.0), n(600.0), n(400.0)])
            .unwrap();
        data.query("balls", &[n(9000.0), n(300.0), n(300.0)])
            .unwrap();
        assert_eq!(
            a,
            data.query("balls", &[n(5000.0), n(600.0), n(400.0)])
                .unwrap()
        );
    }
}
