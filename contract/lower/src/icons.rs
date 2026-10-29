//! @ref LLP 1121 D10 (stage 6 F) — icon tokens at build. Each icon's SVG is
//! read ([`crate::icon_svg`]) and each target the app ships is checked to
//! have a source (`tamber-icon-missing`); a literal `symbol:icon.reply` is
//! written as the source form every host reads,
//! `symbol:icon|<sf>|<view_box>|<path>|<svg>`, and the plan's `icons` table
//! keeps what a computed source may name. `symbol:sf/<name>` names the role
//! whose Apple name it is, else the icon whose `sf` it is.

mod confined;
pub use confined::{read as read_confined, Unread};

use crate::LowerError;
use contract_syntax::{DesignSource, Span};
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
    /// An even-odd icon's starts `evenodd:` (stage 14 C2), in the plan's row
    /// and the source form alike; a nonzero one's is the `d`s alone, as
    /// before. Every reader strips it through `exact_kernel::Icon`.
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
    static ICONS: RefCell<(Vec<Resolved>, Vec<Target>)> = const { RefCell::new((Vec::new(), Vec::new())) };
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

/// The SVG every host but the web draws ([`crate::icon_svg`]'s template).
pub(crate) const TEMPLATE: &str = "filled `<path>`s in one opaque colour under the `<svg>`'s viewBox, all by one `fill-rule` (nonzero or evenodd), with no stroke, transform, opacity or other element";

/// An icon's SVG's most bytes.
const MAX_SVG: usize = 64 * 1024;

/// Whether a target draws SF Symbols.
fn apple(target: &str) -> bool {
    matches!(target, "ios" | "macos" | "tvos" | "watchos")
}

/// The source `target` draws for `icon`, by D10's rule (the one
/// `tamber-icon-missing` checks): Apple the SF Symbol, else a template
/// SVG's path (stage 9 A; not yet a watch's), every other host a
/// template's path, and the web an SVG as written; `None` where it draws
/// none.
fn drawn(
    target: &str,
    icon: &tamber::Icon,
    svg: Option<&crate::icon_svg::Svg>,
) -> Option<&'static str> {
    let template = svg.is_some_and(|s| s.not_template.is_none());
    match apple(target) {
        true if icon.sf.is_some() => Some("sf"),
        true => (template && target != "watchos").then_some("svg"),
        false => (template || (target == "web" && svg.is_some())).then_some("svg"),
    }
}

/// The package an icon's sources are written in (LLP 1121 D11, stage 13),
/// by name, and its directory: a package's icon (its line in the package's
/// band), a package component's icon param's default, in its design or its
/// Contract (its line a param's, `params` the list the design was laid
/// with), or one naming either; none for the app's (its icons, an inline
/// theme's, and its `params` lines). Its SVG is read under that package's
/// `assets/`, the app's under the app's.
pub fn written_in<'a>(
    icon: &tamber::Icon,
    packages: &'a [DesignSource],
    params: &'a [crate::params::ParamToken],
) -> Option<(&'a str, Option<&'a str>)> {
    if let Some((band, _)) = crate::theme::packages::band(icon.written) {
        let p = packages.get(band)?;
        return Some((p.package.as_deref()?, p.root.as_deref()));
    }
    let at = icon.written.checked_sub(crate::params::LINE)? as usize;
    let p = params.get(at)?;
    Some((p.package.as_deref()?, p.root.as_deref()))
}

/// Where an icon's SVG is read: under `root`'s `assets/`, and, for a
/// package's, the package by name.
#[derive(Debug, Clone, Copy)]
pub struct Home<'a> {
    /// The directory whose `assets/` it is under; none when the source
    /// was compiled apart from any file.
    pub root: Option<&'a Path>,
    /// The package whose file writes it; none for the app's.
    pub package: Option<&'a str>,
}

impl<'a> Home<'a> {
    /// `icon`'s: its package's root ([`written_in`]), else `app`.
    pub fn of(
        icon: &tamber::Icon,
        app: Option<&'a Path>,
        packages: &'a [DesignSource],
        params: &'a [crate::params::ParamToken],
    ) -> Self {
        match written_in(icon, packages, params) {
            Some((package, root)) => Home {
                root: root.map(Path::new),
                package: Some(package),
            },
            None => Home {
                root: app,
                package: None,
            },
        }
    }

    /// The SVG's file on disk.
    pub fn svg(&self, file: &str) -> Option<std::path::PathBuf> {
        Some(self.root?.join("assets").join(file))
    }

    /// Whether `path` (this home's SVG, [`Home::svg`]) leaves its package:
    /// a package's file must be its own, links followed, under its own
    /// `assets/` (round 1, Astra 4 = Grok 2; [`confined::read`], the one
    /// rule every reader of a package's file applies), so no link passes off
    /// the app's or another package's bytes as the package's. One not there
    /// leaves nothing. The app's are not confined: its `assets/` are its own
    /// to arrange (a link to a shared directory is the author's choice), and
    /// the web stages them as they are.
    pub fn leaves(&self, path: &Path) -> bool {
        let (Some(_), Some(root)) = (self.package, self.root) else {
            return false;
        };
        matches!(confined::read(path, root), Err(Unread::Leaves))
    }

    /// `assets/<file>` as a refusal names it: a package's by its package.
    fn shown(&self, file: &str) -> String {
        match self.package {
            Some(p) => format!("the package `{p}`'s `assets/{file}`"),
            None => format!("`assets/{file}`"),
        }
    }
}

/// An icon's SVG as the build reads it: none without one; refused where
/// its path has a `|` (it joins the source form's parts), it is not under
/// its design file's `assets/` (the app's, or its package's), it doesn't
/// read as one, a template is over its size (its path travels in the plan
/// and in every literal source), or a package's is no template (stage 13,
/// Q2: the web, which alone draws an SVG as written, stages the app's
/// `assets/` only).
fn svg_of(icon: &tamber::Icon, home: Home) -> Result<Option<crate::icon_svg::Svg>, String> {
    let (file, root) = match (&icon.svg, home.root) {
        (None, _) => return Ok(None),
        (Some(_), None) => return Err(format!("`{}`: an icon's SVG is under the app's `assets/`; compile this source with `compile_path`", icon.name)),
        (Some(file), _) if file.contains('|') => {
            return Err(format!("`{}`: an SVG's path has no `|`", icon.name))
        }
        (Some(file), Some(root)) => (file, root),
    };
    let path = root.join("assets").join(file);
    // A package's read only as its own file, from the file checked (round 3,
    // R3-4: [`confined::read`]); the app's as it is.
    let bytes = match home.package {
        Some(_) => confined::read(&path, root).map_err(|why| match why {
            Unread::Leaves => format!(
                "`{}`: {} leaves the package (a link to a file outside its `assets/`): a package's SVG is its own file",
                icon.name,
                home.shown(file)
            ),
            Unread::Missing(e) | Unread::Failed(e) => format!("`{}`: {}: {e}", icon.name, home.shown(file)),
        })?,
        None => std::fs::read(&path).map_err(|e| format!("`{}`: {}: {e}", icon.name, home.shown(file)))?,
    };
    String::from_utf8(bytes)
        .map_err(|e| e.to_string())
        .and_then(|t| Ok((t.len(), crate::icon_svg::read(&t)?)))
        .and_then(|(len, svg)| match &svg.not_template {
            None if len > MAX_SVG => Err(format!("it is over {} KiB", MAX_SVG / 1024)),
            Some(why) if home.package.is_some() => Err(format!(
                "a package's pictures are one-colour templates in v1 ({TEMPLATE}), and this is not ({why}). To fix it, {}",
                svg.fix
            )),
            _ => Ok(Some(svg)),
        })
        .map_err(|why| format!("`{}`: {}: {why}", icon.name, home.shown(file)))
}

/// Which source each target the app at `root` ships draws for `icon`
/// (`describe`'s icon entry, LLP 1121 stage 9 C), its SVG read as the build
/// reads it ([`svg_of`], under `home`); one the build refuses draws as none.
pub fn draws(icon: &tamber::Icon, root: &Path, home: Home) -> Vec<(String, Option<&'static str>)> {
    let svg = svg_of(icon, home).ok().flatten();
    targets(Some(root))
        .into_iter()
        .map(|t| {
            let source = drawn(&t.name, icon, svg.as_ref());
            (t.name, source)
        })
        .collect()
}

/// The targets the app at `root` ships, by name, in order (what `draws` is
/// keyed by).
pub fn shipped_targets(root: &Path) -> Vec<String> {
    targets(Some(root)).into_iter().map(|t| t.name).collect()
}

/// A target the app ships, and each thing in the app that makes it shipped,
/// so a refusal can say what to remove.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Target {
    pub name: String,
    /// "the `linux/` crate", "app.json `deploy.store.linux`".
    pub from: Vec<String>,
}

impl Target {
    /// "linux (the `linux/` crate, app.json `deploy.store.linux`)".
    pub fn described(&self) -> String {
        format!("{} ({})", self.name, self.from.join(", "))
    }
}

/// The targets an app ships: its manifest's `host`, `deploy.store` and
/// `deploy.binaries` keys, and its target crates (`android/`, `linux/`,
/// `web/`). An app with no manifest is checked for none.
fn targets(root: Option<&Path>) -> Vec<Target> {
    // A game's resolved manifest first, as the build reads it (`Manifest::read`).
    let read = |file: &str| {
        let text = std::fs::read_to_string(root?.join(file)).ok()?;
        serde_json::from_str::<serde_json::Value>(&text).ok()
    };
    let Some(app) = read(".shells/app.json").or_else(|| read("app.json")) else {
        return Vec::new();
    };
    let mut out: Vec<Target> = Vec::new();
    let mut add = |name: &str, from: String| match out.iter_mut().find(|t| t.name == name) {
        Some(t) => t.from.push(from),
        None => out.push(Target {
            name: name.to_string(),
            from: vec![from],
        }),
    };
    // An app's target crate is a target it builds (theme2's ruling).
    for dir in ["android", "linux", "web"] {
        if root.is_some_and(|r| r.join(dir).join("Cargo.toml").is_file()) {
            add(dir, format!("the `{dir}/` crate"));
        }
    }
    for (path, keys) in [
        ("host", &app["host"]),
        ("deploy.store", &app["deploy"]["store"]),
        ("deploy.binaries", &app["deploy"]["binaries"]),
    ] {
        for key in keys.as_object().into_iter().flat_map(|o| o.keys()) {
            add(key, format!("app.json `{path}.{key}`"));
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Read each icon's SVG (under its `home`) and check each target the app
/// ships (read by [`clear`]) has a source; `span` places a refusal at the
/// icon's line. `exported`: whether an icon is kept for code outside the
/// views (D7).
pub(crate) fn declare<'a>(
    icons: &[tamber::Icon],
    home: &dyn Fn(&tamber::Icon) -> Home<'a>,
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
        let home = home(icon);
        let svg = match svg_of(icon, home) {
            Ok(svg) => svg,
            Err(why) => {
                errors.push(refuse("tamber-icon-svg", why));
                continue;
            }
        };
        for target in targets.iter().map(|t| t.name.as_str()) {
            if drawn(target, icon, svg.as_ref()).is_some() {
                continue;
            }
            errors.push(match (&svg, &icon.svg) {
                // A one-colour SVG a watch doesn't draw yet: it needs the SF Symbol.
                (Some(s), Some(_)) if s.not_template.is_none() => refuse(
                    "tamber-icon-missing",
                    format!("{} has no source for {target}: a watch draws an icon only as an SF Symbol (an `sf` source)", icon.name),
                ),
                (Some(s), Some(file)) => refuse(
                    "tamber-icon-svg",
                    format!(
                        "`{}`: {} is no one-colour shape ({}), and the app ships {target}, which draws an icon only as {TEMPLATE}{} (the web alone draws any SVG as written). To fix it, {}; or start from an SVG already in that form (Material Symbols' are){}",
                        icon.name,
                        home.shown(file),
                        s.not_template.as_deref().unwrap_or(""),
                        if apple(target) { " or as an SF Symbol (an `sf` source)" } else { "" },
                        s.fix,
                        without(target)
                            .map(|step| format!(". Not shipping {target}? {step} removes it"))
                            .unwrap_or_default()
                    ),
                ),
                // No SVG for a host that draws only one (LLP 1121 D10 refuses it: the app ships
                // that host). The euxy port, Apple-only with no SVGs, needed the form that builds.
                (None, _) if !apple(target) => {
                    let file = icon.name.trim_start_matches("icon.").replace('.', "-");
                    // A bare SF Symbol blank on the web alone is one line of a build (the CLI's `web_only`).
                    let blank: Vec<&str> = targets.iter().map(|t| t.name.as_str()).filter(|t| !apple(t)).map(|t| if t == "web" { "the web" } else { t }).collect();
                    // An SF Symbol that is a role's Apple name is that role, which every host draws.
                    let sf = icon.sf.as_deref().map(|sf| match crate::symbols::role_by_apple_name(sf) {
                        Some(role) => format!(". With no SVG for it, declare no icon and write `symbol:{role}`, the role Apple draws as `{sf}`, which every host draws"),
                        None => format!(
                            ". With no SVG for it, declare no icon and write `symbol:sf/{sf}` where it is used: Apple draws it, and {} it blank, {}",
                            if blank.len() == 1 { format!("{} draws", blank[0]) } else { format!("{} draw", blank.join(" and ")) },
                            if blank == ["the web"] { "which the build says in one warning" } else { "with a warning at each use" },
                        ),
                    }).unwrap_or_default();
                    refuse(
                        "tamber-icon-missing",
                        format!(
                            "{} has no source for {target}, which draws an icon only from an `svg` source: add `svg \"icons/{file}.svg\"`, with `assets/icons/{file}.svg` drawn as {TEMPLATE} (a Material Symbols SVG is){sf}{}",
                            icon.name,
                            without(target)
                                .map(|step| format!(". Not shipping {target}? {step} removes it"))
                                .unwrap_or_default()
                        ),
                    )
                }
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
            path: template
                .map(|s| match s.even_odd {
                    true => format!("{}{}", exact_kernel::Icon::EVEN_ODD, s.path),
                    false => s.path.clone(),
                })
                .unwrap_or_default(),
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

/// The targets the app ships, as the last design's lowering read them.
pub(crate) fn shipped() -> Vec<Target> {
    ICONS.with(|i| i.borrow().1.clone())
}

/// The targets the app ships that are not Apple's, which draw no SF Symbol.
pub(crate) fn non_apple_targets() -> Vec<Target> {
    ICONS.with(|i| {
        let targets = &i.borrow().1;
        targets
            .iter()
            .filter(|t| !apple(&t.name))
            .cloned()
            .collect()
    })
}

/// How the app stops shipping `target`, when one step does: the Linux
/// crate (and Android, which builds it) goes with `update --without`; the
/// web host is the dev loop's and the agent's, so it stays.
pub(crate) fn without(target: &str) -> Option<String> {
    matches!(target, "linux" | "android")
        .then(|| format!("`bun exact.mjs update --without {target}`"))
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
