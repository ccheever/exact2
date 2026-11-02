use super::*;

fn message(p: &Presenter<()>) -> ViewId {
    p.host()
        .kernel()
        .node(id(p, "message"))
        .unwrap()
        .parent
        .unwrap()
}

fn source(words: &str) -> String {
    with_alert(
        "        text \"The count goes back to zero.\"",
        &format!("        text \"{words}\" testId=\"message\""),
    )
}

fn press(p: &mut Presenter<()>, name: &str) {
    let reply = key(p, name);
    assert!(reply.get("error").is_none(), "{reply}");
    p.run_commands(|| ());
    p.frame();
}

#[test]
fn long_message_takes_tab_and_scroll_keys_without_leaving_the_alert() {
    setup();
    let mut p = opened(&source(&long()).replace("height=\"100%\"", "height=2000"));
    let message = message(&p);
    let clear = id(&p, "clear-confirm-clear");
    let cancel = id(&p, "clear-confirm-cancel");
    assert_eq!(p.focus(), Some(clear), "the message is not initial focus");
    let before = p.frame();
    press(&mut p, "Shift+Tab");
    assert_eq!(p.focus(), Some(message));
    let after = p.frame();
    let (x, y, w, h) = p.rect_of(message).unwrap();
    let edge = |pixels: &[u8]| {
        let mut result = Vec::new();
        for py in (y as usize + 4)..((y + h) as usize - 4) {
            for px in (x as usize - 2)..=(x as usize + 2) {
                let at = (py * before.width() as usize + px) * 4;
                result.extend_from_slice(&pixels[at..at + 4]);
            }
        }
        result
    };
    assert!(w > 0. && h > 8.);
    assert_ne!(
        edge(before.data()),
        edge(after.data()),
        "the message paints a focus ring"
    );
    press(&mut p, "Tab");
    for expected in [cancel, message, clear] {
        press(&mut p, "Tab");
        assert_eq!(p.focus(), Some(expected));
    }
    for expected in [message, cancel, clear, message] {
        press(&mut p, "Shift+Tab");
        assert_eq!(p.focus(), Some(expected));
    }
    let page = p.host().kernel().node(message).unwrap().frame.height * 0.875;
    let text = id(&p, "message");
    let top = p.rect_of(text).unwrap().1;
    for (key, expected) in [
        ("ArrowDown", 40.),
        ("ArrowUp", 0.),
        ("PageDown", page),
        ("PageUp", 0.),
        ("Space", page),
        ("Shift+Space", 0.),
    ] {
        press(&mut p, key);
        assert!((p.scroll_of(message).1 - expected).abs() < 0.01, "{key}");
        assert!((p.rect_of(text).unwrap().1 - (top - expected)).abs() < 0.01);
    }
    press(&mut p, "End");
    let end = p.scroll_of(message);
    assert!(end.1 > page);
    p.wheel(message, 0., 100_000.).unwrap();
    assert_eq!(
        p.scroll_of(message),
        end,
        "keyboard and wheel share the limit"
    );
    for key in ["End", "ArrowDown", "PageDown", "Space"] {
        press(&mut p, key);
        assert_eq!(p.scroll_of(message), end);
        assert_eq!(p.page(), (0., 0.), "no scroll chains outside the modal");
    }
    for key in ["Home", "ArrowUp", "PageUp", "Shift+Space"] {
        press(&mut p, key);
        assert_eq!(p.scroll_of(message), (0., 0.));
    }
    press(&mut p, "Enter");
    assert_eq!(p.focus(), Some(message));
    press(&mut p, "Tab");
    press(&mut p, "Space");
    assert_eq!(words(&p, "counts"), "Count 0");
    assert_eq!(p.focus(), Some(id(&p, "ask-clear")));
}

#[test]
fn message_stop_tracks_overflow_on_either_axis_and_keeps_focus_when_it_shrinks() {
    use exact_kernel::StyleId;
    setup();
    let mut p = opened(&source("Short."));
    let message = message(&p);
    let clear = id(&p, "clear-confirm-clear");
    let cancel = id(&p, "clear-confirm-cancel");
    for expected in [cancel, clear, cancel, clear] {
        press(&mut p, "Tab");
        assert_eq!(p.focus(), Some(expected), "a short message is no stop");
    }
    let text = id(&p, "message");
    style(&mut p, text, StyleId::Width, "1000px");
    press(&mut p, "Shift+Tab");
    assert_eq!(p.focus(), Some(message), "horizontal overflow is a stop");
    press(&mut p, "ArrowRight");
    assert_eq!(p.scroll_of(message), (40., 0.));
    press(&mut p, "ArrowLeft");
    assert_eq!(p.scroll_of(message), (0., 0.));
    style(&mut p, text, StyleId::Width, "100px");
    assert_eq!(p.focus(), Some(message), "shrinking does not move focus");
    press(&mut p, "Tab");
    assert_eq!(p.focus(), Some(clear));
    press(&mut p, "Shift+Tab");
    assert_eq!(p.focus(), Some(cancel), "the short box has left the order");
    press(&mut p, "Enter");
    assert_eq!(p.focus(), Some(id(&p, "ask-clear")));
}

#[test]
fn alert_key_handler_can_prevent_message_scrolling() {
    setup();
    let source = source(&long())
        .replace("  action askClear", "  action stop(key: string, e: KeyboardEvent)\n    if key == \"ArrowDown\"\n      preventDefault()\n  action askClear")
        .replace("role=\"alertdialog\"", "role=\"alertdialog\" key=stop tabindex=-1");
    let mut p = opened(&source);
    let message = message(&p);
    press(&mut p, "Shift+Tab");
    assert_eq!(p.focus(), Some(message));
    press(&mut p, "ArrowDown");
    assert_eq!(p.scroll_of(message), (0., 0.));
    press(&mut p, "PageDown");
    assert!(p.scroll_of(message).1 > 0.);
}

#[test]
fn shortening_the_focused_message_removes_its_stop_without_blurring() {
    setup();
    let source = source("")
        .replace("  state count = 7", &format!("  state count = 7\n  state message = \"{}\"\n  action shorten(key: string, e: KeyboardEvent)\n    if key == \"s\"\n      message = \"Short.\"", long()))
        .replace("text \"\" testId=\"message\"", "text message testId=\"message\"")
        .replace("role=\"alertdialog\"", "role=\"alertdialog\" key=shorten tabindex=-1");
    let mut p = opened(&source);
    let message = message(&p);
    press(&mut p, "Shift+Tab");
    assert_eq!(p.focus(), Some(message));
    press(&mut p, "End");
    assert!(p.scroll_of(message).1 > 0.);
    press(&mut p, "s");
    assert_eq!(words(&p, "message"), "Short.");
    assert_eq!(p.focus(), Some(message));
    press(&mut p, "Tab");
    assert_eq!(p.focus(), Some(id(&p, "clear-confirm-clear")));
    press(&mut p, "Shift+Tab");
    assert_eq!(p.focus(), Some(id(&p, "clear-confirm-cancel")));
}
