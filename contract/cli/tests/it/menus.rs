//! LLP 1021 (Menus): HTML's `hr`, a void separator row with the UA
//! stylesheet's rows, and the confirmation shape the compiler refuses where a
//! literal shows an Apple host could not present it (`lower-alertdialog`),
//! rather than at the tap.

use exact_kernel::{BorderStyle, Color, ColorValue, Dimension, Kernel, Offer, Overflow, PropId};
use exact_kernel::{StyleId, StyleMask};
use exact_runner::{DataError, DataSource, Event, Runner, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn boot(src: &str) -> Runner<NoData> {
    let src = src.replace("  resource items = items() as shape list<string>\n", "");
    let plan = contract::compile(&src).unwrap_or_else(|e| panic!("{e}"));
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn node<'a>(r: &'a Runner<NoData>, id: &str) -> exact_kernel::NodeRef<'a> {
    let key = r.kernel().find_by_test_id(id)[0];
    r.kernel().node_by_key(key).unwrap()
}

/// A component whose view is `body`, indented under one `column`; `items`
/// is a list from the data module.
fn app(body: &str) -> String {
    let body = body
        .lines()
        .map(|l| format!("      {l}\n"))
        .collect::<String>();
    format!(
        "component App\n  resource items = items() as shape list<string>\n  state busy = false\n  state n = 0\n  action go\n    n = n + 1\n  action pick(s: string)\n    n = n + 1\n  view\n    column\n{body}"
    )
}

fn refused(body: &str) -> (String, String) {
    let e = contract::compile(&app(body)).unwrap_err();
    (e.id.to_string(), e.message)
}

#[test]
fn an_hr_is_a_view_with_the_ua_sheets_rows_and_its_own_win() {
    let mut r = boot(&app(
        "view width=200\n  hr testId=\"rule\"\n  hr testId=\"flat\" margin=0 border-style=\"solid\" border-color=\"#336699\"\n  hr testId=\"red\" color=\"red\" border-color=\"currentcolor\"",
    ));
    let root = r.kernel().roots()[0];
    r.kernel_mut()
        .compute_layout(root, Offer::definite(400.0, 400.0))
        .unwrap();
    let rule = node(&r, "rule");
    assert_eq!(rule.props.str(PropId::SemanticTag), Some("hr"));
    let s = rule.style;
    // `margin: 0.5em auto`, the em against the 16px font.
    assert_eq!(s.margin_top, Dimension::Points(8.0));
    assert_eq!(s.margin_bottom, Dimension::Points(8.0));
    assert_eq!(s.margin_left, Dimension::Auto);
    assert_eq!(s.margin_right, Dimension::Auto);
    assert_eq!(s.border_style_top, BorderStyle::Inset);
    assert_eq!(s.border_style_left, BorderStyle::Inset);
    assert_eq!(s.border_widths(), [1.0; 4]);
    assert_eq!(s.overflow_x, Overflow::Hidden);
    assert_eq!(s.overflow_y, Overflow::Hidden);
    // A block in a block: the parent's width, its two borders tall.
    assert_eq!((rule.frame.width, rule.frame.height), (200.0, 2.0));
    // `currentcolor` inset paints Chrome's grey pair whatever `color` is
    // (Chrome 154's pixels; its UA sheet gives `hr` no `border-color`).
    let current = rule
        .computed_style(StyleMask::of(StyleId::TextColor))
        .text_color;
    assert_eq!(current, ColorValue::Fixed(Color::rgba(128, 128, 128, 255)));
    let grey = |v| ColorValue::Fixed(Color::rgba(v, v, v, 255));
    assert_eq!(
        s.border_colors(current),
        [grey(154), grey(238), grey(238), grey(154)]
    );
    // Red, and `currentcolor` written: still the grey pair, as Chrome paints.
    let red = node(&r, "red");
    let red_current = red
        .computed_style(StyleMask::of(StyleId::TextColor))
        .text_color;
    assert_eq!(red_current, ColorValue::Fixed(Color::rgba(255, 0, 0, 255)));
    assert_eq!(
        red.style.border_colors(red_current),
        [grey(154), grey(238), grey(238), grey(154)]
    );
    // The author's rows replace the sheet's.
    let flat = node(&r, "flat").style;
    assert_eq!(flat.margin_top, Dimension::Points(0.0));
    assert_eq!(flat.margin_left, Dimension::Points(0.0));
    assert_eq!(flat.border_style_bottom, BorderStyle::Solid);
    let blue = ColorValue::Fixed(Color::rgba(0x33, 0x66, 0x99, 255));
    assert_eq!(flat.border_colors(current), [blue; 4]);
}

#[test]
fn an_hr_is_void_and_inset_is_a_border_style() {
    let (id, message) = refused("hr\n  text \"no\"");
    assert_eq!(id, "lower-void", "{message}");
    let r = boot(&app("view testId=\"box\" border=\"2px inset #808080\""));
    let s = node(&r, "box").style;
    assert_eq!(s.border_style_right, BorderStyle::Inset);
    let shade = |v| ColorValue::Fixed(Color::rgba(v, v, v, 255));
    assert_eq!(
        s.border_colors(ColorValue::Fixed(Color::BLACK)),
        [shade(44), shade(212), shade(212), shade(44)]
    );
}

const CANCEL: &str =
    "  button popovertarget=\"c\" popovertargetaction=\"hide\"\n    text \"Cancel\"";

#[test]
fn a_confirmation_or_chooser_shape_compiles() {
    for body in [
        // One action, explanatory text and a cancel: Messages' Block Contact.
        format!("column id=\"c\" popover=\"auto\" role=\"alertdialog\"\n  text \"Block?\"\n  button press=go popovertarget=\"c\" popovertargetaction=\"hide\"\n    text \"Block\"\n{CANCEL}"),
        // A chooser: one action per row of an `each`, then the cancel.
        format!("column id=\"c\" popover=\"auto\" role=\"alertdialog\"\n  each s in items key=s\n    button press=pick(s) popovertarget=\"c\" popovertargetaction=\"hide\"\n      text s\n{CANCEL}"),
        // A cancel on each arm of one `when` is still one cancel.
        "column id=\"c\" popover=\"auto\" role=\"alertdialog\"\n  button press=go popovertarget=\"c\" popovertargetaction=\"hide\"\n    text \"Go\"\n  when busy\n    button popovertarget=\"c\" popovertargetaction=\"hide\"\n      text \"Stop\"\n  else\n    button popovertarget=\"c\" popovertargetaction=\"hide\"\n      text \"Cancel\"".into(),
        // A value known only at run time is the host's to check.
        "column id=\"c\" popover=\"auto\" role=\"alertdialog\"\n  button press=go popovertarget=\"c\" popovertargetaction=(busy ? \"hide\" : \"show\")\n    text \"Go\"".into(),
        // A modal `dialog` confirmation closes by `command`.
        "dialog id=\"c\" role=\"alertdialog\" closedby=\"any\"\n  button press=go commandfor=\"c\" command=\"close\"\n    text \"Delete\"\n  button commandfor=\"c\" command=\"close\"\n    text \"Cancel\"".into(),
        // A modal laid out as written is a `dialog` role: any content.
        "dialog id=\"c\" role=\"dialog\" aria-modal=true closedby=\"any\"\n  column gap=12\n    text \"Delete account?\"\n    row\n      button commandfor=\"c\" command=\"close\"\n        text \"Cancel\"\n      button press=go commandfor=\"c\" command=\"close\"\n        text \"Delete\"".into(),
        // Not an alertdialog: a menu takes any rows, `hr` among them.
        format!("column id=\"c\" popover=\"auto\" role=\"menu\"\n  button press=go popovertarget=\"c\" popovertargetaction=\"hide\"\n    text \"Go\"\n  hr\n{CANCEL}"),
    ] {
        if let Err(e) = contract::compile(&app(&body)) {
            panic!("{body}\n{e}");
        }
    }
}

#[test]
fn a_shape_the_native_sheet_cannot_present_is_refused_naming_the_row() {
    let action =
        "  button press=go popovertarget=\"c\" popovertargetaction=\"hide\"\n    text \"Go\"";
    let open = "column id=\"c\" popover=\"auto\" role=\"alertdialog\"";
    for (body, why) in [
        (format!("{open}\n{action}\n  hr\n{CANCEL}"), "a `hr` row"),
        (format!("{open}\n{action}\n  view\n    text \"Hi\""), "a `view` row"),
        // A laid-out modal is told what takes any content (x2apps onboarding).
        (format!("{open}\n  column\n{action}"), "give it `role=\"dialog\"`"),
        // The hosts present buttons only: a `link` action is any other row.
        (
            format!("{open}\n{action}\n  link press=go popovertarget=\"c\" popovertargetaction=\"hide\"\n    text \"Go\""),
            "a `link` row is not text, an action or the cancel",
        ),
        (
            format!("{open}\n  button press=go popovertarget=\"c\"\n    text \"Go\""),
            "does not also hide it",
        ),
        (
            format!("{open}\n{action}\n  button\n    text \"Huh\""),
            "neither an action nor the cancel",
        ),
        (format!("{open}\n{action}\n{CANCEL}\n{CANCEL}"), "a second cancel"),
        (
            format!("{open}\n{action}\n  each s in items key=s\n    button popovertarget=\"c\" popovertargetaction=\"hide\"\n      text s"),
            "inside `each`",
        ),
        (format!("{open}\n  text \"Sure?\"\n{CANCEL}"), "has no action"),
        (
            "dialog id=\"c\" role=\"alertdialog\"\n  button press=go commandfor=\"c\" command=\"close\"\n    text \"Go\"".into(),
            "closedby=\"any\"",
        ),
    ] {
        let (id, message) = refused(&body);
        assert_eq!(id, "lower-alertdialog", "{body}: {message}");
        assert!(message.contains(why), "{body}: {message}");
    }
}

#[test]
fn one_cancel_component_on_both_arms_of_a_when_is_one_cancel() {
    // Inlined, the two uses are two nodes with one span; they are still on
    // different arms. Two uses on one arm are still two cancels.
    let cancel = "component Cancel\n  view\n    button popovertarget=\"c\" popovertargetaction=\"hide\"\n      text \"Cancel\"\n";
    let open = "column id=\"c\" popover=\"auto\" role=\"alertdialog\"\n  button press=go popovertarget=\"c\" popovertargetaction=\"hide\"\n    text \"Go\"";
    let arms = format!(
        "{}{cancel}",
        app(&format!(
            "{open}\n  when busy\n    Cancel()\n  else\n    Cancel()"
        ))
    );
    if let Err(e) = contract::compile(&arms) {
        panic!("{arms}\n{e}");
    }
    let twice = format!(
        "{}{cancel}",
        app(&format!("{open}\n  when busy\n    Cancel()\n    Cancel()"))
    );
    let e = contract::compile(&twice).unwrap_err();
    assert_eq!(e.id, "lower-alertdialog", "{}", e.message);
    assert!(e.message.contains("a second cancel"), "{}", e.message);
}

#[test]
fn dialog_actions_validate_ids_and_deliver_cancel_and_close_without_payloads() {
    for command in [
        "showModal()",
        "showModal(1)",
        "showModal(\"a\", \"b\")",
        "close(false)",
        "close(\"a\", \"b\")",
    ] {
        let source = format!(
            "component App\n  action go\n    {command}\n  view\n    button \"Go\" press=go\n"
        );
        let error = contract::compile(&source).unwrap_err();
        assert_eq!(error.id, "type-dialog-command", "{command}: {error}");
    }
    let mut r = boot(
        r#"component App
  state heard = ""
  action opened
    showModal("form")
    close("form")
    close()
  action cancelled
    heard = `${heard}cancel;`
    preventDefault()
  action closed
    heard = `${heard}close;`
  view
    column
      button "Open" press=opened testId="open"
      dialog id="form" cancel=cancelled close=closed testId="form"
        text "Dialog"
      dialog testId="unhandled"
        text "No handlers"
      text heard testId="heard"
"#,
    );
    let open = node(&r, "open").id;
    r.dispatch(open, Event::Press).unwrap();
    let commands = r.take_commands();
    assert_eq!(
        commands
            .iter()
            .map(|c| (c.name.as_str(), c.args.len()))
            .collect::<Vec<_>>(),
        [("showModal", 1), ("close", 1), ("close", 0)]
    );
    let form = node(&r, "form").id;
    r.dispatch(form, Event::Cancel).unwrap();
    assert_eq!(r.take_commands()[0].name, "preventDefault");
    r.dispatch(form, Event::Close).unwrap();
    assert_eq!(
        node(&r, "heard").props.str(PropId::Text),
        Some("cancel;close;")
    );
    let unhandled = node(&r, "unhandled").id;
    r.dispatch(unhandled, Event::Cancel).unwrap();
    r.dispatch(unhandled, Event::Close).unwrap();
    assert!(r.take_commands().is_empty());
}
