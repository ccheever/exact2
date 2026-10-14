//! `theme` (LLP 1095 D4): the app's own colours, named once at its root.
//! Each entry is a colour any row takes, another entry's name or a role's;
//! an entry named like a role overrides it. Entries resolve here, cycles
//! refused, to the colour text each stands for, which is bound while the
//! app's styles are parsed (`exact_kernel::style::theme`), so a row writes
//! `color="brand"` and the kernel reads the entry's colour.

use crate::{LowerError, Lowerer};
use contract_syntax::{Attr, Expr, File};
use exact_kernel::style::roles;
use exact_kernel::{Color, ColorValue};

/// One resolved entry: the names it answers to and the colour text it
/// stands for, which names no entry.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ThemeEntry {
    /// The name as written.
    pub name: String,
    /// The role it overrides, as a row writes it (`-exact-secondary-label`,
    /// `Canvas`).
    pub role: Option<String>,
    /// The colour text.
    pub value: String,
}

/// A role's id, as a row writes it (`-exact-label`, `CanvasText`) or as a
/// theme key may (LLP 1095 D4's bare `secondary-label`).
fn role_named(name: &str) -> Option<(u8, String)> {
    let id = roles::role(name).or_else(|| {
        (!name.starts_with('-'))
            .then(|| roles::role(&format!("-exact-{name}")))
            .flatten()
    })?;
    let role = roles::role_of(id)?;
    let row = if roles::is_css_system(role) {
        role.name.to_string()
    } else {
        format!("-exact-{}", role.name)
    };
    Some((id, row))
}

/// A colour the kernel reads with no theme bound.
fn parses(text: &str) -> bool {
    ColorValue::parse_light_dark(text).is_some() || Color::parse(text).is_some()
}

impl Lowerer<'_> {
    /// Resolve the app's theme and bind it for the styles that follow.
    pub(crate) fn declare_theme(&mut self, file: &File) -> exact_kernel::style::theme::ThemeScope {
        let entries = file.theme.as_ref().map_or(&[][..], |t| &t.entries[..]);
        let mut resolved: Vec<Option<ThemeEntry>> = vec![None; entries.len()];
        for i in 0..entries.len() {
            let mut path = Vec::new();
            self.resolve_entry(entries, i, &mut path, &mut resolved);
        }
        self.theme = resolved.into_iter().flatten().collect();
        for e in &self.theme {
            self.b
                .theme_entry(&e.name, e.role.as_deref().unwrap_or(""), &e.value);
        }
        exact_kernel::style::theme::declarations(self.theme.iter().flat_map(|e| {
            let tint = e
                .role
                .as_deref()
                .and_then(roles::role)
                .is_some_and(roles::tint);
            (!tint)
                .then_some((e.name.as_str(), e.value.as_str(), false))
                .into_iter()
                .chain(
                    e.role
                        .as_deref()
                        .filter(|_| !tint)
                        .map(|r| (r, e.value.as_str(), true)),
                )
        }))
    }

    /// Entry `i`'s colour text, following names; `None` once refused.
    fn resolve_entry(
        &mut self,
        entries: &[Attr],
        i: usize,
        path: &mut Vec<usize>,
        resolved: &mut Vec<Option<ThemeEntry>>,
    ) -> Option<String> {
        if let Some(done) = &resolved[i] {
            return Some(done.value.clone());
        }
        let entry = &entries[i];
        if let Some(at) = path.iter().position(|&p| p == i) {
            let chain: Vec<&str> = path[at..]
                .iter()
                .chain(std::iter::once(&i))
                .map(|&p| entries[p].name.as_str())
                .collect();
            self.refuse(
                entry,
                "lower-theme-cycle",
                format!("a theme entry reaches itself: {}", chain.join(" → ")),
            );
            return None;
        }
        let Expr::Str(text, _) = &entry.value else {
            return None;
        };
        let text = text.trim();
        let role = role_named(&entry.name);
        // One override a role: its spellings are one name (LLP 1121 D3).
        if let Some((id, _)) = role {
            if let Some(first) = entries[..i]
                .iter()
                .find(|e| role_named(&e.name).is_some_and(|(other, _)| other == id))
            {
                let message = format!(
                    "`{}` overrides the role `{}` already does",
                    entry.name, first.name
                );
                self.refuse(entry, "lower-theme-duplicate", message);
                return None;
            }
            // The tint is the app's: `AccentColor` sets it; a role drawn in
            // it follows (LLP 1095 D1, an authored accent over the design).
            if roles::tint(id) && id != roles::accent() {
                let message = format!(
                    "`{}` is drawn in the app's tint: a theme sets the tint with `AccentColor`",
                    entry.name
                );
                self.refuse(entry, "lower-theme-tint", message);
                return None;
            }
        }
        let reserved = [
            "none",
            "auto",
            "inherit",
            "initial",
            "unset",
            "revert",
            "revert-layer",
            "currentcolor",
        ];
        if role.is_none() && reserved.contains(&entry.name.to_ascii_lowercase().as_str()) {
            let message = format!(
                "`{}` is a CSS keyword; name the entry otherwise",
                entry.name
            );
            self.refuse(entry, "lower-theme-name", message);
            return None;
        }
        if role.is_none() && parses(&entry.name) {
            self.refuse(
                entry,
                "lower-theme-name",
                format!(
                    "`{}` is a CSS colour's name; name the entry otherwise",
                    entry.name
                ),
            );
            return None;
        }
        // Another entry's name, or a role the theme overrides: that entry.
        let named = entries.iter().position(|e| {
            e.name == text
                || role_named(&e.name)
                    .zip(role_named(text))
                    .is_some_and(|((a, _), (b, _))| a == b)
        });
        path.push(i);
        let value = match (named, role_named(text)) {
            (Some(j), _) if j == i => {
                self.refuse(entry, "lower-theme-cycle", format!("`{}` names its own role: an override is a colour, a platform colour or another role", entry.name));
                None
            }
            (Some(j), _) => self.resolve_entry(entries, j, path, resolved),
            (None, Some((_, row))) => Some(row),
            (None, None) if parses(text) => Some(text.to_string()),
            (None, None) => {
                self.refuse(entry, "lower-theme-value", format!(
                    "`{}=\"{text}\"` is not a colour: a theme entry is a colour any row takes, another entry's name, or a role's \
                     (`\"secondary-grouped-background\"`); a name inside `light-dark()` or a platform colour's fallback is not read (LLP 1095 D1)",
                    entry.name
                ));
                None
            }
        };
        path.pop();
        let value = value?;
        resolved[i] = Some(ThemeEntry {
            name: entry.name.clone(),
            role: role.map(|(_, row)| row),
            value: value.clone(),
        });
        Some(value)
    }

    fn refuse(&mut self, entry: &Attr, id: &'static str, message: String) {
        if !self
            .errors
            .iter()
            .any(|e| e.id == id && e.span == entry.span)
        {
            self.errors.push(LowerError {
                id,
                message,
                span: entry.span,
            });
        }
    }
}
