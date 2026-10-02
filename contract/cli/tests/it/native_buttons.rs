//! LLP 1069.011: a `button` whose `appearance` is the literal `auto` is the
//! platform's own button — a `Control` of type `button`, its `text` and
//! symbol `image` children its face, its style from `buttonStyles` — and
//! everything Contract can see that the platform cannot draw is refused.

use exact_kernel::{ButtonFace, Kernel, NodeType, PropId, PropValue};
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
        "keyframes fade\n  from opacity=0\n  to opacity=1\nkeyframes flash\n  from background-color=\"#ff0000\"\n  to background-color=\"#0000ff\"\ncomponent App\n  state busy = false\n  state n = 0\n  action go writes n\n    n = n + 1\n  action keys(k: string) writes n\n    n = n + 1\n  view\n    column\n{body}"
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
        r.kernel().button_face(b),
        ButtonFace {
            title: Some("Send".into()),
            symbol: Some("send".into()),
            leading: true
        }
    );
    r.dispatch(b, Event::Press).unwrap();
    assert_eq!(
        r.kernel().button_face(b).title.as_deref(),
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
        (app("button appearance=(busy ? \"auto\" : \"none\") press=go\n  text \"Go\""), "lower-button-appearance"),
        // One arm sets it: a bound value.
        (format!("style A\n  appearance=\"auto\"\nstyle B\n  opacity=1\n{}", app("button class=(busy ? A : B) press=go\n  text \"Go\"")), "lower-button-appearance"),
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
            "symbol role",
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
        "button appearance=\"auto\" press=go press-scale=0.9\n  text \"Go\"",
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

#[test]
fn it_is_refused_where_its_children_or_input_are_read_another_way() {
    for body in [
        "row role=\"tablist\"\n  button appearance=\"auto\" press=go\n    text \"Tab\"",
        "row role=(busy ? \"tablist\" : \"group\")\n  button appearance=\"auto\" press=go\n    text \"Tab\"",
        "header\n  button appearance=\"auto\" press=go\n    text \"Back\"",
        "row toolbarPlacement=\"top\"\n  button appearance=\"auto\" press=go\n    text \"Edit\"",
        "button appearance=\"auto\" press=go popovertarget=\"menu\"\n  text \"More\"",
        "button appearance=\"auto\" press=go href=\"/x\"\n  text \"Open\"",
        "button appearance=\"auto\" press=go role=\"tab\"\n  text \"Tab\"",
        "button appearance=\"auto\" press=go type=\"submit\"\n  text \"Go\"",
        "button appearance=\"auto\" press=go aria-keyshortcuts=\"Meta+S\"\n  text \"Save\"",
    ] {
        let (id, message) = refused(body);
        assert_eq!(id, "lower-button-context", "{body}: {message}");
        assert!(
            message.contains("custom `button`") || message.contains("`\"button\"`"),
            "{body}: {message}"
        );
    }
}
