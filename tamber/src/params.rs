//! Component params in the design (LLP 1121 D15, stage 7 B). A param is a
//! token its Contract component owns, named below the IR by its dotted token
//! name and above it by its full name (`post.contract#PostButton.radius`) or
//! short one (`PostButton.radius`). The app's value of one is a line of the
//! design's `params` block. The compiler, the design server's `plan`,
//! `resolve` and `describe` all fold a design the same way: each param as a
//! token of its kind, then each `params` line in place of its default.

use crate::{At, Design, Token};

/// A param as the design's folds take it: its token name, full name, kind
/// and default (another param named by its token name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// `param.f3f9a01c2.PostButton.radius`.
    pub token: String,
    /// `post.contract#PostButton.radius`.
    pub full: String,
    /// Its kind as its component writes it (`length`, `icon`).
    pub kind: String,
    /// Its default.
    pub default: String,
}

impl Param {
    /// `PostButton.radius`.
    pub fn short(&self) -> &str {
        self.full
            .rsplit_once('#')
            .map_or(self.full.as_str(), |(_, s)| s)
    }
}

/// Where a param's token is said to be written, past any design's own
/// lines (as the compiler numbers it).
pub const LINE: u32 = 1 << 29;

/// Why a name names no single param.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unnamed {
    /// No param of the app is called that.
    Unknown,
    /// A short name more than one component's param answers.
    Ambiguous,
}

/// The param `name` names: its full name, or its short one where one
/// component of that name declares it.
pub fn named<'a>(params: &'a [Param], name: &str) -> Result<&'a Param, Unnamed> {
    if let Some(p) = params.iter().find(|p| p.full == name) {
        return Ok(p);
    }
    let mut found = params.iter().filter(|p| p.short() == name);
    match (found.next(), found.next()) {
        (Some(p), None) => Ok(p),
        (None, _) => Err(Unnamed::Unknown),
        (Some(_), Some(_)) => Err(Unnamed::Ambiguous),
    }
}

/// `design` with each of `params` a token of its kind (an icon param among
/// the icons) on line [`LINE`] + its index, and each line of its `params`
/// block that names one in place of that one's default and `when` lines
/// (at its own line). A line naming none, or ambiguously, is left out: the
/// compiler says why.
pub fn laid(design: &Design, params: &[Param]) -> Design {
    let mut out = design.clone();
    for (i, p) in params.iter().enumerate() {
        let at = |value: &str| At {
            value: value.to_string(),
            line: LINE + i as u32,
            col: 1,
        };
        let icon = p.kind == "icon";
        let mut token = Token {
            name: at(&p.token),
            kind: (!icon).then(|| p.kind.clone()),
            value: at(&p.default),
            whens: Vec::new(),
            meta: Vec::new(),
        };
        if let Some(entry) = design
            .params
            .iter()
            .find(|e| named(params, &e.name.value).is_ok_and(|q| q.token == p.token))
        {
            token.value = entry.value.clone();
            token.whens = entry.whens.clone();
        }
        match icon {
            true => out.icons.push(token),
            false => out.tokens.push(token),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn param(full: &str, default: &str) -> Param {
        Param {
            token: {
                let (file, short) = full.rsplit_once('#').unwrap();
                format!("param.f{}.{short}", file.split('.').next().unwrap())
            },
            full: full.into(),
            kind: "length".into(),
            default: default.into(),
        }
    }

    #[test]
    fn a_params_line_takes_its_params_place() {
        let params = [
            param("a.contract#Card.inset", "4px"),
            param("b.contract#Card.inset", "6px"),
            param("a.contract#Row.gap", "2px"),
        ];
        assert_eq!(named(&params, "Card.inset"), Err(Unnamed::Ambiguous));
        assert_eq!(
            named(&params, "b.contract#Card.inset").map(|p| &p.default[..]),
            Ok("6px")
        );
        assert_eq!(named(&params, "Row.gap").map(|p| &p.default[..]), Ok("2px"));
        assert_eq!(named(&params, "Row.nope"), Err(Unnamed::Unknown));
        let design =
            crate::parse("tamber t\n\nparams\n  Row.gap = 9px\n  Card.inset = 1px\n").unwrap();
        let laid = laid(&design, &params);
        let value = |n: &str| {
            laid.tokens
                .iter()
                .find(|t| t.name.value.ends_with(n))
                .map(|t| t.value.value.clone())
        };
        assert_eq!(value("Row.gap").as_deref(), Some("9px"));
        assert_eq!(
            value("Card.inset").as_deref(),
            Some("4px"),
            "an ambiguous line sets nothing"
        );
        assert!(crate::check_design(laid, &crate::Vocab::default()).is_ok());
    }
}
