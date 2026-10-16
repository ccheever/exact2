//! LLP 1021 (Menus): HTML's `hr`, a menu's separator row, and the shape a
//! confirmation must have to present natively — refused here where it can
//! be seen, rather than at the tap on the iOS host (`MenusIOS.swift`,
//! `openConfirmation`, which logs `confirmation <id> refused: <why>`). And
//! what its shape implies of its outside (LLP 1115.000 D13.4): an alert has
//! none.

use crate::{err, LowerError, Lowerer};
use contract_syntax::{Attr, Expr, Node, Span};
use exact_kernel::StyleId;

/// LLP 1021 §5.3: a root's bounded opt-out from the Mac's standard items.
pub(crate) fn check_menu_remove(attrs: &[Attr], root: bool) -> Result<(), LowerError> {
    const IDS: &[&str] = &[
        "close-all",
        "autofill",
        "show-tab-bar",
        "show-all-tabs",
        "enter-full-screen",
        "minimize",
        "zoom",
        "bring-all-to-front",
        "show-previous-tab",
        "show-next-tab",
        "move-tab-to-new-window",
        "merge-all-windows",
    ];
    for attr in attrs.iter().filter(|a| a.name == "-exact-menu-remove") {
        if !root
            || !matches!(&attr.value, Expr::Str(value, _) if value.split_whitespace().all(|id| IDS.contains(&id)))
        {
            return err("lower-menu-remove", format!(
                "`-exact-menu-remove` belongs to a root and takes a literal whitespace-separated list from: {}",
                IDS.join(", ")
            ), attr.span);
        }
    }
    Ok(())
}

/// HTML's `hr`, as the platform draws a separator (LLP 1115 §3, wave 1; LLP
/// 1115.000 D10.3): one hairline, a single solid top edge one device pixel
/// thick (`-exact-hairline`) in the separator role (`-exact-separator`:
/// `separatorColor` on Apple), with no margin of its own, as `Divider()` and
/// a table's separator have none and the common resets zero an `hr`'s. In
/// place of the UA stylesheet's `margin: 0.5em auto` and its two-toned
/// `inset` 1px box, which the web's sheet resets (`host/web/index.html`);
/// its `overflow: hidden` stays. The edge is `currentcolor`, so an author's
/// `color`, `border-*` or `margin` still wins (LLP 1021 D1).
pub(crate) const HR: &[(StyleId, &str)] = &[
    (StyleId::BorderStyleTop, "solid"),
    (StyleId::BorderWidthTop, "-exact-hairline"),
    (StyleId::TextColor, "-exact-separator"),
    (StyleId::OverflowX, "hidden"),
    (StyleId::OverflowY, "hidden"),
];

/// A `dialog` whose role is `alertdialog` is presented as the platform's
/// confirmation, and its `closedby`, unless written, is what that
/// confirmation does (LLP 1115 D6; LLP 1115.000 D13.4), so every host and
/// the web's `dialog` behave alike. `alert` is whether its rows are an
/// alert's ([`Lowerer::check_menu_shapes`]). **An alert has no outside:**
/// `closerequest`, so Escape and Back cancel it and a tap outside does
/// nothing, as `UIAlertController(.alert)`, `NSAlert` and Material's dialog
/// do. Any other shape is the platform's chooser (an action sheet, a menu),
/// which a tap outside dismisses: `any`. The attributes with it added, or
/// `None` when nothing is implied.
pub(crate) fn implied_closedby(
    tag: &str,
    attrs: &[Attr],
    span: Span,
    alert: bool,
) -> Option<Vec<Attr>> {
    if tag != "dialog" || literal(attrs, "role") != Some("alertdialog") || has(attrs, "closedby") {
        return None;
    }
    let mut attrs = attrs.to_vec();
    attrs.push(Attr {
        name: "closedby".into(),
        value: Expr::Str(if alert { "closerequest" } else { "any" }.into(), span),
        span,
        written: crate::IMPLIED,
    });
    Some(attrs)
}

/// The most of `rows` that can show at once: rows on different arms of one
/// `if` or `match` never show together, so a choice counts its fullest
/// arm. `depth` is how far along each row's path of arms this looks.
fn most(rows: &[&Row], depth: usize) -> usize {
    let mut here = 0;
    let mut choices: std::collections::BTreeMap<
        usize,
        std::collections::BTreeMap<usize, Vec<&Row>>,
    > = Default::default();
    for row in rows {
        match row.arms.get(depth) {
            None => here += 1,
            Some(&(choice, arm)) => choices
                .entry(choice)
                .or_default()
                .entry(arm)
                .or_default()
                .push(row),
        }
    }
    let fullest = |arms: &std::collections::BTreeMap<usize, Vec<&Row>>| {
        arms.values().map(|rows| most(rows, depth + 1)).max()
    };
    here + choices.values().filter_map(fullest).sum::<usize>()
}

/// Whether a confirmation's rows, as written, are an alert's: the shape
/// rule every host presents by (`exact_kernel::confirmation::is_alert`),
/// read off the source. It counts the most rows that can show at once: a
/// text on any arm is its text, and a button under `for` can be any
/// number of buttons, so it is a chooser's. Where an `if` leaves a state
/// that is not an alert's, the host that meets it presents what it finds.
fn alert_shaped(rows: &[Row]) -> bool {
    let of = |keep: &dyn Fn(&Row) -> bool| rows.iter().filter(|r| keep(r)).collect::<Vec<_>>();
    // A prompt's text field is what it asks, as explanatory text is.
    let texts = of(&|r| r.tag == "text" || text_field(r));
    let actions = of(&|r| r.tag == "button" && has(&r.attrs, "press"));
    let cancels = of(&|r| r.tag == "button" && !has(&r.attrs, "press"));
    !actions.iter().chain(&cancels).any(|r| r.repeats)
        && exact_kernel::confirmation::is_alert(texts.len(), most(&actions, 0), most(&cancels, 0))
}

/// HTML's one-line text fields, what a prompt asks with
/// (`UIAlertController.addTextField`, `NSAlert.accessoryView`): an `input`
/// whose `type` edits a line of text. A `type` known only at run time is
/// the host's to read.
fn text_field(row: &Row) -> bool {
    const TYPES: &[&str] = &[
        "text", "password", "email", "url", "search", "tel", "number",
    ];
    row.tag == "input" && literal(&row.attrs, "type").is_none_or(|t| TYPES.contains(&t))
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

/// A confirmation's direct rows, as its host reads them: `for`, `if`
/// and `match` are not elements, so the rows they produce are its rows.
/// `repeats` is whether a row is inside a `for`; `arms` is the `if` and
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
    /// Whether `self` and `other` are on different arms of one `if` or
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
    /// Its rows are text, text fields when it is an alert (a prompt),
    /// actions (a `button` with `press` that hides it) and at most one
    /// cancel (a `button` without `press` that hides it), with at least one
    /// action. What a literal shows breaks that is refused;
    /// a value known only at run time is left to the host. Returns whether
    /// it is a confirmation whose rows are an alert's ([`alert_shaped`]),
    /// which decides the `closedby` a `dialog` implies and may take.
    pub(crate) fn check_menu_shapes(
        &self,
        tag: &str,
        attrs: &[Attr],
        children: &[Node],
        span: Span,
    ) -> Result<bool, LowerError> {
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
            return Ok(false);
        }
        let mut rows = Vec::new();
        self.rows(children, false, &mut Vec::new(), &mut rows);
        let alert = alert_shaped(&rows);
        let Some(id) = literal(attrs, "id") else {
            return Ok(alert);
        };
        // An unwritten `closedby` is implied (`implied_closedby`); one written
        // otherwise asks for what the native confirmation of this shape
        // cannot do (LLP 1115.000 D13.4). An alert has no outside and always
        // has its cancel: `closerequest`, or `any` for the platform's
        // dismissible form. A chooser is dismissed by a tap outside: `any`.
        let closedby = attrs.iter().rev().find(|a| a.name == "closedby");
        if let Some(Expr::Str(value, _)) = closedby.filter(|_| dialog).map(|c| &c.value) {
            let refused = match (alert, value.as_str()) {
                (_, "any") | (true, "closerequest") => None,
                (true, "none") => Some("an alert always has its cancel, which Escape and Back press, so it is `closedby=\"closerequest\"` (leave it unwritten: a tap outside does nothing), or `closedby=\"any\"` for the platform's dismissible form, an action sheet or a light-dismiss dialog"),
                (true, _) => Some("an alert's is `closerequest` (leave it unwritten: Escape and Back cancel it, a tap outside does nothing) or `any`, the platform's dismissible form"),
                (false, _) => Some("with no explanatory text, no cancel, more than three buttons or a button under `for` it is the platform's chooser (an action sheet, a menu), which a tap outside dismisses, so it is `closedby=\"any\"` (leave it unwritten)"),
            };
            if let Some(why) = refused {
                return err(
                    "lower-alertdialog",
                    format!(
                        "alertdialog `dialog` `{id}` cannot take `closedby=\"{value}\"`: {why}"
                    ),
                    span,
                );
            }
        }
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
                format!("alertdialog `{id}` cannot be presented as a native confirmation: {why}. Its rows are text, text fields (`input`), actions (a `button` with `press` and {how}) and at most one cancel (a `button` without `press`, with {how})"),
                row.span,
            )
        };
        let mut actions = 0;
        let mut fields: Option<&Row> = None;
        let mut cancels: Vec<&Row> = Vec::new();
        for row in &rows {
            match row.tag {
                "text" => {}
                // A prompt (a rename's): the alert with a text field, whose
                // edits are the field's `input` as typing in place is.
                "input" if text_field(row) => {
                    if row.repeats {
                        return refuse(row, "a text field inside `for` can repeat; a prompt's fields are written one by one".into());
                    }
                    fields = fields.or(Some(row));
                }
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
                        return refuse(row, "a cancel inside `for` can repeat, and there is at most one".into())
                    }
                    _ => cancels.push(row),
                },
                // An author laying a modal out (onboarding's Delete account?
                // in a `column` of rows) wants a sheet: say so.
                other => return refuse(row, format!("a `{other}` row is not text, an action or the cancel; a modal you lay out yourself is a sheet, a route with `navigationPresentation=\"modal\"` (docs/recipes/navigation-stack.md)")),
            }
        }
        for (i, later) in cancels.iter().enumerate() {
            if cancels[..i].iter().any(|earlier| !earlier.exclusive(later)) {
                return refuse(later, "a second cancel; there is at most one".into());
            }
        }
        if let Some(field) = fields.filter(|_| !alert) {
            return refuse(field, "a text field belongs to the alert's shape, a prompt: with it one cancel and at most three buttons in all, none under `for`".into());
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
        Ok(alert)
    }

    /// The element rows `nodes` produce, through `for`, `if` and `match`;
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
