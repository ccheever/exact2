//! LLP 1021 (Menus): HTML's `hr`, a menu's separator row, and the shape a
//! confirmation must have to present natively — refused here where it can
//! be seen, rather than at the tap on the iOS host (`MenusIOS.swift`,
//! `openConfirmation`, which logs `confirmation <id> refused: <why>`).

use crate::{err, LowerError, Lowerer};
use contract_syntax::{Attr, Expr, Node, Span};
use exact_kernel::StyleId;

/// HTML's `hr`, as the platform draws a separator (LLP 1115 §3, wave 1):
/// the UA stylesheet's `margin: 0.5em auto; overflow: hidden`, and in place
/// of its two-toned `inset` 1px box a single solid top edge in the
/// separator role (`-exact-separator`: `separatorColor` on Apple). The edge
/// is `currentcolor`, so an author's `color` or `border-*` still wins
/// (LLP 1021 D1).
pub(crate) const HR: &[(StyleId, &str)] = &[
    (StyleId::MarginTop, "0.5em"),
    (StyleId::MarginBottom, "0.5em"),
    (StyleId::MarginLeft, "auto"),
    (StyleId::MarginRight, "auto"),
    (StyleId::BorderStyleTop, "solid"),
    (StyleId::BorderWidthTop, "1"),
    (StyleId::TextColor, "-exact-separator"),
    (StyleId::OverflowX, "hidden"),
    (StyleId::OverflowY, "hidden"),
];

/// A `dialog` whose role is `alertdialog` is presented as the platform's
/// confirmation, which a tap outside dismisses: its `closedby` is `any`
/// unless written (LLP 1115 D6), so the web's `dialog` behaves the same.
/// The attributes with it added, or `None` when nothing is implied.
pub(crate) fn implied_closedby(tag: &str, attrs: &[Attr], span: Span) -> Option<Vec<Attr>> {
    if tag != "dialog" || literal(attrs, "role") != Some("alertdialog") || has(attrs, "closedby") {
        return None;
    }
    let mut attrs = attrs.to_vec();
    attrs.push(Attr {
        name: "closedby".into(),
        value: Expr::Str("any".into(), span),
        span,
    });
    Some(attrs)
}

fn literal<'a>(attrs: &'a [Attr], name: &str) -> Option<&'a str> {
    match &attrs.iter().rev().find(|a| a.name == name)?.value {
        Expr::Str(v, _) => Some(v),
        _ => None,
    }
}

fn has(attrs: &[Attr], name: &str) -> bool {
    attrs.iter().any(|a| a.name == name)
}

/// A confirmation's direct rows, as its host reads them: `each`, `when`
/// and `match` are not elements, so the rows they produce are its rows.
/// `repeats` is whether a row is inside an `each`; `arms` is the `when` and
/// `match` arms it is on, outermost first, each as the choice node's address
/// and the arm's index. Addresses, not spans: a component used on two arms
/// is inlined as two nodes with one span.
struct Row<'a> {
    tag: &'a str,
    attrs: Vec<Attr>,
    span: Span,
    repeats: bool,
    arms: Vec<(usize, usize)>,
}

impl Row<'_> {
    /// Whether `self` and `other` are on different arms of one `when` or
    /// `match`, so never both present.
    fn exclusive(&self, other: &Row) -> bool {
        for (a, b) in self.arms.iter().zip(&other.arms) {
            if a.0 != b.0 {
                return false;
            }
            if a.1 != b.1 {
                return true;
            }
        }
        false
    }
}

impl Lowerer<'_> {
    /// `hr` is void; a popover or `dialog` whose role is `alertdialog` is a
    /// confirmation, which an Apple host presents as the platform's sheet.
    /// Its rows are text, actions (a `button` with `press` that hides it)
    /// and at most one cancel (a `button` without `press` that hides it),
    /// with at least one action. What a literal shows breaks that is refused;
    /// a value known only at run time is left to the host.
    pub(crate) fn check_menu_shapes(
        &self,
        tag: &str,
        attrs: &[Attr],
        children: &[Node],
        span: Span,
    ) -> Result<(), LowerError> {
        if tag == "hr" {
            if let Some(child) = children.first() {
                return err(
                    "lower-void",
                    "`hr` is a void element, HTML's separator: it takes no children",
                    child.span(),
                );
            }
        }
        let dialog = tag == "dialog";
        if literal(attrs, "role") != Some("alertdialog") || !(dialog || has(attrs, "popover")) {
            return Ok(());
        }
        let Some(id) = literal(attrs, "id") else {
            return Ok(());
        };
        // An unwritten `closedby` is implied (`implied_closedby`); one written
        // otherwise asks for what the native confirmation cannot do.
        let closedby = attrs.iter().rev().find(|a| a.name == "closedby");
        if dialog && closedby.is_some_and(|c| matches!(&c.value, Expr::Str(v, _) if v != "any")) {
            return err(
                "lower-alertdialog",
                format!("alertdialog `dialog` `{id}` cannot take this `closedby`: a native confirmation is dismissed by a tap outside it, so it is `closedby=\"any\"` (leave it unwritten)"),
                span,
            );
        }
        let mut rows = Vec::new();
        self.rows(children, false, &mut Vec::new(), &mut rows);
        // How it hides this one: the popover's `popovertarget` with
        // `popovertargetaction="hide"`, the dialog's `commandfor` with
        // `command="close"`. `None` when a value is not a literal.
        let (target, action, hide) = if dialog {
            ("commandfor", "command", "close")
        } else {
            ("popovertarget", "popovertargetaction", "hide")
        };
        let hides = |attrs: &[Attr]| -> Option<bool> {
            let names = |name| attrs.iter().any(|a| a.name == name);
            if !names(target) {
                return Some(false);
            }
            let to = literal(attrs, target)?;
            let how = if names(action) {
                literal(attrs, action)?
            } else {
                "toggle"
            };
            Some(to == id && how == hide)
        };
        let how = format!("`{target}=\"{id}\" {action}=\"{hide}\"`");
        let refuse = |row: &Row, why: String| {
            err(
                "lower-alertdialog",
                format!("alertdialog `{id}` cannot be presented as a native confirmation: {why}. Its rows are text, actions (a `button` with `press` and {how}) and at most one cancel (a `button` without `press`, with {how})"),
                row.span,
            )
        };
        let mut actions = 0;
        let mut cancels: Vec<&Row> = Vec::new();
        for row in &rows {
            match row.tag {
                "text" => {}
                // The hosts present buttons only; a `link` is refused as
                // any other element row is.
                "button" if has(&row.attrs, "press") => {
                    if hides(&row.attrs) == Some(false) {
                        return refuse(row, format!("this action's `press` does not also hide it ({how})"));
                    }
                    actions += 1;
                }
                "button" => match hides(&row.attrs) {
                    Some(false) => {
                        return refuse(row, "this `button` has no `press` and does not hide it, so it is neither an action nor the cancel".into())
                    }
                    _ if row.repeats => {
                        return refuse(row, "a cancel inside `each` can repeat, and there is at most one".into())
                    }
                    _ => cancels.push(row),
                },
                // An author laying a modal out (onboarding's Delete account?
                // in a `column` of rows) wants `role="dialog"`: say so.
                other => return refuse(row, format!("a `{other}` row is not text, an action or the cancel; to lay the dialog out yourself, give it `role=\"dialog\"` (with `aria-modal=true`), which takes any content and is drawn as written")),
            }
        }
        for (i, later) in cancels.iter().enumerate() {
            if cancels[..i].iter().any(|earlier| !earlier.exclusive(later)) {
                return refuse(later, "a second cancel; there is at most one".into());
            }
        }
        if actions == 0 {
            return err(
                "lower-alertdialog",
                format!(
                    "alertdialog `{id}` has no action: give it a `button` with `press` and {how}"
                ),
                span,
            );
        }
        Ok(())
    }

    /// The element rows `nodes` produce, through `each`, `when` and `match`;
    /// `arms` is the path of arms `nodes` are on.
    fn rows<'n>(
        &self,
        nodes: &'n [Node],
        repeats: bool,
        arms: &mut Vec<(usize, usize)>,
        out: &mut Vec<Row<'n>>,
    ) {
        for node in nodes {
            let choice = std::ptr::from_ref(node) as usize;
            let branches: Vec<&'n [Node]> = match node {
                Node::Element {
                    tag, attrs, span, ..
                } => {
                    let mut all = self
                        .class_rows(attrs)
                        .ok()
                        .flatten()
                        .map_or_else(Vec::new, |(_, rows)| rows);
                    all.extend(attrs.iter().cloned());
                    out.push(Row {
                        tag,
                        attrs: all,
                        span: *span,
                        repeats,
                        arms: arms.clone(),
                    });
                    continue;
                }
                Node::When {
                    then, otherwise, ..
                } => vec![then, otherwise],
                Node::Match { some, none, .. } => vec![&some.1, none],
                Node::Each { body, .. } => {
                    self.rows(body, true, arms, out);
                    continue;
                }
                // A component's use is inlined before lowering; a slot's
                // fill is its caller's to check.
                Node::Use { .. } | Node::Children { .. } => continue,
            };
            for (arm, branch) in branches.into_iter().enumerate() {
                arms.push((choice, arm));
                self.rows(branch, repeats, arms, out);
                arms.pop();
            }
        }
    }
}
