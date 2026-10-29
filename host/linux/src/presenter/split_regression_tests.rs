use super::*;
use exact_runner::Value;

pub(super) fn act(p: &mut Presenter<()>, name: &str, args: Vec<Value>) {
    let receipt = p.host.runner_mut().act(name, args).unwrap();
    let at_ms = p.host.now();
    assert_eq!(
        p.host
            .commit(&[exact_runner::Timed { at_ms, receipt }], None),
        None
    );
    assert_eq!(p.after_commit(), None);
}

#[test]
fn split_refuses_a_content_box_percentage_width_once() {
    let source = APP
        .replace("SPLIT", "auto")
        .replace("position=\"absolute\" inset=0", "width=\"100%\" height=800");
    let mut p = boot_source(1200., &source);
    let home = id(&p, "route-home");
    for width in [1200., 1400., 1000.] {
        assert_eq!(p.resize(width, 800.), None);
        for _ in 0..4 {
            let stamp = p.host.kernel().layout_stamp();
            p.sync_split();
            assert_eq!(p.host.kernel().layout_stamp(), stamp);
            assert_eq!(p.after_commit(), None);
            assert!(state(&mut p).is_null());
            assert_eq!(cover(&p, home), None);
            assert_eq!(p.host.kernel().node(home).unwrap().frame.width, width);
        }
    }
    let logs = p.host.agent(r#"{"op":"logs"}"#);
    assert_eq!(logs.matches("cannot narrow").count(), 1, "{logs}");
}

#[test]
fn split_parent_scroll_moves_runner_frames_and_point_reads_with_paint() {
    let source = APP.replace("SPLIT", "auto")
        .replace("main navigationKey", "main overflow=\"auto\" testId=\"parent\" navigationKey")
        .replace("  state value", "  state bounds = []\n  state found = \"\"\n  action probe(x: number, y: number)\n    bounds = [frame(\"editor\").x, frame(\"editor\").y]\n    found = match elementFromPoint(x, y) { case some(id) => id, case none => \"none\" }\n  state value")
        .replace("input value=", "input id=\"editor\" value=") + "      box position=\"absolute\" top=1200 width=1 height=1\n";
    let mut p = boot_source(1200., &source);
    tap(&mut p, "a");
    let parent = id(&p, "parent");
    let field = id(&p, "field-item");
    let before = p.box_of(field).unwrap().rect;
    p.wheel(parent, 0., 100.).unwrap();
    assert_eq!(p.scroll_of(parent).1, 100.);
    let painted = p.box_of(field).unwrap().rect;
    assert_eq!(painted.1, before.1 - 100.);
    let receipt = p
        .host
        .runner_mut()
        .act(
            "probe",
            vec![
                Value::Number(painted.0 as f64 + 4.),
                Value::Number(painted.1 as f64 + 4.),
            ],
        )
        .unwrap();
    assert_eq!(
        p.host.runner().slot("bounds"),
        Some(&Value::list(vec![
            Value::Number(painted.0 as f64),
            Value::Number(painted.1 as f64)
        ]))
    );
    assert_eq!(p.host.runner().slot("found"), Some(&Value::str("editor")));
    assert!(!receipt.layout_invalidated);
}

#[test]
fn split_parent_scroll_moves_a_detail_lists_reorder_clip() {
    let source = APP.replace("SPLIT", "auto")
        .replace("main navigationKey", "main overflow=\"auto\" testId=\"parent\" navigationKey")
        .replace(
            "            input value=value change=change testId=`field-${e.name}`",
            "            box height=600 flex-shrink=0\n            list height=200 flex-shrink=0 testId=`dlist-${e.name}`\n              box height=40\n                text \"Row\"",
        ) + "      box position=\"absolute\" top=1200 width=1 height=1\n";
    let mut p = boot_source(1200., &source);
    tap(&mut p, "a");
    let parent = id(&p, "parent");
    let list = id(&p, "dlist-item");
    let key = p.host.kernel().node(list).unwrap().key;
    p.wheel(parent, 0., 100.).unwrap();
    assert_eq!(p.scroll_of(parent).1, 100.);
    let painted = p.box_of(list).unwrap().rect;
    let (_, clip, _) = p.list_port(key).unwrap();
    // The pane ends at 800 - 100 in the viewport once its parent scrolled.
    assert_eq!(clip.1, painted.1);
    assert_eq!(clip.1 + clip.3, 700.);
}

#[test]
fn split_fullscreen_hides_the_list_while_modal_leaves_it_visible() {
    for presentation in ["modal", "fullscreen"] {
        let mut p = boot_source(
            1200.,
            &APP.replace("SPLIT", "auto")
                .replace("\"modal\"", &format!("\"{presentation}\"")),
        );
        tap(&mut p, "a");
        tap(&mut p, "sheet-item");
        let row = id(&p, "a");
        assert_eq!(
            p.host.route_visibility(row),
            (presentation == "fullscreen", true)
        );
        assert_eq!(p.box_of(row).is_none(), presentation == "fullscreen");
        assert!(p.host.runner().inactive(row));
        tap(&mut p, "close");
        assert_eq!(p.host.route_visibility(row), (false, false));
    }
}

#[test]
fn split_scroller_accessibility_stops_at_the_list_column() {
    let source = APP.replace("SPLIT", "auto")
        .replace("position=\"absolute\" inset=0", "position=\"absolute\" inset=0 overflow=\"auto\"")
        .replace("            box navigationSplit", "            box width=1000 height=1200 flex-shrink=0 aria-label=\"overflow\" role=\"region\"\n            box navigationSplit");
    let mut p = boot_source(1200., &source);
    let home = id(&p, "route-home");
    let nodes = p.access_tree().0;
    let port = nodes.iter().find(|n| n.view == home).unwrap();
    assert_eq!(port.role, access::role::SCROLLER);
    assert_eq!(port.rect[0] + port.rect[2], 320.);
    let child = nodes.iter().find(|n| n.name == "overflow").unwrap();
    assert_eq!(child.rect[0] + child.rect[2], 320.);
}

#[test]
fn split_detail_text_damages_only_the_pane() {
    let source = APP.replace("SPLIT", "auto").replace(
        "            input value=value change=change",
        "            text value width=250 height=40",
    );
    let mut p = boot_source(1200., &source);
    tap(&mut p, "a");
    p.pointer = None;
    p.blur();
    p.frame();
    act(&mut p, "change", vec![Value::str("A changed detail")]);
    let partial = p.frame();
    let damage = p.damage_rects().to_vec();
    assert!(!damage.is_empty());
    assert!(
        damage
            .iter()
            .all(|&(x, _, w, _)| x >= 321. && x + w <= 1200.),
        "{damage:?}"
    );
    p.brush.colors_changed();
    p.dirty = true;
    assert_eq!(
        partial.data(),
        p.frame().data(),
        "partial repaint matches a full repaint"
    );
}

#[test]
fn split_detail_labels_inline_and_overflowing_text_repaint_whole() {
    for label in [
        "button width=250 height=40\n              text value",
        "text width=250 height=40\n              text \"Value: \"\n              text value font-weight=700",
        "text value width=40 height=20 white-space=\"nowrap\"",
    ] {
        let source = APP.replace("SPLIT", "auto").replace(
            "            input value=value change=change",
            &format!("            {label}"),
        );
        let mut p = boot_source(1200., &source);
        tap(&mut p, "a");
        p.pointer = None;
        p.blur();
        p.frame();
        act(&mut p, "change", vec![Value::str("A changed detail")]);
        let partial = p.frame();
        p.brush.colors_changed();
        p.dirty = true;
        assert!(partial.data() == p.frame().data(), "{label}");
    }
}

#[test]
fn split_back_comes_from_its_own_navigation_ancestor() {
    let source = APP.replace("SPLIT", "auto")
        .replace("  view\n    main", "  view\n    column navigationBack=\"other-back\"\n      text \"Other navigator\"\n    main")
        .replace("            header", "            button id=\"other-back\" press=back testId=\"wrong-back\"\n              text \"Other Back\"\n            header");
    let source = source.replace(" navigate=follow", "");
    let (head, body) = source.split_once("  view\n").unwrap();
    let source = format!(
        "{head}  view\n    box width=\"100%\" height=\"100%\"\n{}",
        body.lines()
            .map(|line| format!("  {line}\n"))
            .collect::<String>()
    );
    let mut p = boot_source(1200., &source);
    // Select the second navigator's stack, as a shell can do for its active container.
    let home = id(&p, "route-home");
    p.host.navigation.stack = Some(vec![home]);
    assert_eq!(p.after_commit(), None);
    act(&mut p, "open", vec![Value::str("a")]);
    let item = id(&p, "route-item");
    p.host.navigation.stack = Some(vec![home, item]);
    assert_eq!(p.after_commit(), None);
    assert_eq!(cover(&p, id(&p, "back-item")), Some(HostCover::Whole));
    assert_eq!(cover(&p, id(&p, "wrong-back")), None);
}

#[test]
fn split_recycled_action_row_forgets_the_previous_selection() {
    let items = (0..200)
        .map(|n| format!("\"{n}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let mut source = APP.replace("SPLIT", "auto").replace(
        "  state value",
        &format!("  state items = [{items}]\n  state value"),
    );
    let start = source.find("            list testId").unwrap();
    let end = source[start..]
        .find("            box navigationSplit")
        .unwrap()
        + start;
    source.replace_range(start..end, "            list virtualized=true height=240 testId=\"rows\"\n              for x in items key=x\n                box press=open(x) height=48 testId=`row-${x}`\n                  text x\n");
    let mut p = boot_source(1200., &source);
    p.host.runner_mut().set_row_reuse(true);
    assert_eq!(p.settle_collections(), None);
    tap(&mut p, "row-0");
    let selected = id(&p, "row-0");
    let key = p.host.kernel().node(selected).unwrap().key;
    assert_eq!(split(&p).row, Some(selected));
    let list = id(&p, "rows");
    for top in [1000., 2000.] {
        p.scroll_to(list, (0., top)).unwrap();
        assert_eq!(p.settle_collections(), None);
    }
    let reused = p.host.kernel().node_by_key(key).unwrap();
    assert_ne!(reused.props.str(PropId::TestId), Some("row-0"));
    assert_eq!(split(&p).row, None);
    assert!(split(&p).detail.is_some());
}

#[test]
fn split_row_recycled_by_its_own_press_is_not_selected() {
    // The runner recycles rows as they scroll, so a press whose commits
    // recycle its own row is staged: the press is captured, its row renewed,
    // then the route it pushed is associated.
    let mut p = boot(1200., "auto");
    let row = id(&p, "a");
    let press = p.capture_split_row(row);
    assert!(press.is_some());
    let key = p.host.kernel().node(row).unwrap().key;
    let mut state = std::mem::take(&mut p.host.navigation.split);
    state.renewed(p.host.kernel(), &[key]);
    p.host.navigation.split = state;
    act(&mut p, "open", vec![Value::str("a")]);
    p.associate_split_row(press);
    assert_eq!(p.after_commit(), None);
    let item = id(&p, "route-item");
    assert_eq!(split(&p).base, Some(item));
    assert_eq!(split(&p).row, None);
}

#[test]
fn split_detail_menu_and_alert_anchor_and_block_underlying_presses() {
    let source = APP.replace("SPLIT", "auto")
        .replace("  state value", "  state presses = 0\n  action poke\n    presses = presses + 1\n  action ask\n    showModal(\"confirm\")\n  state value")
        .replace("            input value=value change=change", "            button popovertarget=\"menu\" testId=\"menu-open\" width=80 height=40\n              text \"Menu\"\n            button press=ask testId=\"ask\" width=80 height=40\n              text \"Ask\"\n            button press=poke testId=\"poke\" width=80 height=40\n              text \"Poke\"\n            input value=value change=change") + r#"      column id="menu" popover="auto" role="menu"
        button press=poke popovertarget="menu" popovertargetaction="hide"
          text "Poke"
      dialog id="confirm" role="alertdialog" aria-label="Confirm"
        text "Continue?"
        button commandfor="confirm" command="close"
          text "Cancel"
"#;
    let mut p = boot_source(1200., &source);
    p.serve_overlays();
    tap(&mut p, "a");
    let list_point = p.box_of(id(&p, "b")).unwrap().center();
    let pane_point = p.box_of(id(&p, "poke")).unwrap().center();
    let anchor = p.box_of(id(&p, "menu-open")).unwrap().rect;
    let click = |p: &mut Presenter<()>, (x, y)| {
        p.pointer_down(x, y, p.host.now() + 1.).unwrap();
        p.pointer_up(x, y, p.host.now() + 1.).unwrap();
    };
    for point in [list_point, pane_point] {
        tap(&mut p, "menu-open");
        let menu = p.surface().unwrap();
        assert_eq!(menu.kind, top::SurfaceKind::Menu);
        p.frame();
        let frame = p.brush.top_surface.as_ref().unwrap().rect;
        assert!(frame.0 >= anchor.0, "{frame:?}");
        assert!(frame.1 >= anchor.1 + anchor.3, "{frame:?}");
        click(&mut p, point);
        assert!(p.surface().is_none());
        assert_eq!(split(&p).row, Some(id(&p, "a")));
        assert_eq!(p.host.runner().slot("presses"), Some(&Value::Number(0.)));
    }
    tap(&mut p, "ask");
    p.run_commands(|| ());
    assert_eq!(p.surface().unwrap().kind, top::SurfaceKind::Alert);
    for point in [list_point, pane_point] {
        click(&mut p, point);
    }
    assert!(p.surface().is_some());
    assert_eq!(split(&p).row, Some(id(&p, "a")));
    assert_eq!(p.host.runner().slot("presses"), Some(&Value::Number(0.)));
    assert!(p.system_back(p.host.now() + 1.));
    assert!(p.surface().is_none());
    tap(&mut p, "poke");
    assert_eq!(p.host.runner().slot("presses"), Some(&Value::Number(1.)));
    tap(&mut p, "b");
    assert_eq!(split(&p).row, Some(id(&p, "b")));
}

#[test]
fn split_list_padding_stays_inside_the_column() {
    let source = APP.replace("SPLIT", "auto").replace(
        "testId=`route-${e.name}`",
        "testId=`route-${e.name}` padding=16 border-width=2 border-style=\"solid\"",
    );
    let mut p = boot_source(1200., &source);
    let home = id(&p, "route-home");
    for _ in 0..3 {
        p.sync_split();
        assert_eq!(cover(&p, home), Some(HostCover::Edges([0., 880., 0., 0.])));
        let rows = p.host.kernel().node(id(&p, "rows")).unwrap().frame;
        assert_eq!((rows.x, rows.x + rows.width), (18., 302.));
    }
    assert!(!state(&mut p).is_null());
}

#[test]
fn split_base_outlives_its_row() {
    let source = APP
        .replace("SPLIT", "auto")
        .replace(
            "  state value",
            "  state showB = true\n  action hideB\n    showB = false\n  state value",
        )
        .replace(
            "              button press=open(\"b\") height=48 testId=\"b\"\n                text \"Beta\"",
            "              if showB\n                button press=open(\"b\") height=48 testId=\"b\"\n                  text \"Beta\"",
        );
    let mut p = boot_source(1200., &source);
    tap(&mut p, "a");
    tap(&mut p, "deeper-item");
    tap(&mut p, "b");
    let b = split(&p).detail.unwrap();
    assert_eq!(split(&p).base, Some(b));
    assert_eq!(split(&p).row, Some(id(&p, "b")));
    act(&mut p, "hideB", vec![]);
    assert_eq!(split(&p).detail, Some(b));
    assert_eq!(
        split(&p).base,
        Some(b),
        "B stays the pane's base without its row"
    );
    assert_eq!(split(&p).row, None);
    let k = p.host.kernel();
    assert!(p
        .host
        .navigation
        .split
        .covers
        .values()
        .any(|(id, cover)| { *cover == HostCover::Whole && within(k, *id, b) }));
}
