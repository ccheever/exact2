//! What an open top-layer box inherits for point reads from the tree it is
//! lifted out of: it escapes its ancestors' clips and transforms, not their
//! `inert` (HTML's inert subtrees, which only a modal `dialog` escapes) nor
//! their `pointer-events` and `visibility` (CSS inheritance follows the
//! tree, not the top layer).

use exact_kernel::{Kernel, Offer};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// `elementFromPoint` at (x, y) once the boxes named `open` are open in the
/// top layer, in an app whose view is `view`.
fn hit_with(view: &str, open: &[&str], (x, y): (u32, u32)) -> Option<String> {
    let source = format!(
        r#"component App
  state hit = "unset"
  action read
    hit = match elementFromPoint({x}, {y}) {{ case some(id) => id, case none => "none" }}
  view
    column
{view}
      button press=read testId="read" position="absolute" top=500
        text "read"
"#
    );
    let mut r = Runner::boot(
        contract::compile(&source).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let root = r.kernel().roots()[0];
    r.kernel_mut()
        .compute_layout(root, Offer::definite(400.0, 800.0))
        .unwrap();
    let ids: Vec<_> = open
        .iter()
        .map(|name| {
            let key = r.kernel().find_by_id(name)[0];
            r.kernel().arena().local_id(key.index)
        })
        .collect();
    r.top_layer(&ids);
    r.act("read", vec![]).unwrap();
    r.slot("hit").and_then(Value::as_str).map(str::to_owned)
}

/// A popover open under an `inert` ancestor is not reached, and its button
/// is once `inert` goes; it inherits `visibility` and `pointer-events` from
/// its ancestors, which its button can set again.
#[test]
fn an_open_popover_inherits_inert_visibility_and_pointer_events() {
    let hit = |page: &str, button: &str| {
        let view = format!(
            r#"      column id="page" height=400 {page}
        column id="pop" popover="auto" position="absolute" left=0 top=0 width=200 height=100
          button id="pop-button" width=100 height=40 {button}
            text "pop""#
        );
        hit_with(&view, &["pop"], (20, 20))
    };
    assert_eq!(hit("", "").as_deref(), Some("pop-button"));
    assert_eq!(
        hit("inert=true", "").as_deref(),
        Some("none"),
        "inert above it"
    );
    let hidden = r#"visibility="hidden""#;
    assert_eq!(hit(hidden, "").as_deref(), Some("none"), "hidden above it");
    assert_eq!(
        hit(hidden, r#"visibility="visible""#).as_deref(),
        Some("pop-button"),
        "visible again on the button"
    );
    let none = r#"pointer-events="none""#;
    assert_eq!(
        hit(none, "").as_deref(),
        Some("none"),
        "no pointer events above it"
    );
    assert_eq!(
        hit(none, r#"pointer-events="auto""#).as_deref(),
        Some("pop-button"),
        "auto again on the button"
    );
}

/// A modal `dialog` escapes the inertness of its ancestors (HTML's modal
/// dialogs), whether or not its host reports it open, and so does a popover
/// open inside it; `inert` on the dialog itself still holds, and a dialog
/// that is a popover is not modal.
#[test]
fn a_modal_dialog_escapes_an_inert_ancestor_but_not_its_own_inert() {
    let view = |dialog: &str| {
        format!(
            r#"      column id="page" height=400 inert=true
        dialog id="dialog" position="absolute" left=0 top=0 width=200 height=200 {dialog}
          button id="dialog-button" width=100 height=40
            text "dialog"
          column id="menu" popover="auto" position="absolute" left=0 top=100 width=100 height=40
            button id="menu-button" width=100 height=40
              text "menu""#
        )
    };
    let open = ["dialog", "menu"];
    assert_eq!(
        hit_with(&view(""), &open, (20, 20)).as_deref(),
        Some("dialog-button")
    );
    assert_eq!(
        hit_with(&view(""), &open, (20, 120)).as_deref(),
        Some("menu-button"),
        "a popover open in the dialog"
    );
    assert_eq!(
        hit_with(&view(""), &["menu"], (20, 120)).as_deref(),
        Some("menu-button"),
        "the dialog not in the host's top layer, as iOS's sheet is not"
    );
    assert_eq!(
        hit_with(&view(r#"popover="manual""#), &open, (20, 20)).as_deref(),
        Some("none"),
        "a dialog that is a popover is not modal"
    );
    assert_eq!(
        hit_with(&view("inert=true"), &open, (20, 20)).as_deref(),
        Some("none"),
        "inert itself"
    );
    assert_eq!(
        hit_with(&view("inert=true"), &open, (20, 120)).as_deref(),
        Some("none")
    );
}
