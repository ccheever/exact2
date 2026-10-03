use crate::{Data, Vec2};
use std::sync::Arc;

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
/// A unit-clamped directional action; keyboard groups and touch can coexist.
#[derive(Clone, Debug, Default, Data)]
pub struct Stick {
    keys: Vec<[String; 4]>,
    touch: Vec<Region>,
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

    /// Bind up, down, left, right KeyboardEvent.code spellings.
    pub fn keys(up: &str, down: &str, left: &str, right: &str) -> Self {
        Self::default().or_keys(up, down, left, right)
    }
    /// Add an alternative set of directional keys.
    pub fn or_keys(mut self, up: &str, down: &str, left: &str, right: &str) -> Self {
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
}
#[derive(Clone, Debug, Default, Data)]
struct Action {
    name: String,
    keys: Vec<String>,
    touch: Vec<Region>,
    stick: Option<Stick>,
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
        self.entries.push(a);
    }
    /// Declare a button held by any of these physical keys.
    pub fn button(mut self, name: &str, keys: &[&str]) -> Self {
        self.add(Action {
            name: name.into(),
            keys: keys.iter().map(|s| (*s).into()).collect(),
            ..Action::default()
        });
        self
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
/// Raw device input stamped in the host clock's milliseconds.
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
    },
    /// Wheel movement since the preceding device event.
    Wheel {
        /// Horizontal movement.
        dx: f32,
        /// Vertical movement.
        dy: f32,
        /// Host clock milliseconds.
        at_ms: f64,
    },
    /// Loss of focus releases all devices.
    Blur {
        /// Host clock milliseconds.
        at_ms: f64,
    },
    /// Text the app posted into the world (Contract's `postMessage(surface, text)`).
    /// Each message reaches exactly one tick, in arrival order, never coalesced.
    Message {
        /// The posted text, at most 64 KiB.
        text: String,
        /// Host clock milliseconds.
        at_ms: f64,
    },
}
impl Default for InputEvent {
    fn default() -> Self {
        Self::Blur { at_ms: 0.0 }
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
        if !self.is_move() || !other.is_move() {
            return false;
        }
        match (self, other) {
            (
                Self::Pointer {
                    id: a, buttons: ab, ..
                },
                Self::Pointer {
                    id: b, buttons: bb, ..
                },
            ) => a == b && ab == bb,
            (
                Self::Control {
                    name: a, id: ai, ..
                },
                Self::Control {
                    name: b, id: bi, ..
                },
            ) => a == b && ai == bi,
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
            | Self::Message { at_ms, .. }
            | Self::Blur { at_ms } => *at_ms = value,
        }
    }
    pub(crate) fn at_ms(&self) -> f64 {
        match self {
            Self::Key { at_ms, .. }
            | Self::Pointer { at_ms, .. }
            | Self::Control { at_ms, .. }
            | Self::Wheel { at_ms, .. }
            | Self::Message { at_ms, .. }
            | Self::Blur { at_ms } => *at_ms,
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
        if let InputEvent::Message { text, .. } = event {
            if text.len() > MAX_MESSAGE {
                return Err(format!(
                    "posted message of {} bytes exceeds {MAX_MESSAGE}; post a short command and keep its data in the world",
                    text.len()
                ));
            }
        }
        if let InputEvent::Control { name, x, y, .. } = event {
            if !self.actions().entries.iter().any(|a| &a.name == name) {
                return Err(format!(
                    "unknown control `{name}`; declared actions: {}",
                    self.actions()
                        .entries
                        .iter()
                        .map(|a| a.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if !x.is_finite() || !y.is_finite() {
                return Err(format!("control `{name}` needs finite points"));
            }
        }
        match event {
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
            || saved
                .pointer
                .is_some_and(|p| !p.position.is_finite() || !p.delta.is_finite())
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
                })?;
            }
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
        let a = self.actions().entries.iter().find(|a| a.name == name);
        assert!(
            a.is_some(),
            "unknown action `{name}`; declared actions: {}",
            self.actions()
                .entries
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        a.unwrap()
    }
    fn touching(&self, regions: &[Region]) -> bool {
        self.viewport.min_element() > 0.0
            && self.contacts.iter().any(|p| {
                p.action.is_empty() && regions.iter().any(|r| r.contains(p.origin, self.viewport))
            })
    }
    fn direction(&self, name: &str, s: &Stick) -> Vec2 {
        let mut v = Vec2::ZERO;
        let down = |i| s.keys.iter().any(|keys| self.keys.contains(&keys[i]));
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
        v.clamp_length_max(1.0)
    }
    fn active(&self, a: &Action) -> bool {
        a.keys.iter().any(|k| self.keys.contains(k))
            || self.touching(&a.touch)
            || (a.stick.is_none() && self.contacts.iter().any(|p| p.action == a.name))
            || a.stick
                .as_ref()
                .is_some_and(|s| self.direction(&a.name, s) != Vec2::ZERO)
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
        self.action(name)
            .stick
            .as_ref()
            .map_or(Vec2::ZERO, |s| self.direction(name, s))
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
    /// Each is delivered to exactly one tick; none is saved once delivered.
    pub fn messages(&self) -> &[String] {
        &self.messages
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
        self.clear_edges();
    }
    pub(crate) fn apply_paused(&mut self, event: InputEvent) {
        if !matches!(event, InputEvent::Wheel { .. }) {
            self.apply_state(event);
        }
        self.clear_edges();
    }
    fn mouse_buttons(&self) -> u32 {
        MOUSE_BUTTONS
            .iter()
            .filter(|(code, _)| self.keys.binary_search_by(|k| k.as_str().cmp(code)).is_ok())
            .fold(0, |mask, (_, bit)| mask | bit)
    }
    pub(crate) fn apply(&mut self, event: InputEvent) {
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
        let before: Vec<_> = self
            .actions()
            .entries
            .iter()
            .map(|a| self.active(a))
            .collect();
        self.apply_state(event);
        let actions = self.actions.as_deref().unwrap_or(&NO_ACTIONS);
        for (a, was) in actions.entries.iter().zip(before) {
            let now = self.active(a);
            if now && !was && !self.pressed.contains(&a.name) {
                self.pressed.push(a.name.clone());
            }
            if was && !now && !self.released.contains(&a.name) {
                self.released.push(a.name.clone());
            }
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
                self.wheel = Vec2::ZERO;
                self.pointer = None;
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
                        p.down = matches!(phase, PointerPhase::Down)
                            || (p.down && phase == PointerPhase::Move);
                    } else if !p.down && phase == PointerPhase::Down {
                        self.pointer = None;
                    }
                }
                if self.pointer.is_none() {
                    self.pointer = Some(PointerState {
                        id,
                        position,
                        down: phase == PointerPhase::Down,
                        delta: motion,
                    });
                }
                match phase {
                    PointerPhase::Down => {
                        self.contacts.retain(|p| p.id != id || !p.action.is_empty());
                        self.contacts.push(Contact {
                            id,
                            action: String::new(),
                            origin: position,
                            position,
                        });
                    }
                    PointerPhase::Move => {
                        if let Some(p) = self
                            .contacts
                            .iter_mut()
                            .find(|p| p.id == id && p.action.is_empty())
                        {
                            p.position = position;
                        }
                    }
                    PointerPhase::Up | PointerPhase::Cancel => {
                        self.contacts.retain(|p| p.id != id || !p.action.is_empty())
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod control_tests {
    use super::*;

    #[test]
    fn restore_rejects_unknown_saved_control_atomically() {
        use crate::{Clock, Game, Sim, World};
        struct Old;
        struct New;
        impl Game for Old {
            const ID: &'static str = "renamed";
            type Args = ();
            fn actions() -> Actions {
                Actions::new().button("old", &[])
            }
            fn setup(_: &mut World, _: &()) {}
            fn tick(_: &mut World, _: &Input, _: &()) {}
        }
        impl Game for New {
            const ID: &'static str = "renamed";
            type Args = ();
            fn actions() -> Actions {
                Actions::new().button("new", &[])
            }
            fn setup(_: &mut World, _: &()) {}
            fn tick(_: &mut World, _: &Input, _: &()) {}
        }
        for queued in [true, false] {
            let mut old = Sim::<Old>::new(()).unwrap();
            old.advance(0., Clock::Seekable);
            old.input(control("old", 7, PointerPhase::Down, 10., 20.));
            if !queued {
                old.advance(100., Clock::Seekable);
            }
            let mut new = Sim::<New>::new(()).unwrap();
            let before = new.save().unwrap();
            let error = new.restore(&old.save().unwrap()).unwrap_err().to_string();
            assert!(
                error.contains("old") && error.contains("control"),
                "{error}"
            );
            assert_eq!(new.save().unwrap(), before);
        }
    }
    #[test]
    fn saved_keys_require_sorted_unique_order_before_restore_commits() {
        use crate::{Clock, Game, Sim, World};
        struct Keys;
        impl Game for Keys {
            const ID: &'static str = "saved-key-order";
            type Args = ();
            fn actions() -> Actions {
                Actions::new().button("z", &["KeyZ"])
            }
            fn setup(_: &mut World, _: &()) {}
            fn tick(w: &mut World, input: &Input, _: &()) {
                w.publish("held", input.held("z"));
            }
        }
        let mut source = Sim::<Keys>::new(()).unwrap();
        let mut live = Sim::<Keys>::new(()).unwrap();
        live.advance(0., Clock::Seekable);
        live.key_down("KeyB");
        live.advance(17., Clock::Seekable);
        live.key_down("KeyZ"); // Preserve a pending press across refusal too.
        let before = live.save().unwrap();
        let state = live.agent(r#"{"op":"state"}"#);
        let hash = live.world().hash();
        for keys in [
            vec!["KeyZ", "KeyA"],
            vec!["KeyA", "KeyA"],
            vec!["KeyZ", "KeyA", "KeyZ"],
        ] {
            source.input.keys = keys.iter().map(|s| (*s).into()).collect();
            let malformed = source.save().unwrap();
            for bound in [false, true] {
                let error = if bound {
                    live.restore_bound(&malformed)
                } else {
                    live.restore(&malformed)
                }
                .unwrap_err()
                .to_string();
                assert!(
                    error.contains("saved keys must be sorted and unique"),
                    "{error}"
                );
                assert_eq!(live.save().unwrap(), before);
                assert_eq!(live.agent(r#"{"op":"state"}"#), state);
                assert_eq!(live.world().hash(), hash);
            }
        }
        for keys in [vec![], vec!["KeyZ"], vec!["KeyA", "KeyZ"]] {
            source.input.keys = keys.iter().map(|s| (*s).into()).collect();
            let saved = source.save().unwrap();
            let mut restored = Sim::<Keys>::new(()).unwrap();
            restored.restore(&saved).unwrap();
            assert_eq!(restored.save().unwrap(), saved);
            restored.advance(1000., Clock::Seekable);
            restored.key_up("KeyZ");
            restored.advance(1017., Clock::Seekable);
            assert!(!restored.input.keys.iter().any(|key| key == "KeyZ"));
            assert_eq!(
                restored.take_published().as_deref(),
                Some(r#"{"held":false}"#)
            );
        }
    }

    #[test]
    fn saved_contacts_validate_origins_positions_and_keyboard_identity() {
        let mut original = input();
        original.apply(control("jump", 4294967294, PointerPhase::Down, 10., 20.));
        let record = crate::json::to_string(&original.control_contacts()).unwrap();
        assert!(
            record.contains("\"code\":[\"Space\"]") && record.contains("\"id\":4294967294"),
            "{record}"
        );
        for origin in [true, false] {
            let mut bad = original.clone();
            if origin {
                bad.contacts[0].origin.x = f32::NAN;
            } else {
                bad.contacts[0].position.y = f32::INFINITY;
            }
            assert!(input().validate_saved(&bad).unwrap_err().contains("finite"));
        }
        for event in [
            control("jump", 1, PointerPhase::Move, f32::NAN, 0.),
            InputEvent::Pointer {
                id: 1,
                phase: PointerPhase::Down,
                x: f32::INFINITY,
                y: 0.,
                dx: 0.,
                dy: 0.,
                buttons: 1,
                at_ms: 0.,
            },
            InputEvent::Pointer {
                id: 1,
                phase: PointerPhase::Move,
                x: 0.,
                y: 0.,
                dx: f32::NAN,
                dy: 0.,
                buttons: 0,
                at_ms: 0.,
            },
        ] {
            assert!(input().validate(&event).unwrap_err().contains("finite"));
        }
    }
    #[test]
    fn invalid_control_application_never_panics() {
        let mut input = input();
        input.apply(control("missing", 1, PointerPhase::Down, 0., 0.));
        assert!(input.held_controls().is_empty());
    }
    #[test]
    fn wheel_preserves_action_edges_and_expires_after_the_tick() {
        let mut input = input();
        input.apply(control("jump", 1, PointerPhase::Down, 0., 0.));
        input.apply(control("jump", 1, PointerPhase::Up, 0., 0.));
        input.apply(control("move", 2, PointerPhase::Down, 0., 0.));
        input.apply(control("move", 2, PointerPhase::Move, 60., 0.));
        for (dx, dy) in [(2.5, -3.), (-1.5, 7.), (f32::NAN, 1.)] {
            input.apply(InputEvent::Wheel { dx, dy, at_ms: 0. });
        }
        assert_eq!(input.wheel(), Vec2::new(1., 4.));
        assert!(input.pressed("jump") && input.released("jump") && !input.held("jump"));
        assert!(input.pressed("move") && input.held("move") && !input.released("move"));
        assert_eq!(input.stick_xz("move"), crate::Vec3::X);
        input.clear_edges();
        assert_eq!(input.wheel(), Vec2::ZERO);
        assert!(!input.pressed("jump") && !input.released("jump"));
        assert!(input.held("move") && !input.pressed("move"));
        input.apply_paused(InputEvent::Wheel {
            dx: 2.,
            dy: 3.,
            at_ms: 0.,
        });
        assert_eq!(input.wheel(), Vec2::ZERO);
        assert!(input.held("move"));
    }
    #[test]
    fn hover_keeps_pointer_motion_and_key_edges_without_suppressing_drag_edges() {
        let mut input = Input::new(
            Actions::new()
                .stick("move", Stick::wasd().or_touch(Region::Left))
                .button("jump", &["Space"]),
        );
        input.viewport = Vec2::new(640., 480.);
        for (code, down) in [("Space", true), ("Space", false), ("KeyD", true)] {
            input.apply(InputEvent::Key {
                code: code.into(),
                down,
                at_ms: 0.,
            });
        }
        let last = std::cell::Cell::new(Vec2::new(20., 40.));
        let pointer = |phase, x, y| {
            let to = Vec2::new(x, y);
            let by = to - last.get();
            if to.is_finite() {
                last.set(to);
            }
            InputEvent::Pointer {
                id: 1,
                phase,
                x,
                y,
                dx: by.x,
                dy: by.y,
                buttons: u32::from(phase == PointerPhase::Down),
                at_ms: 0.,
            }
        };
        for (x, y) in [(20., 40.), (50., 70.), (f32::NAN, 70.), (80., 90.)] {
            input.apply(pointer(PointerPhase::Move, x, y));
        }
        let p = input.pointer().unwrap();
        assert_eq!(p.position, Vec2::new(80., 90.));
        assert_eq!(p.delta, Vec2::new(60., 50.));
        assert!(!p.down);
        assert!(input.pressed("jump") && input.released("jump") && !input.held("jump"));
        assert!(input.pressed("move") && input.held("move") && !input.released("move"));
        input.clear_edges();
        assert_eq!(input.pointer().unwrap().delta, Vec2::ZERO);
        assert!(!input.pressed("jump") && !input.released("jump"));
        input.apply(pointer(PointerPhase::Down, 80., 90.));
        input.apply(pointer(PointerPhase::Move, 20., 90.));
        assert!(input.pointer().unwrap().down);
        assert!(input.released("move") && !input.held("move"));
        assert_eq!(input.stick_xz("move"), crate::Vec3::ZERO);
        input.apply(pointer(PointerPhase::Move, 80., 90.));
        assert!(input.pressed("move") && input.released("move") && input.held("move"));
        assert_eq!(input.stick_xz("move"), crate::Vec3::X);
        input.apply(pointer(PointerPhase::Up, 80., 90.));
        input.apply(pointer(PointerPhase::Move, 90., 100.));
        assert!(!input.pointer().unwrap().down);
        assert_eq!(input.pointer().unwrap().delta, Vec2::new(10., 10.));
        assert!(input.pressed("move") && input.released("move") && input.held("move"));
    }
    #[test]
    fn mouse_buttons_bind_as_keys_and_motion_is_the_devices() {
        let mut input = Input::new(
            Actions::new()
                .button("fire", &["MouseLeft"])
                .button("aim", &["MouseRight"])
                .button("ping", &["MouseMiddle"]),
        );
        let mouse = |phase, dx, buttons| InputEvent::Pointer {
            id: 1,
            phase,
            x: 10.,
            y: 10.,
            dx,
            dy: -1.,
            buttons,
            at_ms: 0.,
        };
        // A locked pointer: the position stays, the motion is unbounded.
        for _ in 0..3 {
            input.apply(mouse(PointerPhase::Move, 500., 0));
        }
        let p = input.pointer().unwrap();
        assert_eq!(
            (p.position, p.delta),
            (Vec2::new(10., 10.), Vec2::new(1500., -3.))
        );
        input.clear_edges();
        // The secondary button alone begins the contact, as the web's pointerdown.
        input.apply(mouse(PointerPhase::Down, 0., 2));
        assert!(input.pressed("aim") && input.held("aim") && !input.held("fire"));
        input.clear_edges();
        // Chorded buttons change on moves.
        input.apply(mouse(PointerPhase::Move, 2., 3));
        input.apply(mouse(PointerPhase::Move, 2., 7));
        assert!(input.pressed("fire") && input.pressed("ping") && !input.pressed("aim"));
        assert!(input.held("aim") && input.held("fire") && input.held("ping"));
        input.clear_edges();
        input.apply(mouse(PointerPhase::Move, 2., 6));
        assert!(input.released("fire") && input.held("aim"));
        input.apply(mouse(PointerPhase::Up, 0., 0));
        assert!(input.released("aim") && input.released("ping"));
        assert!(!input.held("aim") && !input.held("fire") && !input.held("ping"));
        input.clear_edges();
        // A hover move with a button held (pressed off the canvas) holds it;
        // blur releases every device.
        input.apply(mouse(PointerPhase::Move, 1., 2));
        assert!(input.pressed("aim") && input.held("aim"));
        input.apply(InputEvent::Blur { at_ms: 0. });
        assert!(!input.held("aim"));
    }
    #[test]
    fn repeated_keys_preserve_edges_and_independent_control_ownership() {
        let mut input = Input::new(Actions::new().button("jump", &["Space", "KeyJ"]));
        let key = |code: &str, down, at_ms| InputEvent::Key {
            code: code.into(),
            down,
            at_ms,
        };
        input.apply(key("Space", true, 0.));
        assert!(input.held("jump") && input.pressed("jump") && !input.released("jump"));
        let saved = crate::bin::to_vec(&input);
        for at_ms in [1., f64::NAN] {
            input.apply(key("Space", true, at_ms));
            assert_eq!(crate::bin::to_vec(&input), saved);
        }
        input.clear_edges();
        input.apply(key("Space", true, 2.));
        assert!(input.held("jump") && !input.pressed("jump") && !input.released("jump"));
        input.apply(key("KeyJ", true, 3.));
        input.apply(key("Space", false, 4.));
        assert!(input.held("jump") && !input.pressed("jump") && !input.released("jump"));
        let saved = crate::bin::to_vec(&input);
        for code in ["Space", "KeyZ"] {
            input.apply(key(code, false, 5.));
            assert_eq!(crate::bin::to_vec(&input), saved);
        }
        input.apply(control("jump", 1, PointerPhase::Down, 0., 0.));
        input.apply(key("KeyJ", false, 6.));
        input.apply(key("KeyJ", false, 7.));
        assert!(input.held("jump") && !input.pressed("jump") && !input.released("jump"));
        input.apply(control("jump", 1, PointerPhase::Up, 0., 0.));
        assert!(!input.held("jump") && !input.pressed("jump") && input.released("jump"));
        let saved = crate::bin::to_vec(&input);
        input.apply(key("KeyJ", false, 8.));
        assert_eq!(crate::bin::to_vec(&input), saved);
        input.clear_edges();
        input.apply(key("Space", true, 9.));
        assert!(input.held("jump") && input.pressed("jump") && !input.released("jump"));
    }
    fn control(name: &str, id: u64, phase: PointerPhase, x: f32, y: f32) -> InputEvent {
        InputEvent::Control {
            name: name.into(),
            id,
            phase,
            x,
            y,
            at_ms: 0.0,
        }
    }
    fn input() -> Input {
        Input::new(
            Actions::new()
                .stick("move", Stick::wasd())
                .button("jump", &["Space"])
                .button("light", &["KeyE"]),
        )
    }
    #[test]
    fn cloned_inputs_keep_keys_contacts_and_edges_independent() {
        fn send_sync<T: Send + Sync>() {}
        send_sync::<Input>();
        let mut original = input();
        original.apply(InputEvent::Key {
            code: "Space".into(),
            down: true,
            at_ms: 0.,
        });
        original.apply(control("move", 1, PointerPhase::Down, 10., 10.));
        original.apply(control("move", 1, PointerPhase::Move, 70., 10.));
        let saved = crate::bin::to_vec(&original);
        let mut copy = original.clone();
        copy.apply(InputEvent::Blur { at_ms: 0. });
        assert!(!copy.held("jump"));
        assert!(copy.released("jump"));
        assert_eq!(copy.stick_xz("move"), crate::Vec3::ZERO);
        assert!(original.held("jump") && original.pressed("jump"));
        assert_eq!(original.stick_xz("move"), crate::Vec3::X);
        assert_eq!(crate::bin::to_vec(&original), saved);
        original.clear_edges();
        assert!(!original.pressed("jump"));
        assert!(copy.released("jump"));
        copy.restore_dynamic(crate::bin::from_slice(&saved).unwrap());
        assert!(copy.held("jump") && !copy.pressed("jump"));
        assert_eq!(copy.stick_xz("move"), crate::Vec3::X);
    }
    #[test]
    fn app_controls_share_actions_with_keys_and_release_independently() {
        let mut input = input();
        input.apply(control("jump", 1, PointerPhase::Down, 0., 0.));
        assert!(input.pressed("jump") && input.held("jump"));
        input.clear_edges();
        input.apply(InputEvent::Key {
            code: "Space".into(),
            down: true,
            at_ms: 0.,
        });
        assert!(!input.pressed("jump"));
        input.apply(control("jump", 1, PointerPhase::Cancel, 0., 0.));
        assert!(input.held("jump") && !input.released("jump"));
        input.apply(InputEvent::Key {
            code: "Space".into(),
            down: false,
            at_ms: 0.,
        });
        assert!(input.released("jump") && !input.held("jump"));
        input.apply(control("light", 2, PointerPhase::Down, 0., 0.));
        input.apply(control("light", 2, PointerPhase::Up, 0., 0.));
        assert!(input.pressed("light") && input.released("light"));
        input.apply(control("light", 3, PointerPhase::Down, 0., 0.));
        input.apply(control("jump", 4, PointerPhase::Down, 0., 0.));
        input.apply(control("jump", 5, PointerPhase::Down, 0., 0.));
        assert_eq!(input.held_controls(), ["jump", "light"]);
        input.apply(control("jump", 4, PointerPhase::Up, 0., 0.));
        assert_eq!(input.held_controls(), ["jump", "light"]);
        input.apply(control("jump", 5, PointerPhase::Cancel, 0., 0.));
        assert_eq!(input.held_controls(), ["light"]);
        input.apply(control("light", 3, PointerPhase::Up, 0., 0.));
        assert!(input.held_controls().is_empty());
    }
    #[test]
    fn app_stick_is_local_unit_clamped_saved_and_cancelled_on_blur() {
        let mut input = input();
        input.apply(control("move", 1, PointerPhase::Down, 40., 40.));
        input.apply(control("move", 1, PointerPhase::Move, 100., 40.));
        input.apply(control("jump", 2, PointerPhase::Down, 0., 0.));
        assert_eq!(input.stick_xz("move"), crate::Vec3::X);
        assert!(input.held("jump"));
        let saved = crate::bin::to_vec(&input);
        let mut restored = self::input();
        restored.restore_dynamic(crate::bin::from_slice(&saved).unwrap());
        assert_eq!(restored.stick_xz("move"), crate::Vec3::X);
        assert!(restored.held("jump") && !restored.pressed("jump"));
        restored.apply(control("move", 1, PointerPhase::Move, 100., -20.));
        let direction = restored.stick_xz("move");
        assert!((direction.length() - 1.).abs() < 1e-6 && direction.z < 0.);
        restored.apply(InputEvent::Blur { at_ms: 0. });
        assert_eq!(restored.stick_xz("move"), crate::Vec3::ZERO);
        assert!(!restored.held("jump"));
    }

    #[test]
    fn queued_ui_contact_continues_identically_in_a_fresh_simulation() {
        use crate::{Clock, Game, Sim, Transform, World};
        struct Controlled;
        impl Game for Controlled {
            const ID: &'static str = "ui-control-test";
            const HZ: u32 = 60;
            type Args = ();
            fn actions() -> Actions {
                input().actions().clone()
            }
            fn setup(w: &mut World, _: &()) {
                w.spawn_named("player", (Transform::default(),));
            }
            fn tick(w: &mut World, input: &Input, _: &()) {
                let mut t = w.get_mut::<Transform>("player").unwrap();
                t.position += input.stick_xz("move");
                if input.pressed("jump") {
                    t.position.y += 1.;
                }
            }
        }
        for mode in [
            crate::Paranoid::Off,
            crate::Paranoid::Save,
            crate::Paranoid::FreshGame,
        ] {
            let mut sim = Sim::<Controlled>::new(()).unwrap().paranoid(mode);
            sim.advance(1000., Clock::Seekable);
            let mut down = control("move", 1, PointerPhase::Down, 0., 0.);
            down.set_at_ms(800.); // Past stamps, like keys, become due now.
            sim.input(down);
            let mut motion = control("move", 1, PointerPhase::Move, 60., 0.);
            motion.set_at_ms(900.);
            sim.input(motion);
            sim.advance(1100., Clock::Seekable);
            assert_eq!(sim.get::<Transform>("player").unwrap().position.x, 6.);
            assert_eq!(sim.held_controls(), ["move"]);
            let saved = sim.save().unwrap(); // Contact is held inside Input, not just queued.
            let mut restored = Sim::<Controlled>::new(()).unwrap().paranoid(mode);
            restored.restore(&saved).unwrap();
            restored.advance(1100., Clock::Seekable);
            for game in [&mut sim, &mut restored] {
                let mut jump = control("jump", 2, PointerPhase::Down, 0., 0.);
                jump.set_at_ms(1200.);
                game.input(jump);
                game.advance(1200., Clock::Seekable);
                assert_eq!(game.get::<Transform>("player").unwrap().position.y, 0.);
                game.advance(1217., Clock::Seekable);
                assert_eq!(game.get::<Transform>("player").unwrap().position.y, 1.);
                assert!(game
                    .agent(r#"{"op":"state"}"#)
                    .contains(r#""controls":["jump","move"]"#));
                let bad = control("typo", 3, PointerPhase::Down, 0., 0.);
                assert!(game.validate_input(&bad).unwrap_err().contains("typo"));
                game.input(bad); // Never panics even for direct callers.
                let mut invalid = control("jump", 4, PointerPhase::Down, 0., 0.);
                invalid.set_at_ms(f64::NAN);
                assert!(game.validate_input(&invalid).is_err());
                game.input(invalid);
                game.input(InputEvent::Blur { at_ms: 1217. });
                game.advance(1300., Clock::Seekable);
                assert!(game.held_controls().is_empty());
            }
            assert_eq!(sim.save().unwrap(), restored.save().unwrap());
        }
    }
}
