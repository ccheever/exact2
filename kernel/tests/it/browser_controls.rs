//! Form-control boxes recorded from Chrome 154.0.8037.92 (Linux) on 2026-10-02.
//!
//! Each case is a control with `display:block;margin:0` in a 400 px parent.
//! The page uses `host/web/index.html`'s control reset. The intrinsic pairs
//! below are Chrome's measured boxes; the matrix records how those boxes are
//! used in block flow, flex rows and columns, grid and absolute positioning.

use crate::browser_cases::{css_rows as rows, mismatches, props};
use crate::support::reader::number as n;
use exact_kernel::{
    Kernel, NodeType, Offer, Op, PropId, PropValue, StyleId, TextMeasureRequest, TextMeasurer,
    TextMetrics,
};

#[derive(Clone, Copy)]
struct Kind {
    name: &'static str,
    ty: &'static str,
    role: Option<&'static str>,
    intrinsic: (f32, f32),
    default: (f32, f32),
    expected: &'static [[(f32, f32); 10]; 6],
}

const KINDS: &[Kind] = &[
    Kind {
        name: "checkbox",
        ty: "checkbox",
        role: None,
        intrinsic: (13.0, 13.0),
        default: (13.0, 13.0),
        expected: &CHECKBOX,
    },
    // Chromium does not implement HTML's `switch` attribute, so its oracle
    // is the specified checkbox fallback. Apple hosts report their UISwitch/
    // NSSwitch size through the same intrinsic seam.
    Kind {
        name: "switch",
        ty: "checkbox",
        role: Some("switch"),
        intrinsic: (13.0, 13.0),
        default: (13.0, 13.0),
        expected: &SWITCH,
    },
    Kind {
        name: "file",
        ty: "file",
        role: None,
        intrinsic: (347.0, 25.0),
        default: (347.0, 25.0),
        expected: &FILE,
    },
    Kind {
        name: "range",
        ty: "range",
        role: None,
        intrinsic: (129.0, 16.0),
        default: (129.0, 16.0),
        expected: &RANGE,
    },
    Kind {
        name: "date",
        ty: "date",
        role: None,
        intrinsic: (150.0, 21.0),
        default: (150.0, 21.0),
        expected: &DATE,
    },
    Kind {
        name: "time",
        ty: "time",
        role: None,
        intrinsic: (111.796_875, 22.796_875),
        default: (111.796_875, 22.796_875),
        expected: &TIME,
    },
    Kind {
        name: "datetime-local",
        ty: "datetime-local",
        role: None,
        intrinsic: (240.0, 21.0),
        default: (240.0, 21.0),
        expected: &DATETIME_LOCAL,
    },
    // `Long choice` was the widest option in the recorder.
    Kind {
        name: "select",
        ty: "select",
        role: None,
        intrinsic: (102.0, 19.0),
        // A select with no options; a host reports the chosen option's box.
        default: (22.0, 19.0),
        expected: &SELECT,
    },
];

#[derive(Clone, Copy)]
enum Context {
    Block,
    Row,
    Column,
    Grid,
    Absolute,
    AbsoluteInsets,
}

impl Context {
    const ALL: [Self; 6] = [
        Self::Block,
        Self::Row,
        Self::Column,
        Self::Grid,
        Self::Absolute,
        Self::AbsoluteInsets,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Row => "flex row",
            Self::Column => "flex column",
            Self::Grid => "grid",
            Self::Absolute => "absolute",
            Self::AbsoluteInsets => "absolute between insets",
        }
    }

    fn root(self) -> &'static str {
        match self {
            Self::Block => "",
            Self::Row => "display:flex",
            Self::Column => "display:flex;flex-direction:column",
            Self::Grid => "display:grid",
            Self::Absolute | Self::AbsoluteInsets => "position:relative;height:300px",
        }
    }

    fn item(self, css: &str) -> String {
        if matches!(self, Self::AbsoluteInsets) {
            format!("position:absolute;left:0;right:0;{css}")
        } else if matches!(self, Self::Absolute) {
            format!("position:absolute;{css}")
        } else {
            css.to_owned()
        }
    }
}

#[derive(Clone, Copy)]
enum Variant {
    Auto,
    Width,
    MinMax,
    Ratio,
    Padding,
    Height,
    MinHeight,
    MaxHeight,
    MinWidth,
    MaxWidth,
}

impl Variant {
    const ALL: [Self; 10] = [
        Self::Auto,
        Self::Width,
        Self::MinMax,
        Self::Ratio,
        Self::Padding,
        Self::Height,
        Self::MinHeight,
        Self::MaxHeight,
        Self::MinWidth,
        Self::MaxWidth,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Width => "width",
            Self::MinMax => "min/max",
            Self::Ratio => "aspect-ratio",
            Self::Padding => "padding",
            Self::Height => "height",
            Self::MinHeight => "min-height",
            Self::MaxHeight => "max-height",
            Self::MinWidth => "min-width",
            Self::MaxWidth => "max-width",
        }
    }

    fn css(self) -> &'static str {
        match self {
            Self::Auto => "",
            // 300 px keeps Chrome's file control on one line; its wrapping is
            // content presentation, not the block-sizing rule under test.
            Self::Width => "width:300px",
            Self::MinMax => "min-width:100px;max-width:300px;min-height:20px;max-height:40px",
            Self::Ratio => "aspect-ratio:2",
            Self::Padding => "width:300px;padding:10px",
            Self::Height => "height:40px",
            Self::MinHeight => "min-height:40px",
            Self::MaxHeight => "max-height:10px",
            Self::MinWidth => "min-width:100px",
            Self::MaxWidth => "max-width:300px",
        }
    }
}

fn laid_out(kind: Kind, context: Context, variant: Variant, report_intrinsic: bool) -> Kernel {
    let mut root = rows(context.root());
    root.push((StyleId::Width, n(400.0)));
    let item = rows(&context.item(variant.css()));
    let mut ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(props(&root)),
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::Control,
        },
        Op::SetStyle {
            id: 2,
            patch: Box::new(props(&item)),
        },
        Op::SetProp {
            id: 2,
            prop: PropId::Type,
            value: PropValue::Str(kind.ty.into()),
        },
    ];
    if let Some(role) = kind.role {
        ops.push(Op::SetProp {
            id: 2,
            prop: PropId::AccessibilityRole,
            value: PropValue::Str(role.into()),
        });
    }
    ops.extend([
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ]);
    let mut kernel = Kernel::with_monospace();
    kernel.apply(0, 1, &ops).unwrap();
    if report_intrinsic {
        kernel.set_intrinsic_size(2, Some(kind.intrinsic)).unwrap();
    }
    kernel
        .compute_layout(1, Offer::definite(800.0, 600.0))
        .unwrap();
    kernel
}

#[test]
fn every_control_kind_uses_chromes_box_in_every_placement() {
    let mut failures = Vec::new();
    for &kind in KINDS {
        for context in Context::ALL {
            for variant in Variant::ALL {
                let kernel = laid_out(kind, context, variant, true);
                let (width, height) = kind.expected[context as usize][variant as usize];
                failures.extend(mismatches(
                    &format!("{} {} {}", kind.name, context.name(), variant.name()),
                    &kernel,
                    &[(2, [0.0, 0.0, width, height])],
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{}\n{} failures",
        failures.join("\n"),
        failures.len()
    );
}

#[test]
fn controls_without_a_host_intrinsic_use_chromes_bare_defaults() {
    let mut failures = Vec::new();
    for &kind in KINDS {
        let kernel = laid_out(kind, Context::Block, Variant::Auto, false);
        failures.extend(mismatches(
            kind.name,
            &kernel,
            &[(2, [0.0, 0.0, kind.default.0, kind.default.1])],
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn an_auto_width_control_at_the_document_root_shrinks_to_its_intrinsic() {
    let ops = [
        Op::CreateView {
            id: 1,
            node_type: NodeType::Control,
        },
        Op::SetProp {
            id: 1,
            prop: PropId::Type,
            value: PropValue::Str("checkbox".into()),
        },
        Op::AttachRoot { id: 1 },
    ];
    let mut kernel = Kernel::with_monospace();
    kernel.apply(0, 1, &ops).unwrap();
    kernel
        .compute_layout(1, Offer::definite(400.0, 600.0))
        .unwrap();
    assert_eq!(kernel.node(1).unwrap().frame.width, 13.0);
}

#[test]
fn an_appearance_none_checkbox_keeps_the_reset_border_box_and_its_padding() {
    let ops = [
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(props(&rows("width:400px"))),
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::Control,
        },
        Op::SetStyle {
            id: 2,
            patch: Box::new(props(&rows("width:200px;padding:10px"))),
        },
        Op::SetProp {
            id: 2,
            prop: PropId::Type,
            value: PropValue::Str("checkbox".into()),
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ];
    let mut kernel = Kernel::with_monospace();
    kernel.apply(0, 1, &ops).unwrap();
    kernel
        .compute_layout(1, Offer::definite(400.0, 600.0))
        .unwrap();
    assert_eq!(kernel.node(2).unwrap().frame.width, 200.0);
    assert_eq!(kernel.node(2).unwrap().frame.height, 13.0);

    let receipt = kernel
        .apply(
            1,
            2,
            &[Op::SetStyle {
                id: 2,
                patch: Box::new(props(&rows("appearance:none"))),
            }],
        )
        .unwrap();
    assert!(receipt.layout_invalidated);
    kernel
        .compute_layout(1, Offer::definite(400.0, 600.0))
        .unwrap();
    assert_eq!(kernel.node(2).unwrap().frame.width, 200.0);
    assert_eq!(kernel.node(2).unwrap().frame.height, 33.0);
}

struct ChromeFieldMeasurer;

impl TextMeasurer for ChromeFieldMeasurer {
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        let textarea = request.runs.iter().any(|run| run.text.contains('\n'));
        let (width, height) = if textarea {
            (178.0, 38.0)
        } else {
            (200.0, 19.0)
        };
        TextMetrics {
            width,
            height,
            first_baseline: Some(height * 0.8),
        }
    }
}

#[derive(Clone, Copy)]
struct OtherKind {
    name: &'static str,
    node_type: NodeType,
    semantic: Option<&'static str>,
    role: Option<&'static str>,
    expected: &'static [[(f32, f32); 10]; 6],
}

const OTHER_KINDS: &[OtherKind] = &[
    OtherKind {
        name: "text",
        node_type: NodeType::TextInput,
        semantic: None,
        role: None,
        expected: &TEXT,
    },
    OtherKind {
        name: "password",
        node_type: NodeType::TextInput,
        semantic: None,
        role: None,
        expected: &TEXT,
    },
    OtherKind {
        name: "email",
        node_type: NodeType::TextInput,
        semantic: None,
        role: None,
        expected: &TEXT,
    },
    OtherKind {
        name: "url",
        node_type: NodeType::TextInput,
        semantic: None,
        role: None,
        expected: &TEXT,
    },
    OtherKind {
        name: "tel",
        node_type: NodeType::TextInput,
        semantic: None,
        role: None,
        expected: &TEXT,
    },
    OtherKind {
        name: "search",
        node_type: NodeType::TextInput,
        semantic: None,
        role: None,
        expected: &TEXT,
    },
    OtherKind {
        name: "textarea",
        node_type: NodeType::TextInput,
        semantic: Some("textarea"),
        role: None,
        expected: &TEXTAREA,
    },
    OtherKind {
        name: "button",
        node_type: NodeType::Pressable,
        semantic: None,
        role: Some("button"),
        expected: &BUTTON,
    },
    OtherKind {
        name: "button role=tab",
        node_type: NodeType::Pressable,
        semantic: None,
        role: Some("tab"),
        expected: &BUTTON,
    },
];

fn other_laid_out(kind: OtherKind, context: Context, variant: Variant) -> Kernel {
    let mut root = rows(context.root());
    root.push((StyleId::Width, n(400.0)));
    let item = rows(&context.item(variant.css()));
    let mut ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(props(&root)),
        },
        Op::CreateView {
            id: 2,
            node_type: kind.node_type,
        },
        Op::SetStyle {
            id: 2,
            patch: Box::new(props(&item)),
        },
    ];
    if kind.node_type == NodeType::Pressable {
        ops.extend([
            Op::SetProp {
                id: 2,
                prop: PropId::AccessibilityRole,
                value: PropValue::Str(kind.role.expect("button role").into()),
            },
            Op::CreateView {
                id: 3,
                node_type: NodeType::View,
            },
            Op::SetStyle {
                id: 3,
                patch: Box::new(props(&rows("width:40px;height:18px"))),
            },
            Op::SetChildren {
                id: 2,
                children: vec![3],
            },
        ]);
    } else {
        ops.push(Op::SetProp {
            id: 2,
            prop: PropId::Type,
            value: PropValue::Str(kind.name.into()),
        });
    }
    if let Some(semantic) = kind.semantic {
        ops.push(Op::SetProp {
            id: 2,
            prop: PropId::SemanticTag,
            value: PropValue::Str(semantic.into()),
        });
    }
    ops.extend([
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ]);
    let mut kernel = Kernel::new(Box::new(ChromeFieldMeasurer));
    kernel.apply(0, 1, &ops).unwrap();
    if kind.node_type == NodeType::Pressable {
        assert_eq!(
            kernel.node(2).unwrap().props.str(PropId::AccessibilityRole),
            kind.role
        );
    }
    kernel
        .compute_layout(1, Offer::definite(800.0, 600.0))
        .unwrap();
    kernel
}

#[test]
fn text_fields_textareas_and_buttons_use_chromes_form_control_block_sizing() {
    let mut failures = Vec::new();
    for &kind in OTHER_KINDS {
        for context in Context::ALL {
            for variant in Variant::ALL {
                let kernel = other_laid_out(kind, context, variant);
                let (width, height) = kind.expected[context as usize][variant as usize];
                failures.extend(mismatches(
                    &format!("{} {} {}", kind.name, context.name(), variant.name()),
                    &kernel,
                    &[(2, [0.0, 0.0, width, height])],
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{}\n{} failures",
        failures.join("\n"),
        failures.len()
    );
}

#[test]
fn a_block_button_clamps_its_preferred_width_to_the_available_line() {
    let ops = [
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(props(&rows("width:400px"))),
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::Pressable,
        },
        Op::SetStyle {
            id: 2,
            patch: Box::new(props(&rows("display:block"))),
        },
        Op::SetProp {
            id: 2,
            prop: PropId::AccessibilityRole,
            value: PropValue::Str("button".into()),
        },
        Op::CreateView {
            id: 3,
            node_type: NodeType::Text,
        },
        Op::SetProp {
            id: 3,
            prop: PropId::Text,
            value: PropValue::Str(
                "word word word word word word word word word word word word".into(),
            ),
        },
        Op::SetChildren {
            id: 2,
            children: vec![3],
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ];
    let mut kernel = Kernel::with_monospace();
    kernel.apply(0, 1, &ops).unwrap();
    kernel
        .compute_layout(1, Offer::definite(800.0, 600.0))
        .unwrap();
    assert_eq!(kernel.node(2).unwrap().frame.width, 400.0);
}

#[test]
fn the_block_sizing_marker_follows_a_pressables_href_from_its_initial_props() {
    let ops = [
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(props(&rows("width:400px"))),
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::Pressable,
        },
        Op::SetProp {
            id: 2,
            prop: PropId::Href,
            value: PropValue::Str("/initial-link".into()),
        },
        Op::CreateView {
            id: 3,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 3,
            patch: Box::new(props(&rows("width:40px;height:18px"))),
        },
        Op::SetChildren {
            id: 2,
            children: vec![3],
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ];
    let mut kernel = Kernel::with_monospace();
    kernel.apply(0, 1, &ops).unwrap();
    kernel
        .compute_layout(1, Offer::definite(800.0, 600.0))
        .unwrap();
    assert_eq!(kernel.node(2).unwrap().frame.width, 400.0);

    kernel
        .apply(
            1,
            2,
            &[Op::ClearProp {
                id: 2,
                prop: PropId::Href,
            }],
        )
        .unwrap();
    kernel
        .compute_layout(1, Offer::definite(800.0, 600.0))
        .unwrap();
    assert_eq!(kernel.node(2).unwrap().frame.width, 40.0);

    kernel
        .apply(
            2,
            3,
            &[Op::SetProp {
                id: 2,
                prop: PropId::Href,
                value: PropValue::Str("/later-link".into()),
            }],
        )
        .unwrap();
    kernel
        .compute_layout(1, Offer::definite(800.0, 600.0))
        .unwrap();
    assert_eq!(kernel.node(2).unwrap().frame.width, 400.0);
}

// Literal getBoundingClientRect recordings, context × variant in the order above.
// CDP, createElement/append (no parsed text between controls). The font is the
// web reset's 16px system-ui. Button: 40x18 child, the reset's display:block
// (re-recorded 2026-10-04; equal to the earlier flex-column recording).
// Intrinsic sizing is host-owned (LLP 1069.001 D3); the field measurer supplies
// Chrome's measured default size=20/cols=20/rows=2 for this box-layout test.
// password/email/url/tel/search were recorded separately and equal TEXT.
#[rustfmt::skip]
const CHECKBOX: [[(f32, f32); 10]; 6] = [
    [(13.0, 13.0), (300.0, 13.0), (100.0, 20.0), (13.0, 13.0), (300.0, 13.0), (13.0, 40.0), (13.0, 40.0), (13.0, 10.0), (100.0, 13.0), (13.0, 13.0)],
    [(13.0, 13.0), (300.0, 13.0), (100.0, 20.0), (13.0, 13.0), (300.0, 13.0), (13.0, 40.0), (13.0, 40.0), (13.0, 10.0), (100.0, 13.0), (13.0, 13.0)],
    [(400.0, 13.0), (300.0, 13.0), (300.0, 20.0), (400.0, 200.0), (300.0, 13.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 13.0), (300.0, 13.0)],
    [(400.0, 13.0), (300.0, 13.0), (300.0, 20.0), (400.0, 200.0), (300.0, 13.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 13.0), (300.0, 13.0)],
    [(13.0, 13.0), (300.0, 13.0), (100.0, 20.0), (13.0, 13.0), (300.0, 13.0), (13.0, 40.0), (13.0, 40.0), (13.0, 10.0), (100.0, 13.0), (13.0, 13.0)],
    [(400.0, 13.0), (300.0, 13.0), (300.0, 20.0), (400.0, 200.0), (300.0, 13.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 13.0), (300.0, 13.0)],
];
#[rustfmt::skip]
const SWITCH: [[(f32, f32); 10]; 6] = [
    [(13.0, 13.0), (300.0, 13.0), (100.0, 20.0), (13.0, 13.0), (300.0, 13.0), (13.0, 40.0), (13.0, 40.0), (13.0, 10.0), (100.0, 13.0), (13.0, 13.0)],
    [(13.0, 13.0), (300.0, 13.0), (100.0, 20.0), (13.0, 13.0), (300.0, 13.0), (13.0, 40.0), (13.0, 40.0), (13.0, 10.0), (100.0, 13.0), (13.0, 13.0)],
    [(400.0, 13.0), (300.0, 13.0), (300.0, 20.0), (400.0, 200.0), (300.0, 13.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 13.0), (300.0, 13.0)],
    [(400.0, 13.0), (300.0, 13.0), (300.0, 20.0), (400.0, 200.0), (300.0, 13.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 13.0), (300.0, 13.0)],
    [(13.0, 13.0), (300.0, 13.0), (100.0, 20.0), (13.0, 13.0), (300.0, 13.0), (13.0, 40.0), (13.0, 40.0), (13.0, 10.0), (100.0, 13.0), (13.0, 13.0)],
    [(400.0, 13.0), (300.0, 13.0), (300.0, 20.0), (400.0, 200.0), (300.0, 13.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 13.0), (300.0, 13.0)],
];
#[rustfmt::skip]
const FILE: [[(f32, f32); 10]; 6] = [
    [(347.0, 25.0), (300.0, 25.0), (300.0, 25.0), (347.0, 173.5), (320.0, 45.0), (347.0, 40.0), (347.0, 40.0), (347.0, 10.0), (347.0, 25.0), (300.0, 25.0)],
    [(347.0, 25.0), (300.0, 25.0), (300.0, 25.0), (347.0, 173.5), (320.0, 45.0), (347.0, 40.0), (347.0, 40.0), (347.0, 10.0), (347.0, 25.0), (300.0, 25.0)],
    [(400.0, 25.0), (300.0, 25.0), (300.0, 25.0), (400.0, 200.0), (320.0, 45.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 25.0), (300.0, 25.0)],
    [(400.0, 25.0), (300.0, 25.0), (300.0, 25.0), (400.0, 200.0), (320.0, 45.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 25.0), (300.0, 25.0)],
    [(347.0, 25.0), (300.0, 25.0), (300.0, 25.0), (347.0, 173.5), (320.0, 45.0), (347.0, 40.0), (347.0, 40.0), (347.0, 10.0), (347.0, 25.0), (300.0, 25.0)],
    [(400.0, 25.0), (300.0, 25.0), (300.0, 25.0), (400.0, 200.0), (320.0, 45.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 25.0), (300.0, 25.0)],
];
#[rustfmt::skip]
const RANGE: [[(f32, f32); 10]; 6] = [
    [(129.0, 16.0), (300.0, 16.0), (129.0, 20.0), (129.0, 64.5), (320.0, 36.0), (129.0, 40.0), (129.0, 40.0), (129.0, 10.0), (129.0, 16.0), (129.0, 16.0)],
    [(129.0, 16.0), (300.0, 16.0), (129.0, 20.0), (129.0, 64.5), (320.0, 36.0), (129.0, 40.0), (129.0, 40.0), (129.0, 10.0), (129.0, 16.0), (129.0, 16.0)],
    [(400.0, 16.0), (300.0, 16.0), (300.0, 20.0), (400.0, 200.0), (320.0, 36.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 16.0), (300.0, 16.0)],
    [(400.0, 16.0), (300.0, 16.0), (300.0, 20.0), (400.0, 200.0), (320.0, 36.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 16.0), (300.0, 16.0)],
    [(129.0, 16.0), (300.0, 16.0), (129.0, 20.0), (129.0, 64.5), (320.0, 36.0), (129.0, 40.0), (129.0, 40.0), (129.0, 10.0), (129.0, 16.0), (129.0, 16.0)],
    [(400.0, 16.0), (300.0, 16.0), (300.0, 20.0), (400.0, 200.0), (320.0, 36.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 16.0), (300.0, 16.0)],
];
#[rustfmt::skip]
const DATE: [[(f32, f32); 10]; 6] = [
    [(150.0, 21.0), (300.0, 21.0), (150.0, 21.0), (150.0, 75.0), (320.0, 41.0), (150.0, 40.0), (150.0, 40.0), (150.0, 10.0), (150.0, 21.0), (150.0, 21.0)],
    [(150.0, 21.0), (300.0, 21.0), (150.0, 21.0), (150.0, 75.0), (320.0, 41.0), (150.0, 40.0), (150.0, 40.0), (150.0, 10.0), (150.0, 21.0), (150.0, 21.0)],
    [(400.0, 21.0), (300.0, 21.0), (300.0, 21.0), (400.0, 200.0), (320.0, 41.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 21.0), (300.0, 21.0)],
    [(400.0, 21.0), (300.0, 21.0), (300.0, 21.0), (400.0, 200.0), (320.0, 41.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 21.0), (300.0, 21.0)],
    [(150.0, 21.0), (300.0, 21.0), (150.0, 21.0), (150.0, 75.0), (320.0, 41.0), (150.0, 40.0), (150.0, 40.0), (150.0, 10.0), (150.0, 21.0), (150.0, 21.0)],
    [(400.0, 21.0), (300.0, 21.0), (300.0, 21.0), (400.0, 200.0), (320.0, 41.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 21.0), (300.0, 21.0)],
];
#[rustfmt::skip]
const TIME: [[(f32, f32); 10]; 6] = [
    [(111.796875, 22.796875), (300.0, 22.796875), (111.796875, 22.796875), (111.796875, 55.890625), (320.0, 42.796875), (111.796875, 40.0), (111.796875, 40.0), (111.796875, 10.0), (111.796875, 22.796875), (111.796875, 22.796875)],
    [(111.796875, 22.796875), (300.0, 22.796875), (111.796875, 22.796875), (111.796875, 55.890625), (320.0, 42.796875), (111.796875, 40.0), (111.796875, 40.0), (111.796875, 10.0), (111.796875, 22.796875), (111.796875, 22.796875)],
    [(400.0, 22.796875), (300.0, 22.796875), (300.0, 22.796875), (400.0, 200.0), (320.0, 42.796875), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 22.796875), (300.0, 22.796875)],
    [(400.0, 22.796875), (300.0, 22.796875), (300.0, 22.796875), (400.0, 200.0), (320.0, 42.796875), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 22.796875), (300.0, 22.796875)],
    [(111.796875, 22.796875), (300.0, 22.796875), (111.796875, 22.796875), (111.796875, 55.890625), (320.0, 42.796875), (111.796875, 40.0), (111.796875, 40.0), (111.796875, 10.0), (111.796875, 22.796875), (111.796875, 22.796875)],
    [(400.0, 22.796875), (300.0, 22.796875), (300.0, 22.796875), (400.0, 200.0), (320.0, 42.796875), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 22.796875), (300.0, 22.796875)],
];
#[rustfmt::skip]
const DATETIME_LOCAL: [[(f32, f32); 10]; 6] = [
    [(240.0, 21.0), (300.0, 21.0), (240.0, 21.0), (240.0, 120.0), (320.0, 41.0), (240.0, 40.0), (240.0, 40.0), (240.0, 10.0), (240.0, 21.0), (240.0, 21.0)],
    [(240.0, 21.0), (300.0, 21.0), (240.0, 21.0), (240.0, 120.0), (320.0, 41.0), (240.0, 40.0), (240.0, 40.0), (240.0, 10.0), (240.0, 21.0), (240.0, 21.0)],
    [(400.0, 21.0), (300.0, 21.0), (300.0, 21.0), (400.0, 200.0), (320.0, 41.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 21.0), (300.0, 21.0)],
    [(400.0, 21.0), (300.0, 21.0), (300.0, 21.0), (400.0, 200.0), (320.0, 41.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 21.0), (300.0, 21.0)],
    [(240.0, 21.0), (300.0, 21.0), (240.0, 21.0), (240.0, 120.0), (320.0, 41.0), (240.0, 40.0), (240.0, 40.0), (240.0, 10.0), (240.0, 21.0), (240.0, 21.0)],
    [(400.0, 21.0), (300.0, 21.0), (300.0, 21.0), (400.0, 200.0), (320.0, 41.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 21.0), (300.0, 21.0)],
];
#[rustfmt::skip]
const SELECT: [[(f32, f32); 10]; 6] = [
    [(102.0, 19.0), (300.0, 19.0), (102.0, 20.0), (102.0, 51.0), (300.0, 39.0), (102.0, 40.0), (102.0, 40.0), (102.0, 10.0), (102.0, 19.0), (102.0, 19.0)],
    [(102.0, 19.0), (300.0, 19.0), (102.0, 20.0), (102.0, 51.0), (300.0, 39.0), (102.0, 40.0), (102.0, 40.0), (102.0, 10.0), (102.0, 19.0), (102.0, 19.0)],
    [(400.0, 19.0), (300.0, 19.0), (300.0, 20.0), (400.0, 200.0), (300.0, 39.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 19.0), (300.0, 19.0)],
    [(400.0, 19.0), (300.0, 19.0), (300.0, 20.0), (400.0, 200.0), (300.0, 39.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 19.0), (300.0, 19.0)],
    [(102.0, 19.0), (300.0, 19.0), (102.0, 20.0), (102.0, 51.0), (300.0, 39.0), (102.0, 40.0), (102.0, 40.0), (102.0, 10.0), (102.0, 19.0), (102.0, 19.0)],
    [(400.0, 19.0), (300.0, 19.0), (300.0, 20.0), (400.0, 200.0), (300.0, 39.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 19.0), (300.0, 19.0)],
];
#[rustfmt::skip]
const TEXT: [[(f32, f32); 10]; 6] = [
    [(200.0, 19.0), (300.0, 19.0), (200.0, 20.0), (200.0, 100.0), (320.0, 39.0), (200.0, 40.0), (200.0, 40.0), (200.0, 10.0), (200.0, 19.0), (200.0, 19.0)],
    [(200.0, 19.0), (300.0, 19.0), (200.0, 20.0), (200.0, 100.0), (320.0, 39.0), (200.0, 40.0), (200.0, 40.0), (200.0, 10.0), (200.0, 19.0), (200.0, 19.0)],
    [(400.0, 19.0), (300.0, 19.0), (300.0, 20.0), (400.0, 200.0), (320.0, 39.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 19.0), (300.0, 19.0)],
    [(400.0, 19.0), (300.0, 19.0), (300.0, 20.0), (400.0, 200.0), (320.0, 39.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 19.0), (300.0, 19.0)],
    [(200.0, 19.0), (300.0, 19.0), (200.0, 20.0), (200.0, 100.0), (320.0, 39.0), (200.0, 40.0), (200.0, 40.0), (200.0, 10.0), (200.0, 19.0), (200.0, 19.0)],
    [(400.0, 19.0), (300.0, 19.0), (300.0, 20.0), (400.0, 200.0), (320.0, 39.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 19.0), (300.0, 19.0)],
];
#[rustfmt::skip]
const TEXTAREA: [[(f32, f32); 10]; 6] = [
    [(178.0, 38.0), (300.0, 38.0), (178.0, 38.0), (178.0, 89.0), (320.0, 58.0), (178.0, 40.0), (178.0, 40.0), (178.0, 10.0), (178.0, 38.0), (178.0, 38.0)],
    [(178.0, 38.0), (300.0, 38.0), (178.0, 38.0), (178.0, 89.0), (320.0, 58.0), (178.0, 40.0), (178.0, 40.0), (178.0, 10.0), (178.0, 38.0), (178.0, 38.0)],
    [(400.0, 38.0), (300.0, 38.0), (300.0, 38.0), (400.0, 200.0), (320.0, 58.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 38.0), (300.0, 38.0)],
    [(400.0, 38.0), (300.0, 38.0), (300.0, 38.0), (400.0, 200.0), (320.0, 58.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 38.0), (300.0, 38.0)],
    [(178.0, 38.0), (300.0, 38.0), (178.0, 38.0), (178.0, 89.0), (320.0, 58.0), (178.0, 40.0), (178.0, 40.0), (178.0, 10.0), (178.0, 38.0), (178.0, 38.0)],
    [(400.0, 38.0), (300.0, 38.0), (300.0, 38.0), (400.0, 200.0), (320.0, 58.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 38.0), (300.0, 38.0)],
];
#[rustfmt::skip]
const BUTTON: [[(f32, f32); 10]; 6] = [
    [(40.0, 18.0), (300.0, 18.0), (100.0, 20.0), (40.0, 20.0), (320.0, 38.0), (40.0, 40.0), (40.0, 40.0), (40.0, 10.0), (100.0, 18.0), (40.0, 18.0)],
    [(40.0, 18.0), (300.0, 18.0), (100.0, 20.0), (40.0, 20.0), (320.0, 38.0), (40.0, 40.0), (40.0, 40.0), (40.0, 10.0), (100.0, 18.0), (40.0, 18.0)],
    [(400.0, 18.0), (300.0, 18.0), (300.0, 20.0), (400.0, 200.0), (320.0, 38.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 18.0), (300.0, 18.0)],
    [(400.0, 18.0), (300.0, 18.0), (300.0, 20.0), (400.0, 200.0), (320.0, 38.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 18.0), (300.0, 18.0)],
    [(40.0, 18.0), (300.0, 18.0), (100.0, 20.0), (40.0, 20.0), (320.0, 38.0), (40.0, 40.0), (40.0, 40.0), (40.0, 10.0), (100.0, 18.0), (40.0, 18.0)],
    [(400.0, 18.0), (300.0, 18.0), (300.0, 20.0), (400.0, 200.0), (320.0, 38.0), (400.0, 40.0), (400.0, 40.0), (400.0, 10.0), (400.0, 18.0), (300.0, 18.0)],
];
