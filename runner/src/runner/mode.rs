//! The window's app modes (LLP 1121 D5): `setMode(axis, value)` from an
//! action, or the agent's `prefer mode`. A node's tuple is the window's,
//! overridden by the nearest `-exact-mode` pin; a new window tuple sets every
//! themed row again in the commit that changed it, which paints (a colour
//! token moves no geometry). Neither persists across a launch (§9 Q5); a
//! reload carries the window's.

use super::{DataSource, Runner, RunnerError};
use exact_kernel::CommitReceipt;
use exact_plan::Value;

impl<D: DataSource> Runner<D> {
    /// The window's app modes: each axis of the plan with its value.
    pub fn modes(&self) -> Vec<(String, String)> {
        self.modes_of(self.ids.theme.window)
    }

    /// Tuple `tuple`'s modes: each axis of the plan with its value.
    fn modes_of(&self, tuple: u32) -> Vec<(String, String)> {
        let mut rest = tuple;
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

    /// A reload's carried window modes: each axis the new plan still has
    /// at that value keeps it; one it lacks, or a value gone, starts again.
    pub(super) fn carry_modes(&mut self, modes: &[(String, String)]) {
        let pins: Vec<(&str, &str)> = modes
            .iter()
            .map(|(a, v)| (a.as_str(), v.as_str()))
            .filter(|pin| self.window_with(&[*pin]).is_some())
            .collect();
        if let Some(tuple) = self.window_with(&pins) {
            self.set_window(tuple);
        }
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
        let tuple = self.checked_modes(pins)?;
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

    /// Whether [`Runner::set_modes`] would take `pins`, changing nothing: a
    /// host's `prefer` asks before it sets any of its facts.
    pub fn check_modes(&self, pins: &[(&str, &str)]) -> Result<(), RunnerError> {
        self.checked_modes(pins).map(|_| ())
    }

    /// The window's tuple `pins` make; refused on a poisoned runner, or for
    /// an axis or value the design lacks, or an axis twice.
    fn checked_modes(&self, pins: &[(&str, &str)]) -> Result<u32, RunnerError> {
        if self.poisoned {
            return Err(RunnerError::Poisoned);
        }
        self.window_with(pins)
            .ok_or_else(|| RunnerError::InvalidValue {
                event: "setMode",
                reason: format!(
                    "{} names an axis or value the app's design lacks, or an axis twice",
                    pins.iter()
                        .map(|(a, v)| format!("`{a}={v}`"))
                        .collect::<Vec<_>>()
                        .join(" ")
                ),
            })
    }

    /// [`Runner::set_modes`] for one axis.
    pub fn set_mode(
        &mut self,
        axis: &str,
        value: &str,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        self.set_modes(&[(axis, value)])
    }

    /// `view`'s `style` row as D7's index holds it, for `layout <node>`:
    /// the tokens it read (the theme's names its text names), its text, the
    /// tuple it was set in as `axis=value` by commas, and whether a boundary
    /// set it (an implied row) rather than a binding. `None` when it read no
    /// token.
    pub fn row_token(
        &self,
        view: exact_kernel::ViewId,
        style: exact_kernel::StyleId,
    ) -> Option<(Vec<String>, String, String, bool)> {
        let row = self.ids.theme.row(view, style)?;
        let text = match &row.text {
            t @ exact_plan::str_value!() => t.text().to_string(),
            other => {
                let mut s = String::new();
                crate::agent::untyped_json(other, &mut s);
                s
            }
        };
        // The entries a parse of its text reads in its tuple, each by its
        // token's name (an override read by a role's spelling too).
        let tuple = crate::bridge::set_tuple(row.tuple);
        let mut scratch = exact_kernel::StyleProps::default();
        let (_, read) = exact_kernel::style::theme::recorded(|| {
            crate::bridge::set_plan_style(&mut scratch, style as u16, &row.text, &self.plan)
        });
        crate::bridge::set_tuple(tuple);
        let mut names: Vec<String> = Vec::new();
        for entry in read {
            let token = self.plan.theme.iter().find(|t| {
                *self.plan.str(t.name) == *entry
                    || self.plan.str(t.role).eq_ignore_ascii_case(&entry)
            });
            let name = token.map_or(entry.to_string(), |t| self.plan.str(t.name).to_string());
            if !names.contains(&name) {
                names.push(name);
            }
        }
        let modes = self
            .modes_of(row.tuple)
            .into_iter()
            .map(|(a, v)| format!("{a}={v}"))
            .collect::<Vec<_>>()
            .join(",");
        Some((names, text, modes, row.implied))
    }

    /// The app modes `view`'s node is in, `[(axis, value)]` (LLP 1121 D5):
    /// the window's, overridden by the pins above it.
    pub fn modes_of_view(&self, view: exact_kernel::ViewId) -> Option<Vec<(String, String)>> {
        self.tree
            .as_ref()
            .and_then(|t| t.tuple_of(view))
            .map(|tuple| self.modes_of(tuple))
    }

    /// Every remembered instance view with its plan site and the plan's
    /// digest, as JSON `{"planDigest": …, "sites": [[view, site], …]}`: a
    /// development page's map from views to sites (LLP 1122 D5's outline),
    /// in one read.
    pub fn view_sites_json(&self) -> String {
        let mut out = String::from("{\"planDigest\":");
        crate::agent::quote(self.inspection_digest(), &mut out);
        out.push_str(",\"sites\":[");
        for (i, (view, site)) in self.ids.sites().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&format!("[{view},{}]", site.0));
        }
        out.push_str("]}");
        out
    }

    /// [`Self::modes_of_view`] as JSON, `{"axis": "value", …}`.
    pub fn modes_of_view_json(&self, view: exact_kernel::ViewId) -> Option<String> {
        let modes = self.modes_of_view(view)?;
        let mut out = String::from("{");
        for (i, (axis, value)) in modes.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            crate::agent::quote(axis, &mut out);
            out.push(':');
            crate::agent::quote(value, &mut out);
        }
        out.push('}');
        Some(out)
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
