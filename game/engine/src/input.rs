use crate::{Data, Vec2};

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
            (Self::Pointer { id: a, .. }, Self::Pointer { id: b, .. }) => a == b,
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
    pub(crate) fn set_at_ms(&mut self, value: f64) {
        match self {
            Self::Key { at_ms, .. }
            | Self::Pointer { at_ms, .. }
            | Self::Control { at_ms, .. }
            | Self::Wheel { at_ms, .. }
            | Self::Blur { at_ms } => *at_ms = value,
        }
    }
    pub(crate) fn at_ms(&self) -> f64 {
        match self {
            Self::Key { at_ms, .. }
            | Self::Pointer { at_ms, .. }
            | Self::Control { at_ms, .. }
            | Self::Wheel { at_ms, .. }
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
    /// Movement accumulated since the last tick.
    pub delta: Vec2,
}
#[derive(Clone, Default, Data)]
struct Contact {
    id: u64,
    action: String,
    origin: Vec2,
    position: Vec2,
}
/// Tick-local action edges and accumulated device state. Only Sim mutates it.
#[derive(Clone, Default, Data)]
pub struct Input {
    #[data(skip)]
    pub(crate) actions: Actions,
    pub(crate) keys: Vec<String>,
    pressed: Vec<String>,
    released: Vec<String>,
    pointer: Option<PointerState>,
    contacts: Vec<Contact>,
    wheel: Vec2,
    pub(crate) viewport: Vec2,
}
impl Input {
    pub(crate) fn new(actions: Actions) -> Self {
        Self {
            actions,
            ..Self::default()
        }
    }
    fn action(&self, name: &str) -> Option<&Action> {
        let a = self.actions.entries.iter().find(|a| a.name == name);
        assert!(
            a.is_some(),
            "unknown action `{name}`; declared actions: {}",
            self.actions
                .entries
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        a
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
        self.action(name).is_some_and(|a| self.active(a))
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
            .and_then(|a| a.stick.as_ref())
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
    pub(crate) fn clear_edges(&mut self) {
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
    pub(crate) fn apply(&mut self, event: InputEvent) {
        let before: Vec<_> = self
            .actions
            .entries
            .iter()
            .map(|a| self.active(a))
            .collect();
        self.apply_state(event);
        for (a, was) in self.actions.entries.iter().zip(before) {
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
        match event {
            InputEvent::Key { code, down, .. } => match (self.keys.binary_search(&code), down) {
                (Err(i), true) => self.keys.insert(i, code),
                (Ok(i), false) => {
                    self.keys.remove(i);
                }
                _ => {}
            },
            InputEvent::Wheel { dx, dy, .. } => self.wheel += Vec2::new(dx, dy),
            InputEvent::Control {
                name,
                id,
                phase,
                x,
                y,
                ..
            } => {
                self.action(&name);
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
                id, phase, x, y, ..
            } => {
                let position = Vec2::new(x, y);
                if let Some(p) = &mut self.pointer {
                    if p.id == id {
                        p.delta += position - p.position;
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
                        delta: Vec2::ZERO,
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
                input().actions
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
        let mut sim = Sim::<Controlled>::new(()).unwrap();
        sim.advance(0., Clock::Seekable);
        sim.input(control("move", 1, PointerPhase::Down, 0., 0.));
        sim.input(control("move", 1, PointerPhase::Move, 30., 0.));
        sim.input(control("move", 1, PointerPhase::Move, 60., 0.));
        sim.input(control("jump", 2, PointerPhase::Down, 0., 0.));
        sim.input(control("jump", 2, PointerPhase::Up, 0., 0.));
        let saved = sim.save().unwrap();
        let mut restored = Sim::<Controlled>::new(()).unwrap();
        restored.restore(&saved).unwrap();
        restored.advance(0., Clock::Seekable);
        for game in [&mut sim, &mut restored] {
            game.advance(100., Clock::Seekable);
            assert_eq!(
                game.get::<Transform>("player").unwrap().position,
                crate::Vec3::new(6., 1., 0.)
            );
            game.input(control("move", 1, PointerPhase::Cancel, 60., 0.));
            game.advance(200., Clock::Seekable);
            assert_eq!(
                game.get::<Transform>("player").unwrap().position,
                crate::Vec3::new(6., 1., 0.)
            );
        }
        assert_eq!(sim.save().unwrap(), restored.save().unwrap());
    }
}
