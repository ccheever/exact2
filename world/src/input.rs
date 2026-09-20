use crate::{Data, DataError, Reader, Writer};
use std::{borrow::Cow, collections::BTreeMap};
/// Static action declaration; no strings are allocated at construction.
#[derive(Clone, Copy, Debug)]
pub struct Action {
    pub name: &'static str,
    pub keys: &'static [&'static str],
    pub axis_keys: Option<(&'static str, &'static str)>,
}
impl Action {
    pub const fn button(name: &'static str, keys: &'static [&'static str]) -> Self {
        Self {
            name,
            keys,
            axis_keys: None,
        }
    }
    pub const fn axis(name: &'static str, negative: &'static str, positive: &'static str) -> Self {
        Self {
            name,
            keys: &[],
            axis_keys: Some((negative, positive)),
        }
    }
}
#[derive(Clone, Debug, Data)]
pub enum InputEvent {
    Key {
        code: String,
        down: bool,
        at_ms: f64,
    },
    Action {
        name: String,
        down: bool,
        at_ms: f64,
    },
    Axis {
        name: String,
        value: f32,
        at_ms: f64,
    },
    Blur {
        at_ms: f64,
    },
}
impl Default for InputEvent {
    fn default() -> Self {
        Self::Blur { at_ms: 0. }
    }
}
impl InputEvent {
    pub fn at_ms(&self) -> f64 {
        match self {
            Self::Key { at_ms, .. }
            | Self::Action { at_ms, .. }
            | Self::Axis { at_ms, .. }
            | Self::Blur { at_ms } => *at_ms,
        }
    }
}
type Declarations = &'static [Action];
type StaticText = Cow<'static, str>;
/// Saved tick-boundary input. Declarations are reconstructed, never decoded from a save.
#[derive(Clone, Default)]
pub struct Input {
    actions: Declarations,
    keys: Vec<String>,
    buttons: Vec<String>,
    axes: BTreeMap<String, f32>,
    pressed: Vec<StaticText>,
    released: Vec<StaticText>,
}
impl Data for Input {
    fn write(&self, w: &mut dyn Writer) {
        w.begin_struct();
        w.field("keys");
        self.keys.write(w);
        w.field("buttons");
        self.buttons.write(w);
        w.field("axes");
        self.axes.write(w);
        w.field("pressed");
        self.pressed.write(w);
        w.field("released");
        self.released.write(w);
        w.end_struct();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        use crate::data::limits::{read_map, read_vec};
        self.actions = &[];
        r.begin_struct()?;
        while let Some(field) = r.field()? {
            match field {
                "keys" => read_vec(r, &mut self.keys, 16)?,
                "buttons" => read_vec(r, &mut self.buttons, 64)?,
                "axes" => read_map(r, &mut self.axes, 64, 128)?,
                "pressed" => read_vec(r, &mut self.pressed, 64)?,
                "released" => read_vec(r, &mut self.released, 64)?,
                _ => r.skip()?,
            }
        }
        Ok(())
    }
}
impl Input {
    pub(crate) fn new(actions: &'static [Action]) -> Result<Self, DataError> {
        if actions.len() > 64 {
            return Err(DataError::new("action limit (64)"));
        }
        for (i, a) in actions.iter().enumerate() {
            if a.name.len() > 128
                || a.keys.len() > 8
                || a.keys.iter().any(|k| k.len() > 128)
                || a.axis_keys
                    .is_some_and(|(a, b)| a.len() > 128 || b.len() > 128)
                || actions[..i].iter().any(|old| old.name == a.name)
            {
                return Err(DataError::new(
                    "invalid action declarations (8 bindings per action, 128 bytes per name/key)",
                ));
            }
        }
        Ok(Self {
            actions,
            ..Self::default()
        })
    }
    fn action(&self, name: &str) -> &Action {
        self.actions
            .iter()
            .find(|a| a.name == name)
            .unwrap_or_else(|| panic!("unknown action `{name}`"))
    }
    pub fn key(&self, code: &str) -> bool {
        self.keys.iter().any(|k| k == code)
    }
    pub fn held(&self, name: &str) -> bool {
        let a = self.action(name);
        self.buttons.iter().any(|b| b == name)
            || a.keys.iter().any(|k| self.key(k))
            || (a.axis_keys.is_some() && self.axis(name) != 0.)
    }
    pub fn axis(&self, name: &str) -> f32 {
        let a = self.action(name);
        let keyboard = a.axis_keys.map_or(0., |(negative, positive)| {
            i32::from(self.key(positive)) as f32 - i32::from(self.key(negative)) as f32
        });
        (keyboard + self.axes.get(name).copied().unwrap_or(0.)).clamp(-1., 1.)
    }
    pub fn pressed(&self, name: &str) -> bool {
        self.action(name);
        self.pressed.iter().any(|n| n == name)
    }
    pub fn released(&self, name: &str) -> bool {
        self.action(name);
        self.released.iter().any(|n| n == name)
    }
    pub(crate) fn validate_event(&self, event: &InputEvent) -> Result<(), DataError> {
        if !event.at_ms().is_finite()
            || event.at_ms() < 0.
            || event.at_ms() > (i64::MAX / 1000) as f64
        {
            return Err(DataError::new("invalid input timestamp"));
        }
        match event {
            InputEvent::Key { code, .. } if code.len() > 128 => {
                return Err(DataError::new("key exceeds 128 bytes"))
            }
            InputEvent::Axis { name, value, .. }
                if !value.is_finite()
                    || value.abs() > 1.
                    || !self
                        .actions
                        .iter()
                        .any(|a| a.name == name && a.axis_keys.is_some()) =>
            {
                return Err(DataError::new("invalid scalar axis"))
            }
            InputEvent::Action { name, .. } if !self.actions.iter().any(|a| a.name == name) => {
                return Err(DataError::new("unknown action"))
            }
            _ => {}
        }
        Ok(())
    }
    pub(crate) fn clear_edges(&mut self) {
        self.pressed.clear();
        self.released.clear();
    }
    /// Sim admission guarantees at most 16 keys; preflight borrows their names.
    /// Buttons and axes are already bounded by the 64 unique declarations.
    pub(crate) fn preflight<'a>(
        &'a self,
        events: impl Iterator<Item = &'a InputEvent>,
    ) -> Result<(), DataError> {
        let mut keys = [None; 16];
        for (slot, key) in keys.iter_mut().zip(&self.keys) {
            *slot = Some(key.as_str());
        }
        for event in events {
            self.validate_event(event)?;
            match event {
                InputEvent::Blur { .. } => keys.fill(None),
                InputEvent::Key { code, down, .. } => {
                    match (
                        keys.iter_mut().find(|key| **key == Some(code.as_str())),
                        down,
                    ) {
                        (Some(key), false) => *key = None,
                        (None, true) => {
                            *keys
                                .iter_mut()
                                .find(|key| key.is_none())
                                .ok_or_else(|| DataError::new("held input limit (16)"))? =
                                Some(code)
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    /// Only consume batches admitted by preflight; moving events preserves heap ownership.
    pub(crate) fn apply(&mut self, event: InputEvent) {
        let before = self.actions.iter().enumerate().fold(0u64, |bits, (i, a)| {
            bits | (u64::from(self.held(a.name)) << i)
        });
        match event {
            InputEvent::Key { code, down, .. } => set(&mut self.keys, code, down),
            InputEvent::Action { name, down, .. } => set(&mut self.buttons, name, down),
            InputEvent::Axis { name, value, .. } => {
                self.axes.insert(name, value);
            }
            InputEvent::Blur { .. } => {
                self.keys.clear();
                self.buttons.clear();
                self.axes.clear();
            }
        }
        for (i, a) in self.actions.iter().enumerate() {
            let now = self.held(a.name);
            let was = before & (1 << i) != 0;
            if now && !was && !self.pressed.iter().any(|n| n == a.name) {
                self.pressed.push(a.name.into());
            }
            if was && !now && !self.released.iter().any(|n| n == a.name) {
                self.released.push(a.name.into());
            }
        }
    }
    pub(crate) fn validate_saved(&mut self, actions: &'static [Action]) -> Result<(), DataError> {
        self.actions = Self::new(actions)?.actions;
        if self.keys.len() > 16
            || self.buttons.len() > 64
            || self.axes.len() > 64
            || self.pressed.len() > 64
            || self.released.len() > 64
        {
            return Err(DataError::new("input state limit"));
        }
        for keys in [&self.keys, &self.buttons] {
            if keys.windows(2).any(|p| p[0] >= p[1]) || keys.iter().any(|k| k.len() > 128) {
                return Err(DataError::new("invalid input set"));
            }
        }
        for name in self
            .buttons
            .iter()
            .map(String::as_str)
            .chain(self.pressed.iter().map(|s| s.as_ref()))
            .chain(self.released.iter().map(|s| s.as_ref()))
        {
            if !actions.iter().any(|a| a.name == name) {
                return Err(DataError::new("unknown saved action"));
            }
        }
        for (name, value) in &self.axes {
            self.validate_event(&InputEvent::Axis {
                name: name.clone(),
                value: *value,
                at_ms: 0.,
            })?;
        }
        Ok(())
    }
}
fn set(values: &mut Vec<String>, value: String, down: bool) {
    match (values.binary_search(&value), down) {
        (Err(i), true) => {
            values.insert(i, value);
        }
        (Ok(i), false) => {
            values.remove(i);
        }
        _ => {}
    }
}
/// Contact offset from a module-owned anchor, 60-point radius, positive Y upward.
/// The kernel owns the offset-to-axis rule; modules own contacts and anchoring.
pub fn stick_axis(origin: [f32; 2], position: [f32; 2]) -> Result<[f32; 2], DataError> {
    if !origin.into_iter().chain(position).all(f32::is_finite) {
        return Err(DataError::new("non-finite stick coordinate"));
    }
    let x = (position[0] - origin[0]) / 60.;
    let y = (origin[1] - position[1]) / 60.;
    let squared = x * x + y * y;
    if !squared.is_finite() {
        return Err(DataError::new("stick offset outside finite range"));
    }
    let scale = if squared > 1. {
        1. / libm::sqrtf(squared)
    } else {
        1.
    };
    Ok([x * scale, y * scale])
}

#[cfg(test)]
mod admission_tests {
    use super::*;
    #[test]
    fn hardware_input_limits_refuse_before_applying_or_restoring() {
        const BAD: &[Action] = &[Action::button("too-many", &["a"; 9])];
        assert!(Input::new(BAD).is_err());
        const GOOD: &[Action] = &[Action::button("ok", &["a"; 8])];
        let input = Input::new(GOOD).unwrap();
        let events: Vec<_> = (0..17)
            .map(|i| InputEvent::Key {
                code: format!("key{i}"),
                down: true,
                at_ms: 0.,
            })
            .collect();
        input.preflight(events[..16].iter()).unwrap();
        assert!(input
            .preflight(events.iter())
            .unwrap_err()
            .message
            .contains("16"));
        assert!(input.keys.is_empty());
        let mut hostile = Input {
            keys: (0..17).map(|i| format!("key{i:02}")).collect(),
            ..Input::default()
        };
        assert!(hostile.validate_saved(&[]).is_err());
        let bytes = crate::bin::to_vec(&hostile).unwrap();
        assert!(crate::bin::from_slice::<Input>(&bytes).is_err());
    }
    #[test]
    #[ignore = "worst admitted input batch timing"]
    fn worst_admitted_input_batch() {
        let key = |suffix| format!("{}{:03}", "x".repeat(125), suffix);
        let bindings = Box::leak(
            (0..8)
                .map(|i| &*Box::leak(key(i).into_boxed_str()))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let actions = Box::leak(
            (0..64)
                .map(|i| Action::button(Box::leak(key(i).into_boxed_str()), bindings))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let mut input = Input::new(actions).unwrap();
        for i in 8..24 {
            input.apply(InputEvent::Key {
                code: key(i),
                down: true,
                at_ms: 0.,
            });
        }
        let events: Vec<_> = (0..1024)
            .map(|_| InputEvent::Key {
                code: key(23),
                down: true,
                at_ms: 0.,
            })
            .collect();
        let start = std::time::Instant::now();
        input.preflight(events.iter()).unwrap();
        for event in events {
            input.apply(event);
        }
        println!(
            "worst_input_64_actions_8_bindings_16_keys_1024_events: {:?}",
            start.elapsed()
        );
        assert_eq!(input.keys.len(), 16);
        assert!(input.pressed.is_empty());
        input.apply(InputEvent::Key {
            code: key(8),
            down: false,
            at_ms: 0.,
        });
        input.apply(InputEvent::Key {
            code: key(7),
            down: true,
            at_ms: 0.,
        });
        assert_eq!(
            input.pressed.len(),
            64,
            "nonmatching benchmark still computes real edges"
        );
    }
}
