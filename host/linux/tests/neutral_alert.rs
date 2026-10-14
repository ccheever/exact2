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
    let mut failures = Vec::new();
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
                if tap.is_err() || clear_rect.is_none() || cancel_rect.is_none() {
                    failures.push(name);
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "the desktop must accept the opener and paint its Clear and Cancel targets: {failures:?}"
    );
}
