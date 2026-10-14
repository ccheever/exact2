//! The window's app modes (LLP 1121 D5): `setMode(axis, value)` from an
//! action, or the agent's `prefer mode`. A node's tuple is the window's,
//! overridden by the nearest `-exact-mode` pin; a new window tuple sets every
//! themed row again in the commit that changed it, which paints (a colour
//! token moves no geometry). Neither persists across a launch (§9 Q5).

use super::{DataSource, Runner, RunnerError};
use exact_kernel::CommitReceipt;
use exact_plan::Value;

impl<D: DataSource> Runner<D> {
    /// The window's app modes: each axis of the plan with its value.
    pub fn modes(&self) -> Vec<(String, String)> {
        let mut rest = self.ids.theme.window;
        let mut out: Vec<(String, String)> = Vec::new();
        for axis in self.plan.axes.iter().rev() {
            let values: Vec<&str> = self.plan.str(axis.values).split('|').collect();
            let digit = rest as usize % values.len();
            rest /= values.len() as u32;
            out.push((self.plan.str(axis.name).into(), values[digit].into()));
        }
        out.reverse();
        out
    }

    /// The window's tuple with each `(axis, value)` of `pins`; `None` when
    /// one names no axis or value of the plan's, or an axis twice.
    fn window_with(&self, pins: &[(&str, &str)]) -> Option<u32> {
        let axes: Vec<(&str, Vec<&str>)> = self
            .plan
            .axes
            .iter()
            .map(|a| {
                (
                    self.plan.str(a.name),
                    self.plan.str(a.values).split('|').collect(),
                )
            })
            .collect();
        let mut digits = vec![0u32; axes.len()];
        let mut rest = self.ids.theme.window;
        for (i, (_, values)) in axes.iter().enumerate().rev() {
            digits[i] = rest % values.len() as u32;
            rest /= values.len() as u32;
        }
        let mut seen = vec![false; axes.len()];
        for (axis, value) in pins {
            let i = axes.iter().position(|(name, _)| name == axis)?;
            if std::mem::replace(&mut seen[i], true) {
                return None;
            }
            digits[i] = axes[i].1.iter().position(|v| v == value)? as u32;
        }
        Some(
            axes.iter()
                .zip(&digits)
                .fold(0, |tuple, ((_, values), d)| tuple * values.len() as u32 + d),
        )
    }

    /// Set the window's tuple, to be set again in the next visit.
    fn set_window(&mut self, tuple: u32) -> bool {
        let moved = tuple != self.ids.theme.window;
        self.ids.theme.window = tuple;
        self.ids.theme.moved |= moved;
        moved
    }

    /// The commit's `setMode`s from `from` on, each applied and taken out:
    /// the runner's alone, set in this commit's update; no host is told.
    pub(super) fn take_set_modes(&mut self, from: usize) {
        let mut i = from;
        while i < self.commands.len() {
            match self.commands[i].name == "setMode" {
                true => {
                    let args = self.commands.remove(i).args;
                    self.app_set_mode(&args);
                }
                false => i += 1,
            }
        }
    }

    /// The app's `setMode(axis, value)`, in the commit of the action that
    /// called it. `false`, and a journal line, for an axis or value the plan
    /// does not have, which changes nothing.
    pub(super) fn app_set_mode(&mut self, args: &[Value]) -> bool {
        let tuple = match args {
            [axis, value] => axis
                .as_str()
                .zip(value.as_str())
                .and_then(|pin| self.window_with(&[pin])),
            _ => None,
        };
        match tuple {
            Some(tuple) => {
                self.set_window(tuple);
                true
            }
            None => {
                let mut said = String::new();
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        said.push_str(", ");
                    }
                    crate::agent::untyped_json(a, &mut said);
                }
                self.log(format!(
                    "setMode({said}) refused: an axis of the app's design and one of its values"
                ));
                false
            }
        }
    }

    /// Set the app modes `pins` names, each axis to its value, and commit
    /// what they move in one commit (the agent's `prefer mode`, D13): every
    /// pin is checked before any is set. `None` when they are the window's
    /// already.
    pub fn set_modes(
        &mut self,
        pins: &[(&str, &str)],
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let tuple = self
            .window_with(pins)
            .ok_or_else(|| RunnerError::InvalidValue {
                event: "setMode",
                reason: format!(
                    "{} names an axis or value the app's design lacks, or an axis twice",
                    pins.iter()
                        .map(|(a, v)| format!("`{a}:{v}`"))
                        .collect::<Vec<_>>()
                        .join(" ")
                ),
            })?;
        let previous = self.ids.theme.window;
        if !self.set_window(tuple) {
            return Ok(None);
        }
        let result = self.commit_again(Vec::new(), "app mode");
        // A refused commit put the tree back: so is the tuple, set again in
        // the next visit. A poisoned runner keeps the tree it wrote.
        if result.is_err() && !self.poisoned {
            self.ids.theme.window = previous;
            self.ids.theme.moved = true;
        }
        result.map(Some)
    }

    /// [`Runner::set_modes`] for one axis.
    pub fn set_mode(
        &mut self,
        axis: &str,
        value: &str,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        self.set_modes(&[(axis, value)])
    }

    /// The window's modes as JSON, `{"axis": "value", …}`, for a host's reply.
    pub fn modes_json(&self) -> String {
        let mut out = String::from("{");
        for (i, (axis, value)) in self.modes().into_iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            crate::agent::quote(&axis, &mut out);
            out.push(':');
            crate::agent::quote(&value, &mut out);
        }
        out.push('}');
        out
    }
}
