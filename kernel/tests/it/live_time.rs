//! Width reservation is measured at the element's inherited font, not
//! at a guessed digit width. @ref LLP 1126 D6
use exact_kernel::*;

#[test]
fn live_time_keeps_its_frame_and_following_sibling_when_its_text_ticks() {
    for font_size in [15., 28.] {
        let mut k = Kernel::with_monospace();
        let mut row = StyleProps::default();
        row.set_dynamic(StyleId::Display, &StyleValue::Text("flex".into()))
            .unwrap();
        row.set_dynamic(StyleId::FontSize, &StyleValue::Number(font_size))
            .unwrap();
        let mut ops = vec![
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            Op::SetStyle {
                id: 1,
                patch: Box::new(row),
            },
            Op::AttachRoot { id: 1 },
        ];
        for id in [2, 3] {
            ops.push(Op::CreateView {
                id,
                node_type: NodeType::Text,
            });
        }
        ops.extend([
            Op::SetProp {
                id: 2,
                prop: PropId::Text,
                value: "in 5 min.".into(),
            },
            Op::SetProp {
                id: 2,
                prop: PropId::TextReservation,
                value: "59 min, 59 sec ago\nin 59 min, 59 sec".into(),
            },
            Op::SetProp {
                id: 3,
                prop: PropId::Text,
                value: "Following".into(),
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 3],
            },
        ]);
        k.apply(0, 1, &ops).unwrap();
        k.compute_layout(1, Offer::definite(1000., 100.)).unwrap();
        let before = (k.node(2).unwrap().frame, k.node(3).unwrap().frame);
        assert!(before.0.width >= 18. * font_size as f32 * 0.6);
        k.apply(
            0,
            2,
            &[Op::SetProp {
                id: 2,
                prop: PropId::Text,
                value: "59 min, 59 sec ago".into(),
            }],
        )
        .unwrap();
        k.compute_layout(1, Offer::definite(1000., 100.)).unwrap();
        assert_eq!(before, (k.node(2).unwrap().frame, k.node(3).unwrap().frame));
    }
}

#[test]
fn inline_time_uses_natural_runs_and_can_grow() {
    let mut k = Kernel::with_monospace();
    let mut row = StyleProps::default();
    row.set_dynamic(StyleId::Display, &StyleValue::Text("flex".into()))
        .unwrap();
    let mut ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::AttachRoot { id: 1 },
        Op::SetStyle {
            id: 1,
            patch: Box::new(row),
        },
    ];
    for id in 2..=5 {
        ops.push(Op::CreateView {
            id,
            node_type: NodeType::Text,
        });
    }
    ops.extend([
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::SetChildren {
            id: 2,
            children: vec![3, 4, 5],
        },
        Op::SetProp {
            id: 3,
            prop: PropId::Text,
            value: "Updated ".into(),
        },
        Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: "9 min. ago".into(),
        },
        Op::SetProp {
            id: 4,
            prop: PropId::TextReservation,
            value: "59 min, 59 sec ago".into(),
        },
        Op::SetProp {
            id: 5,
            prop: PropId::Text,
            value: " here".into(),
        },
    ]);
    k.apply(0, 1, &ops).unwrap();
    k.compute_layout(1, Offer::definite(1000., 200.)).unwrap();
    let before = k.node(2).unwrap().frame.width;
    let runs = k.node(2).unwrap().text_runs();
    assert_eq!(
        runs.iter().map(|r| &*r.text).collect::<Vec<_>>(),
        ["Updated ", "9 min. ago", " here"]
    );
    k.apply(
        0,
        2,
        &[Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: "10 min. ago".into(),
        }],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(1000., 200.)).unwrap();
    assert!(k.node(2).unwrap().frame.width > before);
}

#[test]
fn duration_candidates_use_the_platform_digits_and_the_displayed_fields() {
    use exact_kernel::time::{english, reservation, TimeRequest};
    let r = TimeRequest {
        datetime: 2000.,
        now: 0.,
        format: "countdown",
        locale: "ar",
        time_zone: "UTC",
        utc_offset: 0.,
    };
    let local = |q: &TimeRequest<'_>| {
        english(q).map(|s| {
            s.chars()
                .map(|c| {
                    if c.is_ascii_digit() {
                        char::from_u32('٠' as u32 + c as u32 - '0' as u32).unwrap()
                    } else {
                        c
                    }
                })
                .collect()
        })
    };
    let values = reservation(&r, local);
    assert_eq!(values.lines().count(), 11);
    assert!(
        values
            .lines()
            .all(|s| s.chars().count() == 5 && !s.chars().any(|c| c.is_ascii_digit())),
        "{values}"
    );
    for font_size in [15., 28.] {
        let mut k = Kernel::with_monospace();
        let mut style = StyleProps::default();
        style
            .set_dynamic(StyleId::Display, &StyleValue::Text("flex".into()))
            .unwrap();
        style
            .set_dynamic(StyleId::FontSize, &StyleValue::Number(font_size))
            .unwrap();
        k.apply(
            0,
            1,
            &[
                Op::CreateView {
                    id: 1,
                    node_type: NodeType::View,
                },
                Op::CreateView {
                    id: 2,
                    node_type: NodeType::Text,
                },
                Op::SetChildren {
                    id: 1,
                    children: vec![2],
                },
                Op::AttachRoot { id: 1 },
                Op::SetStyle {
                    id: 1,
                    patch: Box::new(style),
                },
                Op::SetProp {
                    id: 2,
                    prop: PropId::Text,
                    value: "٠٠:٠٢".into(),
                },
                Op::SetProp {
                    id: 2,
                    prop: PropId::TextReservation,
                    value: values.clone().into(),
                },
            ],
        )
        .unwrap();
        k.compute_layout(1, Offer::definite(1000., 100.)).unwrap();
        let frame = k.node(2).unwrap().frame;
        k.apply(
            0,
            2,
            &[Op::SetProp {
                id: 2,
                prop: PropId::Text,
                value: "٠٨:٠٠".into(),
            }],
        )
        .unwrap();
        k.compute_layout(1, Offer::definite(1000., 100.)).unwrap();
        assert_eq!(k.node(2).unwrap().frame, frame);
        assert!(frame.width < 6. * font_size as f32);
    }
}

#[test]
fn reservation_measurement_uses_the_painted_case_and_hyphens() {
    use std::{cell::RefCell, rc::Rc};
    struct Captured(Rc<RefCell<Vec<String>>>);
    impl TextMeasurer for Captured {
        fn measure(&mut self, r: &TextMeasureRequest<'_>) -> TextMetrics {
            self.0
                .borrow_mut()
                .extend(r.runs.iter().map(|s| s.text.to_string()));
            MonospaceMeasurer::default().measure(r)
        }
    }
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut k = Kernel::new(Box::new(Captured(seen.clone())));
    let mut style = StyleProps::default();
    style
        .set_dynamic(
            StyleId::TextTransform,
            &StyleValue::Text("uppercase".into()),
        )
        .unwrap();
    style
        .set_dynamic(StyleId::Hyphens, &StyleValue::Text("none".into()))
        .unwrap();
    k.apply(
        0,
        1,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::Text,
            },
            Op::AttachRoot { id: 1 },
            Op::SetStyle {
                id: 1,
                patch: Box::new(style),
            },
            Op::SetProp {
                id: 1,
                prop: PropId::Text,
                value: "now".into(),
            },
            Op::SetProp {
                id: 1,
                prop: PropId::TextReservation,
                value: "wide\u{ad} sample".into(),
            },
        ],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(400., 100.)).unwrap();
    assert!(
        seen.borrow().iter().any(|s| s == "WIDE\u{34f} SAMPLE"),
        "{:?}",
        seen.borrow()
    );
    assert!(!seen.borrow().iter().any(|s| s.contains('\u{ad}')));
}

#[test]
fn archived_durations_fit_the_hour_boundary_and_cache_their_reservation() {
    use exact_kernel::time::{english, reservation, TimeRequest};
    use std::{cell::Cell, rc::Rc};
    struct Counting(Rc<Cell<usize>>);
    impl TextMeasurer for Counting {
        fn measure(&mut self, r: &TextMeasureRequest<'_>) -> TextMetrics {
            MonospaceMeasurer::default().measure(r)
        }
        fn measure_reservation(&mut self, r: &TextMeasureRequest<'_>) -> Option<TextMetrics> {
            self.0.set(self.0.get() + 1);
            Some(self.measure(r))
        }
    }
    for format in ["timer", "countdown"] {
        let request = TimeRequest {
            datetime: if format == "timer" { 0. } else { 7200000. },
            now: 0.,
            format,
            locale: "en-US",
            time_zone: "UTC",
            utc_offset: 0.,
        };
        let values = reservation(&request, english);
        assert!(values.lines().all(|s| s.len() == 7), "{values}");
        let count = Rc::new(Cell::new(0));
        let mut k = Kernel::new(Box::new(Counting(count.clone())));
        k.apply(
            0,
            1,
            &[
                Op::CreateView {
                    id: 1,
                    node_type: NodeType::View,
                },
                Op::AttachRoot { id: 1 },
                Op::CreateView {
                    id: 2,
                    node_type: NodeType::Text,
                },
                Op::SetChildren {
                    id: 1,
                    children: vec![2],
                },
                Op::SetStyle {
                    id: 1,
                    patch: Box::new({
                        let mut style = StyleProps::default();
                        style
                            .set_dynamic(StyleId::Display, &StyleValue::Text("flex".into()))
                            .unwrap();
                        style
                    }),
                },
                Op::SetProp {
                    id: 2,
                    prop: PropId::Text,
                    value: english(&request).unwrap().into(),
                },
                Op::SetProp {
                    id: 2,
                    prop: PropId::TextReservation,
                    value: values.into(),
                },
            ],
        )
        .unwrap();
        let mut frames = Vec::new();
        for (batch, now) in [(2, 3599000.), (3, 3600000.), (4, 3601000.)] {
            k.apply(
                0,
                batch,
                &[Op::SetProp {
                    id: 2,
                    prop: PropId::Text,
                    value: english(&TimeRequest { now, ..request }).unwrap().into(),
                }],
            )
            .unwrap();
            k.compute_layout(1, Offer::definite(400., 100.)).unwrap();
            frames.push(k.node(2).unwrap().frame);
        }
        assert!(frames.iter().all(|f| *f == frames[0]));
        k.compute_layout(1, Offer::definite(300., 100.)).unwrap();
        let before = count.get();
        k.compute_layout(1, Offer::definite(400., 100.)).unwrap();
        assert_eq!(count.get(), before, "cached leaf remeasured reservation");
    }
}
