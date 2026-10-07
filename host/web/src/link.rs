//! What this artifact links beyond the core.
//!
//! @ref LLP 1047 D3 (the generated entry links what the plan uses)
//! @ref LLP 1047 D6 (using an unlinked capability is a named refusal)
//!
//! The core host never names a capability's crate. It reaches one only
//! through [`Linked`], which the app's generated entry fills with the
//! capabilities its plan uses (`exact-web-capabilities` holds them), and
//! registers with [`link`] before every boot. What no entry names, the linker
//! drops. A plan that uses more than the artifact links is refused at boot,
//! by name, before anything runs.

use crate::HostError;
use exact_runner::uses::{Capability, Uses};
use std::cell::Cell;

/// The capabilities an artifact links: one registration per capability.
#[derive(Clone, Copy)]
pub struct Linked {
    /// Markdown: a source as the batch's `markupPieces` JSON.
    pub markup: Option<fn(&str) -> String>,
    /// Motion: the spring engine a host holds instead of
    /// [`crate::motion::Still`].
    pub motion: Option<fn() -> Box<dyn crate::motion::Motion>>,
    /// Lists the host windows: the runner's engines for them, and their
    /// exports (`list_exports!`) are in (LLP 1047.000 §9).
    pub collections: Option<&'static exact_runner::ListLinks>,
    /// Drags: the host's hooks for them, which the entry passes as
    /// [`crate::HostLinks::of`] this set.
    pub drag: bool,
    /// GPU canvas surfaces: the runner's answer for a surface's record, and
    /// their exports (`surface_exports!`) are in.
    pub surface_answer: exact_runner::SurfaceAnswer,
    /// Canvas 2D (LLP 1056), linked with surfaces: a canvas with a surface
    /// is a 2D one when its source draws it.
    pub canvas: exact_runner::CanvasLink,
    /// The router (LLP 1038): the runner's routing for a plan with routes.
    pub router: exact_runner::RouterLink,
    /// `formatDate` and `formatNumber` (LLP 1054.000.003 D8): the runner's
    /// bodies for a plan that calls one.
    pub format: exact_runner::FormatLink,
    /// Inspection (LLP 1012): the agent API's reads. Not a plan's use: the
    /// entry links it by policy, in production too (LLP 1047 §10, Q3).
    pub inspection: bool,
    /// The wide colour forms (LLP 1056 §8.2), linked when the plan or the
    /// data crate's source names one: registered at [`link`].
    pub wide_colors: Option<fn()>,
    /// `backgroundMaterial` (LLP 1053.000 D4): a material's CSS variables
    /// appended to a node's style, and the line to log, once, for a name
    /// the table lacks.
    pub materials: Option<Materials>,
    /// `backdrop-filter`'s grammar (LLP 1053.000 D1): linked at [`link`].
    pub backdrop: Option<fn()>,
    /// `openAuthSession` (LLP 1069.006): linked when the app grants
    /// `auth.session`; [`crate::HostLinks::of`] reads it.
    pub auth: bool,
    /// `share(…)` (LLP 1069.003).
    pub share: bool,
    /// `showNotification` and `closeNotification`.
    pub notifications: bool,
    /// `saveFile` and the file pickers (LLP 1069.010).
    pub documents: bool,
    /// `input type="file"` and `showPicker` (LLP 1069.002): a file input's
    /// `change` payload, read.
    pub picker: Option<PickedPayload>,
    /// Drag timelines' grammar (LLP 1057.003): linked at [`link`].
    pub timelines: Option<fn()>,
    /// `text-transform`'s case mapping (LLP 1064 D5): linked at [`link`].
    pub text_transform: Option<fn()>,
    /// `filter`'s and `clip-path`'s grammars (LLP 1055.000 D10, D14):
    /// linked at [`link`].
    pub effects: Option<fn()>,
    /// CSS animations (LLP 1055 D5): their grammars, linked at [`link`],
    /// and the page's CSS for them.
    pub animations: Option<AnimationsLink>,
    /// `background-image`'s gradient grammar (LLP 1066): linked at [`link`].
    pub gradients: Option<fn()>,
    /// CSS grid's grammar, validation and serialization: linked at [`link`]
    /// when any grid row is bound.
    pub grid: Option<fn()>,
    /// `env(viewport-segment-*)`'s grammar (LLP 1078 D3): linked at [`link`].
    pub segments: Option<fn()>,
    /// `frame` and `measure` (LLP 1051.000 D4): the page's answers, through
    /// one import `geometry-glue.js` answers.
    pub geometry: exact_runner::GeometryLink,
    /// `data-*` words (LLP 1075.003 §3.3): a `dataset` row's pairs, each
    /// written as its own attribute.
    pub dataset: Option<DatasetPairs>,
    /// A navigation root's tabs (LLP 1075.003 §3.7): its routes as each
    /// tab's stack shows them.
    pub tabs: Option<crate::document::TabRoutes>,
}

/// A `dataset` row's `(word, value)` pairs: [`crate::document::dataset`].
pub type DatasetPairs = fn(&str) -> Vec<(String, String)>;

/// `backgroundMaterial`'s pair: a material's CSS variables appended to a
/// style, and the line to log, once, for a name the table lacks.
pub type Materials = (fn(&mut String, &str), fn(&str) -> Option<String>);

/// CSS animations on the page (LLP 1055 D5, D7), which only an artifact that
/// animates carries: the grammars' link, a node's `animation` list as CSS
/// and each entry's `@keyframes` rule, by name and body. `bool` is whether the
/// node's press feedback composes through `--exact-scale` (LLP 1061).
#[derive(Clone, Copy)]
pub struct AnimationsLink {
    /// Link the grammars.
    pub grammars: fn(),
    /// A node's `animation` (or `exit-animation`) list as CSS.
    pub list: fn(&exact_motion::animation::Animations, bool) -> String,
    /// The name of the rule an entry plays.
    pub name: fn(&exact_motion::animation::Animation, bool) -> String,
    /// That rule's body.
    pub body: fn(&exact_motion::animation::Animation, bool) -> String,
}

/// A file input's `change` payload, read: [`exact_runner::Picked::payload`].
pub type PickedPayload = fn(&str) -> Option<Vec<exact_runner::Picked>>;

impl Linked {
    /// The runner's device capabilities this names (LLP 1047 D3): each
    /// linked one's entries, [`exact_runner::DeviceLinks::CORE`]'s none for
    /// the rest.
    pub const fn device_links<D: exact_runner::DataSource>(self) -> exact_runner::DeviceLinks<D> {
        let all = exact_runner::DeviceLinks::<D>::ALL;
        exact_runner::DeviceLinks {
            auth: if self.auth { all.auth } else { None },
            share: if self.share { all.share } else { None },
            documents: if self.documents { all.documents } else { None },
            picker: if self.picker.is_some() {
                all.picker
            } else {
                None
            },
            notifications: if self.notifications {
                all.notifications
            } else {
                None
            },
        }
    }

    /// The core alone.
    pub const CORE: Linked = Linked {
        markup: None,
        motion: None,
        collections: None,
        drag: false,
        surface_answer: None,
        canvas: None,
        router: None,
        format: None,
        inspection: false,
        wide_colors: None,
        materials: None,
        backdrop: None,
        auth: false,
        share: false,
        notifications: false,
        documents: false,
        picker: None,
        timelines: None,
        text_transform: None,
        effects: None,
        animations: None,
        gradients: None,
        grid: None,
        geometry: None,
        segments: None,
        dataset: None,
        tabs: None,
    };

    /// The capabilities registered here.
    pub fn uses(&self) -> Uses {
        let mut uses = Uses::NONE;
        if self.markup.is_some() {
            uses = uses.with(Capability::Markdown);
        }
        if self.motion.is_some() {
            uses = uses.with(Capability::Motion);
        }
        if self.collections.is_some() {
            uses = uses.with(Capability::Collections);
        }
        if self.drag {
            uses = uses.with(Capability::Drag);
        }
        if self.surface_answer.is_some() {
            uses = uses.with(Capability::Surfaces);
        }
        if self.router.is_some() {
            uses = uses.with(Capability::Router);
        }
        if self.format.is_some() {
            uses = uses.with(Capability::Format);
        }
        if self.materials.is_some() {
            uses = uses.with(Capability::Materials);
        }
        if self.backdrop.is_some() {
            uses = uses.with(Capability::Backdrop);
        }
        if self.share {
            uses = uses.with(Capability::Share);
        }
        if self.notifications {
            uses = uses.with(Capability::Notifications);
        }
        if self.documents {
            uses = uses.with(Capability::Documents);
        }
        if self.picker.is_some() {
            uses = uses.with(Capability::Picker);
        }
        if self.timelines.is_some() {
            uses = uses.with(Capability::Timelines);
        }
        if self.text_transform.is_some() {
            uses = uses.with(Capability::TextTransform);
        }
        if self.effects.is_some() {
            uses = uses.with(Capability::Effects);
        }
        if self.animations.is_some() {
            uses = uses.with(Capability::Animations);
        }
        if self.gradients.is_some() {
            uses = uses.with(Capability::Gradients);
        }
        if self.grid.is_some() {
            uses = uses.with(Capability::Grid);
        }
        if self.geometry.is_some() {
            uses = uses.with(Capability::Geometry);
        }
        if self.segments.is_some() {
            uses = uses.with(Capability::Segments);
        }
        if self.dataset.is_some() {
            uses = uses.with(Capability::Dataset);
        }
        if self.tabs.is_some() {
            uses = uses.with(Capability::Tabs);
        }
        uses
    }
}

thread_local! {
    // On the web, one thread and a plain static; natively, a renderer
    // registers on the thread that projects.
    static LINKED: Cell<Linked> = const { Cell::new(Linked::CORE) };
}

/// Register what this artifact links: its entry does before every boot.
pub fn link(linked: Linked) {
    if let Some(link) = linked.wide_colors {
        link();
    }
    if let Some(link) = linked.backdrop {
        link();
    }
    if let Some(link) = linked.timelines {
        link();
    }
    if let Some(link) = linked.text_transform {
        link();
    }
    if let Some(link) = linked.effects {
        link();
    }
    if let Some(a) = linked.animations {
        (a.grammars)();
    }
    if let Some(link) = linked.gradients {
        link();
    }
    if let Some(link) = linked.grid {
        link();
    }
    if let Some(link) = linked.segments {
        link();
    }
    LINKED.with(|cell| cell.set(linked));
}

/// What this artifact links; the core alone until an entry registers.
pub fn linked() -> Linked {
    LINKED.with(Cell::get)
}

/// Link, on this test's thread, what this crate can register itself: the
/// spring engine, lists and drags (Markdown's adapter lives above it), as an
/// entry whose plan uses them does.
#[cfg(test)]
pub(crate) fn link_for_tests() {
    link(Linked {
        motion: Some(crate::motion::springs),
        collections: Some(&exact_runner::LISTS),
        drag: true,
        surface_answer: exact_runner::RunnerLinks::ALL.surface_answer,
        canvas: exact_runner::RunnerLinks::ALL.canvas,
        router: exact_runner::RunnerLinks::ALL.router,
        format: exact_runner::RunnerLinks::ALL.format,
        inspection: true,
        ..linked()
    });
}

/// The runner's half of what this artifact links.
pub(crate) fn runner_links() -> exact_runner::RunnerLinks {
    exact_runner::RunnerLinks {
        surface_answer: linked().surface_answer,
        router: linked().router,
        lists: linked().collections,
        canvas: linked().canvas,
        format: linked().format,
        geometry: linked().geometry,
    }
}

/// Admit a plan only if this artifact links everything it uses (D6).
pub(crate) fn admit(plan: &exact_plan::Plan) -> Result<(), HostError> {
    let missing = exact_runner::uses(plan).beyond(linked().uses());
    if missing.is_empty() {
        Ok(())
    } else {
        Err(HostError::Unlinked(missing.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use crate::{Host, HostError};

    /// The core alone on this test's thread, whatever an earlier test on
    /// it linked (the test harness may reuse a thread).
    #[test]
    fn a_plan_that_uses_an_unlinked_capability_is_refused_by_name() {
        crate::link::link(crate::link::Linked::CORE);
        let markdown =
            contract::compile("component A\n  view\n    text \"**b**\" markup=\"markdown\"\n")
                .unwrap()
                .encode();
        let refused = Host::boot(&markdown, (), Default::default(), "/").map(|_| ());
        assert!(
            matches!(&refused, Err(HostError::Unlinked(names)) if names == "markdown"),
            "{refused:?}"
        );
        let plain = contract::compile("component A\n  view\n    text \"b\"\n").unwrap();
        assert!(Host::boot(&plain.encode(), (), Default::default(), "/").is_ok());
        // LLP 1078 D3: a segment length is linked by use; the fold's fields are the core's.
        let segments = contract::compile(
            "component A\n  view\n    column width=\"env(viewport-segment-width 0 0)\"\n",
        )
        .unwrap()
        .encode();
        let refused = Host::boot(&segments, (), Default::default(), "/").map(|_| ());
        assert!(
            matches!(&refused, Err(HostError::Unlinked(names)) if names == "segments"),
            "{refused:?}"
        );
        let fields = contract::compile("shape F\n  devicePosture: string\ncomponent A\n  resource m = exactViewport() as shape F\n  view\n    text m.devicePosture\n").unwrap();
        assert!(Host::boot(&fields.encode(), (), Default::default(), "/").is_ok());
        let drag = contract::compile(
            "component A\n  view\n    column id=\"sheet\" height=100\n      column heightDragFor=\"sheet\"\n",
        )
        .unwrap()
        .encode();
        let refused = Host::boot(&drag, (), Default::default(), "/").map(|_| ());
        assert!(
            matches!(&refused, Err(HostError::Unlinked(names)) if names == "motion, drag"),
            "{refused:?}"
        );
        // LLP 1054.000.003 D8: `formatNumber` is `format`'s; `formatTime` the core's.
        let count =
            contract::compile("component A\n  view\n    text formatNumber(1250, \"compact\")\n")
                .unwrap()
                .encode();
        let refused = Host::boot(&count, (), Default::default(), "/").map(|_| ());
        assert!(
            matches!(&refused, Err(HostError::Unlinked(names)) if names == "format"),
            "{refused:?}"
        );
        let grid =
            contract::compile("component A\n  view\n    box grid-template-columns=\"1fr 1fr\"\n")
                .unwrap()
                .encode();
        let refused = Host::boot(&grid, (), Default::default(), "/").map(|_| ());
        assert!(
            matches!(&refused, Err(HostError::Unlinked(names)) if names == "grid"),
            "{refused:?}"
        );
        let time = contract::compile("component A\n  view\n    text formatTime(0, 0, \"short\")\n")
            .unwrap();
        assert!(Host::boot(&time.encode(), (), Default::default(), "/").is_ok());
    }
}
