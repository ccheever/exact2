//! LLP 1021 (Menus): HTML's `hr`, a void separator row, one hairline (LLP
//! 1115.000 D10.3), and the confirmation shape the compiler refuses where a
//! literal shows an Apple host could not present it (`lower-alertdialog`),
//! rather than at the tap.

use exact_kernel::{BorderStyle, Color, ColorValue, Dimension, Kernel, Offer, Overflow, PropId};
use exact_kernel::{StyleId, StyleMask};
use exact_runner::{DataError, DataSource, Runner, Value};

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

/// LLP 1115.000 D10.3: an `hr` is one hairline with no margin of its own:
/// a solid top edge `-exact-hairline` thick in the separator role, where it
/// was Chrome's `margin: 0.5em auto` around a 1px edge. The rows are the
/// compiler's, not the author's (LLP 1115.000 D2), and the author's win.
#[test]
fn an_hr_is_one_hairline_with_no_margin_and_its_authors_rows_win() {
    let mut r = boot(&app(
        "view width=200\n  hr testId=\"rule\"\n  hr testId=\"spaced\" margin=\"8px 0\" border-width=2 border-style=\"solid\" border-color=\"#336699\"\n  hr testId=\"red\" color=\"red\" border-color=\"currentcolor\"\ncolumn width=200 testId=\"stack\"\n  hr testId=\"stacked\"",
    ));
    let root = r.kernel().roots()[0];
    r.kernel_mut()
        .compute_layout(root, Offer::definite(400.0, 400.0))
        .unwrap();
    let rule = node(&r, "rule");
    assert_eq!(rule.props.str(PropId::SemanticTag), Some("hr"));
    let s = rule.style;
    // No margin row at all: Chrome's `0.5em auto` is gone.
    for margin in [
        StyleId::MarginTop,
        StyleId::MarginRight,
        StyleId::MarginBottom,
        StyleId::MarginLeft,
    ] {
        assert!(!s.mask.has(margin), "{margin:?}");
    }
    // The platform's separator (LLP 1115): one solid top edge, no box, one
    // device pixel thick: 1 px where no host has said its display's scale.
    assert_eq!(s.border_style_top, BorderStyle::Solid);
    assert_eq!(s.border_style_left, BorderStyle::None);
    assert_eq!(s.border_widths(), [1.0, 0.0, 0.0, 0.0]);
    assert_eq!(
        s.relative.get(StyleId::BorderWidthTop),
        Some((exact_kernel::style::relative::Unit::Hairline, 1.0))
    );
    assert!(!s.written.has(StyleId::BorderWidthTop), "the compiler's");
    assert_eq!(s.overflow_x, Overflow::Hidden);
    assert_eq!(s.overflow_y, Overflow::Hidden);
    // A block in a block: the parent's width, its one edge tall, at the
    // parent's top with nothing above it.
    let frame = rule.frame;
    assert_eq!((frame.y, frame.width, frame.height), (0.0, 200.0, 1.0));
    // In a flex column it is the column's width too: Chrome's `auto` side
    // margins stopped the stretch there and left it 0 wide.
    assert_eq!(node(&r, "stacked").frame.width, 200.0);
    // The edge is `currentcolor`, the separator role.
    let current = rule
        .computed_style(StyleMask::of(StyleId::TextColor))
        .text_color;
    let separator = exact_kernel::style::roles::role("-exact-separator").unwrap();
    assert_eq!(current, ColorValue::Role(separator));
    assert_eq!(s.border_colors(current)[0], current);
    // The author's `color` wins.
    let red = node(&r, "red");
    let red_current = red
        .computed_style(StyleMask::of(StyleId::TextColor))
        .text_color;
    assert_eq!(red_current, ColorValue::Fixed(Color::rgba(255, 0, 0, 255)));
    assert_eq!(red.style.border_colors(red_current)[0], red_current);
    // The author's rows replace the sheet's: a margin, a width, a colour.
    let spaced = node(&r, "spaced");
    let flat = spaced.style;
    assert_eq!(flat.margin_top, Dimension::Points(8.0));
    assert_eq!(flat.margin_left, Dimension::Points(0.0));
    assert!(flat.written.has(StyleId::MarginTop));
    assert_eq!(flat.border_style_bottom, BorderStyle::Solid);
    assert_eq!(flat.border_widths(), [2.0; 4]);
    assert_eq!(flat.relative.get(StyleId::BorderWidthTop), None);
    let blue = ColorValue::Fixed(Color::rgba(0x33, 0x66, 0x99, 255));
    assert_eq!(flat.border_colors(current), [blue; 4]);
}

/// LLP 1115.000 D10.3: `-exact-hairline` is a width an author may write, in
/// place of `0.5`: on a border's width, in the `border` shorthand, as a
/// height or a gap, literal or bound. It is one device pixel, so it follows
/// the display's scale the host reports, and with it the layout; a size of
/// text is no width, and the compiler says so.
#[test]
fn a_hairline_is_one_device_pixel_of_the_scale_the_host_reports() {
    let src = "component App\n  state on = true\n  action flip\n    on = not on\n  view\n    column width=200 testId=\"root\"\n      box testId=\"all\" height=20 border=\"-exact-hairline solid -exact-separator\"\n      box testId=\"bottom\" height=20 border-bottom-width=\"-exact-hairline\" border-bottom-style=\"solid\"\n      box testId=\"bound\" height=20 border-top-width=(on ? \"-exact-hairline\" : 0) border-top-style=\"solid\"\n      box testId=\"line\" height=\"-exact-hairline\"\n      row gap=\"-exact-HAIRLINE\" testId=\"gapped\"\n        box width=20 height=20\n        box width=20 height=20 testId=\"second\"\n      hr testId=\"rule\"\n      box testId=\"half\" height=20 border-bottom-width=0.5 border-bottom-style=\"solid\"\n      button press=flip testId=\"flip\"\n        text \"flip\"\n";
    let mut r = boot(src);
    let root = r.kernel().roots()[0];
    let boxes = |r: &mut Runner<NoData>| {
        r.kernel_mut()
            .compute_layout(root, Offer::definite(400.0, 400.0))
            .unwrap();
        let frame = |id: &str| node(r, id).frame;
        [
            frame("all").height,
            frame("bottom").height,
            frame("bound").height,
            frame("line").height,
            frame("second").x,
            frame("rule").height,
            frame("half").height,
        ]
    };
    assert_eq!(r.display_scale(), 1.0, "until a host says");
    assert_eq!(boxes(&mut r), [22.0, 21.0, 21.0, 1.0, 21.0, 1.0, 20.5]);
    // A 2x display: every hairline is half a point, and layout with it.
    assert!(r.set_display_scale(2.0).unwrap().is_some());
    assert_eq!(boxes(&mut r), [21.0, 20.5, 20.5, 0.5, 20.5, 0.5, 20.5]);
    assert!(
        r.set_display_scale(2.0).unwrap().is_none(),
        "the same scale again commits nothing"
    );
    // A 3x display: a third, where an authored 0.5 is a pixel and a half.
    r.set_display_scale(3.0).unwrap();
    let third = 1.0 / 3.0;
    let at3 = boxes(&mut r);
    for (got, want) in at3.iter().zip([
        20.0 + 2.0 * third,
        20.0 + third,
        20.0 + third,
        third,
        20.0 + third,
        third,
        20.5,
    ]) {
        assert!((got - want).abs() < 1e-4, "{at3:?}");
    }
    // A bound hairline leaves and comes back at the display's scale.
    let flip = node(&r, "flip").id;
    r.dispatch(flip, exact_runner::Event::Press).unwrap();
    assert_eq!(boxes(&mut r)[2], 20.0);
    r.dispatch(flip, exact_runner::Event::Press).unwrap();
    assert!((boxes(&mut r)[2] - (20.0 + third)).abs() < 1e-4);
    // A scale that is no scale is refused and changes nothing.
    assert!(r.set_display_scale(0.0).is_err());
    assert!(r.set_display_scale(f64::NAN).is_err());
    assert_eq!(r.display_scale(), 3.0);
    // A hairline is a width: not a size of text.
    for row in ["font-size", "line-height"] {
        let e = contract::compile(&format!(
            "component App\n  view\n    text \"a\" {row}=\"-exact-hairline\"\n"
        ))
        .unwrap_err();
        assert!(
            e.message.contains("`-exact-hairline` is a width"),
            "{row}: {e}"
        );
    }
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
        // Its `closedby` is implied, by its shape (LLP 1115.000 D13.4).
        "dialog id=\"c\" role=\"alertdialog\"\n  button press=go commandfor=\"c\" command=\"close\"\n    text \"Delete\"\n  button commandfor=\"c\" command=\"close\"\n    text \"Cancel\"".into(),
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
        (
            format!("{open}\n  column\n{action}"),
            "a modal you lay out yourself is a sheet, a route with `navigationPresentation=\"modal\"`",
        ),
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
            "dialog id=\"c\" role=\"alertdialog\" closedby=\"none\"\n  button press=go commandfor=\"c\" command=\"close\"\n    text \"Go\"".into(),
            "closedby=\"any\"",
        ),
    ] {
        let (id, message) = refused(&body);
        assert_eq!(id, "lower-alertdialog", "{body}: {message}");
        assert!(message.contains(why), "{body}: {message}");
    }
}

/// LLP 1115.000 D13.4: an alert has no outside. A `dialog` whose rows are an
/// alert's (explanatory text, one cancel, at most three buttons: the rule
/// every host presents by, `exact_kernel::confirmation::is_alert`) implies
/// `closedby="closerequest"`: Escape and Back cancel it, a tap outside does
/// nothing. Any other confirmation is the platform's chooser, which a tap
/// outside dismisses: `any`, as every `alertdialog` implied until
/// 2026-10-11. The row is the compiler's unless the author writes one.
#[test]
fn an_alert_shaped_dialog_implies_closerequest_and_a_chooser_any() {
    let text = "  text \"This cannot be undone.\"";
    let action = |title: &str| {
        format!("  button press=go commandfor=\"c\" command=\"close\"\n    text \"{title}\"")
    };
    let cancel = "  button commandfor=\"c\" command=\"close\"\n    text \"Cancel\"";
    let open = "dialog id=\"c\" role=\"alertdialog\" aria-label=\"Delete it?\" testId=\"c\"";
    let implied = |rows: &[&str]| {
        let r = boot(&app(&format!("{open}\n{}", rows.join("\n"))));
        node(&r, "c").props.str(PropId::Closedby).map(str::to_owned)
    };
    let (delete, keep, later) = (action("Delete"), action("Keep"), action("Later"));
    for (rows, expected, why) in [
        (vec![text, &delete, cancel], "closerequest", "an alert"),
        (
            vec![text, &delete, &keep, cancel],
            "closerequest",
            "three buttons are still an alert's",
        ),
        (vec![&delete, cancel], "any", "no text: a chooser"),
        (vec![text, &delete], "any", "no cancel"),
        (
            vec![text, &delete, &keep, &later, cancel],
            "any",
            "four buttons: a chooser",
        ),
    ] {
        assert_eq!(implied(&rows).as_deref(), Some(expected), "{why}");
    }
    // The most that can show at once: an action on each arm of a `when` is
    // one button, and a text on one arm is its text.
    let arms = format!(
        "  when busy\n  {}\n  else\n  {}",
        delete.replace('\n', "\n  "),
        keep.replace('\n', "\n  ")
    );
    assert_eq!(
        implied(&[text, &arms, &arms, cancel]).as_deref(),
        Some("closerequest"),
        "two choices of one action each, and the cancel: three buttons"
    );
    let sometimes = "  when busy\n    text \"Working.\"";
    assert_eq!(
        implied(&[sometimes, &delete, cancel]).as_deref(),
        Some("closerequest"),
        "a text on one arm"
    );
    // The author's `closedby` is the author's, and written.
    for (written, shape) in [
        ("any", vec![text, &delete, cancel]),
        ("closerequest", vec![text, &delete, cancel]),
        ("any", vec![&delete, cancel]),
    ] {
        let body = format!("{open} closedby=\"{written}\"\n{}", shape.join("\n"));
        let r = boot(&app(&body));
        assert_eq!(node(&r, "c").props.str(PropId::Closedby), Some(written));
    }
    // A popover is dismissed as popovers are: nothing is implied.
    let popover = format!(
        "column id=\"c\" popover=\"auto\" role=\"alertdialog\" testId=\"c\"\n  text \"Block?\"\n  button press=go popovertarget=\"c\" popovertargetaction=\"hide\"\n    text \"Block\"\n{CANCEL}"
    );
    let r = boot(&app(&popover));
    assert_eq!(node(&r, "c").props.str(PropId::Closedby), None);
    // What a shape cannot do is refused, and no longer by saying a native
    // confirmation "is dismissed by a tap outside it": an alert is not.
    let each = "  each s in items key=s\n    button press=pick(s) commandfor=\"c\" command=\"close\"\n      text s";
    for (closedby, rows, why) in [
        (
            "none",
            vec![text, &delete, cancel],
            "an alert always has its cancel, which Escape and Back press",
        ),
        (
            "sometimes",
            vec![text, &delete, cancel],
            "an alert's is `closerequest`",
        ),
        (
            "closerequest",
            vec![&delete, cancel],
            "it is the platform's chooser",
        ),
        (
            "closerequest",
            vec![text, each, cancel],
            "a button under `each`",
        ),
    ] {
        let (id, message) = refused(&format!(
            "{open} closedby=\"{closedby}\"\n{}",
            rows.join("\n")
        ));
        assert_eq!(id, "lower-alertdialog", "{closedby}: {message}");
        assert!(message.contains(why), "{closedby}: {message}");
        assert!(
            message.contains(&format!("cannot take `closedby=\"{closedby}\"`")),
            "{message}"
        );
    }
    let (_, alert) = refused(&format!(
        "{open} closedby=\"none\"\n{}",
        [text, &delete, cancel].join("\n")
    ));
    assert!(
        !alert.contains("dismissed by a tap outside"),
        "an alert has no outside: {alert}"
    );
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
fn menu_bar_placement_is_a_button_hint_with_fixed_literal_destinations() {
    for menu in ["app", "file", "edit", "view", "go", "window", "help"] {
        let r = boot(&app(&format!(
            "button \"Check for Updates…\" -exact-menu=\"{menu}\" press=go testId=\"updates\""
        )));
        let prop = PropId::from_name("menuPlacement").expect("menu placement prop");
        assert_eq!(node(&r, "updates").props.str(prop), Some(menu));
    }
    for body in [
        "button \"Update\" -exact-menu=\"Develop\" press=go",
        "button \"Update\" -exact-menu=(busy ? \"app\" : \"file\") press=go",
        "column -exact-menu=\"app\"",
    ] {
        assert_eq!(refused(body).0, "lower-menu-placement", "{body}");
    }
}

#[test]
fn menu_bar_removal_accepts_only_root_literal_standard_ids() {
    let ids = "close-all autofill show-tab-bar show-all-tabs enter-full-screen minimize zoom bring-all-to-front show-previous-tab show-next-tab move-tab-to-new-window merge-all-windows";
    for value in [ids, "", "show-tab-bar show-tab-bar"] {
        let source = format!(
            "component App\n  view\n    column -exact-menu-remove=\"{value}\" testId=\"root\"\n"
        );
        let r = boot(&source);
        let prop = PropId::from_name("menuRemove").expect("menu removal prop");
        assert_eq!(node(&r, "root").props.str(prop), Some(value));
    }
    for source in [
        "component App\n  view\n    column -exact-menu-remove=\"develop\"\n".to_owned(),
        "component App\n  state removal = \"show-tab-bar\"\n  view\n    column -exact-menu-remove=removal\n".to_owned(),
        app("column -exact-menu-remove=\"show-tab-bar\""),
        app("when busy\n  column -exact-menu-remove=\"autofill\""),
    ] {
        let e = contract::compile(&source).unwrap_err();
        assert_eq!(e.id, "lower-menu-remove", "{source}: {e}");
        for id in ids.split_whitespace() { assert!(e.message.contains(id), "{e}"); }
    }
}
