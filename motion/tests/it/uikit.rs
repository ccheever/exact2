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
            assert_eq!(r.get("mass"), 1.0);
            assert_eq!(r.get("v0"), r.get("v"));
            assert_eq!(r.get("duration"), d);
            let measured = r.get("damping") / (2.0 * r.get("stiffness").sqrt());
            assert!((measured - z).abs() < 1e-9, "{tag} ζ {z}: {measured}");
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

/// Every (ζ, u) where the reconstruction does not pick UIKit's root: the
/// procedure's W, bit for bit, and whether it validates as a root. All lie in
/// the band (LLP 1099 §4.3). Pinned so that any change to the procedure, or
/// another root that happens to keep the totals, fails here.
const MISSES: &[(f64, f64, f64, bool)] = &[
    (0.05, 0.05, 5.0, false),
    (0.05, 0.1, 1.956893085317737, true),
    (0.1, 0.2, 1.9760429993255104, true),
    (0.2, 0.5, 2.863389818702002e16, false),
    (0.95, 4.0, 4.140966359172943, true),
    (0.99, 4.0, 8.268164196741644, true),
    (0.1, 0.25, 2.4685609302316585, true),
    (1.0, 5.0, 5.0, false),
    (1.01, 5.0, 5.0, false),
    (1.2, 5.0, 5.0, false),
    (1.5, 5.0, 5.0, false),
    (2.0, 5.0, 5.0, false),
    (5.0, 5.0, 5.0, false),
    (0.1, 0.15, 45.76872804352364, true),
    (0.75, 3.0, 3.933699481108475, true),
    (0.1, 0.175, 1.7295420654855544, true),
    (0.85, 3.5, 4.040026985842097, true),
    (0.1, 0.15000000000000002, 45.76872804352364, true),
    (0.3, 0.8500000000000001, 18.619542791137643, true),
    (0.3, 0.9500000000000001, 3.1410387400402793, true),
    (0.3, 1.05, 3.4687740061056065, true),
    (0.4, 1.3, 14.565392156619883, true),
    (0.4, 1.35, 3.345772196387992, true),
    (0.4, 1.4000000000000001, 3.4681824605529767, true),
    (0.5, 1.7000000000000002, 12.0541587939116, true),
    (0.5, 1.8, 3.5633404202224623, true),
    (0.5, 1.85, 3.6604672066442063, true),
    (0.5, 1.9000000000000001, 3.757404986453993, true),
    (0.6, 2.15, 10.322834563157649, true),
    (0.6, 2.2, 10.300067274512031, true),
    (0.6, 2.3000000000000003, 3.7844586230434074, true),
    (0.7, 2.6500000000000004, 9.06761315386303, true),
    (0.7, 2.7, 9.045584684807002, true),
    (0.7, 2.75, 3.8693303403492303, true),
    (0.8, 3.1, 8.193795896584623, true),
    (0.8, 3.1500000000000004, 8.172566769973724, true),
    (0.8, 3.2, 3820286769950989.0, false),
    (0.8, 3.25, 3.989694724044425, true),
    (0.8, 3.3000000000000003, 4.047633267649025, true),
    (0.9, 3.6, 7.660279309881269, true),
    (0.9, 3.6500000000000004, 7.639955125895986, true),
    (0.9, 3.7, 4.037120715216439, true),
    (0.9, 3.75, 4.088213982608586, true),
    (0.95, 3.8000000000000003, 7.6660242890586705, true),
    (0.95, 3.85, 7.648019594341588, true),
    (0.95, 3.9000000000000004, 7.629537672189308, true),
    (0.95, 3.95, 4.092261147405453, true),
    (0.99, 4.05, 8.254370387357355, true),
    (0.99, 4.1000000000000005, 8.240304796827994, true),
    (0.99, 4.15, 4.155680324282418, true),
    (0.99, 4.2, 4.203969149797258, true),
    (1.0, 4.8500000000000005, 8.429301825680309, true),
    (1.0, 4.9, 8.415206116248376, true),
    (1.0, 4.95, 8.400850474841485, true),
    (1.0, 5.050000000000001, 3.99563941169073, true),
    (1.0, 5.1000000000000005, 4.043002764616388, true),
    (1.0, 5.65, 4.7676414514200065, true),
    (1.0, 5.7, 4.8245276315656, true),
    (1.0, 5.75, 4.881878113510588, true),
    (1.0, 5.800000000000001, 4.939732925470782, true),
    (1.0, 5.8500000000000005, 4.998136909353037, true),
    (1.0, 5.9, 5.057140535494202, true),
    (1.0, 5.95, 5.116800902884076, true),
];

/// The reconstruction against every duration call, with the full resolved
/// spring: outside the pinned misses it is UIKit's stiffness, damping, mass,
/// velocity and end; at a miss it is the pinned W, and refused exactly when
/// that W is not a root.
#[test]
fn the_reconstruction_matches_uikit_outside_the_band() {
    let (mut matched, mut missed, mut refused) = (0, 0, 0);
    for (z, v, d, k) in duration_calls() {
        let measured = k.sqrt() * d;
        let u = v * d;
        let w = uikit::duration_w(z, u);
        let resolved = uikit::duration(d, z, v);
        if let Some(&(_, _, pinned, root)) = MISSES.iter().find(|m| m.0 == z && m.1 == u) {
            missed += 1;
            assert_eq!(
                w.to_bits(),
                pinned.to_bits(),
                "ζ {z} u {u}: {w} against the pinned {pinned}"
            );
            assert!(
                (w / measured).ln().abs() >= 1e-5,
                "ζ {z} u {u}: pinned as a miss"
            );
            assert!(uikit::in_band(z, u), "ζ {z} u {u}: a miss outside the band");
            assert_eq!(uikit::is_root(w, z, u), root);
            match resolved {
                Err(UikitError::NoRoot) => {
                    assert!(!root);
                    refused += 1
                }
                Ok(s) => {
                    assert!(root);
                    let omega = pinned / d;
                    assert_eq!(s.config.stiffness, omega * omega, "ζ {z} u {u}");
                    assert_eq!(s.config.damping, 2.0 * z.min(1.0) * omega, "ζ {z} u {u}");
                    assert_eq!(
                        (s.config.mass, s.velocity, s.end, s.branch),
                        (1.0, v, Some(d), Branch::Textbook)
                    );
                }
                Err(e) => panic!("ζ {z} u {u}: {e:?}"),
            }
            continue;
        }
        matched += 1;
        assert!(
            (w / measured).ln().abs() < 1e-5,
            "ζ {z} u {u}: {w} against UIKit's {measured}"
        );
        let s = resolved.unwrap_or_else(|e| panic!("ζ {z} u {u}: {e:?}"));
        let zeta = z.min(1.0);
        assert!(
            (s.config.stiffness / k - 1.0).abs() < 2e-5,
            "ζ {z} u {u}: k {}",
            s.config.stiffness
        );
        let damping = 2.0 * zeta * k.sqrt();
        assert!(
            (s.config.damping / damping - 1.0).abs() < 2e-5,
            "ζ {z} u {u}: c {}",
            s.config.damping
        );
        assert_eq!(
            (s.config.mass, s.velocity, s.end, s.branch),
            (1.0, v, Some(d), Branch::Textbook)
        );
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
    // A large velocity: the second lobe's falling side.
    assert!((uikit::critical_settle(1.0, 1000.0) - 16.62538030859402).abs() < 1e-8);
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
fn the_response_form_and_the_edges_of_the_domain() {
    let r = uikit::response(0.25, 0.645, 0.0).unwrap();
    let omega = 2.0 * std::f64::consts::PI / 0.25;
    assert_eq!(r.config.stiffness, omega * omega);
    assert_eq!(r.config.damping, 2.0 * 0.645 * omega);
    assert_eq!(
        (r.config.mass, r.end, r.branch),
        (1.0, None, Branch::Textbook)
    );
    // Above critical the response form clamps, as Core Animation does for
    // the timing parameters Signal's helper feeds it.
    assert_eq!(
        uikit::response(0.25, 1.5, 0.0),
        uikit::response(0.25, 1.0, 0.0)
    );
    // The smallest duration and ratio still resolve to finite coefficients.
    let tight = uikit::duration(1e-3, 0.01, 0.0).unwrap();
    assert!(tight.config.stiffness.is_finite() && tight.config.damping.is_finite());
    assert!((tight.config.stiffness.sqrt() * 1e-3 - 230.2635).abs() < 1e-3);
}

#[test]
fn bounce_within_an_ulp_of_zero_keeps_its_branch() {
    // Rounded to critical damping, the end is the critical settle time.
    let positive = uikit::bounce(0.4, 1e-17, 0.0).unwrap();
    assert!(
        (positive.end.unwrap() - 0.587817359).abs() < 1e-8,
        "{positive:?}"
    );
    // A hair below 0 is still Core Animation's overdamped pairing, whose
    // early progress is far ahead of the critical curve's.
    let negative = uikit::bounce(0.4, -1e-13, 0.0).unwrap();
    let t = 0.4 / (2.0 * std::f64::consts::PI);
    let p = 1.0 + uikit::sample(&negative.config, negative.branch, -1.0, 0.0, t).displacement;
    let critical = 1.0 - 2.0 * (-1.0f64).exp();
    assert!(p > critical + 0.5, "{p}");
}

#[test]
fn a_critical_spring_on_the_overdamped_branch_is_critical() {
    // The discriminant of k 2, c 2√2 rounds to 4.4e-16, but ζ is exactly 1.
    let config = SpringConfig {
        stiffness: 2.0,
        damping: 2.0 * 2f64.sqrt(),
        mass: 1.0,
    };
    let t = 1.0 / 2f64.sqrt();
    let p = 1.0 + uikit::sample(&config, Branch::Overdamped, -1.0, 0.0, t).displacement;
    assert!((p - (1.0 - 2.0 * (-1.0f64).exp())).abs() < 1e-9, "{p}");
}

/// The textbook branch is exact2's physics at every ζ (LLP 1099 D4): an
/// overdamped spring is neither clamped nor paired as Core Animation's
/// overdamped branch would.
#[test]
fn the_textbook_branch_keeps_overdamped_physics() {
    let config = SpringConfig {
        stiffness: 246.74011002723395,
        damping: 62.83185307179586,
        mass: 1.0,
    };
    let t = 0.3;
    let textbook = uikit::sample(&config, Branch::Textbook, -1.0, 1.0, t);
    assert_eq!(textbook, config.sample(-1.0, 1.0, t));
    let critical = SpringConfig {
        damping: 2.0 * config.stiffness.sqrt(),
        ..config
    }
    .sample(-1.0, 1.0, t);
    assert!((textbook.displacement - critical.displacement).abs() > 0.2);
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
