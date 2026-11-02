//! Authored subtrees through Windows' desktop overlay entry point.
use exact_kernel::{Dimension, Overflow, StyleMask, ViewId};
use exact_linux::presenter::top::SurfaceKind;
use exact_linux::presenter::{PainterChoice, Presenter};
use serde_json::{json, Value};
use std::path::PathBuf;

const APP: &str = include_str!("authored_dialog.contract");
fn boot(source: &str) -> Presenter<()> {
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
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(source).unwrap().encode(),
        (),
        (480., 400.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.serve_overlays();
    p.frame();
    p
}
fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}
fn tap(p: &mut Presenter<()>, name: &str) {
    p.tap(id(p, name)).unwrap();
    p.run_commands(|| ());
    p.frame();
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
fn agent(p: &mut Presenter<()>, request: Value) -> Value {
    serde_json::from_str(&exact_linux::agent::handle(p, &request.to_string())).unwrap()
}
fn key(p: &mut Presenter<()>, name: &str) {
    let target = p
        .focus()
        .unwrap_or_else(|| p.surface_json()["node"].as_u64().unwrap() as ViewId);
    p.type_key(target, name, name, true, false).unwrap();
    p.run_commands(|| ());
    p.frame();
}
fn outside(p: &mut Presenter<()>) {
    // Each click later than the last: one that reaches a popover's live page
    // keeps the pointer's clock.
    static CLICKS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let at = 10. * f64::from(CLICKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)) + 1.;
    p.pointer_down(10., 80., at).unwrap();
    p.pointer_up(10., 80., at + 1.).unwrap();
    p.run_commands(|| ());
    p.frame();
}

#[test]
fn authored_modal_commands_focus_close_events_and_inert_page() {
    let mut p = boot(APP);
    let page_pixel = p.frame().pixel(470, 390).unwrap();
    for (n, opener, closer) in [
        (1, "action", "Escape"),
        (2, "command", "back"),
        (3, "action", "done"),
        (4, "command", "close-action"),
    ] {
        tap(&mut p, opener);
        let root = id(&p, "dialog");
        assert_eq!(p.surface_json()["node"], root);
        assert!(p.surface().is_none());
        let rect = p.rect_of(root).unwrap();
        let content = p.host().kernel().node(root).unwrap().children()[0];
        let content_height = p.host().kernel().node(content).unwrap().frame.height;
        assert_eq!(rect.2, 252.);
        assert_eq!(rect.3, content_height + 32.);
        assert_eq!(
            (rect.0, rect.1),
            ((480. - rect.2) / 2., (400. - rect.3) / 2.)
        );
        assert_eq!(p.focus(), Some(id(&p, "search")));
        let s = p
            .host()
            .kernel()
            .node(root)
            .unwrap()
            .computed_style(StyleMask::INHERITED);
        assert_eq!(s.padding_top, Dimension::Points(16.));
        assert_eq!(s.padding_right, Dimension::Points(16.));
        assert_eq!(
            (s.overflow_x, s.overflow_y),
            (Overflow::Auto, Overflow::Auto)
        );
        assert_ne!(s.background_color, Default::default());
        let frame = p.frame();
        assert_ne!(frame.pixel(124, 144), frame.pixel(470, 390));
        let scrim = frame.pixel(470, 390).unwrap();
        for (clear, shaded) in [page_pixel.red(), page_pixel.green(), page_pixel.blue()]
            .into_iter()
            .zip([scrim.red(), scrim.green(), scrim.blue()])
        {
            let expected = (u32::from(clear) * (255 - 77) + 127) / 255;
            assert_eq!(u32::from(shaded), expected);
        }
        assert!(p.surface_json()["title"].is_null());
        let state = agent(&mut p, json!({"op":"state"}));
        assert_eq!(state["surface"]["kind"], "dialog");
        assert_eq!(state["surface"]["modal"], true);
        assert_eq!(p.hit(10., 80.), None);
        let page = id(&p, "page");
        let refused = agent(&mut p, json!({"op":"tap", "id":page}));
        assert!(
            refused["error"].as_str().unwrap().contains("inert"),
            "{refused}"
        );
        let refused = p.type_text(page, "leak").unwrap_err();
        assert!(!refused.is_empty());
        let (access, _, _) = p.access_tree();
        assert!(access.iter().any(|n| n.view == root));
        for n in access {
            let mut at = Some(n.view);
            while at != Some(root) {
                at = p
                    .host()
                    .kernel()
                    .node(at.expect("only dialog descendants are accessible"))
                    .unwrap()
                    .parent;
            }
        }
        for _ in 0..8 {
            key(&mut p, "Tab");
            assert!(["first", "search", "done", "close-action"]
                .iter()
                .any(|name| Some(id(&p, name)) == p.focus()));
        }
        outside(&mut p);
        assert_eq!(p.surface_json()["node"], root);
        assert!(words(&p, "result").ends_with(":0"));
        match closer {
            "Escape" => key(&mut p, "Escape"),
            "back" => {
                assert!(p.system_back(3.));
            }
            other => tap(&mut p, other),
        }
        p.run_commands(|| ());
        assert!(p.surface_json().is_null());
        assert_eq!(p.focus(), Some(id(&p, opener)));
        assert_eq!(words(&p, "result"), format!(":{n}:0"));
    }
    tap(&mut p, "action");
    p.key(Some('x'), false, 4.);
    assert_eq!(words(&p, "result"), "x:4:0");
    tap(&mut p, "done");
    tap(&mut p, "action");
    assert_eq!(
        p.focus(),
        Some(id(&p, "search")),
        "autofocus is offered on every opening"
    );
}

#[test]
fn modal_close_requests_fire_cancel_and_honor_prevent_default() {
    for prevented in [false, true] {
        let mut source = APP.replace("closedby=\"closerequest\"", "closedby=\"any\"");
        if prevented {
            source = source.replace(
                "  action cancelled\n",
                "  action cancelled\n    preventDefault()\n",
            );
        }
        let mut p = boot(&source);
        let (mut cancels, mut closes, mut events) = (0, 0, String::new());
        for request in ["Escape", "back", "agent-back", "outside"] {
            tap(&mut p, "command");
            match request {
                "Escape" => {
                    let focus = p.focus().unwrap();
                    let reply = agent(&mut p, json!({"op":"type", "id":focus, "key":"Escape"}));
                    assert!(reply.get("error").is_none(), "{reply}");
                }
                "back" => assert!(p.system_back(0.)),
                "agent-back" => {
                    let reply = agent(&mut p, json!({"op":"tap", "back":true}));
                    assert!(reply.get("error").is_none(), "{reply}");
                }
                _ => outside(&mut p),
            }
            p.run_commands(|| ());
            p.frame();
            cancels += 1;
            events.push_str("cancel,");
            if !prevented {
                closes += 1;
                events.push_str("close,");
            }
            let state = agent(&mut p, json!({"op":"state"}));
            assert_eq!(state["surface"].is_null(), !prevented, "{request}");
            assert_eq!(words(&p, "events"), format!("{cancels}:{events}"));
            assert_eq!(words(&p, "result"), format!(":{closes}:0"));
            if !prevented {
                tap(&mut p, "command");
            }
            tap(&mut p, "done");
            closes += 1;
            events.push_str("close,");
            assert_eq!(words(&p, "events"), format!("{cancels}:{events}"));
            assert_eq!(words(&p, "result"), format!(":{closes}:0"));
        }
        tap(&mut p, "command");
        tap(&mut p, "close-action");
        events.push_str("close,");
        assert_eq!(words(&p, "events"), format!("{cancels}:{events}"));
        for request in ["Escape", "outside"] {
            tap(&mut p, "invoker");
            if request == "Escape" {
                let focus = p.focus().unwrap();
                let reply = agent(&mut p, json!({"op":"type", "id":focus, "key":"Escape"}));
                assert!(reply.get("error").is_none(), "{reply}");
            } else {
                outside(&mut p);
            }
            let state = agent(&mut p, json!({"op":"state"}));
            assert!(state["surface"].is_null());
            assert_eq!(words(&p, "events"), format!("{cancels}:{events}"));
        }
    }
}

#[test]
fn non_confirmation_alertdialog_keeps_its_label_title() {
    let mut p = boot(
        &APP.replace(
            "  state query",
            "  state dialogRole = \"alertdialog\"\n  state query",
        )
        .replace(
            "role=\"dialog\" aria-label=\"Switcher\"",
            "role=dialogRole aria-label=\"Account\"",
        ),
    );
    tap(&mut p, "command");
    assert!(p.surface().is_none());
    let state = agent(&mut p, json!({"op":"state"}));
    assert_eq!(state["surface"]["kind"], "alert");
    assert_eq!(state["surface"]["title"]["text"], "Account");
    assert_eq!(p.focus(), Some(id(&p, "search")));
}

#[test]
fn content_dialog_close_requests_can_be_prevented() {
    let source = r#"component App
  state cancels = 0
  state closes = 0
  action cancelled
    cancels = cancels + 1
  action closed
    closes = closes + 1
  view
    column width="100%" height="100%"
      button "Open" testId="command" commandfor="confirm" command="show-modal" width=120 height=32
      text `${cancels}:${closes}` testId="result"
      dialog id="confirm" role="alertdialog" aria-label="Confirm" closedby="any" cancel=cancelled close=closed
        text "Continue?"
        button "Done" testId="done" commandfor="confirm" command="close"
"#;
    for prevented in [false, true] {
        let source = if prevented {
            source.replace(
                "  action cancelled\n",
                "  action cancelled\n    preventDefault()\n",
            )
        } else {
            source.into()
        };
        let mut p = boot(&source);
        for (index, request) in ["Escape", "back", "outside"].into_iter().enumerate() {
            tap(&mut p, "command");
            assert_eq!(p.surface_json()["kind"], "alert");
            assert_eq!(p.surface().unwrap().kind, SurfaceKind::Alert);
            match request {
                "Escape" => key(&mut p, "Escape"),
                "back" => assert!(p.system_back(0.)),
                _ => outside(&mut p),
            }
            p.run_commands(|| ());
            assert_eq!(p.surface().is_some(), prevented);
            let cancels = index + 1;
            let closes = index * (1 + usize::from(!prevented)) + usize::from(!prevented);
            assert_eq!(words(&p, "result"), format!("{cancels}:{closes}"));
            if !prevented {
                tap(&mut p, "command");
            }
            tap(&mut p, "done");
            assert_eq!(words(&p, "result"), format!("{cancels}:{}", closes + 1));
        }
    }
}

#[test]
fn authored_closedby_and_scroll_reveal() {
    let mut p = boot(&APP.replace("closedby=\"closerequest\"", "closedby=\"none\""));
    tap(&mut p, "command");
    key(&mut p, "Escape");
    assert!(p.system_back(0.));
    outside(&mut p);
    assert!(!p.surface_json().is_null());
    tap(&mut p, "done");
    let mut p = boot(&APP.replace("closedby=\"closerequest\"", "closedby=\"any\""));
    tap(&mut p, "command");
    outside(&mut p);
    assert!(p.surface_json().is_null());
    assert_eq!(words(&p, "result"), ":1:0");
    assert_eq!(p.focus(), Some(id(&p, "command")));
    tap(&mut p, "tall");
    let root = id(&p, "tall-dialog");
    let r = p.rect_of(root).unwrap();
    // HTML caps the content box at 100% - 2em - 6px; padding adds 2em.
    assert_eq!(r.3, 400. - 2. * 16. - 6. + 2. * 16.);
    let page = p.page();
    p.wheel_at(r.0 + 10., r.1 + 10., 0., 100.);
    assert!(p.scroll_of(root).1 > 0.);
    assert_eq!(p.page(), page);
    let last = id(&p, "last");
    let revealed = agent(&mut p, json!({"op":"reveal", "id":last}));
    assert_eq!(revealed["scrolled"], true, "{revealed}");
    let reply = agent(&mut p, json!({"op":"tap", "id":last}));
    assert!(
        reply.get("error").is_none(),
        "{reply}; root={:?}, scroll={:?}, last={:?}",
        p.rect_of(root),
        p.scroll_of(root),
        p.rect_of(last)
    );
    assert_eq!(words(&p, "result"), ":1:1");
    assert!(p.scroll_of(root).1 > 100.);
    assert_eq!(p.page(), page);
}

#[test]
fn authored_popovers_anchor_dismiss_and_replace() {
    let mut p = boot(APP);
    let page_pixel = p.frame().pixel(470, 390);
    let invoker = id(&p, "invoker");
    let anchor = p.rect_of(invoker).unwrap();
    tap(&mut p, "invoker");
    let root = id(&p, "picker");
    assert_eq!(
        p.rect_of(root),
        Some((anchor.0, anchor.1 + anchor.3 + 4., 160., 90.))
    );
    assert!(p.host().kernel().host_block_implied(root));
    let s = p
        .host()
        .kernel()
        .node(root)
        .unwrap()
        .computed_style(StyleMask::INHERITED);
    assert_ne!(s.background_color, Default::default());
    assert_eq!(s.box_shadow.shadows().len(), 1);
    assert_eq!(s.overflow_y, Overflow::Auto);
    assert_eq!(p.frame().pixel(470, 390), page_pixel);
    assert_eq!(p.focus(), Some(id(&p, "pop-search")));
    let state = agent(&mut p, json!({"op":"state"}));
    assert_eq!(state["surface"]["kind"], "popover");
    assert_eq!(state["surface"]["modal"], false);
    assert_eq!(words(&p, "toggles"), "closed>open");
    // A click outside light-dismisses it and reaches the live page (HTML),
    // here the Page button: it presses and takes the focus.
    outside(&mut p);
    assert!(p.surface_json().is_null());
    assert_eq!(words(&p, "result"), ":0:1");
    assert_eq!(words(&p, "toggles"), "open>closed");
    assert_eq!(p.focus(), Some(id(&p, "page")));
    tap(&mut p, "invoker");
    key(&mut p, "Escape");
    assert!(p.surface_json().is_null());
    tap(&mut p, "invoker");
    tap(&mut p, "replace");
    assert_eq!(p.surface_json()["node"], id(&p, "dialog"));
    assert!(p.rect_of(root).is_none());
    tap(&mut p, "done");
    assert_eq!(p.focus(), Some(invoker));
    tap(&mut p, "manual");
    outside(&mut p);
    key(&mut p, "Escape");
    // Back under a manual popover is the page's (no close watcher).
    assert!(!p.system_back(3.));
    assert!(!p.surface_json().is_null());
    tap(&mut p, "manual-hide");
    assert!(p.surface_json().is_null());
    tap(&mut p, "source-free");
    assert_eq!(p.rect_of(root), Some((160., 155., 160., 90.)));
    p.system_back(4.);
    let mut p = boot(&APP.replace(
        "testId=\"above\" commandfor",
        "margin-left=100 testId=\"above\" commandfor",
    ));
    let anchor = p.rect_of(id(&p, "above")).unwrap();
    tap(&mut p, "above");
    assert_eq!(
        p.rect_of(id(&p, "above-picker")),
        Some((anchor.0 + anchor.2 - 160., anchor.1 - 96., 160., 90.))
    );
    p.system_back(0.);
    let context = p.rect_of(id(&p, "context")).unwrap();
    assert!(p.long_press(context.0 + 10., context.1 + 10., 5.));
    assert_eq!(
        p.rect_of(id(&p, "picker")),
        Some((context.0 + 10., context.1 + 10., 160., 90.))
    );
}

#[test]
fn popover_tracks_a_reflowed_invoker_and_keeps_its_last_box() {
    let source = APP
        .replace("  state query", "  state anchorDisplay = \"block\"\n  action hideAnchor\n    anchorDisplay = \"none\"\n  state query")
        .replace("id=\"invoker\"", "id=\"invoker\" position=\"absolute\" right=20 top=100 display=anchorDisplay")
        .replace("popovertarget=\"picker\" popovertargetaction=\"hide\"", "press=hideAnchor");
    let mut p = boot(&source);
    let invoker = id(&p, "invoker");
    tap(&mut p, "invoker");
    let root = id(&p, "picker");
    let before = p.rect_of(root).unwrap();
    assert!(p.resize(600., 400.).is_none());
    p.frame();
    let anchor = p.rect_of(invoker).unwrap();
    let moved = p.rect_of(root).unwrap();
    assert_eq!(moved.0, before.0 + 120.);
    assert_eq!(
        moved,
        (
            anchor.0 + anchor.2 - 160.,
            anchor.1 + anchor.3 + 4.,
            160.,
            90.
        )
    );
    tap(&mut p, "hide");
    assert!(p.rect_of(invoker).is_none());
    assert!(p.resize(600., 420.).is_none());
    p.frame();
    assert_eq!(p.rect_of(root), Some(moved));
}

#[test]
fn popover_flips_both_axes_near_the_viewport_corner() {
    let mut p = boot(&APP.replace(
        "id=\"invoker\"",
        "id=\"invoker\" position=\"absolute\" top=350 left=360",
    ));
    tap(&mut p, "invoker");
    assert_eq!(p.rect_of(id(&p, "picker")), Some((320., 256., 160., 90.)));
}

#[test]
fn authored_controls_keep_radio_keys_and_tab_inside() {
    let source = r#"component App
  state choice = ""
  action choose(value)
    choice = value
  view
    column width="100%" height="100%"
      button "Open" testId="open" commandfor="box" command="show-modal" height=30
      button "Outside" testId="outside" height=30
      text choice testId="choice"
      dialog id="box" testId="box"
        column width=200
          input type="radio" name="group" value="a" checked=(choice == "a") change=choose testId="a" width=40 height=30
          input type="radio" name="group" value="b" checked=(choice == "b") change=choose testId="b" width=40 height=30
          button "Close" testId="close" commandfor="box" command="close" height=30
"#;
    for popover in [false, true] {
        let source = if popover {
            source
                .replace("command=\"show-modal\"", "command=\"show-popover\"")
                .replace(
                    "dialog id=\"box\"",
                    "column id=\"box\" popover=\"auto\" role=\"dialog\"",
                )
                // A popover moves the focus in only to what autofocuses (HTML).
                .replace("value=\"a\"", "value=\"a\" autofocus=true")
        } else {
            source.into()
        };
        let mut p = boot(&source);
        tap(&mut p, "open");
        assert_eq!(p.focus(), Some(id(&p, "a")));
        key(&mut p, "ArrowRight");
        assert_eq!(p.focus(), Some(id(&p, "b")));
        assert_eq!(words(&p, "choice"), "b");
        let a = id(&p, "a");
        p.type_key(a, "Space", " ", true, false).unwrap();
        assert_eq!(words(&p, "choice"), "a");
        key(&mut p, "ArrowLeft");
        assert_eq!(p.focus(), Some(id(&p, "b")));
        assert_eq!(words(&p, "choice"), "b");
        // Tab stays inside a modal box; a popover's stops follow its
        // invoker's (HTML's popover focus scope), then the page goes on.
        let order = if popover {
            ["close", "outside", "open", "b"]
        } else {
            ["close", "b", "close", "b"]
        };
        for name in order {
            key(&mut p, "Tab");
            assert_eq!(p.focus(), Some(id(&p, name)));
        }
    }
}

#[test]
fn text_only_dialog_receives_keys_instead_of_the_page() {
    let source = r#"component App
  state heard = ""
  action pageKey(key: string)
    heard = `${heard}page,`
  action dialogKey(key: string)
    heard = `${heard}dialog,`
  view
    column width="100%" height="100%" key=pageKey
      button "Open" testId="open" commandfor="box" command="show-modal" height=30
      text heard testId="heard"
      dialog id="box" testId="box" key=dialogKey
        text "No controls"
"#;
    for source in [
        source.to_owned(),
        source.replace("key=dialogKey", "tabindex=-1 key=dialogKey"),
    ] {
        let mut p = boot(&source);
        tap(&mut p, "open");
        assert_eq!(p.focus(), Some(id(&p, "box")));
        p.key(Some('x'), false, 0.);
        assert_eq!(words(&p, "heard"), "dialog,");
    }
    let mut p = boot(&source.replace(" key=dialogKey", ""));
    tap(&mut p, "open");
    assert_eq!(p.focus(), Some(id(&p, "box")));
    p.key(Some('x'), false, 0.);
    assert_eq!(words(&p, "heard"), "");
}

#[test]
fn a_blur_that_removes_the_dialog_restores_focus() {
    let source = r#"component App
  state exists = true
  action remove
    exists = false
  view
    column width="100%" height="100%"
      button "Open" testId="open" blur=remove commandfor="box" command="show-modal" height=30
      if exists
        dialog id="box" testId="box"
          text "Removed during blur"
"#;
    let mut p = boot(source);
    tap(&mut p, "open");
    assert!(p.surface_json().is_null());
    assert!(p.host().kernel().find_by_test_id("box").is_empty());
    assert_eq!(p.focus(), Some(id(&p, "open")));
}

#[test]
fn a_blur_that_opens_another_dialog_keeps_the_newer_surface() {
    let source = r#"component App
  action other
    showModal("second")
  view
    column width="100%" height="100%"
      button "Open" testId="open" blur=other commandfor="first" command="show-modal" height=30
      dialog id="first" testId="first"
        button "First" testId="first-button" commandfor="first" command="close" height=30
      dialog id="second" testId="second"
        input testId="second-field" width=160 height=30
"#;
    let mut p = boot(source);
    tap(&mut p, "open");
    assert_eq!(p.surface_json()["node"], id(&p, "second"));
    assert_eq!(p.focus(), Some(id(&p, "second-field")));
    assert!(p.rect_of(id(&p, "first")).is_none());
    assert!(p.rect_of(id(&p, "second-field")).is_some());
}

#[test]
fn popover_keeps_the_fitting_option_until_its_size_changes_or_it_reopens() {
    let source = APP
        .replace(
            "id=\"invoker\"",
            "id=\"invoker\" position=\"absolute\" top=350 left=100",
        )
        .replace(
            "  state query",
            "  state popHeight = 90\n  action grow\n    popHeight = 100\n  state query",
        )
        .replace(
            "width=160 height=90 toggle=toggling",
            "width=160 height=popHeight toggle=toggling",
        )
        .replace(
            "commandfor=\"dialog\" command=\"show-modal\" height=30",
            "press=grow height=30",
        );
    let mut p = boot(&source);
    tap(&mut p, "invoker");
    let root = id(&p, "picker");
    assert_eq!(p.rect_of(root), Some((100., 256., 160., 90.)));
    assert!(p.resize(480., 600.).is_none());
    p.frame();
    assert_eq!(p.rect_of(root), Some((100., 256., 160., 90.)));
    tap(&mut p, "replace");
    assert_eq!(p.rect_of(root), Some((100., 386., 160., 100.)));
    key(&mut p, "Escape");
    assert!(p.resize(480., 400.).is_none());
    p.frame();
    tap(&mut p, "invoker");
    assert_eq!(p.rect_of(root), Some((100., 246., 160., 100.)));
    assert!(p.resize(480., 600.).is_none());
    p.frame();
    assert_eq!(p.rect_of(root), Some((100., 246., 160., 100.)));
    key(&mut p, "Escape");
    tap(&mut p, "invoker");
    assert_eq!(p.rect_of(root), Some((100., 386., 160., 100.)));
}

#[test]
fn only_menu_popovers_use_the_menu_flyout() {
    for role in ["", " role=\"menu\"", " role=\"group\"", " role=\"dialog\""] {
        for input in [
            "",
            "        column\n          input testId=\"field\" width=140 height=30\n",
        ] {
            let source = format!(
                r#"component App
  state presses = 0
  action press
    presses = presses + 1
  view
    column width="100%" height="100%"
      button "Open" testId="open" popovertarget="box" height=30
      text `${{presses}}` testId="presses"
      column id="box" testId="box" popover="auto"{role} width=180
{input}        button "Press" testId="press" press=press height=30
"#
            );
            let mut p = boot(&source);
            tap(&mut p, "open");
            let menu = role == " role=\"menu\"" || role.is_empty() && input.is_empty();
            assert_eq!(
                p.surface_json()["kind"],
                if menu { "menu" } else { "popover" },
                "{role:?}, {input:?}"
            );
            if !menu && !input.is_empty() {
                let field = id(&p, "field");
                assert!(p.rect_of(field).is_some());
                assert!(p.type_text(field, "kept").is_ok());
            }
            tap(&mut p, "press");
            assert_eq!(words(&p, "presses"), "1");
            assert_eq!(p.surface_json().is_null(), menu);
        }
    }
}
