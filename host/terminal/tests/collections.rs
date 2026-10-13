//! The terminal's real viewport drives the shared collection window.
//! @ref LLP 1010 §6.5 / LLP 1101 D6-D7.

use exact_terminal::host::{Host, Key, Mode};
use exact_terminal::vt::Vt;

fn app(extra: &str, row: &str) -> String {
    let numbers = (0..200)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "component App\n  state picked = -1\n  action pick(n: number)\n    picked = n\n  view\n    column width=\"100%\" height=\"100%\"\n      text `picked ${{picked}}`\n      list virtualized=true estimated-item-height=16 flex=1 min-height=0 testId=\"items\" {extra}\n        each n in [{numbers}] key=n\n{row}\n"
    )
}

fn boot(source: &str, cols: usize, rows: usize) -> (Host<()>, Vt) {
    let plan = contract::compile_terminal(source).unwrap();
    let mut host = Host::boot(plan, (), Mode::Fullscreen, cols, rows).unwrap();
    host.arm = false;
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    (host, vt)
}

fn rows() -> &'static str {
    "          button press=pick(n) testId=`row-${n}` text-align=\"start\" width=\"100%\"\n            text `row ${n}`"
}

#[test]
fn wheel_and_ends_fill_new_windows_before_paint() {
    let (mut host, mut vt) = boot(&app("", rows()), 40, 8);
    assert!(vt.text(false).contains("row 0"));
    assert!(host.by_test_id("row-199").is_none());
    let initial_nodes = host.kernel().live_count();
    host.scroll_rows(100);
    vt.render(&mut host);
    assert!(vt.text(false).contains("row 100"), "{}", vt.text(false));
    assert!(host.by_test_id("row-0").is_none());
    host.scroll_edge(true);
    vt.render(&mut host);
    assert!(vt.text(false).contains("row 199"), "{}", vt.text(false));
    host.scroll_edge(false);
    vt.render(&mut host);
    assert!(vt.text(false).contains("row 0"));
    assert!(host.by_test_id("row-199").is_none());
    assert!(host.kernel().live_count() < initial_nodes + 40);
}

#[test]
fn resizing_measures_rows_at_the_new_width_and_keeps_the_end() {
    let row =
        "          text `row ${n} with enough words to wrap onto several lines` testId=`row-${n}`";
    let (mut host, mut vt) = boot(
        &app("scroll-start=\"end\" scrollFollowEnd=true", row),
        40,
        8,
    );
    assert!(vt.text(false).contains("row 199"), "{}", vt.text(false));
    host.resize(25, 12);
    vt.render(&mut host);
    assert!(vt.text(false).contains("row 199"), "{}", vt.text(false));
    host.scroll_edge(false);
    vt.render(&mut host);
    assert!(vt.text(false).contains("row 0"));
    host.resize(50, 10);
    vt.render(&mut host);
    assert!(vt.text(false).contains("row 0"));
}

#[test]
fn a_scrolled_away_focused_row_stays_alive_until_blur() {
    let (mut host, mut vt) = boot(&app("", rows()), 40, 8);
    let first = host.by_test_id("row-0").unwrap();
    host.focus(Some(first));
    host.scroll_edge(true);
    vt.render(&mut host);
    assert!(host.kernel().node(first).is_some(), "focus is pinned");
    assert!(vt.text(false).contains("row 199"));
    host.focus(None);
    vt.render(&mut host);
    assert!(host.kernel().node(first).is_none(), "blur releases the pin");
}

#[test]
fn arrow_navigation_continues_through_several_windows() {
    let (mut host, mut vt) = boot(&app("", rows()), 40, 8);
    host.focus(host.by_test_id("row-0"));
    for _ in 0..100 {
        host.key(Key::Named("ArrowDown"));
        vt.render(&mut host);
    }
    host.key(Key::Named("Enter"));
    vt.render(&mut host);
    assert!(vt.text(false).contains("picked 100"), "{}", vt.text(false));
}

#[test]
fn tab_enters_at_the_logical_start_and_arrows_follow_a_scrolled_away_focus() {
    for focused in [false, true] {
        let (mut host, mut vt) = boot(&app("", rows()), 40, 8);
        if focused {
            host.focus(host.by_test_id("row-0"));
        }
        host.scroll_edge(true);
        vt.render(&mut host);
        host.key(Key::Named(if focused { "ArrowDown" } else { "Tab" }));
        vt.render(&mut host);
        host.key(Key::Named("Enter"));
        vt.render(&mut host);
        let wanted = if focused { "picked 1" } else { "picked 0" };
        assert!(vt.text(false).contains(wanted), "{}", vt.text(false));
    }
}

#[test]
fn keys_reach_rows_even_when_the_port_has_no_room() {
    let (mut host, mut vt) = boot(&app("", rows()), 10, 1);
    host.focus(host.by_test_id("row-0"));
    for _ in 0..40 {
        host.key(Key::Named("ArrowDown"));
    }
    host.key(Key::Named("Enter"));
    vt.render(&mut host);
    assert!(vt.text(false).contains("picked 40"), "{}", vt.text(false));
}

#[test]
fn hidden_dialog_rows_retire_but_the_clicked_rows_handler_still_runs() {
    let source = r#"component App
  state picked = "none"
  action pick(v: string)
    picked = v
  view
    column
      text `picked ${picked}`
      button commandfor="dialog" command="show-modal" testId="open"
        text "open"
      dialog id="dialog" closedby="any" width="20ch" height="8lh" display="flex" flex-direction="column"
        list virtualized=true estimated-item-height=16 flex=1 min-height=0
          each v in ["alpha", "beta", "gamma"] key=v
            button press=pick(v) commandfor="dialog" command="close" testId=v
              text v
"#;
    let (mut host, mut vt) = boot(source, 40, 12);
    assert!(host.by_test_id("alpha").is_none());
    host.press(host.by_test_id("open").unwrap());
    vt.render(&mut host);
    assert!(host.by_test_id("alpha").is_some());
    host.key(Key::Named("ArrowDown"));
    host.key(Key::Named("Enter"));
    vt.render(&mut host);
    assert!(vt.text(false).contains("picked beta"), "{}", vt.text(false));
    assert!(host.by_test_id("alpha").is_none());
    assert!(host.by_test_id("beta").is_none());
    host.press(host.by_test_id("open").unwrap());
    vt.render(&mut host);
    assert!(host.by_test_id("alpha").is_some());
}

#[test]
fn resize_after_logical_navigation_matches_an_eager_scroller() {
    for steps in [3, 30, 100] {
        let source = app("", rows());
        let eager = source.replace("list virtualized=true estimated-item-height=16", "scroll");
        let (mut a, _) = boot(&source, 10, 1);
        let (mut b, _) = boot(&eager, 10, 1);
        a.focus(a.by_test_id("row-0"));
        b.focus(b.by_test_id("row-0"));
        for _ in 0..steps {
            a.key(Key::Named("ArrowDown"));
            b.key(Key::Named("ArrowDown"));
        }
        a.resize(40, 15);
        b.resize(40, 15);
        assert_eq!(a.frame().grid, b.frame().grid, "after {steps} steps");
    }
}

#[test]
fn logical_navigation_reaches_zero_height_rows() {
    let row = rows().replace(
        "width=\"100%\"",
        "width=\"100%\" height=0 overflow=\"hidden\"",
    );
    let (mut host, mut vt) = boot(&app("", &row), 40, 8);
    host.key(Key::Named("Tab"));
    for _ in 0..100 {
        host.key(Key::Named("ArrowDown"));
    }
    host.key(Key::Named("Enter"));
    vt.render(&mut host);
    assert!(vt.text(false).contains("picked 100"), "{}", vt.text(false));
}

#[test]
fn navigation_skips_rows_without_enabled_controls_in_both_directions() {
    let row = "          column\n            text `label ${n}`\n            when n == 0 || n == 100 || n == 199\n              button disabled=(n == 100) press=pick(n) testId=`row-${n}`\n                text `row ${n}`";
    let (mut host, mut vt) = boot(&app("", row), 40, 8);
    host.key(Key::Named("Tab"));
    host.key(Key::Named("ArrowDown"));
    host.key(Key::Named("Enter"));
    vt.render(&mut host);
    assert!(vt.text(false).contains("picked 199"), "{}", vt.text(false));
    host.key(Key::Named("ArrowUp"));
    host.key(Key::Named("Enter"));
    vt.render(&mut host);
    assert!(vt.text(false).contains("picked 0"), "{}", vt.text(false));
}

#[test]
fn followed_end_tracks_changed_terminal_list_padding() {
    let source = app(
        "padding-top=\"1lh\" padding-bottom=(pad == 4 ? \"4lh\" : pad == 1 ? \"1lh\" : \"2lh\") scroll-start=\"end\" scrollFollowEnd=true",
        "          text `row ${n}` height=\"1lh\"",
    ).replace(
        "  state picked = -1",
        "  state picked = -1\n  state pad = 2\n  action grow\n    pad = 4\n  action shrink\n    pad = 1",
    );
    let (mut host, mut vt) = boot(&source, 40, 8);
    for (action, last_line) in [(None, 5), (Some("grow"), 3), (Some("shrink"), 6)] {
        if let Some(action) = action {
            host.act(action);
            vt.render(&mut host);
        }
        let screen = vt.text(false);
        assert_eq!(
            screen.lines().position(|line| line.contains("row 199")),
            Some(last_line),
            "{action:?}: {screen}"
        );
    }
}
