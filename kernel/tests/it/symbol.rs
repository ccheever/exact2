//! A system symbol's box is measured in the layout that places it, as a
//! paragraph is: the first frame has it, and a new role or font measures it
//! again; a host that cannot say keeps the size its view reports.

use exact_kernel::{
    AlignItems, Display, FlexDirection, Kernel, MonospaceMeasurer, NodeType, Offer, Op, PropId,
    PropValue, StyleId, StyleProps, SymbolRequest, TextMeasureRequest, TextMeasurer, TextMetrics,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Each symbol request: role, font size, and weight
type Asked = Rc<RefCell<Vec<(String, f32, u16)>>>;

/// The reference measurer, and symbols drawn 1.2 em wide and 1 em tall
/// (`sf/wide` 2 em), with every request it was asked.
struct Symbols {
    text: MonospaceMeasurer,
    asked: Asked,
    /// The measuring traits' revision; each one draws symbols a point wider
    revision: Rc<Cell<u64>>,
}

impl TextMeasurer for Symbols {
    fn measure_revision(&self) -> u64 {
        self.revision.get()
    }
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        self.text.measure(request)
    }
    fn symbol_size(&mut self, r: &SymbolRequest<'_>) -> Option<(f32, f32)> {
        self.asked
            .borrow_mut()
            .push((r.role.to_string(), r.font_size, r.font_weight));
        let em = r.font_size * if r.font_weight >= 700 { 1.1 } else { 1.0 };
        let wider = self.revision.get() as f32;
        match r.role {
            "sf/wide" => Some((2.0 * em + wider, em)),
            "sf/none" => None,
            "sf/regular" if r.font_weight >= 700 => None,
            "sf/even" if self.revision.get() % 2 == 1 => None,
            _ => Some((1.2 * em + wider, em)),
        }
    }
}

fn font(size: f32) -> StyleProps {
    let mut s = StyleProps::default();
    s.font_size = size;
    s.mask.set(StyleId::FontSize);
    s
}

/// A flex column (font `parent_size`) holding one image with source `source`.
fn tree(measurer: Box<dyn TextMeasurer>, source: &str, parent_size: f32) -> Kernel {
    let mut kernel = Kernel::new(measurer);
    let mut root = font(parent_size);
    root.display = Display::Flex;
    root.mask.set(StyleId::Display);
    root.flex_direction = FlexDirection::Column;
    root.mask.set(StyleId::FlexDirection);
    root.align_items = AlignItems::FlexStart;
    root.mask.set(StyleId::AlignItems);
    let ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(root),
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::Image,
        },
        Op::SetProp {
            id: 2,
            prop: PropId::ImageSource,
            value: PropValue::Str(source.into()),
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ];
    kernel.apply(0, 1, &ops).unwrap();
    kernel
}

fn size(kernel: &mut Kernel) -> (f32, f32) {
    kernel
        .compute_layout(1, Offer::definite(390.0, 844.0))
        .unwrap();
    let f = kernel.node(2).unwrap().frame;
    (f.width, f.height)
}

fn symbols() -> (Box<dyn TextMeasurer>, Asked) {
    let asked = Rc::new(RefCell::new(Vec::new()));
    (
        Box::new(Symbols {
            text: MonospaceMeasurer::default(),
            asked: asked.clone(),
            revision: Rc::default(),
        }),
        asked,
    )
}

#[test]
fn the_first_layout_has_the_symbols_box() {
    let (measurer, asked) = symbols();
    let mut kernel = tree(measurer, "symbol:sf/gear", 20.0);
    assert_eq!(
        size(&mut kernel),
        (24.0, 20.0),
        "measured before any view reports"
    );
    assert_eq!(
        asked.borrow().last().cloned(),
        Some(("sf/gear".to_string(), 20.0, 400))
    );
    // A portable role is asked by its role: the host knows its own name for it
    let mut kernel = tree(symbols().0, "symbol:settings", 10.0);
    assert_eq!(size(&mut kernel), (12.0, 10.0));
}

#[test]
fn a_new_role_or_font_measures_the_symbol_again() {
    let (measurer, _) = symbols();
    let mut kernel = tree(measurer, "symbol:sf/gear", 20.0);
    assert_eq!(size(&mut kernel), (24.0, 20.0));
    kernel
        .apply(
            1,
            2,
            &[Op::SetProp {
                id: 2,
                prop: PropId::ImageSource,
                value: PropValue::Str("symbol:sf/wide".into()),
            }],
        )
        .unwrap();
    assert_eq!(size(&mut kernel), (40.0, 20.0), "its role");
    kernel
        .apply(
            2,
            3,
            &[Op::SetStyle {
                id: 1,
                patch: Box::new(font(30.0)),
            }],
        )
        .unwrap();
    assert_eq!(size(&mut kernel), (60.0, 30.0), "an inherited font size");
    let mut bold = font(10.0);
    bold.font_weight = 700;
    bold.mask.set(StyleId::FontWeight);
    kernel
        .apply(
            3,
            4,
            &[Op::SetStyle {
                id: 2,
                patch: Box::new(bold),
            }],
        )
        .unwrap();
    let (w, h) = size(&mut kernel);
    assert!(
        (w - 22.0).abs() < 1e-4 && (h - 11.0).abs() < 1e-4,
        "its own font: {w}×{h}"
    );
}

#[test]
fn a_host_that_cannot_measure_keeps_the_reported_size() {
    let (measurer, _) = symbols();
    let mut kernel = tree(measurer, "symbol:sf/none", 20.0);
    assert_eq!(
        size(&mut kernel),
        (0.0, 0.0),
        "nothing until the view reports"
    );
    kernel.set_intrinsic_size(2, Some((18.0, 17.0))).unwrap();
    assert_eq!(size(&mut kernel), (18.0, 17.0));
    // The reference measurer measures no symbol
    let mut kernel = tree(
        Box::new(MonospaceMeasurer::default()),
        "symbol:sf/gear",
        20.0,
    );
    assert_eq!(size(&mut kernel), (0.0, 0.0));
    kernel.set_intrinsic_size(2, Some((24.0, 20.0))).unwrap();
    assert_eq!(size(&mut kernel), (24.0, 20.0));
}

#[test]
fn a_measured_symbol_is_its_own_size_whatever_a_host_reported() {
    let (measurer, _) = symbols();
    let mut kernel = tree(measurer, "symbol:sf/gear", 20.0);
    kernel.set_intrinsic_size(2, Some((99.0, 99.0))).unwrap();
    assert_eq!(size(&mut kernel), (24.0, 20.0));
    // A picture is its host's to size: the last box until it reports
    kernel
        .apply(
            1,
            2,
            &[Op::SetProp {
                id: 2,
                prop: PropId::ImageSource,
                value: PropValue::Str("https://example.com/a.png".into()),
            }],
        )
        .unwrap();
    assert_eq!(size(&mut kernel), (24.0, 20.0));
    kernel.set_intrinsic_size(2, Some((64.0, 48.0))).unwrap();
    assert_eq!(size(&mut kernel), (64.0, 48.0));
}

#[test]
fn a_measured_symbol_has_its_ratio_when_one_side_is_authored() {
    let (measurer, _) = symbols();
    let mut kernel = tree(measurer, "symbol:sf/wide", 20.0);
    let mut wide = StyleProps::default();
    wide.width = exact_kernel::Dimension::Points(80.0);
    wide.mask.set(StyleId::Width);
    kernel
        .apply(
            1,
            2,
            &[Op::SetStyle {
                id: 2,
                patch: Box::new(wide),
            }],
        )
        .unwrap();
    assert_eq!(
        size(&mut kernel),
        (80.0, 40.0),
        "its 2:1 ratio from the first layout"
    );
}

/// A host's report after the kernel measured a symbol is not its size; a
/// picture's, reported as soon as its source changes and before any
/// layout, is.
#[test]
fn a_report_is_a_pictures_size_from_the_moment_its_source_changes() {
    let (measurer, _) = symbols();
    let mut kernel = tree(measurer, "symbol:sf/gear", 20.0);
    assert_eq!(size(&mut kernel), (24.0, 20.0));
    kernel.set_intrinsic_size(2, Some((99.0, 99.0))).unwrap();
    assert_eq!(
        size(&mut kernel),
        (24.0, 20.0),
        "the view's report for a measured symbol"
    );
    kernel
        .apply(
            1,
            2,
            &[Op::SetProp {
                id: 2,
                prop: PropId::ImageSource,
                value: PropValue::Str("https://example.com/a.png".into()),
            }],
        )
        .unwrap();
    kernel.set_intrinsic_size(2, Some((64.0, 48.0))).unwrap();
    assert_eq!(
        size(&mut kernel),
        (64.0, 48.0),
        "a cached picture's report before the next layout"
    );
}

/// A new measuring revision (a content size, a scale) measures the symbols
/// again, though neither their role nor their font changed.
#[test]
fn a_new_measuring_revision_measures_the_symbols_again() {
    let revision = Rc::new(Cell::new(0));
    let measurer = Symbols {
        text: MonospaceMeasurer::default(),
        asked: Rc::default(),
        revision: revision.clone(),
    };
    let mut kernel = tree(Box::new(measurer), "symbol:sf/gear", 20.0);
    assert_eq!(size(&mut kernel), (24.0, 20.0));
    revision.set(2);
    assert_eq!(size(&mut kernel), (26.0, 20.0));
}

/// A height probe, which runs before the layout it sizes, has the symbol's
/// box too.
#[test]
fn a_height_probe_before_any_layout_has_the_symbols_box() {
    let (measurer, _) = symbols();
    let mut kernel = tree(measurer, "symbol:sf/gear", 20.0);
    let mut section = StyleProps::default();
    section.box_sizing = exact_kernel::BoxSizing::BorderBox;
    section.mask.set(StyleId::BoxSizing);
    kernel
        .apply(
            1,
            2,
            &[
                Op::CreateView {
                    id: 3,
                    node_type: NodeType::View,
                },
                Op::SetStyle {
                    id: 3,
                    patch: Box::new(section),
                },
                Op::SetChildren {
                    id: 3,
                    children: vec![2],
                },
                Op::SetChildren {
                    id: 1,
                    children: vec![3],
                },
            ],
        )
        .unwrap();
    let owner = kernel.node(3).unwrap().key;
    let target = kernel
        .measure_height_targets(1, Offer::definite(390.0, 844.0), &[owner])
        .unwrap();
    assert_eq!(target[0].px, 20.0);
}

/// A measuring revision asks about every symbol again: one the measurer
/// declined is measured once it can be, and one it declines in turn has its
/// host's report until it is measured again.
#[test]
fn a_new_measuring_revision_asks_about_a_declined_symbol_again() {
    let revision = Rc::new(Cell::new(1));
    let measurer = Symbols {
        text: MonospaceMeasurer::default(),
        asked: Rc::default(),
        revision: revision.clone(),
    };
    let mut kernel = tree(Box::new(measurer), "symbol:sf/even", 20.0);
    assert_eq!(size(&mut kernel), (0.0, 0.0), "declined at revision 1");
    revision.set(2);
    assert_eq!(size(&mut kernel), (26.0, 20.0));
    revision.set(3);
    assert_eq!(size(&mut kernel), (26.0, 20.0), "its last box, declined");
    kernel.set_intrinsic_size(2, Some((30.0, 30.0))).unwrap();
    assert_eq!(size(&mut kernel), (30.0, 30.0), "its host's report");
    revision.set(4);
    assert_eq!(size(&mut kernel), (28.0, 20.0));
}

/// A new font releases the size measured in the old one: a report made
/// before the next layout is the symbol's size if the measurer cannot size
/// the new font.
#[test]
fn a_symbol_the_measurer_cannot_size_in_its_new_font_has_its_hosts_report() {
    let (measurer, _) = symbols();
    let mut kernel = tree(measurer, "symbol:sf/regular", 20.0);
    assert_eq!(size(&mut kernel), (24.0, 20.0));
    let mut bold = StyleProps::default();
    bold.font_weight = 700;
    bold.mask.set(StyleId::FontWeight);
    kernel
        .apply(
            1,
            2,
            &[Op::SetStyle {
                id: 2,
                patch: Box::new(bold),
            }],
        )
        .unwrap();
    kernel.set_intrinsic_size(2, Some((27.0, 22.0))).unwrap();
    assert_eq!(size(&mut kernel), (27.0, 22.0));
}

/// A rehydrated kernel's measurer measures the symbols itself: its sizes
/// are its own, and the reference measurer, which sizes none, leaves them to
/// the host's reports.
#[test]
fn a_rehydrated_kernel_measures_its_symbols_with_its_own_measurer() {
    let (measurer, _) = symbols();
    let mut kernel = tree(measurer, "symbol:sf/gear", 20.0);
    assert_eq!(size(&mut kernel), (24.0, 20.0));
    let wider = Symbols {
        text: MonospaceMeasurer::default(),
        asked: Rc::default(),
        revision: Rc::new(Cell::new(0)),
    };
    let mut wider = kernel.rehydrate(Box::new(Wider(wider)));
    assert_eq!(size(&mut wider), (34.0, 20.0));
    let mut reference = kernel.rehydrate(Box::new(MonospaceMeasurer::default()));
    assert_eq!(size(&mut reference), (24.0, 20.0), "the size it was given");
    reference.set_intrinsic_size(2, Some((18.0, 17.0))).unwrap();
    assert_eq!(size(&mut reference), (18.0, 17.0), "its host's report");
}

/// A measurer, at the same revision, that draws symbols 10 points wider
struct Wider(Symbols);

impl TextMeasurer for Wider {
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        self.0.measure(request)
    }
    fn symbol_size(&mut self, r: &SymbolRequest<'_>) -> Option<(f32, f32)> {
        self.0.symbol_size(r).map(|(w, h)| (w + 10.0, h))
    }
}

/// A fit-content trial, which lays out apart from the ordinary layout,
/// has the box of a symbol whose font changed since the last layout.
#[test]
fn a_fit_content_trial_measures_a_symbol_whose_font_changed() {
    let (measurer, _) = symbols();
    let mut kernel = tree(measurer, "symbol:sf/gear", 20.0);
    assert_eq!(size(&mut kernel), (24.0, 20.0));
    kernel
        .apply(
            1,
            2,
            &[Op::SetStyle {
                id: 1,
                patch: Box::new(font(40.0)),
            }],
        )
        .unwrap();
    let image = kernel.node(2).unwrap().key;
    assert_eq!(kernel.fit_content_height(image), Some(40.0));
}

/// A symbol's measured size is the kernel's only under the traits it was
/// measured under: once they change, and in a kernel rehydrated with another
/// measurer, a report made before the next layout is its size until the
/// measurer sizes it again.
#[test]
fn a_report_after_the_measuring_traits_change_is_the_size_until_measured_again() {
    let revision = Rc::new(Cell::new(0));
    let measurer = Symbols {
        text: MonospaceMeasurer::default(),
        asked: Rc::default(),
        revision: revision.clone(),
    };
    let mut kernel = tree(Box::new(measurer), "symbol:sf/even", 20.0);
    assert_eq!(size(&mut kernel), (24.0, 20.0));
    revision.set(1);
    kernel.set_intrinsic_size(2, Some((30.0, 30.0))).unwrap();
    assert_eq!(size(&mut kernel), (30.0, 30.0), "declined at revision 1");
    revision.set(2);
    let mut reference = kernel.rehydrate(Box::new(MonospaceMeasurer::default()));
    reference.set_intrinsic_size(2, Some((18.0, 17.0))).unwrap();
    assert_eq!(size(&mut reference), (18.0, 17.0));
}

/// A report is checked before it is set aside: a nonfinite one for a
/// measured symbol is refused, as for any image.
#[test]
fn a_nonfinite_report_for_a_measured_symbol_is_refused() {
    let (measurer, _) = symbols();
    let mut kernel = tree(measurer, "symbol:sf/gear", 20.0);
    assert_eq!(size(&mut kernel), (24.0, 20.0));
    assert!(kernel
        .set_intrinsic_size(2, Some((f32::NAN, 20.0)))
        .is_err());
}
