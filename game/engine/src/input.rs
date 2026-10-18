use crate::{Data, Vec2};
use std::sync::Arc;

mod contexts;
mod pad;
mod triggers;
pub(crate) use contexts::seed;
pub use contexts::{Binding, Bindings, InputContexts};
pub(crate) use pad::pad_index;
pub use pad::{Curve, Pad, PadState, MAX_PADS, PAD_BUTTONS};
pub use triggers::InputGestures;
use triggers::{Trigger, TriggerState};

/// Canvas regions used by discoverable touch controls (coordinates in points).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub enum Region {
    /// Left half.
    #[default]
    Left,
    /// Right half.
    Right,
    /// Top half.
    Top,
    /// Bottom half.
    Bottom,
    /// Entire canvas.
    All,
}
impl Region {
    fn contains(self, p: Vec2, size: Vec2) -> bool {
        p.x >= 0.0
            && p.y >= 0.0
            && p.x <= size.x
            && p.y <= size.y
            && match self {
                Self::Left => p.x < size.x * 0.5,
                Self::Right => p.x >= size.x * 0.5,
                Self::Top => p.y < size.y * 0.5,
                Self::Bottom => p.y >= size.y * 0.5,
                Self::All => true,
            }
    }
}
/// A unit-clamped directional action; keyboard groups, touch and pads can coexist.
#[derive(Clone, Debug, Default, Data)]
pub struct Stick {
    keys: Vec<[String; 4]>,
    touch: Vec<Region>,
    pads: Vec<Pad>,
    dead_zone: Option<f32>,
    curve: Curve,
}
impl Stick {
    /// W/S/A/D directional controls.
    pub fn wasd() -> Self {
        Self::keys("KeyW", "KeyS", "KeyA", "KeyD")
    }
    /// Add arrow-key directional controls.
    pub fn or_arrows(self) -> Self {
        self.or_keys("ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight")
    }

    /// Bind up, down, left, right KeyboardEvent.code spellings (pad buttons too:
    /// `GamepadUp`, `GamepadDown`, `GamepadLeft`, `GamepadRight` are the d-pad).
    pub fn keys(up: &str, down: &str, left: &str, right: &str) -> Self {
        Self::default().or_keys(up, down, left, right)
    }
    /// Add an alternative set of directional keys.
    pub fn or_keys(mut self, up: &str, down: &str, left: &str, right: &str) -> Self {
        for code in [up, down, left, right] {
            pad::check_code(code);
        }
        self.keys
            .push([up.into(), down.into(), left.into(), right.into()]);
        self
    }
    /// A floating stick, anchored at contact down, with a 60-point radius.
    pub fn touch(region: Region) -> Self {
        Self::default().or_touch(region)
    }
    /// Add a floating touch stick alongside keyboard bindings.
    pub fn or_touch(mut self, region: Region) -> Self {
        self.touch.push(region);
        self
    }
    /// A pad's stick, or its triggers as one axis.
    pub fn pad(pad: Pad) -> Self {
        Self::default().or_pad(pad)
    }
    /// Add a pad's stick, or its triggers, alongside keys and touch. Of the
    /// connected pads the one pushed furthest counts; the sum with keys and
    /// touch is unit-clamped like the rest.
    pub fn or_pad(mut self, pad: Pad) -> Self {
        self.pads.push(pad);
        self
    }
    /// The radial dead zone for this stick's pad readings, as a fraction of
    /// full deflection (0.15 unless set; 0 reads the raw axes). Past it the
    /// reading is rescaled, so full deflection still reads 1. Keys and touch
    /// keep their unit values.
    pub fn dead_zone(mut self, radius: f32) -> Self {
        assert!(
            (0.0..1.0).contains(&radius),
            "dead zone must be in [0, 1), got {radius}"
        );
        self.dead_zone = Some(radius);
        self
    }
    /// Shape a pad reading's magnitude past the dead zone.
    pub fn curve(mut self, curve: Curve) -> Self {
        if let Curve::Power(p) = curve {
            assert!(
                p.is_finite() && p > 0.,
                "curve power must be positive and finite"
            );
        }
        self.curve = curve;
        self
    }
}
#[derive(Clone, Debug, Default, Data)]
struct Action {
    name: String,
    keys: Vec<String>,
    touch: Vec<Region>,
    stick: Option<Stick>,
    trigger: Trigger,
    context: Option<String>,
}
/// The game's ordered, inspectable device bindings.
#[derive(Clone, Debug, Default, Data)]
pub struct Actions {
    entries: Vec<Action>,
}
static NO_ACTIONS: Actions = Actions {
    entries: Vec::new(),
};
impl Actions {
    /// A tap's default window: down and up within 200 ms of world time.
    pub const TAP: f32 = 0.2;
    /// No controls.
    pub fn new() -> Self {
        Self::default()
    }
    fn add(&mut self, a: Action) {
        assert!(
            !self.entries.iter().any(|old| old.name == a.name),
            "duplicate action `{}`",
            a.name
        );
        for code in &a.keys {
            pad::check_code(code);
        }
        self.entries.push(a);
    }
    fn keyed(mut self, name: &str, keys: &[&str], trigger: Trigger) -> Self {
        self.add(Action {
            name: name.into(),
            keys: keys.iter().map(|s| (*s).into()).collect(),
            trigger,
            ..Action::default()
        });
        self
    }
    /// Declare a button held by any of these physical keys or pad buttons.
    pub fn button(self, name: &str, keys: &[&str]) -> Self {
        self.keyed(name, keys, Trigger::Button)
    }
    /// Declare a directional action.
    pub fn stick(mut self, name: &str, stick: Stick) -> Self {
        self.add(Action {
            name: name.into(),
            stick: Some(stick),
            ..Action::default()
        });
        self
    }
    /// Add a touch region to a button, creating the button if necessary.
    pub fn button_touch(mut self, name: &str, region: Region) -> Self {
        if let Some(a) = self.entries.iter_mut().find(|a| a.name == name) {
            assert!(a.stick.is_none(), "action `{name}` is a stick");
            a.touch.push(region);
        } else {
            self.add(Action {
                name: name.into(),
                touch: vec![region],
                ..Action::default()
            });
        }
        self
    }
    /// A hold: `pressed` on the tick its keys have been down for `seconds` of
    /// world time, `held` from then until they lift, `released` then. A
    /// shorter press fires nothing.
    pub fn hold(self, name: &str, keys: &[&str], seconds: f32) -> Self {
        self.keyed(name, keys, Trigger::Hold(triggers::window(seconds)))
    }
    /// A double press: `pressed` on a second press within `seconds` of the
    /// first, `held` while that second press lasts.
    pub fn double(self, name: &str, keys: &[&str], seconds: f32) -> Self {
        self.keyed(name, keys, Trigger::Double(triggers::window(seconds)))
    }
    /// A tap: `pressed` and `released` together on the tick a press of at most
    /// [`Actions::TAP`] lifts; never `held`. It distinguishes a short press
    /// from a hold on the same key, so it fires at the release; a fire button
    /// that should fire at the press is a `button` read with `pressed`.
    pub fn tap(self, name: &str, keys: &[&str]) -> Self {
        self.tap_within(name, keys, Self::TAP)
    }
    /// A tap whose press lasts at most `seconds`.
    pub fn tap_within(self, name: &str, keys: &[&str], seconds: f32) -> Self {
        self.keyed(name, keys, Trigger::Tap(triggers::window(seconds)))
    }
    /// A chord: `pressed` when the last key goes down while the others are
    /// held, `held` while all are, `released` when one lifts. The other order
    /// (the last key first) fires nothing.
    pub fn chord(self, name: &str, keys: &[&str]) -> Self {
        assert!(keys.len() >= 2, "chord `{name}` needs at least two keys");
        self.keyed(name, keys, Trigger::Chord)
    }
    /// Declare actions that read only while the named context is active
    /// (`World::input_context`). A context is a set of actions, not a layer:
    /// there is no priority between contexts.
    pub fn context(mut self, context: &str, declare: impl FnOnce(Actions) -> Actions) -> Self {
        for mut a in declare(Actions::new()).entries {
            assert!(
                a.context.is_none(),
                "action `{}`: contexts do not nest",
                a.name
            );
            a.context = Some(context.into());
            self.add(a);
        }
        self
    }
    fn find(&self, name: &str) -> Option<&Action> {
        self.entries.iter().find(|a| a.name == name)
    }
    pub(crate) fn names(&self) -> String {
        self.entries
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
    pub(crate) fn json(&self) -> String {
        use crate::Writer;
        let mut w = crate::json::Encoder::rounded();
        w.begin_struct();
        for a in &self.entries {
            w.field(&a.name);
            a.write(&mut w);
        }
        w.end_struct();
        w.finish().unwrap()
    }
}
/// Pointer contact phase, matching the platform's input phases.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub enum PointerPhase {
    /// Begin contact.
    #[default]
    Down,
    /// Move or hover.
    Move,
    /// End contact.
    Up,
    /// Abandon contact.
    Cancel,
}
/// Most posted message accepted by `InputEvent::Message`, in bytes.
pub(crate) const MAX_MESSAGE: usize = 64 * 1024;
/// Raw device input stamped in the host clock's milliseconds. `player` is the
/// local player the device belongs to: hosts write 0, and the engine reads no
/// meaning into it.
#[derive(Clone, Debug, Data)]
pub enum InputEvent {
    /// KeyboardEvent.code, including KeyW, Space, and ArrowUp.
    Key {
        /// Physical key spelling.
        code: String,
        /// True for key down, false for key up.
        down: bool,
        /// Host clock milliseconds.
        at_ms: f64,
        /// Local player.
        player: u8,
    },
    /// Pointer coordinates in canvas points, top-left origin.
    Pointer {
        /// Platform contact identifier.
        id: u64,
        /// Contact phase.
        phase: PointerPhase,
        /// Horizontal canvas coordinate.
        x: f32,
        /// Vertical canvas coordinate.
        y: f32,
        /// Horizontal device motion since this pointer's previous event, in
        /// points: unbounded by the canvas or screen edge (a locked pointer).
        dx: f32,
        /// Vertical device motion since this pointer's previous event.
        dy: f32,
        /// Held mouse buttons as the web's `PointerEvent.buttons` bits: 1
        /// primary, 2 secondary, 4 middle. Touch contacts report 0.
        buttons: u32,
        /// Host clock milliseconds.
        at_ms: f64,
        /// Local player.
        player: u8,
    },
    /// A pointer held by an app control bound to a declared action.
    /// Coordinates are control-local points; sticks anchor on Down with a
    /// 60-point radius. Buttons remain held until Up, Cancel or Blur.
    Control {
        /// Declared action name, independent of its keyboard bindings.
        name: String,
        /// Platform contact identifier (multiple controls can be held).
        id: u64,
        /// Contact phase.
        phase: PointerPhase,
        /// Horizontal point in the control.
        x: f32,
        /// Vertical point in the control.
        y: f32,
        /// Host clock milliseconds.
        at_ms: f64,
        /// Local player.
        player: u8,
    },
    /// Wheel movement since the preceding device event.
    Wheel {
        /// Horizontal movement.
        dx: f32,
        /// Vertical movement.
        dy: f32,
        /// Host clock milliseconds.
        at_ms: f64,
        /// Local player.
        player: u8,
    },
    /// A pad's whole standard-mapping state, sent when a button or an axis
    /// changed. `connected: false` releases everything the pad held; a later
    /// connect at the same index is a fresh device.
    Pad {
        /// The host's device index, below [`MAX_PADS`].
        index: u32,
        /// False when the pad went away (or the host lost focus).
        connected: bool,
        /// False for a pad without the standard mapping: reported, ignored.
        standard: bool,
        /// Button values in `[0, 1]`, by [`PAD_BUTTONS`] index.
        buttons: [f32; 17],
        /// Left x, left y, right x, right y in `[-1, 1]`; positive y is down.
        axes: [f32; 4],
        /// Host clock milliseconds.
        at_ms: f64,
        /// Local player.
        player: u8,
    },
    /// Loss of focus releases all devices.
    Blur {
        /// Host clock milliseconds.
        at_ms: f64,
        /// Local player.
        player: u8,
    },
    /// Text the app posted into the world (Contract's `postMessage(text, surface)`).
    /// Each message reaches exactly one tick, in arrival order, never coalesced.
    Message {
        /// The posted text, at most 64 KiB.
        text: String,
        /// Host clock milliseconds.
        at_ms: f64,
        /// Local player.
        player: u8,
    },
}
impl Default for InputEvent {
    fn default() -> Self {
        Self::Blur {
            at_ms: 0.0,
            player: 0,
        }
    }
}
impl InputEvent {
    pub(crate) fn is_move(&self) -> bool {
        matches!(
            self,
            Self::Pointer {
                phase: PointerPhase::Move,
                ..
            } | Self::Control {
                phase: PointerPhase::Move,
                ..
            }
        )
    }
    pub(crate) fn same_motion(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Pointer {
                    id: a,
                    buttons: ab,
                    phase: PointerPhase::Move,
                    ..
                },
                Self::Pointer {
                    id: b,
                    buttons: bb,
                    phase: PointerPhase::Move,
                    ..
                },
            ) => a == b && ab == bb,
            (
                Self::Control {
                    name: a,
                    id: ai,
                    phase: PointerPhase::Move,
                    ..
                },
                Self::Control {
                    name: b,
                    id: bi,
                    phase: PointerPhase::Move,
                    ..
                },
            ) => a == b && ai == bi,
            // A pad whose buttons did not change only moved its sticks: the
            // later state replaces the earlier within one tick.
            (
                Self::Pad {
                    index: a,
                    connected: ac,
                    standard: astd,
                    buttons: ab,
                    ..
                },
                Self::Pad {
                    index: b,
                    connected: bc,
                    standard: bstd,
                    buttons: bb,
                    ..
                },
            ) => a == b && ac == bc && astd == bstd && ab == bb,
            _ => false,
        }
    }
    /// A coalesced move keeps the latest position and the motion of both.
    pub(crate) fn add_motion(&mut self, earlier: &Self) {
        if let (Self::Pointer { dx, dy, .. }, Self::Pointer { dx: ex, dy: ey, .. }) =
            (self, earlier)
        {
            *dx += ex;
            *dy += ey;
        }
    }
    pub(crate) fn set_at_ms(&mut self, value: f64) {
        match self {
            Self::Key { at_ms, .. }
            | Self::Pointer { at_ms, .. }
            | Self::Control { at_ms, .. }
            | Self::Wheel { at_ms, .. }
            | Self::Pad { at_ms, .. }
            | Self::Message { at_ms, .. }
            | Self::Blur { at_ms, .. } => *at_ms = value,
        }
    }
    pub(crate) fn at_ms(&self) -> f64 {
        match self {
            Self::Key { at_ms, .. }
            | Self::Pointer { at_ms, .. }
            | Self::Control { at_ms, .. }
            | Self::Wheel { at_ms, .. }
            | Self::Pad { at_ms, .. }
            | Self::Message { at_ms, .. }
            | Self::Blur { at_ms, .. } => *at_ms,
        }
    }
}
/// Primary pointer snapshot for one fixed step.
#[derive(Clone, Copy, Debug, Default, Data)]
pub struct PointerState {
    /// Platform contact identifier.
    pub id: u64,
    /// Latest canvas position in points.
    pub position: Vec2,
    /// Most recent primary (MouseLeft or touch) Down on this pointer, in points.
    /// Retained through normal Up and saves; cleared on Cancel/Blur/replacement.
    /// This is a snapshot, not an edge: gate commands on pressed/released actions.
    pub press_origin: Option<Vec2>,
    /// Whether contact remains down.
    pub down: bool,
    /// Device motion accumulated since the last tick, in points. A locked
    /// pointer's motion is unbounded while its position stays put.
    pub delta: Vec2,
}
/// Mouse buttons bind as keys of these names, by `PointerEvent.buttons` bit.
pub const MOUSE_BUTTONS: [(&str, u32); 3] =
    [("MouseLeft", 1), ("MouseRight", 2), ("MouseMiddle", 4)];
#[derive(Clone, Default, Data)]
struct Contact {
    id: u64,
    action: String,
    origin: Vec2,
    position: Vec2,
}
#[derive(Clone, Default, Data)]
pub(crate) struct ControlContact {
    id: u64,
    action: String,
    origin: Vec2,
    position: Vec2,
    code: Option<String>,
}
/// Tick-local action edges and accumulated device state. Only Sim mutates it.
/// All of it saves: a restore mid-hold continues the hold.
#[derive(Clone, Default, Data)]
pub struct Input {
    #[data(skip)]
    actions: Option<Arc<Actions>>,
    pub(crate) keys: Vec<String>,
    pressed: Vec<String>,
    released: Vec<String>,
    pointer: Option<PointerState>,
    contacts: Vec<Contact>,
    wheel: Vec2,
    pub(crate) viewport: Vec2,
    messages: Vec<String>,
    // Connected pads, by index.
    pads: Vec<PadState>,
    // Timers and states of trigger actions mid-gesture, by action name, and
    // the latched actions: a working copy of the world's InputGestures.
    #[data(skip)]
    triggers: Vec<TriggerState>,
    // The InputContexts and Bindings resources as of the tick's start.
    contexts: Vec<String>,
    bindings: Vec<Binding>,
    // Actions that read up until their binding lifts: rebound or newly in context while held.
    #[data(skip)]
    latched: Vec<String>,
}
impl Input {
    pub(crate) fn new(actions: Actions) -> Self {
        Self {
            actions: (!actions.entries.is_empty()).then(|| Arc::new(actions)),
            ..Self::default()
        }
    }
    pub(crate) fn actions(&self) -> &Actions {
        self.actions.as_deref().unwrap_or(&NO_ACTIONS)
    }
    pub(crate) fn validate(&self, event: &InputEvent) -> Result<(), String> {
        if !event.at_ms().is_finite() {
            return Err("input stamp must be finite".into());
        }
        match event {
            InputEvent::Message { text, .. } if text.len() > MAX_MESSAGE => {
                return Err(format!(
                    "posted message of {} bytes exceeds {MAX_MESSAGE}; post a short command and keep its data in the world",
                    text.len()
                ));
            }
            InputEvent::Key { code, .. } if code.starts_with("Gamepad") => {
                return Err(format!(
                    "`{code}` is a pad button: a host reports pads as Pad events (Sim::pad_button in a test)"
                ));
            }
            InputEvent::Pad {
                index,
                buttons,
                axes,
                ..
            } => pad::valid(*index, buttons, axes)?,
            InputEvent::Control { name, x, y, .. } => {
                if self.actions().find(name).is_none() {
                    return Err(format!(
                        "unknown control `{name}`; declared actions: {}",
                        self.actions().names()
                    ));
                }
                if !x.is_finite() || !y.is_finite() {
                    return Err(format!("control `{name}` needs finite points"));
                }
            }
            InputEvent::Pointer { x, y, dx, dy, .. }
                if !x.is_finite() || !y.is_finite() || !dx.is_finite() || !dy.is_finite() =>
            {
                return Err("input needs finite points".into())
            }
            InputEvent::Wheel { dx, dy, .. } if !dx.is_finite() || !dy.is_finite() => {
                return Err("wheel needs finite deltas".into())
            }
            _ => {}
        }
        Ok(())
    }
    pub(crate) fn validate_saved(&self, saved: &Self) -> Result<(), String> {
        if saved.keys.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("saved keys must be sorted and unique".into());
        }
        if !saved.viewport.is_finite()
            || saved.pointer.is_some_and(|p| {
                !p.position.is_finite()
                    || !p.delta.is_finite()
                    || p.press_origin.is_some_and(|origin| !origin.is_finite())
            })
        {
            return Err("saved input needs finite points".into());
        }
        for p in &saved.contacts {
            if !p.origin.is_finite() || !p.position.is_finite() {
                return Err(format!("control `{}` needs finite points", p.action));
            }
            if !p.action.is_empty() {
                self.validate(&InputEvent::Control {
                    name: p.action.clone(),
                    id: p.id,
                    phase: PointerPhase::Down,
                    x: p.origin.x,
                    y: p.origin.y,
                    at_ms: 0.,
                    player: 0,
                })?;
            }
        }
        if saved
            .pads
            .windows(2)
            .any(|pair| pair[0].index >= pair[1].index)
        {
            return Err("saved pads must be sorted by unique index".into());
        }
        for pad in &saved.pads {
            pad.valid()?;
        }
        Ok(())
    }
    pub(crate) fn control_contacts(&self) -> Vec<ControlContact> {
        self.contacts
            .iter()
            .filter(|p| !p.action.is_empty())
            .map(|p| ControlContact {
                id: p.id,
                action: p.action.clone(),
                origin: p.origin,
                position: p.position,
                code: match p.id {
                    4294967294 => Some("Space".into()),
                    4294967293 => Some("Enter".into()),
                    _ => None,
                },
            })
            .collect()
    }
    pub(crate) fn held_controls(&self) -> Vec<String> {
        let mut names: Vec<_> = self
            .contacts
            .iter()
            .filter(|p| !p.action.is_empty())
            .map(|p| p.action.as_str())
            .collect();
        names.sort_unstable();
        names.dedup();
        names.into_iter().map(str::to_owned).collect()
    }
    fn action(&self, name: &str) -> &Action {
        let a = self.actions().find(name);
        assert!(
            a.is_some(),
            "unknown action `{name}`; declared actions: {}",
            self.actions().names()
        );
        a.unwrap()
    }
    fn touching(&self, regions: &[Region]) -> bool {
        self.viewport.min_element() > 0.0
            && self.contacts.iter().any(|p| {
                p.action.is_empty() && regions.iter().any(|r| r.contains(p.origin, self.viewport))
            })
    }
    /// Whether a key, mouse button or pad button (on any pad) is down.
    pub(crate) fn down(&self, code: &str) -> bool {
        self.keys.binary_search_by(|k| k.as_str().cmp(code)).is_ok()
            || pad::pad_index(code).is_some_and(|b| self.pads.iter().any(|p| p.down(b)))
    }
    /// The action's keys after `World::rebind` (a stick's are never rebound).
    fn keys_of<'a>(&'a self, a: &'a Action) -> &'a [String] {
        self.bindings
            .iter()
            .find(|b| b.action == a.name && a.stick.is_none())
            .map_or(&a.keys, |b| &b.keys)
    }
    fn in_context(&self, a: &Action) -> bool {
        a.context.as_ref().is_none_or(|c| self.contexts.contains(c))
    }
    fn direction(&self, name: &str, s: &Stick) -> Vec2 {
        let mut v = Vec2::ZERO;
        let down = |i: usize| s.keys.iter().any(|keys| self.down(&keys[i]));
        v.x = i32::from(down(3)) as f32 - i32::from(down(2)) as f32;
        v.y = i32::from(down(0)) as f32 - i32::from(down(1)) as f32;
        for p in &self.contacts {
            if p.action == name
                || (p.action.is_empty()
                    && self.viewport.min_element() > 0.0
                    && s.touch.iter().any(|r| r.contains(p.origin, self.viewport)))
            {
                v += (p.position - p.origin) * Vec2::new(1.0, -1.0) / 60.0;
            }
        }
        // Of the pads, the one pushed furthest steers (the lowest index on a
        // tie), so two players' opposite sticks never cancel.
        for &control in &s.pads {
            let dead_zone = s.dead_zone.unwrap_or(pad::DEAD_ZONE);
            let shaped = self
                .pads
                .iter()
                .map(|p| pad::shape(p.stick(control), dead_zone, s.curve));
            v += shaped.fold(Vec2::ZERO, |best, next| {
                if next.length_squared() > best.length_squared() {
                    next
                } else {
                    best
                }
            });
        }
        v.clamp_length_max(1.0)
    }
    /// Whether the action's devices are down, before contexts and latches.
    fn raw(&self, a: &Action) -> bool {
        let keys = self.keys_of(a);
        if a.trigger == Trigger::Chord {
            return keys.iter().all(|k| self.down(k));
        }
        keys.iter().any(|k| self.down(k))
            || self.touching(&a.touch)
            || (a.stick.is_none() && self.contacts.iter().any(|p| p.action == a.name))
            || a.stick
                .as_ref()
                .is_some_and(|s| self.direction(&a.name, s) != Vec2::ZERO)
    }
    /// Whether the action's binding is down where the game can read it.
    fn bound(&self, a: &Action) -> bool {
        self.in_context(a) && !self.latched.contains(&a.name) && self.raw(a)
    }
    fn active(&self, a: &Action) -> bool {
        match a.trigger {
            Trigger::Button => self.bound(a),
            Trigger::Tap(_) => false,
            Trigger::Hold(_) | Trigger::Double(_) | Trigger::Chord => self.trigger_on(&a.name),
        }
    }
    /// Whether the declared action is held now.
    pub fn held(&self, name: &str) -> bool {
        self.active(self.action(name))
    }
    /// A rising edge since the last tick, even if followed immediately by release.
    pub fn pressed(&self, name: &str) -> bool {
        self.action(name);
        self.pressed.iter().any(|n| n == name)
    }
    /// A falling edge since the last tick.
    pub fn released(&self, name: &str) -> bool {
        self.action(name);
        self.released.iter().any(|n| n == name)
    }
    /// Direction, unit-clamped, with positive Y forward/up.
    pub fn stick_xz(&self, name: &str) -> crate::Vec3 {
        let v = self.stick(name);
        crate::Vec3::new(v.x, 0.0, -v.y)
    }
    /// Read a directional action in its two-dimensional input plane.
    pub fn stick(&self, name: &str) -> Vec2 {
        let a = self.action(name);
        match &a.stick {
            Some(s) if self.in_context(a) => self.direction(name, s),
            _ => Vec2::ZERO,
        }
    }
    /// Primary contact or hover; its delta expires after this tick.
    pub fn pointer(&self) -> Option<PointerState> {
        self.pointer
    }
    /// Accumulated wheel delta since the preceding tick.
    pub fn wheel(&self) -> Vec2 {
        self.wheel
    }
    /// Messages posted into the world that this tick receives, in arrival order.
    /// A message waits in the input queue (and in a save taken meanwhile) until
    /// its tick; once that tick has read it, it is gone from later saves.
    pub fn messages(&self) -> &[String] {
        &self.messages
    }
    /// The canvas size in points that pointer coordinates are relative to; zero
    /// before the host reports one. With `Camera::matrix` it turns a pointer
    /// position into a world ray.
    pub fn viewport(&self) -> Vec2 {
        self.viewport
    }
    /// A connected pad's state as the host last reported it.
    pub fn pad(&self, index: u32) -> Option<PadState> {
        self.pads.iter().find(|p| p.index == index).copied()
    }
    /// The agent's `state` input record: the declarations, the devices held
    /// (`forwarded…` includes events still queued, from `host`), the pads as
    /// reported, contexts, bindings, and the actions held and fired at the
    /// last tick.
    pub(crate) fn state_json(&self, host: &Self, refused_posts: u64) -> Result<String, String> {
        fn json<T: Data>(v: &T) -> Result<String, String> {
            let mut w = crate::json::Encoder::rounded();
            v.write(&mut w);
            w.finish().map_err(|e| e.to_string())
        }
        let held: Vec<String> = self
            .actions()
            .entries
            .iter()
            .filter(|a| self.active(a))
            .map(|a| a.name.clone())
            .collect();
        Ok(format!(
            "{{\"actions\":{},\"held\":{},\"forwarded\":{},\"controls\":{},\"forwardedControls\":{},\"controlContacts\":{},\"pads\":{},\"contexts\":{},\"bindings\":{},\"latched\":{},\"heldActions\":{},\"fired\":{{\"pressed\":{},\"released\":{}}},\"refusedPosts\":{refused_posts}}}",
            self.actions().json(),
            json(&self.keys)?,
            json(&host.keys)?,
            json(&self.held_controls())?,
            json(&host.held_controls())?,
            json(&host.control_contacts())?,
            json(&host.pads)?,
            json(&self.contexts)?,
            json(&self.bindings)?,
            json(&self.latched)?,
            json(&held)?,
            json(&self.pressed)?,
            json(&self.released)?,
        ))
    }
    pub(crate) fn clear_edges(&mut self) {
        self.messages.clear();
        self.pressed.clear();
        self.released.clear();
        self.wheel = Vec2::ZERO;
        if let Some(p) = &mut self.pointer {
            p.delta = Vec2::ZERO;
        }
    }
    pub(crate) fn restore_dynamic(&mut self, saved: Self) {
        self.viewport = saved.viewport;
        self.keys = saved.keys;
        self.pointer = saved.pointer;
        self.contacts = saved.contacts;
        self.pads = saved.pads;
        self.contexts = saved.contexts;
        self.bindings = saved.bindings;
        self.clear_edges();
    }
    /// Held state changes; trigger timers start at `at_us`; no edges remain.
    pub(crate) fn apply_paused(&mut self, event: InputEvent, at_us: i64) {
        if !matches!(event, InputEvent::Wheel { .. }) {
            self.apply(event, at_us);
        }
        self.clear_edges();
    }
    fn mouse_buttons(&self) -> u32 {
        MOUSE_BUTTONS
            .iter()
            .filter(|(code, _)| self.keys.binary_search_by(|k| k.as_str().cmp(code)).is_ok())
            .fold(0, |mask, (_, bit)| mask | bit)
    }
    /// Apply one event stamped at world microsecond `at_us`, recording the
    /// edges it makes.
    pub(crate) fn apply(&mut self, event: InputEvent, at_us: i64) {
        self.mature(|due| due <= at_us);
        if matches!(event, InputEvent::Wheel { .. } | InputEvent::Message { .. })
            || (matches!(
                event,
                InputEvent::Pointer {
                    phase: PointerPhase::Move,
                    buttons,
                    ..
                } if buttons & 7 == self.mouse_buttons()
            ) && self.contacts.is_empty())
            || self.actions.is_none()
            || matches!(&event, InputEvent::Key { code, down, .. } if self.keys.contains(code) == *down)
        {
            return self.apply_state(event);
        }
        let actions = self.actions.clone().unwrap_or_default();
        let before: Vec<_> = actions
            .entries
            .iter()
            .map(|a| (self.bound(a), self.active(a), self.last_key_down(a)))
            .collect();
        self.apply_state(event);
        // A latch lifts with its binding: the next press is a press.
        let latched = std::mem::take(&mut self.latched);
        self.latched = latched
            .into_iter()
            .filter(|name| actions.find(name).is_some_and(|a| self.raw(a)))
            .collect();
        for (a, (was_bound, was, last_was_down)) in actions.entries.iter().zip(before) {
            if a.trigger != Trigger::Button {
                self.step_trigger(a, was_bound, last_was_down, at_us);
                continue;
            }
            let now = self.active(a);
            if now && !was {
                self.press(&a.name);
            }
            if was && !now {
                self.release(&a.name);
            }
        }
        self.triggers.retain(TriggerState::live);
    }
    fn press(&mut self, name: &str) {
        if !self.pressed.iter().any(|n| n == name) {
            self.pressed.push(name.into());
        }
    }
    fn release(&mut self, name: &str) {
        if !self.released.iter().any(|n| n == name) {
            self.released.push(name.into());
        }
    }
    fn apply_state(&mut self, event: InputEvent) {
        if self.validate(&event).is_err() {
            return;
        }
        match event {
            InputEvent::Key { code, down, .. } => match (self.keys.binary_search(&code), down) {
                (Err(i), true) => self.keys.insert(i, code),
                (Ok(i), false) => {
                    self.keys.remove(i);
                }
                _ => {}
            },
            InputEvent::Wheel { dx, dy, .. } => self.wheel += Vec2::new(dx, dy),
            InputEvent::Message { text, .. } => self.messages.push(text),
            InputEvent::Control {
                name,
                id,
                phase,
                x,
                y,
                ..
            } => {
                let position = Vec2::new(x, y);
                match phase {
                    PointerPhase::Down => {
                        self.contacts.retain(|p| p.id != id || p.action != name);
                        self.contacts.push(Contact {
                            id,
                            action: name,
                            origin: position,
                            position,
                        });
                    }
                    PointerPhase::Move => {
                        if let Some(p) = self
                            .contacts
                            .iter_mut()
                            .find(|p| p.id == id && p.action == name)
                        {
                            p.position = position;
                        }
                    }
                    PointerPhase::Up | PointerPhase::Cancel => {
                        self.contacts.retain(|p| p.id != id || p.action != name)
                    }
                }
            }
            InputEvent::Blur { .. } => {
                self.keys.clear();
                self.contacts.clear();
                self.pads.clear();
                self.wheel = Vec2::ZERO;
                self.pointer = None;
            }
            InputEvent::Pad {
                index,
                connected,
                standard,
                buttons,
                axes,
                ..
            } => {
                let at = self.pads.binary_search_by_key(&index, |p| p.index);
                let state = PadState {
                    index,
                    standard,
                    ..PadState::default()
                };
                let state = if standard {
                    PadState {
                        buttons,
                        axes,
                        ..state
                    }
                } else {
                    state
                };
                match (at, connected) {
                    (Ok(i), false) => {
                        self.pads.remove(i);
                    }
                    (Err(_), false) => {}
                    (Ok(i), true) => self.pads[i] = state,
                    (Err(i), true) => self.pads.insert(i, state),
                }
            }
            InputEvent::Pointer {
                id,
                phase,
                x,
                y,
                dx,
                dy,
                buttons,
                ..
            } => {
                let previous_buttons = self.mouse_buttons();
                let primary_down = phase == PointerPhase::Down
                    && (buttons == 0 || (buttons & 1 != 0 && previous_buttons & 1 == 0));
                for (code, bit) in MOUSE_BUTTONS {
                    match (
                        self.keys.binary_search_by(|k| k.as_str().cmp(code)),
                        buttons & bit != 0,
                    ) {
                        (Err(i), true) => self.keys.insert(i, code.into()),
                        (Ok(i), false) => {
                            self.keys.remove(i);
                        }
                        _ => {}
                    }
                }
                let position = Vec2::new(x, y);
                let motion = Vec2::new(dx, dy);
                if let Some(p) = &mut self.pointer {
                    if p.id == id {
                        p.delta += motion;
                        p.position = position;
                        p.down = phase == PointerPhase::Down
                            || (p.down && phase == PointerPhase::Move)
                            || (phase == PointerPhase::Up && buttons != 0);
                        if primary_down {
                            p.press_origin = Some(position);
                        } else if phase == PointerPhase::Cancel {
                            p.press_origin = None;
                        }
                    } else if !p.down && phase == PointerPhase::Down {
                        self.pointer = None;
                    }
                }
                if self.pointer.is_none() {
                    self.pointer = Some(PointerState {
                        id,
                        position,
                        press_origin: primary_down.then_some(position),
                        down: phase != PointerPhase::Cancel
                            && (buttons != 0 || phase == PointerPhase::Down),
                        delta: motion,
                    });
                }
                match phase {
                    PointerPhase::Down => {
                        if let Some(p) = self
                            .contacts
                            .iter_mut()
                            .find(|p| p.id == id && p.action.is_empty())
                        {
                            p.position = position;
                            if primary_down {
                                p.origin = position;
                            }
                        } else {
                            self.contacts.push(Contact {
                                id,
                                action: String::new(),
                                origin: position,
                                position,
                            });
                        }
                    }
                    PointerPhase::Move | PointerPhase::Up => {
                        if phase == PointerPhase::Up && buttons == 0 {
                            self.contacts.retain(|p| p.id != id || !p.action.is_empty());
                        } else if let Some(p) = self
                            .contacts
                            .iter_mut()
                            .find(|p| p.id == id && p.action.is_empty())
                        {
                            p.position = position;
                        }
                    }
                    PointerPhase::Cancel => {
                        self.contacts.retain(|p| p.id != id || !p.action.is_empty())
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
