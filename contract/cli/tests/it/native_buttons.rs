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
            "button appearance=\"auto\" press=go\n  text \"A\"\n  text \"B\"",
            "at most one",
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
            "button appearance=\"auto\" press=go\n  text \"Go\" font-size=20",
            "takes no `font-size`",
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
        assert_eq!(id, "lower-button-content", "{body}: {message}");
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
        "button appearance=\"auto\" press=go padding=8\n  text \"Go\"",
        "button appearance=\"auto\" press=go color=\"#ff0000\"\n  text \"Go\"",
        "button appearance=\"auto\" press=go font-size=20\n  text \"Go\"",
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
        "button appearance=\"auto\" press=go pointer-events=\"none\"\n  text \"Go\"",
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
fn it_goes_where_a_button_goes_but_invokes_no_menu() {
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
        "button appearance=\"auto\" press=go popovertarget=\"menu\"\n  text \"More\"",
        "button appearance=\"auto\" press=go commandfor=\"menu\"\n  text \"More\"",
        "button appearance=\"auto\" press=go href=\"/x\"\n  text \"Open\"",
        "button appearance=\"auto\" press=go role=\"dialog\"\n  text \"Go\"",
        "button appearance=\"auto\" press=go type=\"submit\"\n  text \"Go\"",
        "column id=\"menu\" popover=\"auto\"\n  row\n    button appearance=\"auto\" press=go\n      text \"Deep\"",
        // Only a literal close is not an invoker.
        "button appearance=\"auto\" press=go popovertarget=\"menu\" popovertargetaction=\"toggle\"\n  text \"More\"",
        "button appearance=\"auto\" press=go popovertarget=\"menu\" popovertargetaction=(busy ? \"hide\" : \"show\")\n  text \"More\"",
        "button appearance=\"auto\" press=go commandfor=\"menu\" command=\"show-modal\"\n  text \"More\"",
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
