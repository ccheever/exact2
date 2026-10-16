//! LLP 1116.001 D1.4: `-exact-fit: shrink`. A text too wide for its box is
//! measured at the scale that fits it, down to half its size, as UIKit's
//! `adjustsFontSizeToFitWidth` with a 0.5 `minimumScaleFactor`; the scale is
//! the kernel's, published to every host, and the runs a painter reads are
//! at it. The stopwatch at the largest text size showed only ":48." (round 3).
use exact_kernel::*;

/// "00:00:48.12" at 40 points: 11 glyphs of 24 = 264 wide, 48 high.
const READOUT: &str = "00:00:48.12";

fn rows(rows: &[(StyleId, &str)]) -> Box<StyleProps> {
    let mut style = StyleProps::default();
    for &(id, value) in rows {
        let value = value
            .parse()
            .map_or_else(|_| StyleValue::Text(value.into()), StyleValue::Number);
        style.set_dynamic(id, &value).unwrap();
    }
    Box::new(style)
}

/// A `width`-wide column (1), its items centred as the stopwatch's are,
/// holding the readout (2) styled `text` beside the defaults.
fn readout(width: &str, text: &[(StyleId, &str)]) -> Kernel {
    let mut k = Kernel::new(Box::new(MonospaceMeasurer::default()));
    let mut style = vec![(StyleId::FontSize, "40"), (StyleId::WhiteSpace, "nowrap")];
    style.extend_from_slice(text);
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
            Op::SetStyle {
                id: 1,
                patch: rows(&[
                    (StyleId::Width, width),
                    (StyleId::Display, "flex"),
                    (StyleId::FlexDirection, "column"),
                    (StyleId::AlignItems, "center"),
                ]),
            },
            Op::SetStyle {
                id: 2,
                patch: rows(&style),
            },
            Op::SetProp {
                id: 2,
                prop: PropId::Text,
                value: PropValue::Str(READOUT.into()),
            },
            Op::SetChildren {
                id: 1,
                children: vec![2],
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    k
}

fn laid(k: &mut Kernel) -> (Frame, f32, f32) {
    k.compute_layout(1, Offer::definite(400.0, 800.0)).unwrap();
    let node = k.node(2).unwrap();
    let size = node.text_runs()[0].style.font_size;
    (node.frame, node.fit_scale(), size)
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.05
}

#[test]
fn a_readout_too_wide_shrinks_to_its_box_and_its_runs_follow() {
    let fit = (StyleId::Fit, "shrink");
    // Without the opt-in, CSS's: the line overflows its box at its size.
    let (frame, scale, size) = laid(&mut readout("200", &[]));
    assert_eq!(
        (frame.width, frame.height, scale, size),
        (264.0, 48.0, 1.0, 40.0)
    );
    // With it, 200 / 264 of its size: as wide as the box, and shorter.
    let (frame, scale, size) = laid(&mut readout("200", &[fit]));
    let s = 200.0 / 264.0;
    assert!(near(scale, s), "{scale}");
    assert!(
        near(frame.width, 200.0) && frame.width <= 200.01,
        "{frame:?}"
    );
    assert!(near(frame.height, 48.0 * s), "{frame:?}");
    assert!(
        near(size, 40.0 * s),
        "the painter's runs are at the scale: {size}"
    );
    // A box it fits leaves it at its size.
    assert_eq!(laid(&mut readout("400", &[fit])).1, 1.0);
}

/// Never below half its size, as UIKit's 0.5 `minimumScaleFactor`; display
/// text bigger than twice the root size (16 here) goes down to the root size,
/// so Dynamic Type's growth stays absorbable. Below the floor it overflows,
/// as UIKit's truncates.
#[test]
fn a_fitted_text_stops_at_half_or_a_display_text_at_the_root_size() {
    let fit = (StyleId::Fit, "shrink");
    // 20 points (under 32): half, 66 of a 132-wide line.
    let (frame, scale, size) = laid(&mut readout("50", &[fit, (StyleId::FontSize, "20")]));
    assert_eq!((scale, size), (0.5, 10.0));
    assert!(near(frame.width, 66.0), "{frame:?}");
    // 40 points (over 32): down to 16, 0.4 of its size.
    let (frame, scale, size) = laid(&mut readout("100", &[fit]));
    assert!(near(scale, 0.4) && near(size, 16.0), "{scale} {size}");
    assert!(
        near(frame.width, 105.6) && near(frame.height, 19.2),
        "{frame:?}"
    );
}

/// A wrapping text fits its longest word, not its whole line.
#[test]
fn a_wrapping_text_fits_its_longest_word() {
    let fit = (StyleId::Fit, "shrink");
    let mut k = readout("200", &[fit, (StyleId::WhiteSpace, "normal")]);
    k.apply(
        1,
        2,
        &[Op::SetProp {
            id: 2,
            prop: PropId::Text,
            value: PropValue::Str("Lap 00:00:48.12".into()),
        }],
    )
    .unwrap();
    let (frame, scale, _) = laid(&mut k);
    assert!(near(scale, 200.0 / 264.0), "{scale}");
    assert!(frame.width <= 200.01, "{frame:?}");
    assert!(
        near(frame.height, 2.0 * 48.0 * scale),
        "two lines: {frame:?}"
    );
}

/// The scale follows the text where the box holds still (a fixed height, a
/// stretched width): the node is a publication candidate though its frame
/// did not move, and its paragraph's metric revision moves.
#[test]
fn a_new_scale_in_a_box_that_held_is_published() {
    let fit = (StyleId::Fit, "shrink");
    let mut k = readout(
        "200",
        &[
            fit,
            (StyleId::Height, "50"),
            (StyleId::AlignSelf, "stretch"),
        ],
    );
    let (frame, scale, _) = laid(&mut k);
    assert!(near(scale, 200.0 / 264.0), "{scale}");
    let stamp = k.node(2).unwrap().paragraph_stamp().unwrap();
    k.apply(
        1,
        2,
        &[Op::SetProp {
            id: 2,
            prop: PropId::Text,
            value: PropValue::Str("00:00:48.123".into()),
        }],
    )
    .unwrap();
    let receipt = k.compute_layout(1, Offer::definite(400.0, 800.0)).unwrap();
    let node = k.node(2).unwrap();
    assert_eq!(node.frame, frame, "the box held");
    assert!(
        near(node.fit_scale(), 200.0 / 288.0),
        "{}",
        node.fit_scale()
    );
    assert!(
        receipt.updated.contains(&node.key),
        "a candidate: {receipt:?}"
    );
    assert!(!node.paragraph_stamp().unwrap().same_metrics(&stamp));
    // The opt-in gone, the text is at its size again.
    let mut fit_row = StyleMask::EMPTY;
    fit_row.set(StyleId::Fit);
    k.apply(
        2,
        3,
        &[Op::ClearStyle {
            id: 2,
            mask: fit_row,
            written: false,
        }],
    )
    .unwrap();
    let receipt = k.compute_layout(1, Offer::definite(400.0, 800.0)).unwrap();
    let node = k.node(2).unwrap();
    assert_eq!(node.fit_scale(), 1.0);
    assert_eq!(node.text_runs()[0].style.font_size, 40.0);
    assert!(receipt.updated.contains(&node.key));
}

/// The stopwatch's shape (round 13): a page whose column holds a fitted
/// readout above a list, whose rows are flex rows of a flexing label and two
/// times. A row added as the readout changes took the readout's runs for its
/// label's: the label was measured at the readout's size, so a lap row was 92
/// points tall with its text at the top (the runs the leaf pass shares between
/// consecutive offers of one leaf were left holding the fitted text's, which
/// it measures through the same buffer).
fn stopwatch() -> Kernel {
    let mut k = Kernel::new(Box::new(MonospaceMeasurer::default()));
    let view = |id| Op::CreateView {
        id,
        node_type: NodeType::View,
    };
    let text = |id, s: &str| {
        [
            Op::CreateView {
                id,
                node_type: NodeType::Text,
            },
            Op::SetProp {
                id,
                prop: PropId::Text,
                value: PropValue::Str(s.into()),
            },
        ]
    };
    let mut ops = vec![
        view(1),
        view(2),
        view(3),
        view(4),
        view(6),
        Op::CreateView {
            id: LIST,
            node_type: NodeType::List,
        },
        view(8),
    ];
    ops.extend(text(5, "00:00.00"));
    ops.extend(text(7, "Stopwatch"));
    ops.extend([
        Op::SetStyle {
            id: 1,
            patch: rows(&[
                (StyleId::Width, "402"),
                (StyleId::Height, "800"),
                (StyleId::FontSize, "17"),
            ]),
        },
        Op::SetStyle {
            id: 2,
            patch: rows(&[
                (StyleId::PositionType, "absolute"),
                (StyleId::Top, "0"),
                (StyleId::Right, "0"),
                (StyleId::Bottom, "0"),
                (StyleId::Left, "0"),
                (StyleId::Display, "flex"),
                (StyleId::FlexDirection, "column"),
            ]),
        },
        Op::SetStyle {
            id: 3,
            patch: rows(&[
                (StyleId::Display, "flex"),
                (StyleId::AlignItems, "center"),
                (StyleId::PaddingTop, "8"),
                (StyleId::PaddingBottom, "8"),
            ]),
        },
        Op::SetStyle {
            id: 4,
            patch: rows(&[
                (StyleId::Display, "flex"),
                (StyleId::FlexDirection, "column"),
                (StyleId::AlignItems, "center"),
                (StyleId::RowGap, "12"),
                (StyleId::PaddingTop, "16"),
                (StyleId::PaddingRight, "16"),
                (StyleId::PaddingBottom, "16"),
                (StyleId::PaddingLeft, "16"),
            ]),
        },
        Op::SetStyle {
            id: 5,
            patch: rows(&[
                (StyleId::FontSize, "76.5"),
                (StyleId::WhiteSpace, "nowrap"),
                (StyleId::Fit, "shrink"),
            ]),
        },
        Op::SetStyle {
            id: 6,
            patch: rows(&[
                (StyleId::Display, "flex"),
                (StyleId::JustifyContent, "space-between"),
                (StyleId::Width, "100%"),
            ]),
        },
        Op::SetStyle {
            id: LIST,
            patch: rows(&[
                (StyleId::FlexGrow, "1"),
                (StyleId::FlexBasis, "0%"),
                (StyleId::MinHeight, "0"),
            ]),
        },
        Op::SetChildren {
            id: 3,
            children: vec![7],
        },
        Op::SetChildren {
            id: 4,
            children: vec![5, 6],
        },
        Op::SetChildren {
            id: LIST,
            children: vec![8],
        },
        Op::SetChildren {
            id: 2,
            children: vec![3, 4, LIST],
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ]);
    k.apply(0, 1, &ops).unwrap();
    k
}

/// The list's list; its section.
const LIST: u32 = 9;
const SECTION: u32 = 8;

/// A lap row (view `at`, its label `at + 1`, its times after) at the head of
/// the section, `laps` the rows already in it, and the readout's text.
fn lap(k: &mut Kernel, batch: u64, at: u32, laps: &[u32], readout: &str) {
    let mut ops = vec![
        Op::CreateView {
            id: at,
            node_type: NodeType::View,
        },
        Op::SetProp {
            id: 5,
            prop: PropId::Text,
            value: PropValue::Str(readout.into()),
        },
        Op::SetStyle {
            id: at,
            patch: rows(&[
                (StyleId::Display, "flex"),
                (StyleId::AlignItems, "center"),
                (StyleId::MinHeight, "52"),
                (StyleId::ColumnGap, "8"),
                (StyleId::PaddingRight, "16"),
                (StyleId::MarginLeft, "16"),
            ]),
        },
    ];
    for (i, text) in ["Lap 1", "00:01.00", "00:01.00"].into_iter().enumerate() {
        let id = at + 1 + i as u32;
        ops.push(Op::CreateView {
            id,
            node_type: NodeType::Text,
        });
        ops.push(Op::SetProp {
            id,
            prop: PropId::Text,
            value: PropValue::Str(text.into()),
        });
        if i == 0 {
            ops.push(Op::SetStyle {
                id,
                patch: rows(&[(StyleId::FlexGrow, "1"), (StyleId::FlexBasis, "0%")]),
            });
        }
    }
    ops.push(Op::SetChildren {
        id: at,
        children: vec![at + 1, at + 2, at + 3],
    });
    let mut children = vec![at];
    children.extend(laps);
    ops.push(Op::SetChildren {
        id: SECTION,
        children,
    });
    k.apply(1, batch, &ops).unwrap();
}

#[test]
fn a_row_added_as_the_readout_changes_keeps_its_own_label() {
    let mut k = stopwatch();
    let mut laps: Vec<u32> = Vec::new();
    for (i, readout) in ["00:01.00", "00:02.00", "00:03.00", "00:04.00"]
        .into_iter()
        .enumerate()
    {
        let at = 20 + 10 * i as u32;
        lap(&mut k, i as u64 + 2, at, &laps, readout);
        laps.insert(0, at);
        k.compute_layout(1, Offer::definite(402.0, 874.0)).unwrap();
        for &row in &laps {
            let label = k.node(row + 1).unwrap().frame;
            assert!(
                label.height < 30.0,
                "lap row {row}, label {label:?}: a line of 17 points, not the readout's"
            );
            let frame = k.node(row).unwrap().frame;
            assert_eq!(frame.height, 52.0, "lap row {row}: {frame:?}");
        }
    }
}
