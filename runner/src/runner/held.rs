//! The facts a development host holds over the app's own (LLP 1122 T7,
//! theme2's F2 ruling): in a copy the design tool pins, the app's
//! `setScheme`, `setMode` on a pinned axis and `setRootFontSize` are kept,
//! not applied, until the pin is handed back; then the app's latest choice
//! of each takes effect. What the app last asked of each is kept whether
//! held or not, so a copy pinned after its launch knows it too.
use super::{Command, Runner, RunnerError};
use exact_kernel::CommitReceipt;
use exact_plan::Value;
use std::collections::BTreeMap;

/// What is held: the scheme, each pinned axis (at the pin's value), the root
/// font size; and what the app last asked of each.
#[derive(Default)]
pub(crate) struct Held {
    scheme: bool,
    modes: BTreeMap<String, String>,
    root: bool,
    asked: Asked,
}

/// The app's last `setScheme`, `setMode` on each axis, and `setRootFontSize`,
/// each as it wrote it (`"system"`, `"medium"`, `24`, `"125%"`).
#[derive(Default, Clone, PartialEq, Debug)]
pub struct Asked {
    /// The last `setScheme`'s argument.
    pub scheme: Option<Value>,
    /// The last `setMode` value of each axis.
    pub modes: BTreeMap<String, String>,
    /// The last `setRootFontSize`'s argument.
    pub root: Option<Value>,
}

/// A hold: the scheme, the pinned axes at their values, the root size.
pub type Hold<'a> = (bool, &'a [(&'a str, &'a str)], bool);

impl<D: super::DataSource> Runner<D> {
    /// Holds the scheme, the axes `modes` names and the root font size as
    /// the host has them now, over the app's own; `None` hands them back,
    /// each held fact then the app's latest choice of it, in one commit
    /// (none when the app asked nothing of them). An axis or value the
    /// design lacks is refused, holding nothing.
    pub fn hold_facts(
        &mut self,
        hold: Option<Hold<'_>>,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        if let Some((scheme, modes, root)) = hold {
            if !modes.is_empty() {
                self.check_modes(modes)?;
            }
            self.held.scheme = scheme;
            self.held.modes = modes
                .iter()
                .map(|(a, v)| ((*a).to_owned(), (*v).to_owned()))
                .collect();
            self.held.root = root;
            // (The app's own size gives way to the host's, the pinned one, until handed back.)
            if root {
                self.drop_app_root();
            }
            return Ok(None);
        }
        let held = std::mem::take(&mut self.held.modes);
        let (scheme, root) = (
            std::mem::take(&mut self.held.scheme),
            std::mem::take(&mut self.held.root),
        );
        let asked = self.held.asked.clone();
        let mut moved = false;
        // Each axis the app chose while it was held, at that choice.
        let pins: Vec<(&str, &str)> = held
            .keys()
            .filter_map(|a| {
                asked
                    .modes
                    .get_key_value(a)
                    .map(|(a, v)| (a.as_str(), v.as_str()))
            })
            .collect();
        if let Some(tuple) = self.window_with(&pins).filter(|_| !pins.is_empty()) {
            moved |= self.set_window(tuple);
        }
        // The scheme and root size as the app last asked: its own command again, for the host that mirrors it.
        if let (true, Some(s)) = (scheme, asked.scheme) {
            self.commands.push(Command {
                name: "setScheme".into(),
                args: vec![s],
                source: None,
            });
            moved = true;
        }
        if let (true, Some(r)) = (root, asked.root) {
            if self.app_root_font_size(std::slice::from_ref(&r)) {
                self.commands.push(Command {
                    name: "setRootFontSize".into(),
                    args: vec![r],
                    source: None,
                });
                moved = true;
            }
        }
        if !moved {
            return Ok(None);
        }
        self.commit_again(Vec::new(), "held facts handed back")
            .map(Some)
    }

    /// What the app last asked of the facts a host may hold.
    pub fn asked(&self) -> &Asked {
        &self.held.asked
    }

    /// The app's `setMode(axis, value)` on an axis the host holds: kept as
    /// asked, and not applied. Whether it was held.
    pub(super) fn held_mode(&mut self, axis: &str, value: &str) -> bool {
        self.held
            .asked
            .modes
            .insert(axis.to_owned(), value.to_owned());
        self.held.modes.contains_key(axis)
    }

    /// The commit's `setScheme`s and `setRootFontSize`s from `from` on,
    /// each kept as asked; one the host holds taken out, not applied.
    pub(super) fn take_held(&mut self, from: usize) {
        let mut i = from;
        while i < self.commands.len() {
            let (name, arg) = (
                self.commands[i].name.as_str(),
                self.commands[i].args.first().cloned(),
            );
            let held = match name {
                "setScheme" => {
                    self.held.asked.scheme = arg;
                    self.held.scheme
                }
                "setRootFontSize" => {
                    self.held.asked.root = arg;
                    self.held.root
                }
                _ => false,
            };
            if held {
                self.commands.remove(i);
            } else {
                i += 1;
            }
        }
    }
}
