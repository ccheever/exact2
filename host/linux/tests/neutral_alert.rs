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

// This integration binary has one look and font environment. Initialize them
// once before any case compiles a plan or creates a text engine.
fn setup() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        exact_linux::app::show_neutral_look();
        std::env::set_var(
            "EXACT_FONTS",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/fixtures/fonts/assets"
            ),
        );
        std::env::set_var("EXACT_FONT", "DejaVu Sans");
    });
}

fn confirmation_plan() -> &'static [u8] {
    static PLAN: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    PLAN.get_or_init(|| {
        let source = APP
            .replace(
                "button press=bump",
                "button press=bump aria-keyshortcuts=\"Control+b\"",
            )
            .replace(
                "      text `Count",
                "      input value=\"Outside\" testId=\"field\"\n      text `Count",
            );
        contract::compile(&source).unwrap().encode()
    })
}

#[test]
fn desktop_confirmation_light_320_by_invoker() {
    confirmation("light", 320., "ask-clear");
}

#[test]
fn desktop_confirmation_light_320_by_action() {
    confirmation("light", 320., "ask-action");
}

#[test]
fn desktop_confirmation_light_640_by_invoker() {
    confirmation("light", 640., "ask-clear");
}

#[test]
fn desktop_confirmation_light_640_by_action() {
    confirmation("light", 640., "ask-action");
}

#[test]
fn desktop_confirmation_dark_320_by_invoker() {
    confirmation("dark", 320., "ask-clear");
}

#[test]
fn desktop_confirmation_dark_320_by_action() {
    confirmation("dark", 320., "ask-action");
}

#[test]
fn desktop_confirmation_dark_640_by_invoker() {
    confirmation("dark", 640., "ask-clear");
}

#[test]
fn desktop_confirmation_dark_640_by_action() {
    confirmation("dark", 640., "ask-action");
}

fn confirmation(scheme: &str, width: f32, opener: &str) {
    setup();
    let plan = confirmation_plan();
    // Optional evidence for the proof lane, never an input to the assertion.
    let out = std::env::var_os("EXACT_ALERT_PROOF_OUT").map(PathBuf::from);

    let (mut p, error) = Presenter::boot_with(
        plan,
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
    assert_eq!(state["surface"]["node"], dialog);
    let row = state["surface"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == dialog)
        .expect("the immediate surface state contains the sheet");
    assert_eq!(row["rect"], json!([rect.0, rect.1, rect.2, rect.3]));
    let (access, _, _) = p.access_tree();
    let accessible = access
        .iter()
        .find(|node| node.view == dialog)
        .expect("the painted sheet is in the accessibility tree");
    assert_eq!(accessible.rect, [rect.0, rect.1, rect.2, rect.3]);
    assert_eq!(rect.2, if width == 320. { 282. } else { 416. });
    let title = &state["surface"]["title"];
    assert_eq!(title["text"], "Clear the count?");
    let y = title["rect"][1].as_f64().unwrap() as f32;
    assert!(y > rect.1 && y < clear_rect.1);
    assert!(title["rect"][2].as_f64().unwrap() > 0.);
    assert_eq!(p.focus(), Some(clear));
    assert!(p.tap(id(&p, "bump")).is_err());
    assert!(p.type_text(id(&p, "field"), "leak").is_err());
    assert!(p
        .type_key(id(&p, "field"), "KeyL", "l", true, false)
        .is_err());
    assert!(p.wheel(id(&p, "bump"), 0., 100.).is_err());
    let old_page = p.page();
    p.wheel_at(1., 1., 0., 100.);
    assert_eq!(p.page(), old_page);
    let chord = key(&mut p, "Control+b");
    assert!(chord.get("error").is_none(), "{chord}");
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
        .map(|r| &*r.text)
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
#[test]
fn lifetime_and_authored_rows() {
    use exact_kernel::{Op, PropId, PropValue, StyleId, StyleMask, StyleProps, StyleValue};
    setup();
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
    style(&mut p, dialog, StyleId::FlexDirection, "column");
    assert_eq!(
        p.host().kernel().node(dialog).unwrap().style.flex_direction,
        exact_kernel::FlexDirection::Column,
        "the derived title does not restore the platform flex row"
    );
    assert!(p.host().kernel().alert_prefix_frame(dialog).is_some());
    let mut direction = StyleMask::EMPTY;
    direction.set(StyleId::FlexDirection);
    ops(
        &mut p,
        &[Op::ClearStyle {
            id: dialog,
            mask: direction,
            written: false,
        }],
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
    let very_long = std::iter::repeat(
        "This complete confirmation title remains readable while its actions stay reachable.",
    )
    .take(24)
    .collect::<Vec<_>>()
    .join(" ");
    ops(
        &mut p,
        &[
            Op::SetProp {
                id: dialog,
                prop: PropId::AccessibilityLabel,
                value: PropValue::Str(very_long.into()),
            },
            {
                let mut color = StyleProps::default();
                color
                    .set_dynamic(StyleId::TextColor, &StyleValue::Text("#ff00ff".into()))
                    .unwrap();
                color.written = color.mask;
                Op::SetStyle {
                    id: dialog,
                    patch: Box::new(color),
                }
            },
        ],
    );
    assert!(p.resize(320., 220.).is_none());
    let sheet = visible(p.rect_of(dialog), 320., 220.);
    assert!(sheet.0 + sheet.2 <= 320. && sheet.1 + sheet.3 <= 220.);
    let frame = p.frame();
    let mut title_color = (0, 0);
    for y in 0..220_u32 {
        for x in 0..320_u32 {
            let c = frame.pixel(x, y).unwrap().demultiply();
            if c.red() > 200 && c.green() < 80 && c.blue() > 200 {
                let inside = (x as f32) >= sheet.0
                    && (x as f32) < sheet.0 + sheet.2
                    && (y as f32) >= sheet.1
                    && (y as f32) < sheet.1 + sheet.3;
                if inside {
                    title_color.0 += 1;
                } else {
                    title_color.1 += 1;
                }
            }
        }
    }
    assert!(title_color.0 > 0, "the distinct title ink was painted");
    assert_eq!(title_color.1, 0, "title ink is clipped to the sheet");
    let before_title = read(&mut p, "state")["surface"]["title"]["rect"][1]
        .as_f64()
        .unwrap();
    let before_action = p.rect_of(clear).unwrap();
    assert!(before_action.1 > 220., "the action begins below the clip");
    style(&mut p, dialog, StyleId::OverflowX, "visible");
    style(&mut p, dialog, StyleId::OverflowY, "visible");
    p.wheel(dialog, 0., 10000.).unwrap();
    p.frame();
    assert_eq!(
        p.scroll_of(dialog),
        (0., 0.),
        "authored visible overflow wins"
    );
    let mut overflow = StyleMask::EMPTY;
    overflow.set(StyleId::OverflowX);
    overflow.set(StyleId::OverflowY);
    ops(
        &mut p,
        &[Op::ClearStyle {
            id: dialog,
            mask: overflow,
            written: false,
        }],
    );
    p.wheel(dialog, 0., 10000.).unwrap();
    p.frame();
    let after_title = read(&mut p, "state")["surface"]["title"]["rect"][1]
        .as_f64()
        .unwrap();
    assert!(
        after_title < before_title,
        "the complete title scrolls with its item"
    );
    let action = visible(p.rect_of(clear), 320., 220.);
    assert_eq!(
        p.hit(action.0 + action.2 / 2., action.1 + action.3 / 2.),
        Some(clear),
        "the action is reachable at the end of the title"
    );
    assert!(p.resize(320., 640.).is_none());
    p.frame();
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
    let node = p.host().kernel().node(dialog).unwrap();
    assert!(node.style.written.has(StyleId::BackgroundColor));
    assert!(!node.style.mask.has(StyleId::BackgroundColor));
    let ground = node.computed_style(StyleMask::EMPTY).background_color;
    assert_eq!(
        ground.map_or(0, |c| c.resolve(false).a()),
        0,
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

fn dialog_id(p: &Presenter<()>) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_id("clear-confirm")[0]).unwrap().id
}
fn boot(source: &str, width: f32) -> Presenter<()> {
    let (p, error) = Presenter::boot_with(
        &contract::compile(source).unwrap().encode(),
        (),
        (width, 640.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}
fn style(p: &mut Presenter<()>, id: ViewId, row: exact_kernel::StyleId, value: &str) {
    let mut patch = exact_kernel::StyleProps::default();
    patch
        .set_dynamic(row, &exact_kernel::StyleValue::Text(value.into()))
        .unwrap();
    patch.written = patch.mask;
    ops(
        p,
        &[exact_kernel::Op::SetStyle {
            id,
            patch: Box::new(patch),
        }],
    );
}
#[test]
fn defaults_fonts_and_visibility() {
    use exact_kernel::{Op, PropId, PropValue, StyleId, StyleMask};
    setup();
    let source = APP
        .replace(
            "aria-label=\"Clear the count?\" padding=16",
            "aria-label=\"Clear the count?\"",
        )
        .replace("font-size=16 line-height", "line-height");
    assert_ne!(source, APP);
    let mut p = boot(&source, 800.);
    p.tap(id(&p, "ask-clear")).unwrap();
    p.run_commands(|| ());
    let dialog = dialog_id(&p);
    let clear = id(&p, "clear-confirm-clear");
    let node = p.host().kernel().node(dialog).unwrap();
    let key = node.key;
    assert!(!node.style.mask.has(StyleId::PaddingTop));
    assert!(!node.style.written.has(StyleId::PaddingTop));
    assert_eq!(
        node.computed_style(StyleMask::EMPTY).padding_top,
        exact_kernel::Dimension::Points(24.)
    );
    let title = read(&mut p, "state")["surface"]["title"].clone();
    let rect = p.rect_of(dialog).unwrap();
    assert_eq!(rect.2, 416.);
    assert_eq!(title["rect"][0].as_f64().unwrap() as f32 - rect.0, 24.);
    assert_eq!(title["rect"][1].as_f64().unwrap() as f32 - rect.1, 24.);
    let first_height = title["rect"][3].as_f64().unwrap();
    let first_pixels = p.frame();
    // An unchanged paint keeps authored style/geometry and the measured title.
    let authored = p.host().kernel().node(dialog).unwrap().style.clone();
    let start = std::time::Instant::now();
    for _ in 0..3 {
        assert_eq!(p.frame().data(), first_pixels.data());
    }
    println!(
        "ALERT_STILL_FRAMES {}",
        json!({"frames":3,"elapsedMicros":start.elapsed().as_micros()})
    );
    assert_eq!(p.host().kernel().node(dialog).unwrap().style, &authored);
    assert_eq!(read(&mut p, "state")["surface"]["title"], title);
    assert!(p.set_root_font_size(20.).is_none());
    p.frame();
    assert_eq!(
        p.rect_of(dialog).unwrap().2,
        520.,
        "26rem uses the live root font"
    );
    assert!(
        read(&mut p, "state")["surface"]["title"]["rect"][3]
            .as_f64()
            .unwrap()
            > first_height,
        "the title follows the live inherited font"
    );
    style(&mut p, dialog, StyleId::FontSize, "24px");
    let large = read(&mut p, "state")["surface"]["title"].clone();
    assert!(large["rect"][3].as_f64().unwrap() > first_height);
    let light = p.frame();
    p.set_system_scheme(true);
    let dark = p.frame();
    assert_ne!(dark.data(), light.data());
    assert_eq!(read(&mut p, "state")["surface"]["title"], large);
    assert!(p.resize(320., 640.).is_none());
    p.frame();
    assert_eq!(
        p.rect_of(dialog).unwrap().2,
        266.,
        "UA ceiling uses the live 24px em"
    );
    visible(p.rect_of(clear), 320., 640.);
    let action = p.rect_of(clear).unwrap();
    style(&mut p, dialog, StyleId::Visibility, "hidden");
    assert!(p
        .hit(action.0 + action.2 / 2., action.1 + action.3 / 2.)
        .is_none());
    assert!(
        p.tap(clear).is_err(),
        "opening never reveals an authored hidden subtree"
    );
    style(&mut p, dialog, StyleId::Visibility, "visible");
    assert!(p.tap(clear).is_ok());
    p.run_commands(|| ());
    assert!(p.host().kernel().arena().cover(key.index).is_none());
    // Empty generated content remains a flex item and therefore keeps its gap;
    // removing the attribute removes the item itself.
    p.tap(id(&p, "ask-clear")).unwrap();
    p.run_commands(|| ());
    ops(
        &mut p,
        &[Op::SetProp {
            id: dialog,
            prop: PropId::AccessibilityLabel,
            value: PropValue::Str(Default::default()),
        }],
    );
    assert!(read(&mut p, "state")["surface"]["title"].is_null());
    let empty_y = p.rect_of(clear).unwrap().1;
    assert_eq!(
        p.host().kernel().alert_prefix_frame(dialog).unwrap().height,
        0.
    );
    ops(
        &mut p,
        &[Op::ClearProp {
            id: dialog,
            prop: PropId::AccessibilityLabel,
        }],
    );
    assert!(read(&mut p, "state")["surface"]["title"].is_null());
    assert!(p.host().kernel().alert_prefix_frame(dialog).is_none());
    assert!(
        p.rect_of(clear).unwrap().1 < empty_y,
        "empty content keeps the row gap"
    );
    assert!(p.host().kernel().arena().cover(key.index).is_none());
    p.tap(id(&p, "clear-confirm-cancel")).unwrap();
    p.run_commands(|| ());
}

#[test]
fn a_nonconfirmation_dialog_is_refused_before_host_layout() {
    setup();
    let source = APP.replace("        text \"The count goes back to zero.\"\n", "");
    assert_ne!(source, APP);
    let mut p = boot(&source, 640.);
    let dialog = dialog_id(&p);
    p.tap(id(&p, "ask-clear")).unwrap();
    p.run_commands(|| ());
    assert!(read(&mut p, "state")["surface"].is_null());
    assert!(p.host().kernel().alert_prefix_frame(dialog).is_none());
    assert!(p.host().kernel().host_block(dialog).is_none());
    let logs = read(&mut p, "logs").to_string();
    assert!(
        logs.contains("alert-shaped alertdialog confirmations"),
        "{logs}"
    );
}
fn click(p: &mut Presenter<()>, x: f32, y: f32) {
    let now = p.host().now();
    p.pointer_down(x, y, now).unwrap();
    p.pointer_up(x, y, now + 1.).unwrap();
    p.run_commands(|| ());
}
#[test]
fn coordinate_and_keyboard_modal_input_any() {
    coordinate_and_keyboard_modal_input("any");
}

#[test]
fn coordinate_and_keyboard_modal_input_closerequest() {
    coordinate_and_keyboard_modal_input("closerequest");
}

#[test]
fn escape_closes_after_a_handler_moves_focus_unless_prevented() {
    setup();
    let source = |prevented: bool| {
        APP.replace(
            "  action askClear\n",
            &format!(
                "  action escapeFocus(key: string, e: KeyboardEvent)\n    if key == \"Escape\"\n      focus(\"clear-confirm-cancel\")\n{}  action askClear\n",
                if prevented {
                    "      preventDefault()\n"
                } else {
                    ""
                }
            ),
        )
        .replace(
            "button press=unbump destructive=true",
            "button press=unbump key=escapeFocus destructive=true",
        )
    };
    for prevented in [false, true] {
        let mut p = boot(&source(prevented), 640.);
        let trigger = id(&p, "ask-clear");
        let clear = id(&p, "clear-confirm-clear");
        let cancel = id(&p, "clear-confirm-cancel");
        p.tap(trigger).unwrap();
        p.run_commands(|| ());
        assert_eq!(p.focus(), Some(clear));
        let reply = key(&mut p, "Escape");
        assert!(reply.get("error").is_none(), "{reply}");
        if prevented {
            assert_eq!(p.focus(), Some(cancel));
            assert!(
                p.rect_of(clear).is_some(),
                "preventDefault keeps the alert open"
            );
        } else {
            assert_eq!(p.focus(), Some(trigger));
            assert!(
                p.rect_of(clear).is_none(),
                "focus alone does not prevent Escape"
            );
        }
    }
}

#[test]
fn system_back_is_not_an_authored_escape_key() {
    setup();
    let source = APP
        .replace(
            "  action askClear\n",
            "  state escapes = 0\n  action watchEscape(key: string, e: KeyboardEvent)\n    if key == \"Escape\"\n      escapes = escapes + 1\n      preventDefault()\n  action askClear\n",
        )
        .replace(
            "button press=unbump destructive=true",
            "button press=unbump key=watchEscape destructive=true",
        )
        .replace("Count ${count}", "Count ${count} escape ${escapes}");
    let mut p = boot(&source, 640.);
    let trigger = id(&p, "ask-clear");
    let clear = id(&p, "clear-confirm-clear");
    p.tap(trigger).unwrap();
    p.run_commands(|| ());
    let reply = key(&mut p, "Escape");
    assert!(reply.get("error").is_none(), "{reply}");
    assert!(p.rect_of(clear).is_some(), "preventDefault keeps it open");
    assert_eq!(words(&p, "counts"), "Count 7 escape 1");
    assert!(p.system_back(p.host().now()));
    assert!(p.rect_of(clear).is_none(), "platform Back closes directly");
    assert_eq!(
        words(&p, "counts"),
        "Count 7 escape 1",
        "Back did not synthesize another Escape"
    );

    let source = APP.replace(
        "role=\"alertdialog\"",
        "role=\"alertdialog\" closedby=\"none\"",
    );
    let mut p = boot(&source, 640.);
    p.tap(id(&p, "ask-clear")).unwrap();
    p.run_commands(|| ());
    let clear = id(&p, "clear-confirm-clear");
    assert!(p.system_back(p.host().now()));
    assert!(p.rect_of(clear).is_some(), "closedby none keeps it open");
    let reply = key(&mut p, "Escape");
    assert!(reply.get("error").is_none(), "{reply}");
    assert!(p.rect_of(clear).is_some(), "Escape observes closedby too");
}

#[test]
fn closing_cancels_a_contact_before_its_later_up() {
    setup();
    let mut p = boot(APP, 640.);
    let trigger = id(&p, "ask-clear");
    let clear = id(&p, "clear-confirm-clear");
    p.tap(trigger).unwrap();
    p.run_commands(|| ());
    let rect = p.rect_of(clear).unwrap();
    let at = (rect.0 + rect.2 / 2., rect.1 + rect.3 / 2.);
    assert_eq!(p.hit(at.0, at.1), Some(clear));
    assert!(p.pointer_down(at.0, at.1, p.host().now()).unwrap());
    let reply = key(&mut p, "Escape");
    assert!(reply.get("error").is_none(), "{reply}");
    assert!(p.rect_of(clear).is_none());
    assert!(!p.pointer_up(at.0, at.1, p.host().now()).unwrap());
    p.run_commands(|| ());
    assert_eq!(words(&p, "counts"), "Count 7");
}

const CONTACT_CLOSE_APP: &str = r#"component App
  state count = 0
  state ups = 0
  state replaced = false
  action act
    count = count + 1
  action reopen
    ups = ups + 1
    showModal("confirm")
  action replace
    ups = ups + 1
    replaced = true
    showModal("next")
  action ask
    showModal("confirm")
  action shut
    close("confirm")
  view
    column width="100%" height="100%" font-family="DejaVu Sans"
      button press=ask testId="open"
        text "Open"
      button press=shut testId="shut"
        text "Shut"
      text `Count ${count} ups ${ups}` testId="status"
      if not replaced
        dialog id="confirm" role="alertdialog" aria-label="Proceed?"
          text "Choose one."
          button press=act pointerup=reopen destructive=true testId="same"
            text "Same"
          button press=act pointerup=replace testId="replacement"
            text "Replacement"
          button commandfor="confirm" command="close" testId="cancel"
            text "Cancel"
      if replaced
        dialog id="next" role="alertdialog" aria-label="Again?"
          text "A new dialog."
          button press=act destructive=true testId="next-action"
            text "Act"
          button commandfor="next" command="close" testId="next-cancel"
            text "Cancel"
"#;

#[test]
fn programmatic_close_cancels_callbacks_before_same_node_reopen_or_replacement() {
    setup();
    let mut p = boot(CONTACT_CLOSE_APP, 640.);
    let open = id(&p, "open");
    let shut = id(&p, "shut");
    p.tap(open).unwrap();
    p.run_commands(|| ());
    let same = id(&p, "same");
    let rect = p.rect_of(same).unwrap();
    let at = (rect.0 + rect.2 / 2., rect.1 + rect.3 / 2.);
    assert!(p.pointer_down(at.0, at.1, p.host().now()).unwrap());
    let now = p.host().now();
    assert!(p
        .host_mut()
        .dispatch_at(shut, exact_runner::Event::Press, now)
        .is_none());
    p.run_commands(|| ());
    assert!(p.rect_of(same).is_none(), "the old opening closed first");
    p.run_commands(|| ());
    assert!(p.rect_of(same).is_some(), "the callback reopened that node");
    assert!(!p.pointer_up(at.0, at.1, p.host().now()).unwrap());
    assert_eq!(words(&p, "status"), "Count 0 ups 1");
    let reply = key(&mut p, "Escape");
    assert!(reply.get("error").is_none(), "{reply}");

    p.tap(open).unwrap();
    p.run_commands(|| ());
    let replacement = id(&p, "replacement");
    let rect = p.rect_of(replacement).unwrap();
    let at = (rect.0 + rect.2 / 2., rect.1 + rect.3 / 2.);
    assert!(p.pointer_down(at.0, at.1, p.host().now()).unwrap());
    let now = p.host().now();
    assert!(p
        .host_mut()
        .dispatch_at(shut, exact_runner::Event::Press, now)
        .is_none());
    p.run_commands(|| ());
    p.run_commands(|| ());
    let next = id(&p, "next-action");
    assert!(
        p.rect_of(next).is_some(),
        "the replacement opening survives"
    );
    assert!(!p.pointer_up(at.0, at.1, p.host().now()).unwrap());
    assert_eq!(words(&p, "status"), "Count 0 ups 2");
}

fn coordinate_and_keyboard_modal_input(closedby: &str) {
    setup();
    let source = APP
        .replace("state count = 7", "state count = 7\n  state clears = 0")
        .replace("count = 0", "count = 0\n    clears = clears + 1")
        .replace("Count ${count}", "Count ${count} clear ${clears}");
    let source = source.replace(
        "role=\"alertdialog\"",
        &format!("role=\"alertdialog\" closedby=\"{closedby}\""),
    );
    let mut p = boot(&source, 640.);
    let trigger = id(&p, "ask-clear");
    let clear = id(&p, "clear-confirm-clear");
    let cancel = id(&p, "clear-confirm-cancel");
    let bump = p.rect_of(id(&p, "bump")).unwrap();
    p.tap(trigger).unwrap();
    p.run_commands(|| ());
    click(&mut p, bump.0 + bump.2 / 2., bump.1 + bump.3 / 2.);
    assert_eq!(
        words(&p, "counts"),
        "Count 7 clear 0",
        "outside input never reaches Bump"
    );
    assert_eq!(p.rect_of(clear).is_none(), closedby == "any");
    if closedby == "any" {
        assert_eq!(p.focus(), Some(trigger));
        p.tap(trigger).unwrap();
        p.run_commands(|| ());
    }
    let reply = key(&mut p, "Escape");
    assert!(reply.get("error").is_none(), "{reply}");
    assert!(p.rect_of(clear).is_none());
    assert_eq!(words(&p, "counts"), "Count 7 clear 0");
    p.tap(trigger).unwrap();
    p.run_commands(|| ());
    // Coordinate Cancel hits the authored action in the translated overlay.
    let r = visible(p.rect_of(cancel), 640., 640.);
    click(&mut p, r.0 + r.2 / 2., r.1 + r.3 / 2.);
    assert!(p.rect_of(clear).is_none());
    assert_eq!(p.focus(), Some(trigger));
    assert_eq!(words(&p, "counts"), "Count 7 clear 0");
    p.tap(trigger).unwrap();
    p.run_commands(|| ());
    assert_eq!(p.focus(), Some(clear));
    for expected in [cancel, clear, cancel, clear] {
        let reply = key(&mut p, "Tab");
        assert!(reply.get("error").is_none(), "{reply}");
        assert_eq!(p.focus(), Some(expected));
    }
    let reply = key(&mut p, "Enter");
    assert!(reply.get("error").is_none(), "{reply}");
    p.run_commands(|| ());
    assert_eq!(words(&p, "counts"), "Count 0 clear 1");
    assert!(p.rect_of(clear).is_none());
    assert_eq!(p.focus(), Some(trigger));
}
