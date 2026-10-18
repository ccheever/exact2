//! A session's role overrides hold their wide colours (LLP 1121 stage 5):
//! kept through every sweep while the session's [`Roles`] live, freed after
//! it goes. One test, in its own process: the table is the process's.

use exact_kernel::style::roles::{self, Roles};
use exact_kernel::style::wide;
use exact_kernel::{ColorValue, Kernel, NodeType, Op, StyleId, StyleProps, StyleValue};

fn p3(i: usize) -> String {
    format!(
        "color(display-p3 {} {} 0.75)",
        (i % 100) as f64 / 100.0,
        (i / 100) as f64 / 100.0
    )
}

fn background(id: u32, css: &str) -> Op {
    let mut s = StyleProps::default();
    s.set_dynamic(StyleId::BackgroundColor, &StyleValue::Text(css.into()))
        .unwrap();
    Op::SetStyle {
        id,
        patch: Box::new(s),
    }
}

/// More colours than the table holds, set and replaced in turn on one node.
fn churn(from: usize) {
    let mut k = Kernel::with_monospace();
    k.apply(
        0,
        1,
        &[Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        }],
    )
    .unwrap();
    for (n, i) in (from..from + 6_000).enumerate() {
        k.apply(0, n as u64 + 2, &[background(1, &p3(i))]).unwrap();
    }
}

#[test]
fn a_sessions_overrides_keep_their_wide_colours_while_it_lives() {
    let canvas_text = roles::role("CanvasText").unwrap();
    let ColorValue::Wide(id) =
        ColorValue::parse_light_dark("color(display-p3 0.123 0.456 0.789)").unwrap()
    else {
        panic!("not wide")
    };
    let text = wide::wide(id).unwrap().text.to_string();
    let session = Roles::new(vec![(canvas_text, ColorValue::Wide(id))]);
    churn(0);
    assert_eq!(
        wide::wide(id).map(|w| w.text.to_string()),
        Some(text),
        "kept while the session lives"
    );
    {
        let _bound = roles::bind(&session);
        assert_eq!(roles::overridden(canvas_text), Some(ColorValue::Wide(id)));
    }
    drop(session);
    churn(10_000);
    // Its slot was reclaimed: interned again, the colour is another id.
    assert_ne!(
        ColorValue::parse_light_dark("color(display-p3 0.123 0.456 0.789)"),
        Some(ColorValue::Wide(id)),
        "freed after it goes"
    );
}
