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
        use crate::values::quote;
        let fields: Vec<_> = self
            .entries
            .iter()
            .map(|a| {
                let mut w = crate::json::Encoder::rounded();
                a.write(&mut w);
                format!("{}:{}", quote(&a.name), w.finish().unwrap())
            })
            .collect();
        format!("{{{}}}", fields.join(","))
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
    pub(crate) fn set_at_ms(&mut self, value: f64) {
        match self {
            Self::Key { at_ms, .. }
            | Self::Pointer { at_ms, .. }
            | Self::Wheel { at_ms, .. }
            | Self::Blur { at_ms } => *at_ms = value,
        }
    }
    pub(crate) fn at_ms(&self) -> f64 {
        match self {
            Self::Key { at_ms, .. }
            | Self::Pointer { at_ms, .. }
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
            && self
                .contacts
                .iter()
                .any(|p| regions.iter().any(|r| r.contains(p.origin, self.viewport)))
    }
    fn direction(&self, s: &Stick) -> Vec2 {
        let mut v = Vec2::ZERO;
        let down = |i| s.keys.iter().any(|keys| self.keys.contains(&keys[i]));
        v.x = i32::from(down(3)) as f32 - i32::from(down(2)) as f32;
        v.y = i32::from(down(0)) as f32 - i32::from(down(1)) as f32;
        if self.viewport.min_element() > 0.0 {
            for p in &self.contacts {
                if s.touch.iter().any(|r| r.contains(p.origin, self.viewport)) {
                    v += (p.position - p.origin) * Vec2::new(1.0, -1.0) / 60.0;
                }
            }
        }
        v.clamp_length_max(1.0)
    }
    fn active(&self, a: &Action) -> bool {
        a.keys.iter().any(|k| self.keys.contains(k))
            || self.touching(&a.touch)
            || a.stick
                .as_ref()
                .is_some_and(|s| self.direction(s) != Vec2::ZERO)
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
            .map_or(Vec2::ZERO, |s| self.direction(s))
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
                        self.contacts.retain(|p| p.id != id);
                        self.contacts.push(Contact {
                            id,
                            origin: position,
                            position,
                        });
                    }
                    PointerPhase::Move => {
                        if let Some(p) = self.contacts.iter_mut().find(|p| p.id == id) {
                            p.position = position;
                        }
                    }
                    PointerPhase::Up | PointerPhase::Cancel => self.contacts.retain(|p| p.id != id),
                }
            }
        }
    }
}
