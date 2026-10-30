//! The loop's clock owns the terminal's time (the timers RFC, D1): a repeat
//! that missed beats while the process was stopped fires once.
use super::*;
use exact_plan::Value;
use std::time::{Duration, Instant};

const TICKER: &str = r#"component App
  state ticks = 0
  action tick
    ticks = ticks + 1
  task ticker mount
    every(1000, tick)
  view
    text "x"
"#;

#[test]
fn a_loop_back_from_a_stop_fires_a_missed_repeat_once() {
    let plan = contract::compile(TICKER).unwrap();
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 80, 40).unwrap();
    // The loop started 3.5 s ago and was stopped since.
    let start = Instant::now() - Duration::from_millis(3500);
    host.live_from(start);
    host.tick(start.elapsed().as_secs_f64() * 1000.0);
    let ticks = host.runner.slot("ticks").and_then(Value::as_number);
    assert_eq!(ticks, Some(1.0), "three missed beats, one fire");
}

#[test]
fn input_clock_runs_due_timer_before_routing_key_and_keeps_focus_events_together() {
    let source = r#"component App
  state order = ""
  state armed = false
  action timer
    order = order + "timer;"
    focus("second")
  action key(event)
    order = order + "key;"
    armed = true
  action blur
    order = order + "blur;"
  action focused
    order = order + "focus;"
  action later
    order = order + "after;"
  task first mount
    after(150, timer)
  task second when armed
    after(1, later)
  view
    column
      button "First" id="first" testId="first" focus=focused
      button "Second" id="second" testId="second" key=key blur=blur
"#;
    let mut host = Host::boot(
        contract::compile(source).unwrap(),
        (),
        Mode::Fullscreen,
        80,
        40,
    )
    .unwrap();
    host.arm = false;
    host.tick(100.);
    host.live_from(Instant::now() - Duration::from_millis(200));
    host.key(Key::Named("Tab"));
    assert_eq!(
        host.runner.slot("order"),
        Some(&Value::str("timer;key;blur;focus;"))
    );
    assert_eq!(host.focus, host.by_test_id("first"));
    host.tick(host.runner.now_ms() + 100.);
    assert_eq!(
        host.runner.slot("order"),
        Some(&Value::str("timer;key;blur;focus;after;"))
    );
}

#[test]
fn paste_and_pointer_input_clock_run_due_timers_before_routing() {
    let source = r#"component App
  state first = ""
  state second = ""
  state expired = false
  state pressed = false
  action timer
    expired = true
    focus("second")
  action editFirst(value)
    first = value
  action editSecond(value)
    second = value
  action press
    pressed = expired
  task expiry mount
    after(150, timer)
  view
    column
      input id="first" testId="first" value=first input=editFirst
      input id="second" testId="second" value=second input=editSecond
      button "Go" testId="go" press=press
"#;
    for input in ["paste", "click", "wheel"] {
        let mut host = Host::boot(
            contract::compile(source).unwrap(),
            (),
            Mode::Fullscreen,
            80,
            40,
        )
        .unwrap();
        host.arm = false;
        host.focus(host.by_test_id("first"));
        host.frame();
        let go = host.cells_of(host.by_test_id("go").unwrap()).unwrap();
        host.tick(100.);
        host.live_from(Instant::now() - Duration::from_millis(200));
        match input {
            "paste" => host.paste("hello"),
            "click" => host.click(go.x, go.y),
            _ => host.wheel(go.x, go.y, 1),
        }
        assert_eq!(
            host.runner.slot("expired"),
            Some(&Value::Bool(true)),
            "{input}"
        );
        assert_eq!(
            host.runner.slot("pressed"),
            Some(&Value::Bool(input == "click")),
            "{input}"
        );
        if input == "paste" {
            assert_eq!(host.runner.slot("first"), Some(&Value::str("")));
            assert_eq!(host.runner.slot("second"), Some(&Value::str("hello")));
        }
    }
}

#[test]
fn review_input_refuses_targets_invalidated_by_due_timer() {
    let mut failures = Vec::new();
    for barrier in [
        "disabled=expired",
        "inert=expired",
        "display=(expired ? \"none\" : \"block\")",
        "visibility=(expired ? \"hidden\" : \"visible\")",
        "removed",
    ] {
        for kind in ["button", "field", "link"] {
            for parent in [false, true] {
                for input in ["press", "click", "key"] {
                    let control = match kind {
                        "field" => "input value=\"\" submit=go",
                        "link" => {
                            "text\n          text \"Go\" press=go href=\"https://example.com/\""
                        }
                        _ => "button \"Go\" press=go",
                    };
                    let (ancestor, own) = if parent { (barrier, "") } else { ("", barrier) };
                    let target = if barrier == "removed" {
                        format!("      if !expired\n        {control} testId=\"target\"")
                    } else {
                        format!(
                            "      column {ancestor}\n        {control} testId=\"target\" {own}"
                        )
                    };
                    let source = format!("component App\n  state expired = false\n  state pressed = false\n  action expire\n    expired = true\n  action go\n    pressed = true\n  task expiry mount\n    after(150, expire)\n  view\n    column\n{target}\n");
                    let mut host = Host::boot(
                        contract::compile(&source).unwrap(),
                        (),
                        Mode::Fullscreen,
                        80,
                        40,
                    )
                    .unwrap();
                    host.arm = false;
                    host.frame();
                    let id = host.by_test_id("target").unwrap();
                    let at = host.cells_of(id).unwrap();
                    if input == "key" {
                        host.focus(Some(id));
                    }
                    host.tick(100.);
                    host.live_from(Instant::now() - Duration::from_millis(200));
                    match input {
                        "click" => host.click(at.x, at.y),
                        "key" => {
                            host.key(Key::Named("Enter"));
                        }
                        _ => {
                            host.advance_for_input();
                            if kind == "link" {
                                host.press_run(id);
                            } else {
                                host.press(id);
                            }
                        }
                    }
                    if host.runner.slot("pressed") != Some(&Value::Bool(false))
                        || host.focus == Some(id)
                        || !host.outbound.is_empty()
                    {
                        failures.push(format!(
                            "{barrier}, kind={kind}, parent={parent}, input={input}"
                        ));
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "invalid targets accepted: {failures:?}"
    );
}

#[test]
fn visible_child_press_reaches_a_hidden_ancestor_handler() {
    let source = r#"component App
  state count = 0
  action go
    count = count + 1
  view
    column visibility="hidden" press=go
      button "Go" visibility="visible" testId="child"
"#;
    for click in [false, true] {
        let mut host = Host::boot(
            contract::compile(source).unwrap(),
            (),
            Mode::Fullscreen,
            80,
            40,
        )
        .unwrap();
        host.frame();
        let child = host.by_test_id("child").unwrap();
        if click {
            let at = host.cells_of(child).unwrap();
            host.click(at.x, at.y);
        } else {
            host.press(child);
        }
        assert_eq!(
            host.runner.slot("count"),
            Some(&Value::Number(1.)),
            "click={click}"
        );
    }
}

#[test]
fn nonmodal_popover_allows_page_focus_and_press() {
    let source = r#"component App
  state focused = 0
  state pressed = 0
  action back
    focus("query")
  action focusQuery
    focused = focused + 1
  action go
    pressed = pressed + 1
  view
    column
      input id="query" testId="query" value="" focus=focusQuery
      button "Go" testId="go" press=go
      column id="suggestions" testId="suggestions" popover="auto"
        button "Return" testId="back" press=back
"#;
    let mut host = Host::boot(
        contract::compile(source).unwrap(),
        (),
        Mode::Fullscreen,
        80,
        40,
    )
    .unwrap();
    host.open_layer(host.by_test_id("suggestions").unwrap(), false);
    host.press(host.by_test_id("back").unwrap());
    assert_eq!(host.runner.slot("focused"), Some(&Value::Number(1.)));
    assert_eq!(host.focus, host.by_test_id("query"));
    host.press(host.by_test_id("go").unwrap());
    assert_eq!(host.runner.slot("pressed"), Some(&Value::Number(1.)));
}

#[test]
fn modal_inside_closed_popover_accepts_input_and_blocks_page() {
    let source = r#"component App
  state count = 0
  action go
    count = count + 1
  view
    column
      button "Outside" testId="outside" press=go
      column popover="auto"
        dialog testId="dialog"
          button "Inside" testId="inside" press=go
        button "Closed" testId="closed" press=go
"#;
    let mut host = Host::boot(
        contract::compile(source).unwrap(),
        (),
        Mode::Fullscreen,
        80,
        40,
    )
    .unwrap();
    host.arm = false;
    host.open_layer(host.by_test_id("dialog").unwrap(), true);
    let inside = host.by_test_id("inside").unwrap();
    host.press(inside);
    assert_eq!(host.runner.slot("count"), Some(&Value::Number(1.)));
    assert_eq!(host.focus, Some(inside));
    host.key(Key::Named("Enter"));
    assert_eq!(host.runner.slot("count"), Some(&Value::Number(2.)));
    for name in ["outside", "closed"] {
        let node = host.by_test_id(name).unwrap();
        host.focus(Some(node));
        host.press(node);
        assert_eq!(host.focus, Some(inside));
        assert_eq!(host.runner.slot("count"), Some(&Value::Number(2.)));
    }
}

#[test]
fn modal_boundary_survives_popovers_above_it() {
    let source = r#"component App
  state text = ""
  state focused = 0
  state pagePresses = 0
  state allowedPresses = 0
  action escape
    focus("page-field")
  action focusedPage
    focused = focused + 1
  action edit(value)
    text = value
  action pagePress
    pagePresses = pagePresses + 1
  action allowedPress
    allowedPresses = allowedPresses + 1
  view
    column
      input id="page-field" testId="page-field" value=text input=edit focus=focusedPage
      button "Page" testId="page-button" press=pagePress
      column popover="auto" testId="earlier"
        button "Earlier" testId="earlier-button" press=pagePress
      dialog testId="modal"
        button "Dialog" testId="dialog-button" press=allowedPress
        column popover="auto" testId="nested"
          button "Escape" testId="nested-escape" press=escape
          button "Nested" testId="nested-button" press=allowedPress
      column popover="auto" testId="sibling"
        button "Escape" testId="sibling-escape" press=escape
        button "Sibling" testId="sibling-button" press=allowedPress
"#;
    let mut failures = Vec::new();
    for popover in ["nested", "sibling"] {
        let mut host = Host::boot(
            contract::compile(source).unwrap(),
            (),
            Mode::Fullscreen,
            80,
            40,
        )
        .unwrap();
        host.arm = false;
        host.frame();
        host.open_layer(host.by_test_id("earlier").unwrap(), false);
        host.open_layer(host.by_test_id("modal").unwrap(), true);
        host.open_layer(host.by_test_id(popover).unwrap(), false);
        host.press(host.by_test_id(&format!("{popover}-escape")).unwrap());
        host.key(Key::Char('x'));
        for name in ["page-field", "page-button", "earlier-button"] {
            let node = host.by_test_id(name).unwrap();
            host.focus(Some(node));
            host.press(node);
        }
        host.key(Key::Char('y'));
        if host.runner.slot("text") != Some(&Value::str(""))
            || host.runner.slot("focused") != Some(&Value::Number(0.))
            || host.runner.slot("pagePresses") != Some(&Value::Number(0.))
        {
            failures.push(format!("{popover}: page focus/input/press escaped modal"));
        }
        for name in ["dialog-button".to_string(), format!("{popover}-button")] {
            let node = host.by_test_id(&name).unwrap();
            host.focus(Some(node));
            assert_eq!(host.focus, Some(node), "{name}");
            host.press(node);
        }
        assert_eq!(
            host.runner.slot("allowedPresses"),
            Some(&Value::Number(2.)),
            "{popover}"
        );
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn a_commit_clears_noncontrol_focus_before_key_defaults() {
    let source = r#"component App
  state expired = false
  state presses = 0
  action expire
    expired = true
  action press
    presses = presses + 1
  action park
    focus("plain")
  task expiry mount
    after(150, expire)
  view
    column
      column press=press
        column id="plain" testId="plain"
          text "Plain"
      button "Focus plain column" testId="park" press=park
      text `${expired}`
"#;
    let mut failures = Vec::new();
    for arm in [true, false] {
        for key in [
            Key::Named("ArrowUp"),
            Key::Named("ArrowDown"),
            Key::Char(' '),
            Key::Named("Enter"),
        ] {
            let mut host = Host::boot(
                contract::compile(source).unwrap(),
                (),
                Mode::Fullscreen,
                80,
                40,
            )
            .unwrap();
            host.arm = arm;
            host.frame();
            host.press(host.by_test_id("park").unwrap());
            assert_eq!(host.focus, host.by_test_id("plain"));
            host.tick(100.);
            host.live_from(Instant::now() - Duration::from_millis(200));
            let description = format!("arm={arm}, key={key:?}");
            host.key(key);
            assert_eq!(host.runner.slot("expired"), Some(&Value::Bool(true)));
            if host.focus.is_some() || host.runner.slot("presses") != Some(&Value::Number(0.)) {
                failures.push(description);
            }
        }
    }
    assert!(
        failures.is_empty(),
        "noncontrol focus changed key defaults: {failures:?}"
    );
}

#[test]
fn timer_waits_use_terminal_turns_and_idle_without_display_demand() {
    for kind in ["frame", "update", "scrollend", "idle"] {
        let plan = contract::compile(&TICKER.replace(
            "every(1000, tick)",
            &format!("after(10, tick, wait={kind})"),
        ))
        .unwrap();
        let mut host = Host::boot(plan, (), Mode::Fullscreen, 80, 40).unwrap();
        host.runner.hold_waits(exact_runner::WaitKinds::ALL);
        host.runner.set_live_clock(Some(Box::new(|| 10.0)));
        host.tick(10.0);
        assert_eq!(
            host.runner.slot("ticks"),
            Some(&Value::Number(if kind == "idle" { 0.0 } else { 1.0 }))
        );
        host.idle_timer(10.0);
        assert_eq!(host.runner.slot("ticks"), Some(&Value::Number(1.0)));
    }
}
