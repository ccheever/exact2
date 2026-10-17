//! @ref LLP 1121 D10 (stage 6 F) — icon tokens at build. Each icon's SVG is
//! read ([`crate::icon_svg`]) and each target the app ships is checked to
//! have a source (`tamber-icon-missing`); a literal `symbol:icon.reply` is
//! written as the source form every host reads,
//! `symbol:icon|<sf>|<view_box>|<path>|<svg>`, and the plan's `icons` table
//! keeps what a computed source may name. `symbol:sf/<name>` names the role
//! whose Apple name it is, else the icon whose `sf` it is.

use crate::LowerError;
use contract_syntax::Span;
use exact_tamber as tamber;
use std::cell::RefCell;
use std::path::Path;

/// An icon as the build resolved it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Resolved {
    pub name: String,
    pub sf: String,
    pub view_box: String,
    /// Empty when the SVG is no template, which the web draws as the file.
    pub path: String,
    pub svg: String,
}

impl Resolved {
    /// The source form a host reads, after `symbol:`.
    pub fn form(&self) -> String {
        format!(
            "icon|{}|{}|{}|{}",
            self.sf, self.view_box, self.path, self.svg
        )
    }
}

thread_local! {
    /// The design's icons while its views are lowered, and the targets the
    /// app ships.
    static ICONS: RefCell<(Vec<Resolved>, Vec<String>)> = const { RefCell::new((Vec::new(), Vec::new())) };
    /// The icons a literal named, and whether a source was computed (then
    /// every icon is kept).
    static USED: RefCell<(Vec<String>, bool)> = const { RefCell::new((Vec::new(), false)) };
    /// The icons the design declares that were refused (their own error
    /// said why), so a use of one is not refused again as no role.
    static REFUSED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Mark icons refused: a use of one says nothing more.
pub(crate) fn mark_refused(names: impl IntoIterator<Item = String>) {
    REFUSED.with(|r| r.borrow_mut().extend(names));
}

/// Whether `name` (after `symbol:`) is an icon the design declares and the
/// build refused.
pub(crate) fn is_refused(name: &str) -> bool {
    REFUSED.with(|r| r.borrow().iter().any(|n| n == name))
}

/// An icon's SVG's most bytes.
const MAX_SVG: usize = 64 * 1024;

/// Whether a target draws SF Symbols.
fn apple(target: &str) -> bool {
    matches!(target, "ios" | "macos" | "tvos")
}

/// The targets an app ships: its manifest's `host`, `deploy.store` and
/// `deploy.binaries` keys, and its target crates (`android/`, `linux/`,
/// `web/`). An app with no manifest is checked for none.
fn targets(root: Option<&Path>) -> Vec<String> {
    // A game's resolved manifest first, as the build reads it (`Manifest::read`).
    let read = |file: &str| {
        let text = std::fs::read_to_string(root?.join(file)).ok()?;
        serde_json::from_str::<serde_json::Value>(&text).ok()
    };
    let Some(app) = read(".shells/app.json").or_else(|| read("app.json")) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    // An app's target crate is a target it builds (theme2's ruling).
    for dir in ["android", "linux", "web"] {
        if root.is_some_and(|r| r.join(dir).join("Cargo.toml").is_file()) {
            out.push(dir.to_string());
        }
    }
    for keys in [
        &app["host"],
        &app["deploy"]["store"],
        &app["deploy"]["binaries"],
    ] {
        for key in keys.as_object().into_iter().flat_map(|o| o.keys()) {
            if !out.contains(key) {
                out.push(key.clone());
            }
        }
    }
    out.sort();
    out
}

/// Read each icon's SVG and check each target the app ships (read by
/// [`clear`]) has a source; `span` places a refusal at the icon's line.
/// `exported`: whether an icon is kept for code outside the views (D7).
pub(crate) fn declare(
    icons: &[tamber::Icon],
    root: Option<&Path>,
    span: &dyn Fn(u32, u32) -> Span,
    exported: &dyn Fn(&str) -> bool,
) -> Vec<LowerError> {
    let targets = ICONS.with(|i| i.borrow().1.clone());
    let (mut out, mut errors) = (Vec::new(), Vec::new());
    for icon in icons {
        let at = span(icon.line, icon.col);
        let refuse = |id: &'static str, message: String| LowerError {
            id,
            message,
            span: at,
        };
        let svg = match (&icon.svg, root) {
            (None, _) => None,
            (Some(_), None) => {
                errors.push(refuse("tamber-icon-svg", format!("`{}`: an icon's SVG is under the app's `assets/`; compile this source with `compile_path`", icon.name)));
                continue;
            }
            // `|` joins the source form's parts.
            (Some(file), _) if file.contains('|') => {
                errors.push(refuse(
                    "tamber-icon-svg",
                    format!("`{}`: an SVG's path has no `|`", icon.name),
                ));
                continue;
            }
            (Some(file), Some(root)) => {
                let path = root.join("assets").join(file);
                // A template's path travels in the plan and in every literal source.
                let read = std::fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|t| Ok((t.len(), crate::icon_svg::read(&t)?)))
                    .and_then(
                        |(len, svg)| match svg.not_template.is_none() && len > MAX_SVG {
                            true => Err(format!("it is over {} KiB", MAX_SVG / 1024)),
                            false => Ok(svg),
                        },
                    );
                match read {
                    Ok(svg) => Some(svg),
                    Err(why) => {
                        errors.push(refuse(
                            "tamber-icon-svg",
                            format!("`{}`: `assets/{file}`: {why}", icon.name),
                        ));
                        continue;
                    }
                }
            }
        };
        for target in &targets {
            // Apple draws the SF Symbol (its hosts draw no path yet); every
            // other host a template's path, and the web a file as written.
            let template = svg.as_ref().is_some_and(|s| s.not_template.is_none());
            let drawn = if apple(target) {
                icon.sf.is_some()
            } else {
                template || (target == "web" && svg.is_some())
            };
            if drawn {
                continue;
            }
            errors.push(match (&svg, &icon.svg) {
                (Some(s), Some(file)) if !apple(target) => refuse(
                    "tamber-icon-svg",
                    format!(
                        "`{}`: `assets/{file}` is no one-colour shape ({}): only the web draws such a file, and the app ships {target}",
                        icon.name,
                        s.not_template.as_deref().unwrap_or("")
                    ),
                ),
                _ => refuse(
                    "tamber-icon-missing",
                    format!("{} has no source for {target}", icon.name),
                ),
            });
        }
        let template = svg.as_ref().filter(|s| s.not_template.is_none());
        out.push(Resolved {
            name: icon.name.clone(),
            sf: icon.sf.clone().unwrap_or_default(),
            view_box: svg.as_ref().map(|s| s.view_box.clone()).unwrap_or_default(),
            path: template.map(|s| s.path.clone()).unwrap_or_default(),
            svg: icon.svg.clone().unwrap_or_default(),
        });
    }
    // One whose SVG was refused is out of the table; its uses say no more.
    mark_refused(
        icons
            .iter()
            .filter(|i| !out.iter().any(|r: &Resolved| r.name == i.name))
            .map(|i| i.name.clone()),
    );
    let kept = out
        .iter()
        .map(|r| r.name.clone())
        .filter(|n| exported(n))
        .collect();
    ICONS.with(|i| *i.borrow_mut() = (out, targets));
    USED.with(|u| *u.borrow_mut() = (kept, false));
    errors
}

/// Forget the last design's icons, and read the targets the app at `root`
/// ships, which a bare `symbol:sf/…` is linted against with or without one.
pub(crate) fn clear(root: Option<&Path>) {
    ICONS.with(|i| *i.borrow_mut() = (Vec::new(), targets(root)));
    REFUSED.with(|r| r.borrow_mut().clear());
    USED.with(|u| *u.borrow_mut() = (Vec::new(), false));
}

/// The icon a name after `symbol:` means: `icon.reply` by its name, or
/// `sf/<name>` by its `sf`.
pub(crate) fn named(name: &str) -> Option<Resolved> {
    ICONS.with(|i| {
        let (icons, _) = &*i.borrow();
        match name.strip_prefix("sf/") {
            Some(sf) => icons.iter().find(|r| !sf.is_empty() && r.sf == sf).cloned(),
            None => icons.iter().find(|r| r.name == name).cloned(),
        }
    })
}

/// The role a grouped row's trailing symbol is: by any spelling, or an icon
/// token's accessory by its SF Symbol as Apple draws it (a disclosure, a
/// checkmark, an info button).
pub(crate) fn role_or_accessory(name: &str) -> Option<&'static str> {
    if let Some(role) = exact_kernel::generated::symbol_role(name) {
        return Some(role);
    }
    let sf = named(name)?.sf;
    exact_kernel::generated::symbol_role(&sf)
        .filter(|r| ["forward-chevron", "checkmark", "info"].contains(r))
}

/// A literal named `icon`, which the plan keeps.
pub(crate) fn used(icon: &str) {
    USED.with(|u| {
        let (names, _) = &mut *u.borrow_mut();
        if !names.iter().any(|n| n == icon) {
            names.push(icon.to_string());
        }
    });
}

/// A `symbol:` source was computed: a value may name any icon.
pub(crate) fn computed() {
    USED.with(|u| u.borrow_mut().1 = true);
}

/// The targets the app ships that are not Apple's, which draw no SF Symbol.
pub(crate) fn non_apple_targets() -> Vec<String> {
    ICONS.with(|i| i.borrow().1.iter().filter(|t| !apple(t)).cloned().collect())
}

/// The icons the plan keeps: those a literal named or `exports` keeps,
/// and every one when a source is computed.
pub(crate) fn kept() -> Vec<Resolved> {
    let (names, any) = USED.with(|u| u.borrow().clone());
    ICONS.with(|i| {
        i.borrow()
            .0
            .iter()
            .filter(|r| any || names.contains(&r.name))
            .cloned()
            .collect()
    })
}
