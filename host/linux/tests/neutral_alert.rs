//! The desktop confirmation through the public presenter, separate from
//! Android's windowed Material tests and its process-wide look.
use exact_kernel::ViewId;
use exact_linux::presenter::{PainterChoice, Presenter};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const APP: &str = include_str!("neutral_alert.contract");

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

fn read(p: &mut Presenter<()>, op: &str) -> Value {
    serde_json::from_str(&exact_linux::agent::handle(
        p,
        &json!({"op":op}).to_string(),
    ))
    .unwrap()
}

fn picture(p: &mut Presenter<()>, out: Option<&Path>, name: &str) -> Option<Value> {
    out.map(|dir| {
        let path = dir.join(format!("{name}.png"));
        assert!(!path.exists(), "the diagnostic must retain earlier pixels");
        serde_json::from_str(&p.screenshot(path.to_str().unwrap()).unwrap()).unwrap()
    })
}

#[test]
fn a_desktop_clear_confirmation_opens_by_invoker_and_action() {
    exact_linux::app::show_neutral_look();
    std::env::set_var(
        "EXACT_FONTS",
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/fixtures/fonts/assets"
        ),
    );
    std::env::set_var("EXACT_FONT", "DejaVu Sans");
    let plan = contract::compile(APP).unwrap().encode();
    // Optional evidence for the proof lane, never an input to the assertion.
    let out = std::env::var_os("EXACT_ALERT_PROOF_OUT").map(PathBuf::from);

    for scheme in ["light", "dark"] {
        for width in [320., 640.] {
            for opener in ["ask-clear", "ask-action"] {
                let (mut p, error) = Presenter::boot_with(
                    &plan,
                    (),
                    (width, 640.),
                    1.,
                    PathBuf::new(),
                    PainterChoice::Cpu,
                )
                .unwrap();
                assert!(error.is_none(), "{error:?}");
                let prefer = exact_linux::agent::handle(
                    &mut p,
                    &json!({"op":"prefer", "media":{"prefers-color-scheme":scheme},
                        "page":{"root-font-size":16}})
                    .to_string(),
                );
                let prefer: Value = serde_json::from_str(&prefer).unwrap();
                assert!(prefer.get("error").is_none(), "{prefer}");
                let clear = id(&p, "clear-confirm-clear");
                let cancel = id(&p, "clear-confirm-cancel");
                assert!(p.rect_of(clear).is_none() && p.rect_of(cancel).is_none());
                let name = format!("{scheme}-{width}-{opener}");
                let before = picture(&mut p, out.as_deref(), &format!("{name}-closed"));
                let trigger = id(&p, opener);
                let tap = p.tap(trigger);
                p.run_commands(|| ());
                let state = read(&mut p, "state");
                let layout = read(&mut p, "layout");
                let clear_rect = p.rect_of(clear);
                let cancel_rect = p.rect_of(cancel);
                let after = picture(&mut p, out.as_deref(), &format!("{name}-opened"));
                let logs = read(&mut p, "logs");
                println!(
                    "ALERT_PROOF {}",
                    json!({"case":name,"preferences":prefer,"tap":tap,
                        "clearNode":clear,"cancelNode":cancel,"clearRect":clear_rect,
                        "cancelRect":cancel_rect,"before":before,"after":after,
                        "state":state,"layout":layout,"logs":logs})
                );
                assert!(tap.is_ok(), "{tap:?}");
                let clear_rect = visible(clear_rect, width, 640.);
                let cancel_rect = visible(cancel_rect, width, 640.);
                assert!(clear_rect.0 < cancel_rect.0, "authored action order");
                assert_eq!(clear_rect.1, cancel_rect.1);
                let dialog = p
                    .host()
                    .kernel()
                    .node_by_key(p.host().kernel().find_by_id("clear-confirm")[0])
                    .unwrap()
                    .id;
                let rect = visible(p.rect_of(dialog), width, 640.);
                assert_eq!(rect.2, if width == 320. { 282. } else { 416. });
                let title = &state["surface"]["title"];
                assert_eq!(title["text"], "Clear the count?");
                let y = title["rect"][1].as_f64().unwrap() as f32;
                assert!(y > rect.1 && y < clear_rect.1);
                assert!(title["rect"][2].as_f64().unwrap() > 0.);
                assert_eq!(p.focus(), Some(clear));
                assert!(p.tap(id(&p, "bump")).is_err());
                assert!(p.type_text(id(&p, "bump"), "leak").is_err());
                assert!(p.wheel(id(&p, "bump"), 0., 100.).is_err());
                let old_page = p.page();
                p.wheel_at(1., 1., 0., 100.);
                assert_eq!(p.page(), old_page);
                assert_eq!(words(&p, "counts"), "Count 7");
                p.tap(cancel).unwrap();
                p.run_commands(|| ());
                assert!(p.rect_of(clear).is_none());
                assert_eq!(p.focus(), Some(trigger));
                assert_eq!(words(&p, "counts"), "Count 7");
                p.tap(trigger).unwrap();
                p.run_commands(|| ());
                let reply = key(&mut p, "Escape");
                assert!(reply.get("error").is_none(), "{reply}");
                assert!(p.rect_of(clear).is_none());
                assert_eq!(words(&p, "counts"), "Count 7");
                p.tap(trigger).unwrap();
                p.run_commands(|| ());
                p.tap(clear).unwrap();
                p.run_commands(|| ());
                assert!(p.rect_of(clear).is_none());
                assert_eq!(words(&p, "counts"), "Count 0");
                assert!(p.host().kernel().host_block(dialog).is_none());
            }
        }
    }
    lifetime_and_authored_rows();
}

fn visible(rect: Option<(f32, f32, f32, f32)>, w: f32, h: f32) -> (f32, f32, f32, f32) {
    let r = rect.expect("painted rectangle");
    assert!([r.0, r.1, r.2, r.3].iter().all(|n| n.is_finite()));
    assert!(
        r.2 > 0. && r.3 > 0. && r.0 >= 0. && r.1 >= 0. && r.0 + r.2 <= w && r.1 + r.3 <= h,
        "{r:?}"
    );
    r
}
fn words(p: &Presenter<()>, name: &str) -> String {
    p.host()
        .kernel()
        .node(id(p, name))
        .unwrap()
        .text_runs()
        .iter()
        .map(|r| r.text.as_str())
        .collect()
}
fn key(p: &mut Presenter<()>, name: &str) -> Value {
    let id = p.focus().unwrap();
    serde_json::from_str(&exact_linux::agent::handle(
        p,
        &json!({"op":"type", "id":id,"key":name}).to_string(),
    ))
    .unwrap()
}
fn ops(p: &mut Presenter<()>, ops: &[exact_kernel::Op]) {
    p.host_mut()
        .runner_mut()
        .kernel_mut()
        .apply(0, 1, ops)
        .unwrap();
    assert!(p.resize(p.viewport().0, p.viewport().1).is_none());
    p.frame();
}
fn lifetime_and_authored_rows() {
    use exact_kernel::{Op, PropId, PropValue, StyleId, StyleMask, StyleProps, StyleValue};
    let source = APP
        .replace("state count = 7", "state count = 7\n  state clears = 0")
        .replace("count = 0", "count = 0\n    clears = clears + 1")
        .replace("Count ${count}", "Count ${count} clear ${clears}");
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(&source).unwrap().encode(),
        (),
        (640., 640.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    let trigger = id(&p, "ask-clear");
    let clear = id(&p, "clear-confirm-clear");
    p.tap(trigger).unwrap();
    p.run_commands(|| ());
    let dialog = p
        .host()
        .kernel()
        .node_by_key(p.host().kernel().find_by_id("clear-confirm")[0])
        .unwrap()
        .id;
    let original = p.host().kernel().node(dialog).unwrap().style.clone();
    let mut own = StyleProps::default();
    for (row, value) in [
        (StyleId::Width, "250px"),
        (StyleId::MaxWidth, "500px"),
        (StyleId::BackgroundColor, "#123456"),
        (StyleId::PaddingTop, "30px"),
        (StyleId::FontSize, "20px"),
    ] {
        own.set_dynamic(row, &StyleValue::Text(value.into()))
            .unwrap();
    }
    own.written = own.mask;
    ops(
        &mut p,
        &[Op::SetStyle {
            id: dialog,
            patch: Box::new(own.clone()),
        }],
    );
    assert_eq!(p.rect_of(dialog).unwrap().2, 250.);
    let long =
        "A long confirmation title which wraps at this narrow width and stays above the actions";
    ops(
        &mut p,
        &[Op::SetProp {
            id: dialog,
            prop: PropId::AccessibilityLabel,
            value: PropValue::Str(long.into()),
        }],
    );
    let title = read(&mut p, "state")["surface"]["title"].clone();
    assert_eq!(title["text"], long);
    assert!(title["rect"][3].as_f64().unwrap() > 27.);
    assert!(
        title["rect"][1].as_f64().unwrap() + title["rect"][3].as_f64().unwrap()
            <= p.rect_of(clear).unwrap().1 as f64
    );
    let before = p.host().kernel().node(dialog).unwrap().style.clone();
    p.frame();
    p.frame();
    assert_eq!(
        p.host().kernel().node(dialog).unwrap().style,
        &before,
        "title reservation never writes author rows"
    );
    assert!(p.resize(320., 640.).is_none());
    p.frame();
    assert_eq!(p.rect_of(dialog).unwrap().2, 250.);
    let mut mask = own.mask;
    mask.clear(StyleId::PaddingTop);
    ops(
        &mut p,
        &[Op::ClearStyle {
            id: dialog,
            mask,
            written: false,
        }],
    );
    assert_eq!(
        p.rect_of(dialog).unwrap().2,
        282.,
        "absence restores responsive width"
    );
    let mut clear_own = StyleMask::EMPTY;
    clear_own.set(StyleId::BackgroundColor);
    ops(
        &mut p,
        &[Op::ClearStyle {
            id: dialog,
            mask: clear_own,
            written: true,
        }],
    );
    assert_eq!(
        p.host()
            .kernel()
            .node(dialog)
            .unwrap()
            .computed_style(StyleMask::EMPTY)
            .background_color,
        None,
        "explicit initial beats platform ground"
    );
    ops(
        &mut p,
        &[Op::ClearStyle {
            id: dialog,
            mask: clear_own,
            written: false,
        }],
    );
    assert!(p
        .host()
        .kernel()
        .node(dialog)
        .unwrap()
        .computed_style(StyleMask::EMPTY)
        .background_color
        .is_some());
    p.tap(clear).unwrap();
    p.run_commands(|| ());
    assert_eq!(words(&p, "counts"), "Count 0 clear 1");
    assert!(p.tap(clear).is_err());
    assert_eq!(words(&p, "counts"), "Count 0 clear 1");
    // Removed dialog generations cannot keep a modal, title cover or focus.
    p.tap(trigger).unwrap();
    p.run_commands(|| ());
    let old = p.host().kernel().node(dialog).unwrap().key;
    ops(&mut p, &[Op::DestroyView { id: dialog }]);
    assert!(p.host().kernel().node_by_key(old).is_none());
    assert!(read(&mut p, "state")["surface"].is_null());
    assert_eq!(p.focus(), Some(trigger));
    assert!(p.tap(id(&p, "bump")).is_ok());
    assert_eq!(words(&p, "counts"), "Count 1 clear 1");
    // A reload recreates the same authored ids, but never the opening.
    p.reload(&contract::compile(&source).unwrap().encode(), ())
        .unwrap();
    assert!(p.rect_of(id(&p, "clear-confirm-clear")).is_none());
    p.tap(id(&p, "ask-action")).unwrap();
    p.run_commands(|| ());
    visible(p.rect_of(id(&p, "clear-confirm-clear")), 320., 640.);
    assert_eq!(original.padding_top, exact_kernel::Dimension::Points(16.));
}
