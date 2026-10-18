//! LLP 1116.014 D1: a refused attribute value leads with the edit that fixes
//! it, then the rule in a clause. Nothing refused becomes accepted here and
//! no second spelling enters a table; this only names the first spelling the
//! author meant.

use contract_syntax::Expr;
use exact_kernel::style::roles;
use exact_kernel::{ColorRole, StyleId, StyleProps, StyleValue, StyleValueError, COLOR_ROLES};
use std::ops::Range;

/// Where the roles are listed, for the end of a message.
const LISTED: &str = "the roles: `contract vocab color`";

/// A word of a colour value that is not a colour, and the edit that makes it
/// one (LLP 1081 D2: Exact's roles are `-exact-<role>`, CSS's system colours
/// their own names).
pub(crate) struct RoleEdit {
    word: String,
    at: Range<usize>,
    fix: Fix,
}

enum Fix {
    /// A role written bare (`secondary-label`): it is `-exact-secondary-label`.
    Bare(String),
    /// A role by a near spelling: UIKit's `-exact-system-fill` is `-exact-fill`.
    Spelling(String),
    /// The platform's words for the tint (`-exact-tint`, `-exact-accent`): the
    /// accent is the CSS system colour `AccentColor`.
    Accent,
    /// A tint as a background: `AccentColor` is the full accent, `Highlight`
    /// a tinted wash.
    Wash,
    /// No role is near.
    Unknown,
}

/// The first word of `text` that is not a colour but names one, for a refused
/// value. `colour` is whether the value is a colour: only then is a
/// `-exact-` word that is no role (a typo, a UIKit name) read as a role
/// meant, since `-exact-` also begins a length, a font and a corner shape.
pub(crate) fn role_hint(text: &str, colour: bool) -> Option<RoleEdit> {
    let mut start = None;
    // Splitting at spaces, commas and parentheses finds roles in gradients,
    // shadows, filters and `light-dark()`; a word before `(` is a function.
    for (at, c) in text.char_indices().chain([(text.len(), ' ')]) {
        if c.is_whitespace() || matches!(c, ',' | '(' | ')') {
            if let Some(from) = start.take() {
                let word = &text[from..at];
                if c != '(' {
                    if let Some(fix) = fix(word, colour) {
                        return Some(RoleEdit {
                            word: word.into(),
                            at: from..at,
                            fix,
                        });
                    }
                }
            }
        } else if start.is_none() {
            start = Some(at);
        }
    }
    None
}

/// A role as it is written: Exact's with `-exact-`, CSS's by name.
fn spelled(role: &ColorRole) -> String {
    if roles::is_css_system(role) {
        role.name.into()
    } else {
        format!("-exact-{}", role.name)
    }
}

fn fix(word: &str, colour: bool) -> Option<Fix> {
    if let Some(role) = COLOR_ROLES
        .iter()
        .find(|r| !roles::is_css_system(r) && r.name.eq_ignore_ascii_case(word))
    {
        return Some(Fix::Bare(spelled(role)));
    }
    if !colour || roles::role(word).is_some() {
        return None;
    }
    let lower = word.to_ascii_lowercase();
    let name = lower
        .strip_prefix("-exact-")
        .or_else(|| lower.strip_prefix("-apple-system-"))?;
    // UIKit's `system` is in the names Exact's roles keep it in (`system-red`)
    // and out of the rest (`systemGroupedBackground` is `grouped-background`).
    let bare = name.strip_prefix("system-").unwrap_or(name);
    match bare {
        "tint" | "accent" | "tint-color" | "accent-color" => return Some(Fix::Accent),
        "tint-background" | "accent-background" => return Some(Fix::Wash),
        _ => {}
    }
    let role = |name: &str| {
        COLOR_ROLES
            .iter()
            .find(|r| r.name.eq_ignore_ascii_case(name))
    };
    let near = |name: &str| contract_syntax::suggestion(name, COLOR_ROLES.iter().map(|r| r.name));
    let found = role(bare)
        .or_else(|| near(name).and_then(role))
        .or_else(|| near(bare).and_then(role));
    Some(found.map_or(Fix::Unknown, |r| Fix::Spelling(spelled(r))))
}

impl RoleEdit {
    /// The same edit for a `word` that stands at `offset` in the whole value.
    pub(crate) fn shifted(mut self, offset: usize) -> Self {
        self.at = self.at.start + offset..self.at.end + offset;
        self
    }

    /// The edit as the lead of the refusal of `attr="text"`.
    pub(crate) fn lead(&self, attr: &str, text: &str) -> String {
        let value =
            |with: &str| format!("{}{with}{}", &text[..self.at.start], &text[self.at.end..]);
        let write = |with: &str| format!("write `{attr}=\"{}\"`", value(with));
        let word = &self.word;
        match &self.fix {
            Fix::Bare(role) => format!(
                "{}: a role is spelled `-exact-<role>` (LLP 1081; {LISTED})",
                write(role)
            ),
            Fix::Spelling(role) => {
                format!("{}: `{word}` is not a role ({LISTED})", write(role))
            }
            Fix::Accent => format!(
                "{}, the accent: `{word}` is not a role ({LISTED})",
                write("AccentColor")
            ),
            Fix::Wash => format!(
                "{} for the full accent, or `\"{}\"` for a tinted wash: `{word}` is not a role ({LISTED})",
                write("AccentColor"),
                value("Highlight")
            ),
            Fix::Unknown => format!("choose a role ({LISTED}): `{word}` is not one"),
        }
    }

    /// The edit as a clause after another refusal (`…; <clause>`).
    pub(crate) fn clause(&self) -> String {
        let word = &self.word;
        match &self.fix {
            Fix::Bare(role) => format!("write `{role}` for `{word}` (LLP 1081)"),
            Fix::Spelling(role) => {
                format!("write `{role}` for `{word}`, which is not a role ({LISTED})")
            }
            Fix::Accent => format!("write `AccentColor` for `{word}`, which is not a role"),
            Fix::Wash => format!(
                "write `AccentColor` (the full accent) or `Highlight` (a tinted wash) for `{word}`, which is not a role"
            ),
            Fix::Unknown => format!("`{word}` is not a role ({LISTED})"),
        }
    }
}

/// The edit that fixes a refused attribute value, for the refusals whose fix
/// is one (`None` for the rest): a colour that is a near role or the tint
/// (`color="-exact-tint"` is `color="AccentColor"`), and a quoted number
/// where the row wants a number (`font-weight="700"` is `font-weight=700`).
pub(crate) fn value_edit(
    name: &str,
    value: &Expr,
    rows: &[StyleId],
    error: &StyleValueError,
) -> Option<String> {
    let Expr::Str(text, _) = value else {
        return None;
    };
    match error {
        StyleValueError::BadColor { .. } => role_hint(text, true).map(|e| e.lead(name, text)),
        StyleValueError::WrongKind { .. } => {
            let number = text.trim();
            let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
            let unsigned = number.strip_prefix('-').unwrap_or(number);
            let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, "0"));
            // The edit is offered only if the row takes the number.
            if !(digits(whole) && digits(fraction)) {
                return None;
            }
            let n: f64 = number.parse().ok()?;
            // Unquoted, a line-height this large is that many times the font size: the points
            // it means take a unit (`lower-line-height-multiplier`).
            if name == "line-height" && n >= crate::warnings::line_height::IMPLAUSIBLE {
                return Some(format!("write `line-height=\"{number}px\"` for {number} points: a bare `line-height={number}` is {number} times the font size, as in CSS"));
            }
            rows.iter()
                .all(|row| {
                    StyleProps::default()
                        .set_dynamic(*row, &StyleValue::Number(n))
                        .is_ok()
                })
                .then(|| format!("write `{name}={number}`: a number is written without quotes"))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lead(attr: &str, text: &str) -> Option<String> {
        role_hint(text, true).map(|e| e.lead(attr, text))
    }

    #[test]
    fn a_near_role_is_named_by_its_spelling() {
        for (written, role) in [
            (
                "-exact-system-grouped-background",
                "-exact-grouped-background",
            ),
            ("-exact-system-fill", "-exact-fill"),
            ("-exact-system-background", "-exact-background"),
            ("-apple-system-fill", "-exact-fill"),
            ("-exact-secondary-labels", "-exact-secondary-label"),
            ("-exact-canvas", "Canvas"),
            ("-exact-system-gray6", "-exact-system-gray"),
        ] {
            let message = lead("background-color", written).unwrap();
            assert!(
                message.starts_with(&format!("write `background-color=\"{role}\"`:")),
                "{written}: {message}"
            );
        }
    }

    #[test]
    fn the_tint_words_are_the_accent() {
        for written in ["-exact-tint", "-exact-accent", "-exact-tint-color"] {
            let message = lead("color", written).unwrap();
            assert!(
                message.starts_with("write `color=\"AccentColor\"`, the accent:"),
                "{written}: {message}"
            );
        }
        let wash = lead("background-color", "-exact-tint-background").unwrap();
        assert!(
            wash.starts_with("write `background-color=\"AccentColor\"` for the full accent, or `\"Highlight\"` for a tinted wash:"),
            "{wash}"
        );
    }

    #[test]
    fn a_word_in_a_longer_value_is_replaced_in_place() {
        let message = lead("border", "1px solid -exact-tint").unwrap();
        assert!(
            message.starts_with("write `border=\"1px solid AccentColor\"`"),
            "{message}"
        );
        let message = lead("color", "light-dark(-exact-tint, #fff)").unwrap();
        assert!(
            message.starts_with("write `color=\"light-dark(AccentColor, #fff)\"`"),
            "{message}"
        );
    }

    #[test]
    fn a_valid_role_a_function_and_a_non_colour_are_not_edited() {
        assert!(role_hint("-exact-system-red", true).is_none());
        assert!(role_hint("-exact-platform-color(ios tintColor, #fff)", true).is_none());
        assert!(role_hint("var(--exact-label, red)", true).is_none());
        // `-exact-` begins a corner shape and a font too.
        assert!(role_hint("-exact-continuous", false).is_none());
        // A bare role is a hint wherever it stands.
        assert!(role_hint("secondary-label", false).is_some());
    }

    /// A quoted `line-height="28"` is not sent to the bare 28, which is 28
    /// times the font size (the euxy port's 616-point header); a ratio is.
    #[test]
    fn a_quoted_line_height_in_points_is_sent_to_px() {
        let edit = |text: &str| {
            let error = StyleValueError::WrongKind {
                style: StyleId::LineHeight,
                expected: "a number or a length",
            };
            let value = Expr::Str(text.into(), contract_syntax::Span::default());
            value_edit("line-height", &value, &[StyleId::LineHeight], &error)
        };
        assert_eq!(
            edit("28").as_deref(),
            Some("write `line-height=\"28px\"` for 28 points: a bare `line-height=28` is 28 times the font size, as in CSS")
        );
        assert_eq!(
            edit("1.4").as_deref(),
            Some("write `line-height=1.4`: a number is written without quotes")
        );
    }

    #[test]
    fn a_role_nothing_is_near_sends_to_the_list() {
        let message = lead("color", "-exact-text").unwrap();
        assert_eq!(
            message,
            "choose a role (the roles: `contract vocab color`): `-exact-text` is not one"
        );
    }
}
