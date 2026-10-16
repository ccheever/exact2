//! A batch parsed before its commit keeps its colours through the sweeps
//! its own parsing sets off, however many it parses (LLP 1121 stage 4). Its
//! own process: the table is the process's.

use exact_kernel::style::wide;
use exact_kernel::{ColorValue, Kernel, NodeType, Op, RowValue, StyleId, StyleProps, StyleValue};

/// A distinct display-p3 colour for each `i` below 10,000.
fn p3(i: usize) -> String {
    format!(
        "color(display-p3 {} {} 0.75)",
        (i % 100) as f64 / 100.0,
        (i / 100) as f64 / 100.0
    )
}

#[test]
fn a_batch_keeps_what_it_parsed_until_its_commit() {
    // The kernel that will hold the batch: this thread is a holder's, so
    // its batches are tracked.
    let mut k = Kernel::with_monospace();
    // Many more colours than `GRACE`, parsed before one commit: the table
    // passes three quarters full partway, and sweeps.
    const N: usize = 3_000;
    assert!(N > 2 * wide::GRACE as usize && N as u64 <= wide::SPAN);
    let mut ops = Vec::new();
    for i in 0..N {
        let id = i as u32 + 1;
        let mut s = StyleProps::default();
        s.set_dynamic(StyleId::BackgroundColor, &StyleValue::Text(p3(i).into()))
            .unwrap();
        ops.push(Op::CreateView {
            id,
            node_type: NodeType::View,
        });
        ops.push(Op::SetStyle {
            id,
            patch: Box::new(s),
        });
        ops.push(Op::AttachRoot { id });
    }
    assert!(wide::used() > 4096 / 2);
    k.apply(0, 1, &ops).unwrap();
    for i in 0..N {
        let node = k.node(i as u32 + 1).unwrap();
        let RowValue::ColorValue(ColorValue::Wide(id)) = node.computed(StyleId::BackgroundColor)
        else {
            panic!("not wide")
        };
        // A reclaimed id reads as stale, and a debug build panics on it.
        let text = wide::wide(id).expect("kept").text.to_string();
        assert_eq!(text, p3(i));
    }
    // A batch left open (parsed, never committed) protects no more than
    // `SPAN` interns: colours churned on a thread with no holder are still
    // reclaimed, and nothing is refused.
    drop(k);
    let idle = Kernel::with_monospace();
    let _ = ColorValue::parse_light_dark("color(display-p3 0.111 0.222 0.333)");
    let refusals = wide::refusals();
    std::thread::spawn(|| {
        for i in 0..2 * 4096 {
            let _ = ColorValue::parse_light_dark(&format!(
                "color(srgb {} {} 0.5)",
                (i % 128) as f64 / 128.0,
                (i / 128) as f64 / 128.0
            ));
        }
    })
    .join()
    .unwrap();
    assert_eq!(wide::refusals(), refusals);
    drop(idle);
}
