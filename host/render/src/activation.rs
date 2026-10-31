//! A page's activation (LLP 1071 D6): the route's, or for an undeclared
//! route the one its render implies (LLP 1048.007 D1).

use exact_plan::Plan;
use exact_web::document::route_at;
use std::sync::Mutex;

/// The page's activation (LLP 1071 D6) and, for one that waits for the
/// reader, its viewport comparisons ([`Rendered::wake`]): the route's,
/// unless a partial document or a continuous handler needs the host's
/// ordinary boot. An undeclared route waits when its render left the
/// runtime nothing to do before the reader acts (LLP 1048.007 D1, as
/// `render.mjs` `infer` decides it): nothing in the plan only the runtime
/// keeps (`exact_runner::activation`), no answer pending, nothing due on
/// the clock, no handler the capture script cannot replay.
pub(crate) fn activation(
    plan: &Plan,
    location: &str,
    state: &exact_runner::Checkpoint,
    handlers: &exact_kernel::SortedMap<exact_kernel::ViewId, Vec<exact_plan::EventKind>>,
    clocked: bool,
    viewport: &exact_runner::Viewport,
) -> (exact_plan::ActivatePolicy, String) {
    use exact_runner::activation::REPLAYED;
    let mut activation = route_at(plan, location)
        .map_or(exact_plan::ActivatePolicy::Inferred, |route| route.activate);
    if activation == exact_plan::ActivatePolicy::Inferred {
        let inference = inference(plan);
        let waits = !inference.never_waits()
            && state.pending.is_empty()
            && !clocked
            && handlers
                .values()
                .flatten()
                .all(|kind| REPLAYED.contains(kind));
        if let Some(media) = inference.media.as_deref().filter(|_| waits) {
            let wake = if media.is_empty() {
                String::new()
            } else {
                let rows: Vec<String> = media
                    .iter()
                    .map(|m| {
                        format!(
                            "[{},{}]",
                            serde_json::to_string(&m.query).unwrap_or_default(),
                            m.holds(viewport)
                        )
                    })
                    .collect();
                format!("[{}]", rows.join(","))
            };
            return (exact_plan::ActivatePolicy::Interaction, wake);
        }
        return (activation, String::new());
    }
    // A partial document needs to finish without waiting for an action. Gesture,
    // media and other continuous handlers likewise need the host's ordinary
    // boot (undeclared: idle on the wasm host, eager on the JavaScript one);
    // interaction activation only replays discrete form and press semantics.
    if activation == exact_plan::ActivatePolicy::Interaction
        && (!state.pending.is_empty()
            || handlers.values().flatten().any(|kind| {
                !matches!(
                    kind,
                    exact_plan::EventKind::Press
                        | exact_plan::EventKind::Change
                        | exact_plan::EventKind::Focus
                        | exact_plan::EventKind::Blur
                        | exact_plan::EventKind::Key
                        | exact_plan::EventKind::Submit
                        | exact_plan::EventKind::Navigate
                )
            }))
    {
        activation = exact_plan::ActivatePolicy::Inferred;
    }
    (activation, String::new())
}

/// What an inferred activation reads of a plan ([`activation`]), found once
/// per plan a process renders.
pub(crate) struct Inference {
    /// The plan's viewport comparisons as media queries; `None` when some
    /// reading has none.
    media: Option<Vec<exact_runner::viewport_reads::Media>>,
    /// The plan holds something only its runtime keeps.
    kept: bool,
}

impl Inference {
    /// Whether no undeclared route's page of the plan can wait: then a
    /// server names the entry's preloads in the head it sends before the
    /// render ([`page::preloads_early`]).
    pub(crate) fn never_waits(&self) -> bool {
        self.kept || self.media.is_none()
    }
}

pub(crate) fn inference(plan: &Plan) -> std::sync::Arc<Inference> {
    static KNOWN: Mutex<Vec<(Plan, std::sync::Arc<Inference>)>> = Mutex::new(Vec::new());
    let mut known = KNOWN
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((_, found)) = known.iter().find(|(p, _)| p == plan) {
        return std::sync::Arc::clone(found);
    }
    let found = std::sync::Arc::new(Inference {
        media: exact_runner::activation::wake(plan),
        kept: exact_runner::activation::runtime_kept(plan),
    });
    if known.len() == 4 {
        known.remove(0);
    }
    known.push((plan.clone(), std::sync::Arc::clone(&found)));
    found
}
