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

/// A gradient the presenter resolves later, once it knows the box, carries
/// its session's overrides as their colours (a tint role's as the theme's
/// accent), and resolves with none of another session's.
#[test]
#[allow(unsafe_code)] // The presenter's resolve, over buffers this test owns.
fn a_boxed_gradient_carries_its_sessions_accent() {
    let boot = |accent: &str, stop: &str| {
        let plan = contract::compile(&format!(
            "theme\n  AccentColor = \"{accent}\"\n\ncomponent App\n  view\n    view width=100 height=20 background-image=\"linear-gradient(90deg, {stop} 0px, transparent 100px)\"\n"
        ))
        .unwrap()
        .encode();
        let rt = RUNTIMES.with(|r| r.borrow_mut().create());
        crate::abi::registry::with_runtime(
            &RUNTIMES,
            rt,
            false,
            |b, _| {
                let len = b.boot(&plan, StorageModule::default(), Hooks::none(), 200., 200.);
                String::from_utf8(b.output_bytes(len as usize).to_vec()).unwrap()
            },
            |_| String::new(),
        )
    };
    // What the presenter resolves for the `css` a batch carries.
    let resolve = |batch: &str| {
        let at = batch.find(r#""css":""#).expect("a boxed gradient") + 7;
        let css = &batch[at..at + batch[at..].find('"').unwrap()];
        let mut out = vec![0u8; 4096];
        // SAFETY: both buffers are the lengths given.
        let n = unsafe {
            crate::gradient::resolve(
                css.as_ptr(),
                css.len(),
                [100., 20.],
                false,
                out.as_mut_ptr(),
                out.len(),
            )
        };
        String::from_utf8(out[..n].to_vec()).unwrap()
    };
    let red = r#""stops":[0,255,0,0,255"#;
    let first = boot("#ff0000", "AccentColor");
    let link = boot("#ff0000", "LinkText");
    let second = boot("#0000ff", "AccentColor");
    assert!(second.contains("#0000ffff 0px"), "{second}");
    for batch in [&first, &link] {
        let resolved = resolve(batch);
        assert!(resolved.contains(red), "{batch}\n{resolved}");
    }
    // A role the session leaves alone resolves as the platform's, though
    // the session installed last (the second) replaces it.
    let css = "linear-gradient(90deg, AccentColor 0px, transparent 100px)";
    let resolved = resolve(&format!(r#""css":"{css}""#));
    assert!(
        !resolved.contains(r#""stops":[0,0,0,255,255"#),
        "{resolved}"
    );
}

/// The binding core through the export's path (LLP 1121 D12, stage 5): each
/// runtime answers a token from its own design, a name its plan lacks is
/// `null`, and `inputs` is the same until something it read moves.
#[test]
fn the_binding_core_answers_each_runtime_from_its_own_design() {
    let (first, _) = boot(None, "#010101");
    let (second, _) = boot(None, "#020202");
    let ask = |rt: u32| {
        crate::abi::registry::with_runtime(
            &RUNTIMES,
            rt,
            false,
            |b, _| {
                let len = b.input_write(br#"{"view": 0, "names": ["CanvasText", "nonesuch"]}"#);
                let n = b.tamber(len) as usize;
                String::from_utf8(b.output_bytes(n).to_vec()).unwrap()
            },
            |_| String::new(),
        )
    };
    let (a, b) = (ask(first), ask(second));
    assert!(a.contains(r#""CanvasText":[1,1,1,255]"#), "{a}");
    assert!(b.contains(r#""CanvasText":[2,2,2,255]"#), "{b}");
    assert!(a.contains(r#""nonesuch":null"#), "{a}");
    assert_eq!(ask(first), a, "nothing moved");
}
