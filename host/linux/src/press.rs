//! Host-owned press feedback: one eased progress per pressed node, which
//! shows as its scale (multiplied into `scale` about transform-origin) and
//! its pressed paint (LLP 1061 D7: an opacity share, a background and a text
//! colour, `Host::presented` and the painter). The user's motion preference
//! never removes a button's feedback (LLP 1061). A node's `:active` (LLP 1121
//! D8) lowers to these rows, its opacity to an absolute target the press eases
//! to from what shows (`press_opacity_to`).
use super::*;
use exact_motion::Easing;

const DURATION: f64 = 120.;
const EASE: Easing = Easing::CubicBezier {
    x1: 0.16,
    y1: 1.,
    x2: 0.3,
    y2: 1.,
};

/// A press's progress, 0 at rest and 1 held, eased from `from` to `to`
/// since `start`.
pub(super) struct Feedback {
    from: f32,
    to: f32,
    start: f64,
}
impl Feedback {
    pub(super) fn progress(&self, now: f64) -> f32 {
        self.from + (self.to - self.from) * EASE.progress((now - self.start) / DURATION) as f32
    }
}

/// A state held as the real one (LLP 1122 D8, LLP 1121 stage 8 E3): the
/// build channel's `hold` or the agent's `holdState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HoldState {
    Active,
    Hover,
    FocusVisible,
}
impl HoldState {
    pub(crate) fn parse(name: &str) -> Option<Self> {
        match name {
            "active" => Some(Self::Active),
            "hover" => Some(Self::Hover),
            "focus-visible" => Some(Self::FocusVisible),
            _ => None,
        }
    }
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Hover => "hover",
            Self::FocusVisible => "focus-visible",
        }
    }
}

/// The one state held: on `view` (`key`), for the hold frame `revision`
/// (none: the agent's), and whether the pointer and the focus were in it
/// when last read, so that their coming or going is real input.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Held {
    pub view: ViewId,
    /// The node it was named on (a node inside the press, for `active`).
    pub asked: ViewId,
    pub key: NodeKey,
    pub state: HoldState,
    pub revision: Option<u64>,
    pub over: bool,
    pub focused: bool,
}

/// The looks that hold: the pointer's (`:hover`), the focus's
/// (`:focus-visible`), and the state held beside them.
#[derive(Default)]
pub(crate) struct Shown {
    pub hover: Vec<ViewId>,
    pub focus: Option<ViewId>,
    pub held: Option<Held>,
}
impl Shown {
    /// Whether `id`'s `:hover` and `:focus-visible` looks hold, real or held.
    pub(crate) fn on(&self, id: ViewId) -> [bool; 2] {
        let held = self.held.filter(|h| h.view == id).map(|h| h.state);
        [
            self.hover.contains(&id) || held == Some(HoldState::Hover),
            self.focus == Some(id) || held == Some(HoldState::FocusVisible),
        ]
    }

    /// Every node with a look on, and which.
    pub(crate) fn marks(&self) -> Vec<(ViewId, [bool; 2])> {
        let mut ids: Vec<ViewId> = self.hover.iter().copied().chain(self.focus).collect();
        ids.extend(self.held.map(|h| h.view));
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter().map(|id| (id, self.on(id))).collect()
    }
}

/// Whether a node writes any of the pressed paint (LLP 1061 D7).
pub(crate) fn paints(style: &exact_kernel::StyleProps) -> bool {
    use exact_kernel::StyleId::{PressBackgroundColor, PressColor, PressOpacity, PressOpacityTo};
    [
        PressOpacity,
        PressOpacityTo,
        PressBackgroundColor,
        PressColor,
    ]
    .into_iter()
    .any(|row| style.mask.has(row))
}

/// Whether the host may paint `property` over `node`'s own: a press's
/// paint, or a `:hover` or `:focus-visible` look naming it.
pub(crate) fn paints_over(node: &exact_kernel::NodeRef<'_>, property: Property) -> bool {
    use exact_kernel::StyleId::*;
    let (rows, row): (&[exact_kernel::StyleId], _) = match property {
        Property::Opacity => (&[PressOpacity, PressOpacityTo], Opacity),
        Property::BackgroundColor => (&[PressBackgroundColor], BackgroundColor),
        Property::Color => (&[PressColor], TextColor),
        _ => return false,
    };
    // A look's `currentcolor` (a colour row set with no value, or a side it
    // clears) reads the node's colour as that moves too.
    let current = |l: &exact_kernel::StyleProps| {
        let sides = [
            (BorderColorTop, l.border_color_top),
            (BorderColorRight, l.border_color_right),
            (BorderColorBottom, l.border_color_bottom),
            (BorderColorLeft, l.border_color_left),
        ];
        (l.mask.has(BackgroundColor) && l.background_color.is_none())
            || sides
                .iter()
                .any(|(id, c)| l.mask.union(l.written).has(*id) && c.is_none())
    };
    rows.iter().any(|r| node.style.mask.has(*r))
        || node
            .looks()
            .iter()
            .any(|(_, l)| l.mask.union(l.written).has(row))
        || property == Property::Color && node.looks().iter().any(|(_, l)| current(l))
}

/// Whether a press on `node` shows anything of its own: a scale or the
/// pressed paint.
pub(crate) fn shows(node: &exact_kernel::NodeRef<'_>) -> bool {
    node.style.press_scale != 1. || paints(node.style)
}

/// `shown` at a press's `progress`, and in the looks that hold (`looks`:
/// `:hover`, `:focus-visible`), whose opacity replaces the node's: the
/// press's opacity is `:active`'s absolute target, eased from what shows, or
/// the pressed paint's share of it.
pub(crate) fn present(
    node: &exact_kernel::NodeRef<'_>,
    progress: f32,
    looks: [bool; 2],
    shown: &mut Presented,
) {
    use exact_kernel::{StyleId, LOOK_FOCUS_VISIBLE, LOOK_HOVER};
    let disabled = node.props.bool(exact_kernel::PropId::Disabled) == Some(true);
    shown.looks = looks.map(|on| on && !disabled);
    for (state, on) in [
        (LOOK_HOVER, shown.looks[0]),
        (LOOK_FOCUS_VISIBLE, shown.looks[1]),
    ] {
        let names = |l: &exact_kernel::StyleProps| l.mask.union(l.written).has(StyleId::Opacity);
        let look = node.looks().iter().find(|(s, l)| *s == state && names(l));
        if let Some((_, l)) = look.filter(|_| on) {
            // A cleared opacity (`unset`) is CSS's initial one.
            shown.opacity = if l.mask.has(StyleId::Opacity) {
                l.opacity
            } else {
                1.0
            };
        }
    }
    // A disabled node shows no press, its release ease included; the
    // painter clamps opacity, so a base beyond 0 to 1 is clamped first.
    let progress = if disabled { 0. } else { progress };
    shown.opacity = shown.opacity.clamp(0., 1.);
    shown.press = scale(node.style.press_scale, progress);
    shown.press_paint = progress;
    let s = node.style;
    if s.mask.has(StyleId::PressOpacityTo) {
        shown.opacity += (s.press_opacity_to.clamp(0., 1.) - shown.opacity) * progress;
    } else if s.mask.has(StyleId::PressOpacity) {
        shown.opacity *= 1. + (s.press_opacity.clamp(0., 1.) - 1.) * progress;
    }
}

/// The scale a press at `progress` shows: none for a scale no press can use.
pub(crate) fn scale(to: f32, progress: f32) -> f32 {
    match to.is_finite() && to > 0. {
        true => 1. + (to - 1.) * progress,
        false => 1.,
    }
}

impl<D: DataSource> Host<D> {
    /// A node's committed presentation, before a press or a look shows on
    /// it: what the engine's cache starts from (`Host::presented` adds them
    /// at every read, so neither is ever kept).
    pub(crate) fn unpresented(&self, view: ViewId) -> Presented {
        let node = self.runner.kernel().node(view);
        node.map_or(Presented::IDENTITY, |n| Presented::from_style(n.style))
    }

    pub(crate) fn press_feedback(&mut self, key: NodeKey, down: bool, now: f64) {
        // A held press is the hold's: a real press's release, or its moving
        // off, leaves it (stage 8 E3).
        let held = self.looks_shown.held;
        if !down && held.is_some_and(|h| h.key == key && h.state == HoldState::Active) {
            return;
        }
        let to = if down { 1. } else { 0. };
        let old = self.presses.get(&key);
        if old.map_or(0., |p| p.to) == to {
            return;
        }
        let from = old.map_or(0., |p| p.progress(now));
        self.presses.insert(
            key,
            Feedback {
                from,
                to,
                start: now,
            },
        );
    }

    /// Every press at its end now: under the agent's clock a press lands,
    /// as Apple's and the web's do, so a read between `tap X down` and
    /// `tap up` sees it held and one after sees it gone.
    pub(crate) fn land_presses(&mut self) {
        let now = self.now_ms;
        for p in self.presses.values_mut() {
            (p.from, p.start) = (p.to, now - DURATION);
        }
    }

    /// Whether `key` is pressed (down, or easing there).
    pub(crate) fn pressed(&self, key: NodeKey) -> bool {
        self.presses.get(&key).is_some_and(|p| p.to == 1.)
    }

    /// The press's progress on `key` now; 0 when none.
    pub(crate) fn press_progress(&self, key: NodeKey) -> f32 {
        self.presses
            .get(&key)
            .map_or(0., |f| f.progress(self.now_ms))
    }

    pub(super) fn press_settle(&self) -> Option<f64> {
        self.presses
            .values()
            .map(|p| p.start + DURATION)
            .filter(|&end| end > self.now_ms)
            .reduce(f64::max)
    }

    pub(super) fn retire_presses(&mut self) {
        let kernel = self.runner.kernel();
        let now = self.now_ms;
        self.presses.retain(|key, p| {
            kernel.node_by_key(*key).is_some() && (p.to != 0. || now < p.start + DURATION)
        });
    }
}
