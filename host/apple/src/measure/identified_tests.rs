use super::*;
use exact_kernel::{
    Kernel, MonospaceMeasurer, NodeType, Op, ParagraphStamp, PropId, StyleId, StyleProps, TextRun,
};
use std::cell::RefCell;
use std::collections::VecDeque;

#[derive(Default)]
struct Foreign {
    calls: usize,
    supplied_bytes: usize,
    answers: VecDeque<CMetrics>,
}
#[allow(unsafe_code)] // Synchronous test callback borrowing its caller-owned C request.
extern "C" fn foreign(ctx: *mut c_void, request: *const CRequest) -> CMetrics {
    // Both pointers are owned by the synchronous test invocation.
    let state = unsafe { &*ctx.cast::<RefCell<Foreign>>() };
    let request = unsafe { &*request };
    let runs = unsafe { std::slice::from_raw_parts(request.runs, request.count) };
    let mut state = state.borrow_mut();
    state.calls += 1;
    state.supplied_bytes += runs.iter().map(|r| r.len).sum::<usize>();
    state.answers.pop_front().unwrap_or(CMetrics {
        width: 73.0,
        height: 17.0,
        baseline: 12.0,
    })
}
fn callback(state: &RefCell<Foreign>) -> CallbackMeasurer {
    CallbackMeasurer::new(foreign, std::ptr::from_ref(state).cast_mut().cast())
}
fn fixture(text: &str) -> Kernel {
    let mut k = Kernel::new(Box::new(MonospaceMeasurer::default()));
    k.apply(
        0,
        0,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::Text,
            },
            Op::SetProp {
                id: 1,
                prop: PropId::Text,
                value: text.into(),
            },
            Op::AttachRoot { id: 1 },
            Op::CreateView {
                id: 2,
                node_type: NodeType::TextInput,
            },
            Op::AttachRoot { id: 2 },
        ],
    )
    .unwrap();
    k
}
fn stamp(k: &Kernel) -> ParagraphStamp {
    k.node(1).unwrap().paragraph_stamp().unwrap()
}
fn run(text: &str) -> TextRun<'_> {
    TextRun {
        text,
        style: exact_kernel::TextStyle::from_style(&StyleProps::default()),
    }
}
fn request<'a>(
    runs: &'a [TextRun<'a>],
    width: AxisOffer,
    height: AxisOffer,
) -> TextMeasureRequest<'a> {
    TextMeasureRequest {
        runs,
        paragraph: exact_kernel::text::Paragraph::from_style(&StyleProps::default()),
        width,
        height,
    }
}

#[test]
fn giant_unchanged_typing_adds_zero_foreign_callbacks() {
    let source = "Ée\u{301}🧪漢字 ".repeat(262_144);
    let mut kernel = fixture(&source);
    let state = RefCell::new(Foreign::default());
    let mut measurer = callback(&state);
    let runs = [run(&source)];
    let request = request(&runs, AxisOffer::Definite(820.0), AxisOffer::MaxContent);
    let before = stamp(&kernel);
    let expected = measurer.measure_identified(&before, &request);
    for value in ["a", "EXACT_Ée\u{301}_🧪漢字", "", "typed again"] {
        kernel
            .apply(
                0,
                0,
                &[Op::SetProp {
                    id: 2,
                    prop: PropId::Value,
                    value: value.into(),
                }],
            )
            .unwrap();
        assert!(before.same_metrics(&stamp(&kernel)));
        assert_eq!(
            measurer.measure_identified(&stamp(&kernel), &request),
            expected
        );
    }
    eprintln!(
        "giant bytes={} initial=1 additional callbacks={}",
        source.len(),
        state.borrow().calls - 1
    );
    assert_eq!(state.borrow().calls, 1);
    assert_eq!(state.borrow().supplied_bytes, source.len());
}

#[test]
fn both_axis_offers_are_exact_and_intrinsic_kinds_are_distinct() {
    let k = fixture("offers");
    let stamp = stamp(&k);
    let state = RefCell::new(Foreign::default());
    let mut m = callback(&state);
    let runs = [run("offers")];
    let pairs = [
        (AxisOffer::Definite(10.0), AxisOffer::MaxContent),
        (
            AxisOffer::Definite(f32::from_bits(10.0f32.to_bits() + 1)),
            AxisOffer::MaxContent,
        ),
        (AxisOffer::Definite(10.0), AxisOffer::MinContent),
        (AxisOffer::MinContent, AxisOffer::MaxContent),
    ];
    for &(w, h) in &pairs {
        m.measure_identified(&stamp, &request(&runs, w, h));
    }
    for &(w, h) in &pairs {
        m.measure_identified(&stamp, &request(&runs, w, h));
    }
    assert_eq!(state.borrow().calls, 4);
    // Fill the owner to capacity with further distinct offers: all retained.
    for i in 0..12u32 {
        let w = AxisOffer::Definite(100.0 + i as f32);
        m.measure_identified(&stamp, &request(&runs, w, AxisOffer::MaxContent));
    }
    assert_eq!(state.borrow().calls, 16);
    for &(w, h) in &pairs {
        m.measure_identified(&stamp, &request(&runs, w, h));
    }
    assert_eq!(state.borrow().calls, 16, "a full pass of offers is retained");
    m.measure_identified(
        &stamp,
        &request(&runs, AxisOffer::MaxContent, AxisOffer::MaxContent),
    );
    assert_eq!(state.borrow().calls, 17);
    m.measure_identified(&stamp, &request(&runs, pairs[0].0, pairs[0].1));
    assert_eq!(state.borrow().calls, 18, "the seventeenth offer evicts the oldest");
}

#[test]
fn paint_only_reuses_metrics_but_source_and_metric_changes_do_not() {
    let mut k = fixture("é");
    let state = RefCell::new(Foreign::default());
    let mut m = callback(&state);
    let runs = [run("é")];
    let req = request(&runs, AxisOffer::Definite(80.0), AxisOffer::MaxContent);
    let original = stamp(&k);
    m.measure_identified(&original, &req);
    k.apply(
        0,
        0,
        &[Op::SetProp {
            id: 1,
            prop: PropId::Href,
            value: "next".into(),
        }],
    )
    .unwrap();
    assert!(original.same_metrics(&stamp(&k)));
    assert_ne!(original, stamp(&k));
    m.measure_identified(&stamp(&k), &req);
    assert_eq!(state.borrow().calls, 1);
    k.apply(
        0,
        0,
        &[Op::SetProp {
            id: 1,
            prop: PropId::Text,
            value: "e\u{301}".into(),
        }],
    )
    .unwrap();
    let changed_runs = [run("e\u{301}")];
    m.measure_identified(
        &stamp(&k),
        &request(
            &changed_runs,
            AxisOffer::Definite(80.0),
            AxisOffer::MaxContent,
        ),
    );
    assert_eq!(state.borrow().calls, 2);
    let mut style = StyleProps {
        font_size: 19.0,
        ..StyleProps::default()
    };
    style.mask.set(StyleId::FontSize);
    k.apply(
        0,
        0,
        &[Op::SetStyle {
            id: 1,
            patch: Box::new(style),
        }],
    )
    .unwrap();
    let changed_runs = k.node(1).unwrap().text_runs();
    m.measure_identified(
        &stamp(&k),
        &request(
            &changed_runs,
            AxisOffer::Definite(80.0),
            AxisOffer::MaxContent,
        ),
    );
    assert_eq!(state.borrow().calls, 3);
}

#[test]
fn namespace_and_reused_node_generation_cannot_alias() {
    let mut a = fixture("same");
    let b = fixture("same");
    let state = RefCell::new(Foreign::default());
    let mut m = callback(&state);
    let runs = [run("same")];
    let req = request(&runs, AxisOffer::Definite(80.0), AxisOffer::MaxContent);
    m.measure_identified(&stamp(&a), &req);
    m.measure_identified(&stamp(&b), &req);
    assert_eq!(state.borrow().calls, 2);
    a.apply(
        0,
        0,
        &[
            Op::DestroyView { id: 1 },
            Op::CreateView {
                id: 1,
                node_type: NodeType::Text,
            },
            Op::SetProp {
                id: 1,
                prop: PropId::Text,
                value: "same".into(),
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    m.measure_identified(&stamp(&a), &req);
    m.measure_identified(&stamp(&a), &req);
    assert_eq!(state.borrow().calls, 3);
}

#[test]
fn malformed_raw_results_are_returned_sanitized_but_never_memoized() {
    let invalid = [
        CMetrics {
            width: f32::NAN,
            height: 17.0,
            baseline: 12.0,
        },
        CMetrics {
            width: -1.0,
            height: 17.0,
            baseline: 12.0,
        },
        CMetrics {
            width: 73.0,
            height: -1.0,
            baseline: 12.0,
        },
        CMetrics {
            width: 73.0,
            height: f32::INFINITY,
            baseline: 12.0,
        },
        CMetrics {
            width: 73.0,
            height: 17.0,
            baseline: f32::NAN,
        },
    ];
    let k = fixture("raw");
    let runs = [run("raw")];
    let req = request(&runs, AxisOffer::Definite(80.0), AxisOffer::MaxContent);
    for bad in invalid {
        let state = RefCell::new(Foreign {
            answers: VecDeque::from([bad]),
            ..Foreign::default()
        });
        let mut m = callback(&state);
        let first = m.measure_identified(&stamp(&k), &req);
        assert!(
            first.width.is_finite()
                && first.height.is_finite()
                && first.width >= 0.0
                && first.height >= 0.0
        );
        if !bad.width.is_finite() || bad.width < 0.0 {
            assert_eq!(first.width, 0.0);
        }
        if !bad.height.is_finite() || bad.height < 0.0 {
            assert_eq!(first.height, 0.0);
        }
        if !bad.baseline.is_finite() {
            assert_eq!(first.first_baseline, None);
        }
        let valid = m.measure_identified(&stamp(&k), &req);
        assert_eq!(
            valid,
            TextMetrics {
                width: 73.0,
                height: 17.0,
                first_baseline: Some(12.0)
            }
        );
        assert_eq!(m.measure_identified(&stamp(&k), &req), valid);
        assert_eq!(state.borrow().calls, 2);
    }
}

#[test]
fn legitimate_zero_and_unknown_baseline_are_cacheable() {
    let state = RefCell::new(Foreign {
        answers: VecDeque::from([CMetrics {
            width: 0.0,
            height: 0.0,
            baseline: -1.0,
        }]),
        ..Foreign::default()
    });
    let mut m = callback(&state);
    let k = fixture("");
    let runs = [run("")];
    let req = request(&runs, AxisOffer::Definite(0.0), AxisOffer::Definite(0.0));
    let expected = TextMetrics {
        width: 0.0,
        height: 0.0,
        first_baseline: None,
    };
    assert_eq!(m.measure_identified(&stamp(&k), &req), expected);
    assert_eq!(m.measure_identified(&stamp(&k), &req), expected);
    assert_eq!(state.borrow().calls, 1);
}

#[test]
fn unidentified_requests_keep_the_existing_callback_behavior() {
    let state = RefCell::new(Foreign::default());
    let mut m = callback(&state);
    let runs = [run("raw")];
    let req = request(&runs, AxisOffer::Definite(80.0), AxisOffer::MaxContent);
    m.measure(&req);
    m.measure(&req);
    assert_eq!(state.borrow().calls, 2);
}

#[test]
fn owner_capacity_and_revision_replacement_do_not_keep_history() {
    let mut k = fixture("bounded");
    let state = RefCell::new(Foreign::default());
    let mut m = callback(&state);
    let runs = [run("bounded")];
    let req = request(&runs, AxisOffer::Definite(80.0), AxisOffer::MaxContent);
    for id in 3..=258 {
        k.apply(
            0,
            0,
            &[
                Op::CreateView {
                    id,
                    node_type: NodeType::Text,
                },
                Op::SetProp {
                    id,
                    prop: PropId::Text,
                    value: "bounded".into(),
                },
            ],
        )
        .unwrap();
        m.measure_identified(&k.node(id).unwrap().paragraph_stamp().unwrap(), &req);
    }
    for _ in 0..2 {
        m.measure_identified(&k.node(3).unwrap().paragraph_stamp().unwrap(), &req);
    }
    assert_eq!(state.borrow().calls, 256);
    m.measure_identified(&stamp(&k), &req);
    m.measure_identified(&k.node(3).unwrap().paragraph_stamp().unwrap(), &req);
    assert_eq!(
        state.borrow().calls,
        258,
        "owner257 evicts oldest owner; hits do not extend FIFO"
    );
    for i in 0..1000 {
        k.apply(
            0,
            0,
            &[Op::SetProp {
                id: 1,
                prop: PropId::Text,
                value: i.to_string().into(),
            }],
        )
        .unwrap();
        let node = k.node(1).unwrap();
        let current = node.text_runs();
        m.measure_identified(
            &node.paragraph_stamp().unwrap(),
            &request(&current, AxisOffer::Definite(80.0), AxisOffer::MaxContent),
        );
    }
    assert_eq!(m.memo.counts(), (256, 256, 256));
    // A still-retained other owner must not be evicted by1000 revisions of owner1.
    let n = state.borrow().calls;
    m.measure_identified(&k.node(5).unwrap().paragraph_stamp().unwrap(), &req);
    assert_eq!(state.borrow().calls, n);
}
