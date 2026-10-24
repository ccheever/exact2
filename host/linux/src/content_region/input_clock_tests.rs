use super::*;

fn binding_source(timer: &str) -> String {
    actions::source()
        .replace(
            "  state revision = 0",
            "  state binding = \"old\"\n  state revision = 0",
        )
        .replace("replyTo(m.id)", "replyTo(binding)")
        .replace("input value=draft", "input id=\"composer\" value=draft")
        .replace("  action body\n", &format!("{timer}  action body\n"))
}

#[test]
fn review_timer_effect_is_serviced_when_only_binding_changes() {
    let _service = crate::content_region::test_service();
    for swipe in [false, true] {
        let source = binding_source("  action expire\n    binding = \"new\"\n    focus(\"composer\")\n  task expiry mount\n    after(150, expire)\n");
        let mut p = actions::boot(&source, true, actions::Rows::default());
        let reply = actions::id(
            &p,
            if swipe {
                "reply-hit-message-0"
            } else {
                "reply-message-0"
            },
        );
        let key = p.host.kernel().node(reply).unwrap().key;
        let _gate = actions::block_body(&mut p, 100.);
        let contact = p.retained_down(key).unwrap();
        let epoch = p.host.kernel().epoch();
        if swipe {
            let held = p
                .host
                .hold_begin(reply, exact_motion::Property::Translate, 100.)
                .unwrap()
                .unwrap();
            assert_eq!(
                p.retained_swipe_dispatch(contact.swipe.as_ref().unwrap(), held.token, 200.),
                Ok(false)
            );
        } else {
            assert_eq!(
                p.retained_press_target(contact.press.as_ref().unwrap(), 200.),
                None
            );
        }
        assert_eq!(p.host.runner().slot("binding"), Some(&Value::str("new")));
        assert_eq!(
            p.host.kernel().epoch(),
            epoch,
            "timer changed no rendered property"
        );
        assert_eq!(actions::selected(&p), Value::str(""));
        assert!(
            p.commands.iter().any(|command| command.name == "focus"),
            "timer command must reach the loop, swipe={swipe}"
        );
        p.run_commands(actions::Rows::default);
        assert_eq!(p.focus, Some(actions::id(&p, "composer")), "swipe={swipe}");
    }
}

#[test]
fn review_timer_revalidates_retained_swipe_policy() {
    let _service = crate::content_region::test_service();
    for (old, new) in [
        ("touch-action=\"pan-y\"", "touch-action=(expired ? \"none\" : \"pan-y\")"),
        ("transition=\"translate -exact-spring(300, 30, 1)\"", "transition=(expired ? \"translate -exact-spring(200, 20, 1)\" : \"translate -exact-spring(300, 30, 1)\")"),
    ] {
        let source = actions::source().replace("  state revision = 0", "  state expired = false\n  state revision = 0")
            .replace("  action body\n", "  action expire\n    expired = true\n  task expiry mount\n    after(150, expire)\n  action body\n")
            .replace(old, new);
        let mut p = actions::boot(&source, true, actions::Rows::default());
        let reply = actions::id(&p, "reply-hit-message-0");
        let key = p.host.kernel().node(reply).unwrap().key;
        let _gate = actions::block_body(&mut p, 100.);
        let contact = p.retained_down(key).unwrap();
        let held = p.host.hold_begin(reply, exact_motion::Property::Translate, 100.).unwrap().unwrap();
        assert_eq!(p.retained_swipe_dispatch(contact.swipe.as_ref().unwrap(), held.token, 200.), Ok(false), "{old}");
        assert_eq!(actions::selected(&p), Value::str(""));
    }
}

#[test]
fn review_timer_error_does_not_accept_an_invalidated_press_or_swipe() {
    let _service = crate::content_region::test_service();
    for swipe in [false, true] {
        let source = binding_source("  state fault = false\n  resource failure = clockFault(fault) as shape number\n  action expire\n    binding = \"new\"\n  action fail\n    fault = true\n  task expiry mount\n    after(150, expire)\n  task failure mount\n    after(175, fail)\n");
        let mut p = actions::boot(&source, true, actions::Rows::default());
        let reply = actions::id(
            &p,
            if swipe {
                "reply-hit-message-0"
            } else {
                "reply-message-0"
            },
        );
        let at = actions::point(&mut p, reply);
        let _gate = actions::block_body(&mut p, 100.);
        let composer = actions::id(&p, "composer");
        p.focus = Some(composer);
        if swipe {
            assert!(p.pointer_down(at.0, at.1, 110.).unwrap());
            assert!(p.pointer_move(at.0 + 10., at.1, 120.).unwrap());
            assert!(p.pointer_move(at.0 + 80., at.1, 130.).unwrap());
            assert_eq!(p.pointer_up(at.0 + 90., at.1, 200.), Ok(false));
            assert!(p.contact.is_none());
        } else {
            assert_eq!(p.press_at(at.0, at.1, 200.), None);
        }
        assert_eq!(p.focus, Some(composer));
        assert_eq!(actions::selected(&p), Value::str(""));
        assert!(
            p.host.runner().journal().any(|line| line.contains("Shape")),
            "timer error stays reported"
        );
    }
}

#[test]
#[ignore = "QUEUE: hidden pending content lacks a final paint offer"]
fn review_hidden_pending_content_paints() {
    let _service = crate::content_region::test_service();
    let mut p = actions::boot(&actions::source(), true, actions::Rows::default());
    let _gate = actions::block_body(&mut p, 100.);
    actions::ack(&mut p);
    actions::action(&mut p, "hide", 150.);
    actions::ack(&mut p);
}

#[test]
fn retained_release_revalidates_the_original_hit_path() {
    let _service = crate::content_region::test_service();
    let mut delivered = Vec::new();
    for wrapper in [false, true] {
        for swipe in [false, true] {
            let source = actions::source()
                .replace("  state revision = 0", "  state expired = false\n  state revision = 0")
                .replace("  action body\n", "  action expire\n    expired = true\n  task expiry mount\n    after(150, expire)\n  action body\n")
                .replace("swiperight=replyTo(m.id)", "press=replyTo(m.id) swiperight=replyTo(m.id)")
                .replace("                MessageBubble(message=m)", &format!(
                    "                box inert={}\n                  box testId=`leaf-${{m.id}}` inert={} width=320 height=32\n                  MessageBubble(message=m)",
                    if wrapper { "expired" } else { "false" },
                    if wrapper { "false" } else { "expired" },
                ));
            let mut p = actions::boot(&source, true, actions::Rows::default());
            let leaf = actions::id(&p, "leaf-message-0");
            let at = actions::point(&mut p, leaf);
            let _gate = actions::block_body(&mut p, 100.);
            assert!(p.pointer_down(at.0, at.1, 110.).unwrap());
            if swipe {
                assert!(p.pointer_move(at.0 + 10., at.1, 120.).unwrap());
                assert!(p.pointer_move(at.0 + 80., at.1, 130.).unwrap());
                assert!(p.contact.as_ref().unwrap().hold.is_some());
            }
            let accepted = p.pointer_up(at.0 + if swipe { 90. } else { 0. }, at.1, 200.);
            assert_eq!(p.host.runner().slot("expired"), Some(&Value::Bool(true)));
            if accepted != Ok(false) || actions::selected(&p) != Value::str("") {
                delivered.push((wrapper, swipe, accepted));
            }
            assert!(p.contact.is_none());
        }
    }
    assert!(
        delivered.is_empty(),
        "invalidated paths delivered: {delivered:?}"
    );
}
