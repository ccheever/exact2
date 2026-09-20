//! Offline geometry: the same clock and viewport produce the same scene.
//! @ref LLP 1043.000 §3 D8, §6.5 — geometry is data committed every 16 ms.
use exact_plan::Value;
use exact_runner::{DataError, DataSource};
mod geometry;

/// Stateless scene source; no network, assets, random seed or wall clock.
#[derive(Default)]
pub struct Textflow {}
impl DataSource for Textflow {
    fn app_id(&self) -> &str {
        "com.exact.textflow"
    }
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        if source != "sceneGeometry" {
            return Err(DataError::UnknownSource(source.into()));
        }
        let bad = || DataError::BadArguments("sceneGeometry(scene, elapsedMs, width)".into());
        if args.len() != 3 {
            return Err(bad());
        }
        let scene = args[0].as_str().ok_or_else(bad)?;
        let ms = args[1]
            .as_number()
            .filter(|v| v.is_finite())
            .ok_or_else(bad)?;
        let width = args[2]
            .as_number()
            .filter(|v| v.is_finite())
            .ok_or_else(bad)?;
        Ok(geometry::scene(scene, ms, width))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_bounds_and_periodic_seeks_have_no_hidden_history() {
        let mut data = Textflow::default();
        let args = |t, w| vec![Value::str("orbs"), Value::Number(t), Value::Number(w)];
        let initial = data.query("sceneGeometry", &args(0., 656.)).unwrap();
        assert_eq!(
            initial,
            data.query("sceneGeometry", &args(60_000., 656.)).unwrap()
        );
        assert_eq!(
            initial,
            data.query("sceneGeometry", &args(-1., 656.)).unwrap()
        );
        // Largest physical replay, then an older time: the result cannot depend on history.
        data.query("sceneGeometry", &args(15_000., 900.)).unwrap();
        assert_eq!(
            initial,
            data.query("sceneGeometry", &args(0., 656.)).unwrap()
        );
        assert_eq!(
            data.query("sceneGeometry", &args(f64::MAX, 1.)).unwrap(),
            data.query("sceneGeometry", &args(f64::MAX, 240.)).unwrap()
        );
        assert!(data
            .query("sceneGeometry", &args(f64::INFINITY, 656.))
            .is_err());
        assert!(data.query("sceneGeometry", &args(0., f64::NAN)).is_err());
    }
}
