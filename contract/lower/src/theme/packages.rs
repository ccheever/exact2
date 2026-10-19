//! Package designs (LLP 1121 D11 "Packages", stage 11 A): a package the
//! app's sources link may ship `<name>.tamber` beside its entry, its tokens
//! named under its namespace (`reading.measure`). Each is checked alone for
//! what only a package may not do, then its tokens join the app's design,
//! which folds them as one table: the app's design sets any of them by full
//! name (the app's definition is the token, whole). One name from two
//! packages can't happen: each names its tokens under its namespace, and
//! two packages taking one namespace are refused where they are read
//! (`contract/cli/src/sources.rs` `package_designs`); one name twice in one
//! package is the fold's `tamber-duplicate`. Dials and axes are the app's
//! alone. A package token's lines are numbered past [`LINE`], each package
//! in a band of its own, so a refusal in the fold names its file.

use super::Origin;
use crate::LowerError;
use contract_syntax::{DesignSource, Span};
use exact_kernel::style::roles;
use exact_tamber as tamber;

/// Where package lines start: below a param's (`params::LINE`).
pub(crate) const LINE: u32 = 1 << 28;
/// Each package's band of lines.
const BAND: u32 = 1 << 20;

/// The base scale's names (stage 11 B, the lead's ruling 2), which a package
/// token may name as it names a role.
pub(crate) const BASE: [&str; 9] = [
    "space.xs",
    "space.sm",
    "space.md",
    "space.lg",
    "space.xl",
    "radius.sm",
    "radius.md",
    "radius.lg",
    "radius.full",
];

thread_local! {
    /// The package designs merged into the last design lowered on this
    /// thread, each its source id, by band. Every merge sets them, and each
    /// pass that maps a banded line merges first (lowering, the lint, motion,
    /// the warnings' fold), so a line maps by the bands its own merge made.
    static SOURCES: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Package `band`'s `line` as the merged design numbers it.
pub(crate) fn banded(band: usize, line: u32) -> u32 {
    LINE + band as u32 * BAND + line
}

/// A merged design's line in a package's band: the package's index and
/// the line in its file.
pub(crate) fn band(line: u32) -> Option<(usize, u32)> {
    (LINE..crate::params::LINE)
        .contains(&line)
        .then(|| (((line - LINE) / BAND) as usize, (line - LINE) % BAND))
}

/// A merged design's line, where it was written: a package token's line
/// in its file; none for the app's own lines.
pub(crate) fn span(line: u32, col: u32) -> Option<Span> {
    if !(LINE..crate::params::LINE).contains(&line) {
        return None;
    }
    let band = ((line - LINE) / BAND) as usize;
    let source_id = SOURCES.with(|s| s.borrow().get(band).copied())?;
    let line = (line - LINE) % BAND;
    Some(Span {
        line,
        col,
        end_col: col,
        source_id,
    })
}

/// The namespace a package's tokens are named under: the last part of its
/// name (`@exact/reading` → `reading`).
pub(crate) fn namespace(package: &str) -> &str {
    package.rsplit('/').next().unwrap_or(package)
}

/// Whether `word` names something every package may name: a role (by its
/// spelling or `-exact-` one) or a base scale name.
fn shared(word: &str) -> bool {
    BASE.contains(&word)
        || roles::role(word).is_some()
        || roles::role(&format!("-exact-{word}")).is_some()
}

fn at(source: &DesignSource, line: u32, col: u32) -> Span {
    Span {
        line,
        col,
        end_col: col,
        source_id: source.source_id,
    }
}

/// `design` (the app's, or none) with each package design's tokens added,
/// and what is refused in them before the fold. A token the app's design
/// defines is the app's, whole: the package's definition is not joined.
/// The fold then checks the package's `when` lines against the app's axes,
/// and [`references`] what its values name. Called once a design lowers;
/// the bands it numbers are this thread's until the next.
pub(crate) fn merge(
    design: Option<tamber::Design>,
    packages: &[DesignSource],
) -> (Option<tamber::Design>, Vec<LowerError>) {
    SOURCES.with(|s| *s.borrow_mut() = packages.iter().map(|p| p.source_id).collect());
    if packages.is_empty() {
        return (design, Vec::new());
    }
    let mut refused = Vec::new();
    let mut merged = design.unwrap_or_default();
    let own: Vec<String> = merged.tokens.iter().map(|t| t.name.value.clone()).collect();
    for (k, source) in packages.iter().enumerate() {
        let package = source.package.as_deref().unwrap_or_default();
        let ns = namespace(package);
        let refuse = |id: &'static str, line: u32, col: u32, message: String| LowerError {
            id,
            message,
            span: at(source, line, col),
        };
        let design = match tamber::parse(&source.text) {
            Ok(d) => d,
            Err(all) => {
                refused.extend(
                    all.into_iter()
                        .map(|d| refuse(d.id, d.line, d.col, d.message)),
                );
                continue;
            }
        };
        if k as u32 >= (crate::params::LINE - LINE) / BAND
            || source.text.lines().count() as u32 >= BAND
        {
            refused.push(refuse("tamber-package-block", 1, 1, format!(
                "the package `{package}`'s design: an app reads at most {} package designs of at most {} lines each",
                (crate::params::LINE - LINE) / BAND,
                BAND - 1
            )));
            continue;
        }
        let mut ok = true;
        if design.name != ns {
            ok = false;
            refused.push(refuse("tamber-package-name", 1, 1, format!(
                "the package `{package}`'s design is `tamber {}`; a package's design is named for its package: `tamber {ns}`",
                design.name
            )));
        }
        // What only the app may declare, and what packages don't carry yet.
        let blocks = design
            .axes
            .iter()
            .map(|a| (&a.name, "an axis: axes are the app's alone (D11); the app declares it and the package's `when` lines name it"))
            .chain(design.dials.iter().map(|d| (&d.name, "a dial: dials are the app's alone (D11)")))
            .chain(design.components.iter().map(|c| (&c.name, "a component's params: a package's components come with stage 12 (QUEUE)")))
            .chain(design.params.iter().map(|t| (&t.name, "a `params` line: the app's design sets params, not a package's")))
            .chain(design.icons.iter().map(|t| (&t.name, "an icon token: a package's icons are not read yet (QUEUE)")))
            .chain(design.exports.iter().flatten().map(|e| (e, "an `exports` line: what a build keeps of a package's tokens is D7's rule, as for the app's")));
        for (name, why) in blocks {
            ok = false;
            refused.push(refuse(
                "tamber-package-block",
                name.line,
                name.col,
                format!(
                    "`{}` in the package `{package}`'s design is {why}",
                    name.value
                ),
            ));
        }
        for t in &design.tokens {
            let name = &t.name.value;
            if !name.starts_with(&format!("{ns}.")) || BASE.contains(&name.as_str()) {
                ok = false;
                refused.push(refuse(
                    "tamber-package-name",
                    t.name.line,
                    t.name.col,
                    format!(
                        "`{name}`: a package's token is named under its package (`{ns}.…`){}",
                        if shared(name) {
                            "; a role or base name is the app's to set"
                        } else {
                            ""
                        }
                    ),
                ));
            }
        }
        if !ok {
            continue;
        }
        let band = LINE + k as u32 * BAND;
        for mut token in design.tokens {
            // The app's definition is the token, whole.
            if own.contains(&token.name.value) {
                continue;
            }
            token.name.line += band;
            token.value.line += band;
            for w in &mut token.whens {
                w.line += band;
                w.value.line += band;
            }
            merged.tokens.push(token);
        }
    }
    (Some(merged), refused)
}

/// `message` with each `line N` in a package's band as its line in its
/// file: a fold's message that names another line (a duplicate's first).
pub(crate) fn unband(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    while let Some(at) = rest.find("line ") {
        let (head, tail) = rest.split_at(at + "line ".len());
        out.push_str(head);
        let end = tail
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(tail.len());
        match tail[..end].parse::<u32>() {
            Ok(n) if (LINE..crate::params::LINE).contains(&n) => {
                out.push_str(&((n - LINE) % BAND).to_string())
            }
            _ => out.push_str(&tail[..end]),
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// What the folded package tokens name that is not theirs to (D11, theme2's
/// ruling Q6): each names, as the fold resolved it (its values' names and
/// its `pair=`), only its own package's tokens (as its design declares
/// them: one the app sets is still its own to name, one the app adds under
/// its namespace is not), roles (an app's override of one included) and
/// the base scale's names (the app may set those for every package), and
/// no dial its own values read (`reads`, not a dial's reach through what it
/// names). A whole alias of a profile's colour, which the build carries as
/// written out of the fold, is read from the package's own text. Each
/// refusal at the token's line in its package's design.
pub(crate) fn references(folding: &tamber::Folding, packages: &[DesignSource]) -> Vec<LowerError> {
    let mut refused = Vec::new();
    let declared: Vec<Vec<tamber::Token>> = packages
        .iter()
        .map(|p| tamber::parse(&p.text).map_or_else(|_| Vec::new(), |d| d.tokens))
        .collect();
    let own = |band: usize, name: &str| declared[band].iter().any(|t| t.name.value == name);
    let tokens: Vec<&str> = folding
        .design
        .tokens
        .iter()
        .map(|t| t.name.value.as_str())
        .collect();
    for (checked, token) in folding.tokens.iter().zip(&folding.design.tokens) {
        let line = token.name.line;
        if !(LINE..crate::params::LINE).contains(&line) {
            continue;
        }
        let band = ((line - LINE) / BAND) as usize;
        let Some(source) = packages.get(band) else {
            continue;
        };
        let package = source.package.as_deref().unwrap_or_default();
        let span = span(line, token.name.col).unwrap_or_default();
        let mut wrong = |what: String| {
            refused.push(LowerError {
                id: "tamber-unknown-token",
                message: format!("`{}` names {what}: a package's token names its own tokens, roles and base names only, and no dial (D11)", token.name.value),
                span,
            })
        };
        for name in checked.names.iter().chain(&checked.pairs) {
            if !own(band, name) && !shared(name) {
                wrong(format!(
                    "`{name}`, a token that isn't the package `{package}`'s"
                ));
            }
        }
        for dial in &checked.reads {
            wrong(format!("the dial `{dial}`"));
        }
        // A whole alias of a profile's colour, as written.
        let Some(written) = declared[band]
            .iter()
            .find(|t| t.name.value == token.name.value)
        else {
            continue;
        };
        for value in std::iter::once(&written.value).chain(written.whens.iter().map(|w| &w.value)) {
            let whole = value.value.trim();
            if tokens.contains(&whole)
                && !own(band, whole)
                && !shared(whole)
                && !checked.names.iter().any(|n| n == whole)
            {
                wrong(format!(
                    "`{whole}`, a token that isn't the package `{package}`'s"
                ));
            }
        }
    }
    refused
}

impl crate::Lowerer<'_> {
    /// Whether the folded design's package tokens name what isn't theirs
    /// ([`references`]), each refusal said.
    pub(super) fn package_references(
        &mut self,
        folding: &tamber::Folding,
        file: &contract_syntax::File,
    ) -> bool {
        let wrong = references(folding, &file.package_designs);
        let any = !wrong.is_empty();
        for e in wrong {
            self.push(e);
        }
        any
    }

    /// The app's design as the crate reads it (`app.tamber` parsed, or the
    /// inline `theme` built, one token an entry; both is refused), with its
    /// packages' designs joined: a package's token refused here is said and
    /// left out of the fold.
    pub(super) fn design(
        &mut self,
        file: &contract_syntax::File,
    ) -> Option<(tamber::Design, Origin)> {
        let (app, origin) = match self.app_design(file) {
            Some((design, origin)) => (Some(design), origin),
            // The app's design was refused: its packages' are not folded.
            None if file.design.is_some() => return None,
            // None of the app's lines; the packages' by band.
            None => (None, Origin::Inline(Vec::new())),
        };
        let (merged, refused) = merge(app, &file.package_designs);
        for e in refused {
            self.push(e);
        }
        merged.map(|design| (design, origin))
    }
}
