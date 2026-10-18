//! Triggers: actions whose edges the engine computes from the world stamps
//! of the events that moved their keys. Every window is world microseconds,
//! so a seek and a 60 Hz panel fire the same ticks.
use super::{Action, Input};
use crate::Data;

/// What makes an action's edges. Windows are world microseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub(super) enum Trigger {
    /// Held while any binding is down.
    #[default]
    Button,
    /// A press at most this long, firing at its release.
    Tap(i64),
    /// Down this long, firing as it matures.
    Hold(i64),
    /// A second press at most this long after the first.
    Double(i64),
    /// The last key down while the others are held.
    Chord,
}
/// A trigger action mid-gesture. Idle triggers are not kept, so a save holds
/// only live gestures.
#[derive(Clone, Default, Data)]
pub(super) struct TriggerState {
    action: String,
    // Tap and Hold: when the binding went down. Double: the first press.
    at_us: Option<i64>,
    // Hold: matured. Double: the second press is down. Chord: complete.
    on: bool,
}
impl TriggerState {
    pub(super) fn live(&self) -> bool {
        self.at_us.is_some() || self.on
    }
}
pub(super) fn window(seconds: f32) -> i64 {
    assert!(
        seconds.is_finite() && seconds > 0. && seconds <= 60.,
        "a trigger's window must be in (0, 60] seconds, got {seconds}"
    );
    (f64::from(seconds) * 1e6).round() as i64
}
impl Input {
    fn state(&mut self, name: &str) -> &mut TriggerState {
        let i = match self
            .triggers
            .binary_search_by(|t| t.action.as_str().cmp(name))
        {
            Ok(i) => i,
            Err(i) => {
                self.triggers.insert(
                    i,
                    TriggerState {
                        action: name.into(),
                        ..TriggerState::default()
                    },
                );
                i
            }
        };
        &mut self.triggers[i]
    }
    pub(super) fn trigger_on(&self, name: &str) -> bool {
        self.triggers.iter().any(|t| t.action == name && t.on)
    }
    /// A chord's last key, down before the event that completes the chord, is
    /// the wrong order.
    pub(super) fn last_key_down(&self, a: &Action) -> bool {
        a.trigger == Trigger::Chord && self.keys_of(a).last().is_some_and(|k| self.down(k))
    }
    /// One event's effect on a trigger whose binding was `was` before it.
    pub(super) fn step_trigger(&mut self, a: &Action, was: bool, last_was_down: bool, at_us: i64) {
        let now = self.bound(a);
        let name = a.name.as_str();
        match a.trigger {
            Trigger::Button => {}
            Trigger::Tap(window) => {
                if now && !was {
                    self.state(name).at_us = Some(at_us);
                }
                if was && !now {
                    let down = self.state(name).at_us.take();
                    if down.is_some_and(|d| at_us - d <= window) {
                        self.press(name);
                        self.release(name);
                    }
                }
            }
            Trigger::Hold(_) => {
                if now && !was {
                    *self.state(name) = TriggerState {
                        action: name.into(),
                        at_us: Some(at_us),
                        on: false,
                    };
                }
                if was && !now {
                    let state = std::mem::take(self.state(name));
                    if state.on {
                        self.release(name);
                    }
                }
            }
            Trigger::Double(window) => {
                if now && !was {
                    let state = self.state(name);
                    if state.at_us.is_some_and(|first| at_us - first <= window) {
                        state.at_us = None;
                        state.on = true;
                        self.press(name);
                    } else {
                        state.at_us = Some(at_us);
                    }
                }
                if was && !now && std::mem::take(&mut self.state(name).on) {
                    self.release(name);
                }
            }
            Trigger::Chord => {
                if now && !was && !last_was_down {
                    self.state(name).on = true;
                    self.press(name);
                }
                if was && !now && std::mem::take(&mut self.state(name).on) {
                    self.release(name);
                }
            }
        }
    }
    /// Mature every hold whose instant `due` passes, and let taps and first
    /// presses whose windows have closed lapse. Before an event, `due` is the
    /// event's stamp; at a tick's end, the tick's end.
    pub(crate) fn mature(&mut self, due: impl Fn(i64) -> bool) {
        let Some(actions) = self.actions.clone() else {
            return;
        };
        for i in 0..self.triggers.len() {
            let Some(a) = actions.find(&self.triggers[i].action) else {
                continue;
            };
            let state = &mut self.triggers[i];
            let Some(at) = state.at_us else {
                continue;
            };
            match a.trigger {
                Trigger::Hold(window) if due(at + window) => {
                    state.at_us = None;
                    state.on = true;
                    let name = a.name.clone();
                    self.press(&name);
                }
                Trigger::Tap(window) | Trigger::Double(window) if due(at + window + 1) => {
                    state.at_us = None;
                }
                _ => {}
            }
        }
        self.triggers.retain(TriggerState::live);
    }
    /// Forget a trigger's gesture (a context or a binding changed under it).
    pub(super) fn reset_trigger(&mut self, name: &str) {
        self.triggers.retain(|t| t.action != name);
    }
    pub(super) fn validate_triggers(&self, saved: &Self) -> Result<(), String> {
        if saved
            .triggers
            .windows(2)
            .any(|pair| pair[0].action >= pair[1].action)
        {
            return Err("saved triggers must be sorted by unique action".into());
        }
        for name in saved
            .triggers
            .iter()
            .map(|t| &t.action)
            .chain(&saved.latched)
        {
            if self.actions().find(name).is_none() {
                return Err(format!(
                    "saved input names action `{name}`; declared actions: {}",
                    self.actions().names()
                ));
            }
        }
        for t in &saved.triggers {
            let trigger = self.actions().find(&t.action).map(|a| a.trigger);
            if matches!(trigger, Some(Trigger::Button)) {
                return Err(format!(
                    "saved trigger state for `{}`, which is a button",
                    t.action
                ));
            }
        }
        Ok(())
    }
}
