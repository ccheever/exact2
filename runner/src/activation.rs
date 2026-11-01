//! What decides whether an undeclared route's page waits for the reader
//! before its runtime runs (LLP 1048.007 D1). Both renderers ask the same
//! questions: the render host here, the JavaScript render server through
//! what the JS build writes from these (`names.js` `activation`).

use crate::viewport_reads::{Media, Reads};
use exact_kernel::{NodeType, PropId};
use exact_plan::{BindingKind, EventKind, Plan};

/// The handlers the capture script replays once the runtime has adopted
/// the page (`host/web-js/capture.js`): a press, a change, typing, and a
/// link, which navigates natively. A page with any other handler runs its
/// runtime at once.
pub const REPLAYED: [EventKind; 4] = [
    EventKind::Press,
    EventKind::Change,
    EventKind::Input,
    EventKind::Navigate,
];

/// The handlers whose first report waits for the reader's view: an
/// `intersect` handler reports its element's visibility, which only matters
/// once the element shows or the reader scrolls, so the capture script
/// wakes the runtime then, and the runtime reports it as it is (LLP
/// 1048.001 D4's `visible`).
pub const WOKEN: [EventKind; 1] = [EventKind::Intersect];

/// The reserved sources a plan may read and its pages still wait: the
/// viewport, whose comparisons every page carries for the capture script
/// to check ([`wake`]), and the build's own facts. Every other one is the
/// reader's device (the time and zone, the page's state), which only the
/// runtime reads; a render's checkpoint carries none of them.
pub const WAITS: [&str; 2] = [crate::viewport::SOURCE, "exactDelivery"];

/// The viewport comparisons a waiting page carries ([`Media`]): none for a
/// plan that reads no viewport, `None` when some reading has no media query
/// (then no page waits).
pub fn wake(plan: &Plan) -> Option<Vec<Media>> {
    Reads::of(plan).media()
}

/// Whether the plan holds something only its runtime keeps: then no page
/// waits. A reserved source of the reader's device (not one of [`WAITS`]);
/// a virtualized list, whose rows past the first the runtime makes as the
/// reader scrolls; a canvas, a native view or a web view, which the runtime
/// draws; page code (`hatch`); an app's frame (`src`); an instant
/// (`datetime`), shown in the reader's zone. Asked of the plan, not the
/// page: a page sent as it is written decides before its document.
pub fn runtime_kept(plan: &Plan) -> bool {
    let device = plan.resources.iter().any(|row| {
        let source = plan.str(row.source);
        source.starts_with("exact") && !WAITS.contains(&source)
    });
    device || drawn(plan)
}

fn drawn(plan: &Plan) -> bool {
    const KEPT: [PropId; 4] = [
        PropId::Virtualized,
        PropId::Hatch,
        PropId::Src,
        PropId::Datetime,
    ];
    plan.nodes.iter().any(|row| {
        matches!(
            NodeType::from_wire(row.node_type),
            Some(NodeType::Canvas | NodeType::NativeView | NodeType::WebView)
        ) || row
            .bindings
            .iter()
            .map(|b| plan.binding(b))
            .any(|b| b.kind == BindingKind::Prop && KEPT.iter().any(|p| b.id == *p as u16))
    })
}
