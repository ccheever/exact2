//! Each session's role overrides, its own (LLP 1121 stage 5).
use super::*;

thread_local! {
    static RUNTIMES: std::cell::RefCell<crate::abi::registry::Registry<StorageModule>> =
        std::cell::RefCell::new(crate::abi::registry::Registry::default());
}

fn plan(canvas: &str) -> Vec<u8> {
    contract::compile(&format!(
        "theme\n  CanvasText = \"{canvas}\"\n\ncomponent App\n  view\n    text \"a\"\n"
    ))
    .unwrap()
    .encode()
}

/// Boot `canvas`'s plan on runtime `rt` (a new one when `None`), through the
/// export's path; its batch.
fn boot(rt: Option<u32>, canvas: &str) -> (u32, String) {
    let rt = rt.unwrap_or_else(|| RUNTIMES.with(|r| r.borrow_mut().create()));
    let batch = crate::abi::registry::with_runtime(
        &RUNTIMES,
        rt,
        false,
        |b, _| {
            let len = b.boot(
                &plan(canvas),
                StorageModule::default(),
                Hooks::none(),
                200.,
                200.,
            );
            String::from_utf8(b.output_bytes(len as usize).to_vec()).unwrap()
        },
        |_| String::new(),
    );
    (rt, batch)
}

fn fixed(text: &str) -> Option<exact_kernel::ColorValue> {
    Some(exact_kernel::ColorValue::Fixed(
        exact_kernel::Color::parse(text).unwrap(),
    ))
}

/// What `CanvasText` resolves to in a call to `rt`, through the setters'
/// path and the exports' (each binds its session's).
fn resolved(rt: u32) -> Option<exact_kernel::ColorValue> {
    let canvas_text = exact_kernel::style::roles::role("CanvasText").unwrap();
    let mut out = None;
    crate::abi::registry::with_entry(&RUNTIMES, rt, |e| {
        assert!(!e
            .bridge
            .host
            .as_ref()
            .unwrap()
            .runner()
            .journal()
            .any(|l| l.contains("role overrides")));
        out = exact_kernel::style::roles::overridden(canvas_text);
    });
    let export = crate::abi::registry::with_runtime(
        &RUNTIMES,
        rt,
        false,
        |_, _| exact_kernel::style::roles::overridden(canvas_text),
        |_| None,
    );
    assert_eq!(out, export);
    out
}

/// Two embedded sessions on one thread, each with its own role overrides:
/// each call to a runtime resolves its session's, both ways round, and no
/// session says another's replaced its own.
#[test]
fn two_sessions_on_one_thread_resolve_their_own_role_overrides() {
    let (first, _) = boot(None, "#010101");
    let (second, _) = boot(None, "#020202");
    for _ in 0..2 {
        assert_eq!(resolved(first), fixed("#010101"));
        assert_eq!(resolved(second), fixed("#020202"));
    }
    assert_eq!(resolved(second), fixed("#020202"));
    assert_eq!(resolved(first), fixed("#010101"));
}

/// A session that reboots with other overrides, inside the call that binds
/// its old ones, presents the new ones: its first batch's colours and role
/// table are the new plan's, and the other session keeps its own.
#[test]
fn a_reboot_with_other_overrides_re_presents_them() {
    let (first, batch) = boot(None, "#010101");
    assert!(batch.contains(r#""text_color":[1,1,1,255]"#), "{batch}");
    let (second, _) = boot(None, "#020202");
    let (_, batch) = boot(Some(first), "#030303");
    assert!(batch.contains(r#""text_color":[3,3,3,255]"#), "{batch}");
    assert!(
        batch.contains(r#"{"op":"roles","roles":{"CanvasText":[3,3,3,255]}}"#),
        "{batch}"
    );
    assert_eq!(resolved(first), fixed("#030303"));
    assert_eq!(resolved(second), fixed("#020202"));
}

/// A colour report to one session, through the export's path, resolves
/// that session's colours from its own overrides, not the session that
/// booted last's.
#[test]
fn a_colour_report_re_presents_its_own_sessions_overrides() {
    let (first, _) = boot(None, "#010101");
    let (second, _) = boot(None, "#020202");
    let report = |rt: u32| {
        crate::abi::registry::with_runtime(
            &RUNTIMES,
            rt,
            false,
            |b, _| b.host.as_mut().unwrap().set_colors(Vec::new()),
            |_| String::new(),
        )
    };
    // Each first report finds what boot resolved, its own: nothing moves.
    for (rt, other) in [(first, "[2,2,2,255]"), (second, "[1,1,1,255]")] {
        let batch = report(rt);
        assert!(!batch.contains(other), "{batch}");
        assert!(!batch.contains("text_color"), "{batch}");
    }
    assert_eq!(resolved(first), fixed("#010101"));
}
