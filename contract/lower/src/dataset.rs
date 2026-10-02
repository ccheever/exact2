//! `data-*` (@ref LLP 1075.003 §3.3): the app's own words on any element,
//! as HTML's custom data attributes. A node's words lower to one `dataset`
//! row — keys sorted, values strings — through the `NativeProps` opcode
//! (LLP 1024 §9), so a new word never touches the schema or the tag table.
//! On the web they are real attributes (`el.dataset`); a native hook reads
//! a route's (LLP 1075.003 §3.2). A word names the app's meaning, not a
//! platform property, and never makes a view (§1.2). The app declares its
//! words in `app.json` (`"data"`), and the bake refuses one it lacks.

use crate::{err, LowerError, Lowerer};
use contract_syntax::{Attr, File, Node, Span};
use contract_types::{Scope, Ty};
use exact_kernel::PropId;
use exact_plan::asm::Asm;
use exact_plan::{BindingKind, BindingsRow};

/// Words a host already writes as `data-<word>` on the elements it makes:
/// the web host's own markers (`host/web`, `host/web-js`), and the props it
/// has no DOM name for, written as `data-<the prop's name, lowercased>`.
/// `host/web`'s `every_data_name_the_host_writes_is_a_reserved_word` keeps
/// the second half whole.
const HOST_WORDS: [&str; 69] = [
    "accept",
    "accessibilitybusy",
    "accessibilitydisabled",
    "accessibilitymodal",
    "accessibilityvaluetext",
    "action",
    "activate",
    "autofocus",
    "bitmapheight",
    "bitmapwidth",
    "boot",
    "boot-ms",
    "dataset",
    "depth",
    "destructive",
    "digest",
    "done",
    "error",
    "estimateditemheight",
    "estimateditemwidth",
    "exiting",
    "flow-fragment",
    "focusable",
    "frame-callback-ms",
    "gpu-input",
    "gpu-ms",
    "headcanonical",
    "headdescription",
    "headimage",
    "headrobots",
    "headstatus",
    "headtitle",
    "heightdragfor",
    "hitslop",
    "hyphen",
    "initialitemcount",
    "keyboarddismissmode",
    "listitemkey",
    "module",
    "module-ready",
    "multiple",
    "nativeid",
    "nativeviewmodulename",
    "nativeviewprops",
    "navigationdetent",
    "navigationscroll",
    "num",
    "pressed",
    "quote",
    "refreshing",
    "reorderfor",
    "scroll",
    "scrolldocument",
    "scrolllocked",
    "scrollrestoration",
    "selectable",
    "showsscrollindicator",
    "site",
    "surface",
    "symbol-fill",
    "symbol-path",
    "tabindex",
    "testid",
    "toolbarplacement",
    "transformdragfor",
    "view",
    "virtualized",
    "wasm",
    "wrap-flow",
];

/// The word of a `data-` attribute, or `None` for any other name.
pub fn word(name: &str) -> Option<&str> {
    name.strip_prefix("data-")
}

/// HTML's lowercase spelling of a custom data attribute's name:
/// `[a-z][a-z0-9]*` words joined by `-` (the lexer joins a `-` only to a
/// following letter).
pub fn is_word(word: &str) -> bool {
    !word.is_empty()
        && word.split('-').all(|w| {
            let mut chars = w.chars();
            chars.next().is_some_and(|c| c.is_ascii_lowercase())
                && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

/// Whether a host already writes `data-<word>`: one of `HOST_WORDS`, or
/// `exact` or `bitmap` and anything under them.
pub fn reserved(word: &str) -> bool {
    let under = |prefix: &str| word == prefix || word.starts_with(&format!("{prefix}-"));
    HOST_WORDS.contains(&word) || under("exact") || under("bitmap")
}

/// A `data-` attribute's refusal: a word HTML does not spell, or one a
/// host already writes. `None` when it is a word the app may use.
pub(crate) fn refused(a: &Attr) -> Option<LowerError> {
    let w = word(&a.name)?;
    let message = if !is_word(w) {
        format!(
            "`{}` is not a data attribute name: `data-` then lowercase words of letters and digits joined by `-`",
            a.name
        )
    } else if reserved(w) {
        format!(
            "`{}` is written by the web host on its own elements; give the app's word another name (LLP 1075.003 §3.3)",
            a.name
        )
    } else {
        return None;
    };
    Some(LowerError {
        id: "lower-data-word",
        message,
        span: a.span,
    })
}

/// Every `data-` word the file's views use, with where: the driver checks
/// them against the app's declared words (`bake-undeclared-data`).
pub fn data_words(file: &File) -> Vec<(String, Span)> {
    fn walk(nodes: &[Node], out: &mut Vec<(String, Span)>) {
        for n in nodes {
            match n {
                Node::Element {
                    attrs, children, ..
                } => {
                    for a in attrs {
                        if let Some(w) = word(&a.name) {
                            out.push((w.to_owned(), a.span));
                        }
                    }
                    walk(children, out);
                }
                Node::Use { children, .. } => walk(children, out),
                Node::Provide { body, .. } => walk(body, out),
                Node::When {
                    then, otherwise, ..
                } => {
                    walk(then, out);
                    walk(otherwise, out);
                }
                Node::Each { body, .. } => walk(body, out),
                Node::Match { some, none, .. } => {
                    walk(&some.1, out);
                    walk(none, out);
                }
                Node::Children { .. } => {}
            }
        }
    }
    let mut out = Vec::new();
    for c in &file.components {
        walk(&c.view, &mut out);
    }
    out
}

impl Lowerer<'_> {
    /// An element's `data-` words, lowered beside its other bindings (a
    /// refusal joins the run's errors); `head` takes none.
    pub(crate) fn data_row(
        &mut self,
        tag: &str,
        attrs: &[Attr],
        scope: &Scope,
        locals: u16,
        bindings: &mut Vec<BindingsRow>,
        origins: &mut Option<Vec<crate::Origin>>,
    ) {
        let words: Vec<&Attr> = attrs.iter().filter(|a| word(&a.name).is_some()).collect();
        if words.is_empty() {
            return;
        }
        let lowered = if tag == "head" {
            err(
                "lower-attr-tag",
                format!(
                    "`head` takes only {}; `{}` is not one",
                    crate::tags::HEAD_FIELDS.join(", "),
                    words[0].name
                ),
                words[0].span,
            )
        } else {
            self.dataset_bindings(&words, scope, locals, bindings)
        };
        if let Err(e) = lowered {
            self.errors.push(e);
        }
        if let Some(origins) = origins {
            origins.resize(bindings.len(), crate::Origin::Own);
        }
    }

    /// A node's `data-` words as its one `dataset` row: keys sorted, the
    /// last binding of a word winning, each value a string, number or bool
    /// (or an option of one, absent when none), evaluated with the node's
    /// bindings — the module props' rule (LLP 1024 D1).
    pub(crate) fn dataset_bindings(
        &mut self,
        attrs: &[&Attr],
        scope: &Scope,
        locals: u16,
        bindings: &mut Vec<BindingsRow>,
    ) -> Result<(), LowerError> {
        let mut keyed: std::collections::BTreeMap<&str, &Attr> = Default::default();
        for a in attrs {
            if let Some(e) = refused(a) {
                return Err(e);
            }
            keyed.insert(word(&a.name).expect("a data attribute"), a);
        }
        let mut asm = Asm::new();
        let mut depth = locals;
        for (key, a) in &keyed {
            asm.str(self.b.str(key));
            let ty = crate::expr::compile(self, &mut asm, &a.value, scope, &mut depth)?;
            let scalar = |t: &Ty| matches!(t, Ty::String | Ty::Number | Ty::Bool | Ty::Unknown);
            let admitted = match &ty {
                Ty::Option(inner) => scalar(inner),
                t => scalar(t),
            };
            if !admitted {
                return err(
                    "lower-data-value",
                    format!(
                        "`{}` is a string, number or bool, or an option of one",
                        a.name
                    ),
                    a.span,
                );
            }
        }
        asm.native_props(keyed.len() as u32);
        bindings.push(BindingsRow {
            kind: BindingKind::Prop,
            id: PropId::Dataset as u16,
            expr: self.b.code(asm),
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{is_word, reserved};

    #[test]
    fn words_are_lowercase_html_data_names() {
        for ok in ["title", "trailing", "large-title", "h1", "a-b2-c"] {
            assert!(is_word(ok), "{ok}");
        }
        for bad in ["", "Title", "-x", "x-", "x--y", "1x", "x_y", "x.y"] {
            assert!(!is_word(bad), "{bad}");
        }
    }

    #[test]
    fn words_a_host_writes_are_reserved() {
        for taken in [
            "testid",
            "view",
            "exact",
            "exact-id",
            "bitmap-width",
            "nativeviewprops",
            "dataset",
            "scroll",
        ] {
            assert!(reserved(taken), "{taken}");
        }
        for free in [
            "title",
            "trailing",
            "tint",
            "large-title",
            "exactly",
            "id",
            "value",
            "mode",
        ] {
            assert!(!reserved(free), "{free}");
        }
    }
}
