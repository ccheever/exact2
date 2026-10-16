//! The recency bound outside a batch (LLP 1121 stage 4): under pressure, a
//! sweep keeps the last `GRACE` interns nothing holds, and frees one older
//! than that. Its own process: the table is the process's. This thread made
//! no kernel, so it has no batch.

use exact_kernel::style::wide::{self, Keep};
use exact_kernel::ColorValue;

fn srgb(i: usize) -> String {
    format!(
        "color(srgb {} {} 0.75)",
        (i % 64) as f64 / 64.0,
        (i / 64) as f64 / 64.0
    )
}

fn id(c: Option<ColorValue>) -> u16 {
    match c {
        Some(ColorValue::Wide(id)) => id,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_sweep_keeps_the_last_grace_unheld_interns_and_frees_older() {
    // A registered holder, so the table sweeps; it holds one colour.
    let _keep = Keep::of(|m| m.color(ColorValue::parse_light_dark(&srgb(4_095)).unwrap()));
    // Unheld colours until the table, under pressure, sweeps: the intern
    // that sweeps first frees, then takes a slot.
    let mut ids = Vec::new();
    let t = loop {
        let before = wide::used();
        ids.push(id(ColorValue::parse_light_dark(&srgb(ids.len()))));
        if wide::used() <= before {
            break ids.len() - 1;
        }
        assert!(ids.len() < 4_000, "never swept");
    };
    // The GRACE - 1 unheld colours before that one all survived.
    let grace = wide::GRACE as usize;
    assert!(t > 2 * grace, "under pressure at {t}");
    for (k, &i) in ids[t - (grace - 1)..t].iter().enumerate() {
        assert!(wide::wide(i).is_some(), "recent {k} reclaimed");
    }
    // One older than GRACE interns, held by nothing, was freed: interned
    // again, it is another id.
    let old = t - grace - 1;
    assert_ne!(id(ColorValue::parse_light_dark(&srgb(old))), ids[old]);
}
