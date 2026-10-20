//! A holder the marker misses is a bug, and loud: the id it keeps reads as
//! stale (`None`) once the table reclaims its slot, and a development build
//! asserts (LLP 1121 stage 4). Its own process: the table is the process's.

use exact_kernel::style::wide::{self, Holder};
use exact_kernel::ColorValue;

#[test]
#[cfg_attr(
    debug_assertions,
    should_panic(expected = "read after its slot was reclaimed")
)]
fn an_id_hidden_from_every_mark_reads_as_stale() {
    // A registered holder makes the table sweep; it holds nothing.
    let holder = Holder::new();
    let ColorValue::Wide(hidden) =
        ColorValue::parse_light_dark("color(display-p3 0.1 0.2 0.3)").unwrap()
    else {
        panic!("not wide")
    };
    // In no mark at all. Fill the table until it sweeps, the id long past
    // its grace (`GRACE` interns).
    for i in 0..4096 {
        let css = format!(
            "color(display-p3 {} {} 0.5)",
            (i % 64) as f64 / 64.0,
            (i / 64) as f64 / 64.0
        );
        let _ = ColorValue::parse_light_dark(&css);
        holder.publish(Default::default());
    }
    // Release builds read `None` and render the fallback; debug builds panic.
    assert!(wide::wide(hidden).is_none());
}
