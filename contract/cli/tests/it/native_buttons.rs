//! LLP 1069.011: a `button` whose `appearance` is the literal `auto` is the
//! platform's own button — a `Control` of type `button`, its `text` and
//! symbol `image` children its face, its style from `buttonStyles` — and
//! everything Contract can see that the platform cannot draw is refused.

use exact_kernel::{Kernel, NodeType, PressFace, PropId, PropValue};
use exact_runner::{DataError, DataSource, Event, Runner, RunnerError, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.to_string()))
    }
}

fn boot(src: &str) -> Runner<NoData> {
    Runner::boot(
        contract::compile(src).unwrap_or_else(|e| panic!("{e}")),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn view_of(r: &Runner<NoData>, test_id: &str) -> u32 {
    let key = r.kernel().find_by_test_id(test_id)[0];
    r.kernel().node_by_key(key).unwrap().id
}

/// A component whose view is `body`, indented under one `column`.
fn app(body: &str) -> String {
    let body = body
        .lines()
        .map(|l| format!("      {l}\n"))
        .collect::<String>();
    format!(
        "keyframes fade\n  from opacity=0\n  to opacity=1\nkeyframes flash\n  from background-color=\"#ff0000\"\n  to background-color=\"#0000ff\"\ncomponent App\n  state busy = false\n  state n = 0\n  action go\n    n = n + 1\n  action keys(k: string)\n    n = n + 1\n  view\n    column\n{body}"
    )
}

fn refused(body: &str) -> (String, String) {
    let e = contract::compile(&app(body)).unwrap_err();
    (e.id.to_string(), e.message)
}

#[test]
fn a_native_button_is_a_control_and_an_ordinary_one_is_unchanged() {
    let r = boot(&app(
        "button appearance=\"auto\" buttonStyle=\"glass\" press=go testId=\"native\" aria-label=\"Send\"\n  image \"symbol:send\"\nbutton press=go testId=\"custom\"\n  text \"Plain\"",
    ));
    let k = r.kernel();
    let native = k.node(view_of(&r, "native")).unwrap();
    assert_eq!(native.node_type, NodeType::Control);
    assert_eq!(native.props.str(PropId::Type), Some("button"));
    assert_eq!(native.props.str(PropId::AccessibilityRole), Some("button"));
    assert_eq!(
        native.props.get(PropId::ButtonStyle),
        Some(&PropValue::Str("glass".into()))
    );
    assert_eq!(native.style.appearance, exact_kernel::Appearance::Auto);
    assert_eq!(native.style.box_sizing, exact_kernel::BoxSizing::BorderBox);
    let custom = k.node(view_of(&r, "custom")).unwrap();
    assert_eq!(custom.node_type, NodeType::Pressable);
    assert_eq!(custom.style.appearance, exact_kernel::Appearance::None);
}

#[test]
fn its_face_is_its_children_read_live() {
    let mut r = boot(&app(
        "button appearance=\"auto\" press=go testId=\"b\"\n  image \"symbol:send\"\n  when n == 0\n    text \"Send\"\n  else\n    text \"Sent  again\"",
    ));
    let b = view_of(&r, "b");
    assert_eq!(
        r.kernel().press_face(b).unwrap(),
        PressFace {
            title: Some("Send".into()),
            subtitle: None,
            placement: exact_kernel::ButtonImagePlacement::Leading,
            symbol: Some("send".into()),
            raster: false,
            leading: true,
            label: None,
            fits: true,
        }
    );
    r.dispatch(b, Event::Press).unwrap();
    assert_eq!(
        r.kernel().press_face(b).unwrap().title.as_deref(),
        Some("Sent again")
    );
    // A press is the button's; `input` and `change` are not its events.
    assert!(matches!(
        r.dispatch(
            b,
            Event::Change(exact_runner::ControlValue::Text("x".into()))
        ),
        Err(RunnerError::InvalidEvent { .. } | RunnerError::NoHandler { .. })
    ));
}

#[test]
fn the_switch_is_a_literal_after_classes() {
    // A class carries it, and `buttonStyle` with it.
    let r = boot(&format!(
        "style Primary\n  appearance=\"auto\"\n  buttonStyle=\"bordered-prominent\"\n  accent-color=\"#34c759\"\n{}",
        app("button class=Primary press=go testId=\"b\"\n  text \"Go\"")
    ));
    let b = r.kernel().node(view_of(&r, "b")).unwrap();
    assert_eq!(b.node_type, NodeType::Control);
    assert_eq!(
        b.props.get(PropId::ButtonStyle),
        Some(&PropValue::Str("bordered-prominent".into()))
    );
    // Its own `appearance="none"` wins over the class's.
    let r = boot(&format!(
        "style Primary\n  appearance=\"auto\"\n{}",
        app("button class=Primary appearance=\"none\" press=go testId=\"b\"\n  text \"Go\"")
    ));
    assert_eq!(
        r.kernel().node(view_of(&r, "b")).unwrap().node_type,
        NodeType::Pressable
    );
    for (src, id) in [
        (app("button appearance=(busy ? \"auto\" : \"none\") press=go\n  text \"Go\""), "lower-appearance"),
        // One arm sets it: a bound value.
        (format!("style A\n  appearance=\"auto\"\nstyle B\n  opacity=1\n{}", app("button class=(busy ? A : B) press=go\n  text \"Go\"")), "lower-appearance"),
        // A styleable prop is set by both arms or neither.
        (format!("style A\n  appearance=\"auto\"\n  buttonStyle=\"glass\"\nstyle B\n  appearance=\"auto\"\n{}", app("button class=(busy ? A : B) press=go\n  text \"Go\"")), "lower-style-prop"),
    ] {
        let e = contract::compile(&src).unwrap_err();
        assert_eq!(e.id, id, "{e}");
    }
}

#[test]
fn the_style_is_a_name_in_the_table_on_a_native_button_only() {
    boot(&app("button appearance=\"auto\" buttonStyle=(busy ? \"glass\" : \"prominent-glass\") press=go\n  text \"Go\""));
    for (body, id, says) in [
        (
            "button appearance=\"auto\" buttonStyle=\"frosted\" press=go\n  text \"Go\"",
            "lower-button-style",
            "not a button style",
        ),
        (
            "button buttonStyle=\"glass\" press=go\n  text \"Go\"",
            "lower-button-style",
            "native button",
        ),
        (
            "box buttonStyle=\"glass\"",
            "lower-button-style",
            "native button",
        ),
    ] {
        let (got, message) = refused(body);
        assert_eq!(got, id, "{body}: {message}");
        assert!(message.contains(says), "{body}: {message}");
    }
}

#[test]
fn its_face_is_a_title_a_symbol_or_both() {
    boot(&app("button appearance=\"auto\" press=go aria-label=\"Add\"\n  image (busy ? \"symbol:close\" : \"symbol:add\")"));
    // An SF Symbol by name, literal or computed (LLP 1035.004.000).
    boot(&app("button appearance=\"auto\" press=go aria-label=\"Send\"\n  image \"symbol:sf/paperplane.fill\""));
    boot(&app("button appearance=\"auto\" press=go\n  image `symbol:sf/${busy ? \"hourglass\" : \"paperplane\"}`\n  text \"Send\""));
    for (body, says) in [
        (
            "button appearance=\"auto\" press=go width=40 height=40",
            "neither",
        ),
        (
            "button appearance=\"auto\" press=go\n  text \"A\"\n  text \"B\"\n  text \"C\"",
            "at most two",
        ),
        (
            "button appearance=\"auto\" press=go\n  image \"symbol:send\"",
            "aria-label",
        ),
        (
            "button appearance=\"auto\" press=go\n  box width=4 height=4",
            "not a `box`",
        ),
        (
            "button appearance=\"auto\" press=go\n  text \"Go\" font-family=\"system-ui\"",
            "takes no `font-family`",
        ),
        (
            "button appearance=\"auto\" press=go aria-label=\"x\"\n  image \"photo.png\"",
            "is a symbol",
        ),
        (
            "button appearance=\"auto\" press=go aria-label=\"x\"\n  image \"symbol:sf/\"",
            "is a symbol",
        ),
        (
            "button appearance=\"auto\" press=go\n  when busy\n    text \"Wait\"",
            "neither",
        ),
        // A blank title is no title (astra's code review).
        (
            "button appearance=\"auto\" press=go\n  image \"symbol:add\"\n  text",
            "give it one",
        ),
        (
            "button appearance=\"auto\" press=go\n  image \"symbol:add\"\n  text \" \"",
            "always empty",
        ),
        (
            "button appearance=\"auto\" press=go\n  image \"symbol:add\"\n  text (busy ? \"Add\" : \"\")",
            "aria-label",
        ),
        // A label that can be empty is no label (grok's code review).
        (
            "button appearance=\"auto\" press=go aria-label=(busy ? \"Add\" : \"\")\n  image \"symbol:add\"",
            "never empty",
        ),
        (
            "button appearance=\"auto\" press=go aria-label=\"\"\n  image \"symbol:add\"",
            "never empty",
        ),
    ] {
        let (id, message) = refused(body);
        assert!(matches!(id.as_str(), "lower-button-content" | "lower-button-style-attr"), "{body}: {message}");
        assert!(message.contains(says), "{body}: {message}");
    }
}

#[test]
fn its_box_carries_place_size_opacity_transforms_and_accent_only() {
    boot(&app(
        "button appearance=\"auto\" press=go key=keys width=120 margin-top=8 align-self=\"center\" display=(busy ? \"none\" : \"flex\") opacity=0.5 translate=\"4px 0px\" accent-color=\"#34c759\" transition=\"opacity 200ms\" animation=\"fade 300ms\"\n  text \"Go\"",
    ));
    for body in [
        "button appearance=\"auto\" press=go background-color=\"#ff0000\"\n  text \"Go\"",
        "button appearance=\"auto\" press=go filter=\"blur(2px)\"\n  text \"Go\"",
        "button appearance=\"auto\" press=go box-sizing=\"content-box\"\n  text \"Go\"",
        "button appearance=\"auto\" press=go -exact-press-scale=0.9\n  text \"Go\"",
        "button appearance=\"auto\" press=go backgroundMaterial=\"glass\"\n  text \"Go\"",
        "button appearance=\"auto\" press=go glassGroup=8\n  text \"Go\"",
        "button appearance=\"auto\" press=go transition=\"background-color 200ms\"\n  text \"Go\"",
        "button appearance=\"auto\" press=go animation=\"flash 300ms\"\n  text \"Go\"",
        // Its face's layout and its hit test are the platform's (both code reviews).
        "button appearance=\"auto\" press=go display=\"grid\"\n  text \"Go\"",
        "button appearance=\"auto\" press=go direction=\"rtl\"\n  text \"Go\"",
    ] {
        let (id, message) = refused(body);
        assert_eq!(id, "lower-button-style-attr", "{body}: {message}");
    }
}

/// LLP 1069.011.000: a native button is a tab, a toolbar item, a header's
/// button, a menu row, a swipe action and a shortcut's button; it is not a
/// menu's invoker, a link, a submit, below a menu row, inside a custom button
/// or in a canvas. A row that only closes its popover or dialog — a
/// confirmation's action or cancel — is not an invoker (astra's code review).
#[test]
fn it_goes_where_a_button_goes_and_admits_dialog_and_popover_invokers() {
    for body in [
        "row role=\"tablist\"\n  button appearance=\"auto\" role=\"tab\" aria-selected=true press=go\n    text \"Tab\"",
        "row role=(busy ? \"tablist\" : \"group\")\n  button appearance=\"auto\" role=(busy ? \"tab\" : \"button\") press=go\n    text \"Tab\"",
        "header\n  button appearance=\"auto\" press=go\n    text \"Back\"",
        "row toolbarPlacement=\"window\" role=\"toolbar\"\n  button appearance=\"auto\" buttonStyle=\"filled\" toolbarPlacement=\"navigation\" press=go\n    text \"Edit\"",
        "button appearance=\"auto\" press=go aria-keyshortcuts=\"Meta+S\"\n  text \"Save\"",
        "column id=\"menu\" popover=\"auto\" role=\"menu\"\n  button appearance=\"auto\" role=\"menuitem\" press=go\n    image \"symbol:send\"\n    text \"Send\"\n  when busy\n    button appearance=\"auto\" press=go\n      text \"Wait\"",
        "scroll swipeContent=\"body\" swipeTrailing=\"mute\" overflow-x=\"scroll\" width=200 height=40\n  row id=\"body\" width=200 height=40\n    button appearance=\"auto\" press=go\n      text \"Open\"\n  button id=\"mute\" appearance=\"auto\" press=go aria-label=\"Mute\"\n    image \"symbol:send\"",
        "column id=\"confirm\" popover=\"auto\" role=\"alertdialog\"\n  text \"Delete it?\"\n  button appearance=\"auto\" popovertarget=\"confirm\" popovertargetaction=\"hide\" press=go destructive=true\n    text \"Delete\"\n  button appearance=\"auto\" popovertarget=\"confirm\" popovertargetaction=\"hide\"\n    text \"Cancel\"",
        "dialog id=\"ask\" closedby=\"any\"\n  button appearance=\"auto\" commandfor=\"ask\" command=\"close\" press=go\n    text \"Send\"",
    ] {
        contract::compile(&app(body)).unwrap_or_else(|e| panic!("{body}: {e}"));
    }
    for body in [
        "button appearance=\"auto\" press=go commandfor=\"menu\"\n  text \"More\"",
        "button appearance=\"auto\" press=go href=\"/x\"\n  text \"Open\"",
        "button appearance=\"auto\" press=go role=\"dialog\"\n  text \"Go\"",
        "button appearance=\"auto\" press=go type=\"submit\"\n  text \"Go\"",
        "column id=\"menu\" popover=\"auto\"\n  row\n    button appearance=\"auto\" press=go\n      text \"Deep\"",
        // A projection makes one item of a custom button, dropping what it holds.
        "row role=\"toolbar\" toolbarPlacement=\"window\"\n  button press=go\n    button appearance=\"auto\" press=go\n      text \"Inner\"",
    ] {
        let (id, message) = refused(body);
        assert_eq!(id, "lower-button-context", "{body}: {message}");
        assert!(
            message.contains("custom `button`") || message.contains("`\"button\"`"),
            "{body}: {message}"
        );
    }
}

/// LLP 1069.011.000 D1: every button has a face — a custom one read as a
/// native one is — and only a button has one.
#[test]
fn every_button_has_a_face_custom_or_native() {
    let r = boot(&app(
        "button press=go testId=\"custom\" aria-label=\"Save it\"\n  image \"symbol:send\"\n  text \"Save\"\nbutton press=go testId=\"raster\"\n  image \"https://example.com/a.png\"\nbutton appearance=\"auto\" press=go testId=\"blank\" aria-label=\"Add\"\n  text (busy ? \"Add\" : \"\")\n  image \"symbol:add\"\nbutton press=go testId=\"badged\"\n  text \"Inbox\"\n  box width=4 height=4\nbox testId=\"plain\" width=4 height=4",
    ));
    let face = |id: &str| r.kernel().press_face(view_of(&r, id));
    assert_eq!(
        face("custom"),
        Some(PressFace {
            title: Some("Save".into()),
            subtitle: None,
            placement: exact_kernel::ButtonImagePlacement::Leading,
            symbol: Some("send".into()),
            raster: false,
            leading: true,
            label: Some("Save it".into()),
            fits: true,
        })
    );
    let raster = face("raster").unwrap();
    assert!(raster.raster && raster.symbol.is_none() && raster.fits);
    let blank = face("blank").unwrap();
    assert_eq!(blank.title, None);
    assert!(
        blank.leading,
        "a symbol after a blank title leads (grok's code review)"
    );
    let badged = face("badged").unwrap();
    assert_eq!(badged.title.as_deref(), Some("Inbox"));
    assert!(!badged.fits, "a box beside the title does not fit");
    assert_eq!(face("plain"), None, "a box is not a button");
}

#[test]
fn every_native_face_row_is_admitted_and_styleable() {
    for row in [
        "gap=8",
        "column-gap=4",
        "row-gap=6",
        "flex-direction=\"row\"",
        "flex-direction=\"column\"",
        "font-size=20",
        "font-weight=600",
        "white-space=\"nowrap\"",
        "line-clamp=2",
        "text-align=\"start\"",
        "color=\"#ff0000\"",
        "border-radius=12",
        "padding=8",
        "padding-top=1",
        "padding-right=2",
        "padding-bottom=3",
        "padding-left=4",
        "pointer-events=\"none\"",
        "pointer-events=\"auto\"",
        "align-items=\"center\"",
        "justify-content=\"center\"",
        "-exact-control-size=\"mini\"",
        "-exact-control-size=\"small\"",
        "-exact-control-size=\"medium\"",
        "-exact-control-size=\"large\"",
        "-exact-corner-style=\"dynamic\"",
        "-exact-corner-style=\"small\"",
        "-exact-corner-style=\"medium\"",
        "-exact-corner-style=\"large\"",
        "-exact-corner-style=\"capsule\"",
    ] {
        let body = format!("button appearance=\"auto\" {row}\n  text \"Go\"");
        contract::compile(&app(&body)).unwrap_or_else(|e| panic!("{row}: {e}"));
        let src = format!(
            "style Face\n  {row}\n{}",
            app("button appearance=\"auto\" class=Face\n  text \"Go\"")
        );
        contract::compile(&src).unwrap_or_else(|e| panic!("class {row}: {e}"));
    }
    for row in [
        "font-size=20",
        "font-weight=600",
        "color=\"#ff0000\"",
        "white-space=\"nowrap\"",
        "line-clamp=2",
        "text-align=\"end\"",
    ] {
        for index in 0..2 {
            let children = if index == 0 {
                format!("text \"Go\" {row}\n  text \"Again\"")
            } else {
                format!("text \"Go\"\n  text \"Again\" {row}")
            };
            let src = app(&format!("button appearance=\"auto\"\n  {children}"));
            contract::compile(&src).unwrap_or_else(|e| panic!("{row}: {e}"));
        }
        let src = format!(
            "style Label\n  {row}\n{}",
            app("button appearance=\"auto\"\n  text \"Go\" class=Label")
        );
        contract::compile(&src).unwrap_or_else(|e| panic!("text class {row}: {e}"));
    }
    for row in [
        "-exact-tint-color=\"#00ff00\"",
        "font-size=24",
        "font-weight=700",
    ] {
        let src = app(&format!(
            "button appearance=\"auto\"\n  image \"symbol:send\" {row}\n  text \"Go\""
        ));
        contract::compile(&src).unwrap_or_else(|e| panic!("{row}: {e}"));
        let src = format!(
            "style Icon\n  {row}\n{}",
            app("button appearance=\"auto\"\n  image \"symbol:send\" class=Icon\n  text \"Go\"")
        );
        contract::compile(&src).unwrap_or_else(|e| panic!("image class {row}: {e}"));
    }
}

#[test]
fn every_refused_face_row_names_the_custom_button() {
    for row in [
        "background-color=\"#ff0000\"",
        "background-image=\"linear-gradient(red, blue)\"",
        "border=\"1px solid red\"",
        "border-width=1",
        "border-color=\"red\"",
        "border-style=\"solid\"",
        "box-shadow=\"0 1px 2px black\"",
        "filter=\"blur(2px)\"",
        "backdrop-filter=\"blur(2px)\"",
        "font-family=\"system-ui\"",
        "font-style=\"italic\"",
        "line-height=2",
        "letter-spacing=1",
        "text-transform=\"uppercase\"",
        "text-indent=2",
        "text-shadow=\"0 1px black\"",
        "text-overflow=\"ellipsis\"",
        "direction=\"rtl\"",
        "-exact-press-scale=0.9",
        "flex-wrap=\"wrap\"",
        "align-items=\"start\"",
        "justify-content=\"space-between\"",
        "flex-direction=\"row-reverse\"",
        "flex-direction=\"column-reverse\"",
    ] {
        let body = format!("button appearance=\"auto\" {row}\n  text \"Go\"");
        let (id, message) = refused(&body);
        assert_eq!(id, "lower-button-style-attr", "{row}: {message}");
        assert!(message.contains("custom `button`"), "{row}: {message}");
    }
    for row in [
        "width=24",
        "height=24",
        "object-fit=\"contain\"",
        "padding=2",
        "color=\"red\"",
    ] {
        let (id, message) = refused(&format!(
            "button appearance=\"auto\"\n  image \"symbol:send\" {row}\n  text \"Go\""
        ));
        assert_eq!(id, "lower-button-style-attr", "{row}: {message}");
        assert!(message.contains("custom `button`"), "{row}: {message}");
    }
    for body in [
        "button appearance=\"auto\"\n  text \"A\"\n  text \"B\"\n  text \"C\"",
        "button appearance=\"auto\"\n  image \"symbol:add\"\n  image \"symbol:send\"\n  text \"Go\"",
        "button appearance=\"auto\"\n  row\n    text \"Go\"",
        "button appearance=\"auto\"\n  text \"Go\"\n    text \"Again\"",
    ] {
        let (id, message) = refused(body);
        assert_eq!(id, "lower-button-style-attr", "{body}: {message}");
        assert!(message.contains("custom `button`"), "{body}: {message}");
    }
    let src = format!(
        "style Bad\n  width=20\n{}",
        app("button appearance=\"auto\"\n  image \"symbol:send\" class=Bad\n  text \"Go\"")
    );
    let e = contract::compile(&src).unwrap_err();
    assert_eq!(e.id, "lower-button-style-attr", "{e}");
    assert!(e.message.contains("custom `button`"), "{e}");
}

#[test]
fn platform_rows_belong_only_to_native_buttons_and_roundtrip_through_classes() {
    for tag in [
        "box width=20 height=20",
        "text \"Go\"",
        "button",
        "button appearance=\"none\"",
        "input",
    ] {
        for row in [
            "-exact-control-size=\"large\"",
            "-exact-corner-style=\"capsule\"",
        ] {
            let body = if tag.starts_with("button") {
                format!("{tag} {row}\n  text \"Go\"")
            } else {
                format!("{tag} {row}")
            };
            let (id, message) = refused(&body);
            assert_eq!(id, "lower-button-style-attr", "{body}: {message}");
            assert!(message.contains("native button"), "{body}: {message}");
        }
    }
    let mut r = boot(&format!("style A\n  -exact-control-size=\"large\"\n  -exact-corner-style=\"capsule\"\nstyle B\n  opacity=1\n{}", app("button appearance=\"auto\" class=(n == 0 ? A : B) press=go testId=\"b\"\n  text \"Go\"")));
    let b = view_of(&r, "b");
    let style = r.kernel().button_face_style(b).unwrap();
    assert!(style.button.mask.has(exact_kernel::StyleId::ControlSize));
    assert_eq!(style.button.control_size, exact_kernel::ControlSize::Large);
    // A one-sided class arm restores absence rather than stamping the enum default.
    r.dispatch(b, Event::Press).unwrap();
    assert!(!r
        .kernel()
        .button_face_style(b)
        .unwrap()
        .button
        .mask
        .has(exact_kernel::StyleId::ControlSize));
}

#[test]
fn native_invokers_take_bound_or_empty_targets_and_checked_commands() {
    for attrs in [
        "commandfor=\"ask\" command=\"show-modal\"",
        "commandfor=\"ask\" command=\"show-popover\"",
        "commandfor=\"ask\" command=\"toggle-popover\"",
        "commandfor=(busy ? \"\" : \"ask\") command=\"show-modal\"",
        "commandfor=\"\" command=\"show-modal\"",
        "popovertarget=(busy ? \"\" : \"ask\")",
        "popovertarget=\"\"",
        "popovertarget=\"ask\" popovertargetaction=(busy ? \"hide\" : \"show\")",
        "commandfor=\"ask\" command=(busy ? \"show-popover\" : \"toggle-popover\")",
    ] {
        contract::compile(&app(&format!(
            "button appearance=\"auto\" {attrs}\n  text \"Open\""
        )))
        .unwrap_or_else(|e| panic!("{attrs}: {e}"));
    }
    for attrs in [
        "href=\"/x\"",
        "action=\"/x\"",
        "swipeContent=\"x\"",
        "swipeLeading=\"x\"",
        "swipeTrailing=\"x\"",
        "swipeIndicator=\"chevron\"",
        "commandfor=\"ask\" command=\"hide-popover\"",
    ] {
        let (id, message) = refused(&format!(
            "button appearance=\"auto\" {attrs}\n  text \"Go\""
        ));
        assert_eq!(
            id,
            if attrs.starts_with("action=") {
                "analyze-control-parent"
            } else {
                "lower-button-context"
            },
            "{attrs}: {message}"
        );
        assert!(
            message.contains("custom `button`") || attrs.starts_with("action="),
            "{attrs}: {message}"
        );
    }
}
