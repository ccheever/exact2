//! UIKit's springs against what UIKit did (LLP 1099 §4, §8.1–§8.2): every
//! measured call in `motion/tests/fixtures/uikit-springs/ios-27.0.txt`, the
//! probe's report from the iOS 27.0 simulator.
//!
//! The mapping (D2, D3) and the renderer (D4) are tested apart: the curve
//! tests feed Core Animation's measured coefficients and end times, so a
//! mapping miss cannot hide a renderer error or the reverse.

use exact_motion::uikit::{self, Branch, UikitError};
use exact_motion::SpringConfig;

const REPORT: &str = include_str!("../fixtures/uikit-springs/ios-27.0.txt");

/// One report line: its `key=value` fields, and its `t:x` samples if any.
struct Row {
    fields: Vec<(String, String)>,
    samples: Vec<(f64, f64)>,
}

impl Row {
    fn get(&self, key: &str) -> f64 {
        self.text(key)
            .parse()
            .unwrap_or_else(|_| panic!("{key} is not a number"))
    }
    fn text(&self, key: &str) -> &str {
        self.fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .unwrap_or_else(|| panic!("no {key}"))
    }
}

fn rows(tag: &str) -> Vec<Row> {
    REPORT
        .lines()
        .filter(|l| l.starts_with(tag) && l.as_bytes().get(tag.len()) == Some(&b' '))
        .map(|l| {
            let mut fields = Vec::new();
            let mut samples = Vec::new();
            for word in l.split(' ').skip(1) {
                if let Some((k, v)) = word.split_once('=') {
                    fields.push((k.to_string(), v.to_string()));
                } else if let Some((t, x)) = word.split_once(':') {
                    if let (Ok(t), Ok(x)) = (t.parse(), x.parse()) {
                        samples.push((t, x));
                    }
                }
            }
            Row { fields, samples }
        })
        .collect()
}

/// A duration-form call: (ζ, velocity, duration, UIKit's stiffness).
fn duration_calls() -> Vec<(f64, f64, f64, f64)> {
    let mut calls = Vec::new();
    for tag in ["A", "B", "C"] {
        for r in rows(tag) {
            if r.fields.iter().any(|(k, _)| k == "z") && r.fields.iter().any(|(k, _)| k == "d") {
                calls.push((r.get("z"), r.get("v"), r.get("d"), r.get("stiffness")));
            }
        }
    }
    for r in rows("W") {
        calls.push((r.get("z"), r.get("u"), 1.0, r.get("k")));
    }
    calls
}

#[test]
fn the_report_is_the_one_the_llp_measured() {
    assert!(REPORT.starts_with("probe iOS 27.0\n"));
    assert_eq!(
        rows("A")
            .iter()
            .filter(|r| r.fields.iter().any(|(k, _)| k == "z"))
            .count(),
        2457
    );
    assert_eq!(rows("W").len(), 7014);
    assert_eq!(rows("E").len() + rows("E2").len(), 60);
}

#[test]
fn every_duration_call_has_mass_one_its_velocity_its_end_and_critical_ratio_damping() {
    for tag in ["A", "B", "C"] {
        for r in rows(tag) {
            if !r.fields.iter().any(|(k, _)| k == "z") || !r.fields.iter().any(|(k, _)| k == "d") {
                continue;
            }
            let (z, d) = (r.get("z").min(1.0), r.get("d"));
            let ok = uikit::duration(d, r.get("z"), r.get("v"))
                .map(|s| (s.config.mass, s.velocity, s.end));
            assert_eq!(r.get("mass"), 1.0);
            assert_eq!(r.get("v0"), r.get("v"));
            assert_eq!(r.get("duration"), d);
            let measured = r.get("damping") / (2.0 * r.get("stiffness").sqrt());
            assert!((measured - z).abs() < 1e-9, "{tag} ζ {z}: {measured}");
            if let Ok((mass, v, end)) = ok {
                assert_eq!((mass, v, end), (1.0, r.get("v"), Some(d)));
            }
        }
    }
}

#[test]
fn the_zero_velocity_closed_form_matches_every_call() {
    let mut n = 0;
    for (z, v, d, k) in duration_calls() {
        if v != 0.0 {
            continue;
        }
        let spring = uikit::duration(d, z, 0.0).expect("zero velocity always resolves");
        let w = spring.config.stiffness.sqrt() * d;
        let measured = k.sqrt() * d;
        assert!(
            (w / measured - 1.0).abs() < 1e-7,
            "ζ {z} d {d}: {w} against {measured}"
        );
        n += 1;
    }
    assert_eq!(n, 308);
}

/// The reconstruction against every duration call. Misses are pinned: each
/// lies in the band, and their count moves only if the procedure does.
#[test]
fn the_reconstruction_matches_uikit_outside_the_band() {
    let (mut matched, mut missed, mut refused) = (0, 0, 0);
    for (z, v, d, k) in duration_calls() {
        let measured = k.sqrt() * d;
        let u = v * d;
        let w = uikit::duration_w(z, u);
        if (w / measured).ln().abs() < 1e-5 {
            matched += 1;
            assert!(
                uikit::is_root(w, z, u),
                "ζ {z} u {u}: a match must validate"
            );
            continue;
        }
        missed += 1;
        assert!(uikit::in_band(z, u), "ζ {z} u {u}: a miss outside the band");
        match uikit::duration(d, z, v) {
            Err(UikitError::NoRoot) => refused += 1,
            Ok(_) => {}
            Err(e) => panic!("ζ {z} u {u}: {e:?}"),
        }
    }
    // 9,510 calls: A, B and C (2,496) and the W sweep (7,014). The LLP's §4.3
    // counts A, W and the U rows (100 misses, 27 refusals); here B and C add
    // one miss, refused, at ζ 1 and u 5, and the U rows are left out.
    assert_eq!((matched, missed, refused), (9410, 100, 28));
}

#[test]
fn signals_calls_resolve_as_the_llp_says() {
    let menu = uikit::duration(0.4, 0.8, 1.0).unwrap();
    assert!((menu.config.stiffness / 497.5361505635245 - 1.0).abs() < 1e-7);
    assert!((menu.config.damping / 35.68882942101944 - 1.0).abs() < 1e-7);
    assert!(!uikit::in_band(0.8, 0.4));
    // The reply icon is in the band: UIKit took k 184.95, the procedure 171.05.
    let icon = uikit::duration(0.2, 0.06, 0.8).unwrap();
    assert!(uikit::in_band(0.06, 0.16));
    assert!(
        (icon.config.stiffness - 171.0543).abs() < 1e-3,
        "{}",
        icon.config.stiffness
    );
}

#[test]
fn bounce_coefficients_and_ends_match_uikit() {
    let mut n = 0;
    for tag in ["E", "E2"] {
        for r in rows(tag) {
            let v = if tag == "E" { r.get("v") } else { 0.0 };
            let s = uikit::bounce(r.get("d"), r.get("b"), v).unwrap();
            assert!((s.config.stiffness / r.get("stiffness") - 1.0).abs() < 1e-12);
            assert!((s.config.damping / r.get("damping") - 1.0).abs() < 1e-12);
            let end = s.end.unwrap();
            assert!(
                (end / r.get("duration") - 1.0).abs() < 2e-4,
                "{tag} d {} b {}: {end}",
                r.get("d"),
                r.get("b")
            );
            assert_eq!(s.branch == Branch::Overdamped, r.get("b") < 0.0);
            n += 1;
        }
    }
    assert_eq!(n, 60);
}

#[test]
fn core_animations_settling_duration_is_the_closed_form_below_critical() {
    let mut n = 0;
    for tag in ["A", "B", "C", "D", "E"] {
        for r in rows(tag) {
            if !r.fields.iter().any(|(k, _)| k == "settling") {
                continue;
            }
            let config = SpringConfig {
                stiffness: r.get("stiffness"),
                damping: r.get("damping"),
                mass: r.get("mass"),
            };
            if config.damping / (2.0 * config.stiffness.sqrt()) >= 1.0 {
                continue;
            }
            let t = uikit::core_animation_settle(&config, r.get("v0"));
            assert!((t / r.get("settling") - 1.0).abs() < 1e-12, "{tag}: {t}");
            n += 1;
        }
    }
    assert_eq!(n, 1817);
}

#[test]
fn the_critical_end_takes_the_last_crossing() {
    assert_eq!(uikit::critical_settle(1.0, 0.0), 9.233413476451585);
    assert!((uikit::critical_settle(1.0, 1.2261769259289865) - 5.421646815).abs() < 1e-8);
    assert!((uikit::critical_settle(1.0, 1.2262) - 5.454495022).abs() < 1e-8);
}

/// Core Animation's rendered curves (rows U: springs UIKit made; rows S:
/// built by hand), fed their measured coefficients and end times.
#[test]
fn the_curve_is_core_animations_before_the_end_and_the_target_after() {
    let mut curves = 0;
    for tag in ["U", "S"] {
        for r in rows(tag) {
            let config = SpringConfig {
                stiffness: r.get("k"),
                damping: r.get("c"),
                mass: 1.0,
            };
            let branch = if r.text("aod") == "true" {
                Branch::Overdamped
            } else {
                Branch::Textbook
            };
            let (v, end) = (r.get("v0"), r.get("dur"));
            // Without allowsOverdamping Core Animation clamps to critical.
            let config =
                if branch == Branch::Textbook && config.damping > 2.0 * config.stiffness.sqrt() {
                    SpringConfig {
                        damping: 2.0 * config.stiffness.sqrt(),
                        ..config
                    }
                } else {
                    config
                };
            for &(t, x) in &r.samples {
                if t < end - 1e-9 {
                    let model = 1.0 + uikit::sample(&config, branch, -1.0, v, t).displacement;
                    assert!(
                        (model - x).abs() < 1e-4,
                        "{tag} k {} t {t}: {model} against {x}",
                        config.stiffness
                    );
                } else if t > end + 1e-9 {
                    assert!((x - 1.0).abs() < 1e-9, "{tag} after the end at {t}: {x}");
                }
            }
            curves += 1;
        }
    }
    assert_eq!(curves, 14);
}

#[test]
fn refusals() {
    assert_eq!(uikit::duration(0.0005, 0.8, 0.0), Err(UikitError::Duration));
    assert_eq!(uikit::duration(61.0, 0.8, 0.0), Err(UikitError::Duration));
    assert_eq!(uikit::duration(0.4, 0.0, 0.0), Err(UikitError::Ratio));
    assert_eq!(
        uikit::duration(0.4, f64::NAN, 0.0),
        Err(UikitError::NonFinite)
    );
    assert_eq!(uikit::duration(0.1, 0.2, 5.0), Err(UikitError::NoRoot));
    assert_eq!(uikit::bounce(0.4, 1.0, 0.0), Err(UikitError::Bounce));
    assert_eq!(uikit::bounce(3.0, 0.95, 0.0), Err(UikitError::EndTooLate));
    assert!(uikit::bounce(1.0, 0.9, 0.0).is_ok());
    // ζ above 1 is clamped, as UIKit clamps it.
    assert_eq!(
        uikit::duration(0.4, 5.0, 1.0),
        uikit::duration(0.4, 1.0, 1.0)
    );
}
