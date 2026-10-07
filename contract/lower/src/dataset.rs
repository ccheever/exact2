//! `data-*` (@ref LLP 1075.003 §3.3): the app's own words on any element,
//! as HTML's custom data attributes. A node's words lower to one `dataset`
//! row — keys sorted, values strings — through the `NativeProps` opcode
//! (LLP 1024 §9), so a new word never touches the schema or the tag table.
//! On the web they are real attributes (`el.dataset`); a native hatch reads
//! a route's (LLP 1075.003 §3.2). A word names the app's meaning, not a
//! platform property, and never makes a view (§1.2). The app declares its
//! words in `app.json` (`"data"`), and the bake refuses one it lacks.

use crate::{err, LowerError, Lowerer};
use contract_syntax::{Attr, Expr, File, Node, Span};
use contract_types::{Scope, Ty};
use exact_kernel::PropId;
use exact_plan::asm::Asm;
use exact_plan::{BindingKind, BindingsRow};

/// Words a host already writes as `data-<word>` on the elements it makes:
/// the web host's own markers (`host/web`, `host/web-js`), and the props it
/// has no DOM name for, written as `data-<the prop's name, lowercased>`.
/// `host/web`'s `every_data_name_the_host_writes_is_a_reserved_word` keeps
/// the second half whole.
const HOST_WORDS: [&str; 85] = [
    "accept",
    "accessibilitybusy",
    "accessibilitydisabled",
    "accessibilitymodal",
    "accessibilityvaluetext",
    "action",
    "activate",
    "agent-view",
    "app-src",
    "autofocus",
    "bitmapheight",
    "bitmapwidth",
    "boot",
    "boot-ms",
    "button-style",
    "carry-type",
    "colorspace",
    "colortype",
    "context-preview",
    "dataset",
    "depth",
    "destructive",
    "digest",
    "done",
    "error",
    "estimateditemheight",
    "estimateditemwidth",
    "exiting",
    "fieldstyle",
    "flow-fragment",
    "flow-hold",
    "focusable",
    "frame-callback-ms",
    "gpu-input",
    "gpu-ms",
    "headcanonical",
    "headdescription",
    "headedited",
    "headimage",
    "headrobots",
    "headstatus",
    "headtitle",
    "heightdragfor",
    "hitslop",
    "hatch",
    "hyphen",
    "initialitemcount",
    "keyboarddismissmode",
    "listitemkey",
    "liststyle",
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
    "reordergroup",
    "scroll",
    "scrolldocument",
    "scrolllocked",
    "scrollrestoration",
    "scrollstart",
    "selectable",
    "seq",
    "shared-element",
    "showsscrollindicator",
    "site",
    "surface",
    "symbol-fill",
    "symbol-path",
    "symboleffectvalue",
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
/// `exact`, `bitmap` or `symbol` and anything under them.
pub fn reserved(word: &str) -> bool {
    let under = |prefix: &str| word == prefix || word.starts_with(&format!("{prefix}-"));
    HOST_WORDS.contains(&word) || under("exact") || under("bitmap") || under("symbol")
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
    attr_words(file, |a| word(&a.name).map(str::to_owned))
}

/// Every `hatch` word the file's views use, with where (LLP 1075.003.000
/// Q1): the driver checks them against `app.json` `hatches`
/// (`bake-undeclared-hatch`). Only a literal is a word; lowering refuses the
/// rest.
pub fn hatch_words(file: &File) -> Vec<(String, Span)> {
    attr_words(file, |a| match &a.value {
        Expr::Str(w, _) if a.name == "hatch" => Some(w.clone()),
        _ => None,
    })
}

fn attr_words(file: &File, pick: impl Fn(&Attr) -> Option<String> + Copy) -> Vec<(String, Span)> {
    fn walk(
        nodes: &[Node],
        out: &mut Vec<(String, Span)>,
        pick: impl Fn(&Attr) -> Option<String> + Copy,
    ) {
        for n in nodes {
            match n {
                Node::Element {
                    attrs, children, ..
                } => {
                    out.extend(attrs.iter().filter_map(|a| pick(a).map(|w| (w, a.span))));
                    walk(children, out, pick);
                }
                Node::Use { children, .. } => walk(children, out, pick),
                Node::When {
                    then, otherwise, ..
                } => {
                    walk(then, out, pick);
                    walk(otherwise, out, pick);
                }
                Node::Each { body, .. } => walk(body, out, pick),
                Node::Match { some, none, .. } => {
                    walk(&some.1, out, pick);
                    walk(none, out, pick);
                }
                Node::Children { .. } => {}
            }
        }
    }
    let mut out = Vec::new();
    for c in &file.components {
        walk(&c.view, &mut out, pick);
    }
    out
}

/// A `hatch` the lowering refuses (LLP 1075.003.000 §3.1): on a module tag,
/// whose instance is already the module's; anything but a literal, since a
/// node hatched at run time would need its view made mid-row; a value that is
/// not a word.
fn hatch_refusal(tag: &str, a: &Attr) -> Option<LowerError> {
    let refused = |id: &'static str, message: String| err::<()>(id, message, a.span).err();
    if crate::is_module_tag(tag) {
        return refused(
            "lower-hatch-module",
            format!("`{tag}` is a module view, already the module's own: it takes no `hatch`"),
        );
    }
    match &a.value {
        Expr::Str(w, _) if is_word(w) => None,
        Expr::Str(w, _) => refused(
            "lower-hatch-word",
            format!("`hatch=\"{w}\"` is not a word: lowercase words of letters and digits joined by `-`"),
        ),
        _ => refused(
            "lower-hatch-value",
            "`hatch` takes a literal word, never a binding: the build decides which nodes leave the fast path".to_owned(),
        ),
    }
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
        if let Some(e) = attrs
            .iter()
            .find(|a| a.name == "hatch")
            .and_then(|a| hatch_refusal(tag, a))
        {
            self.errors.push(e);
        }
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
