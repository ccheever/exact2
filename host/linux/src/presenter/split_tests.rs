use super::*;

const APP: &str = r#"routes nav
  home "/"
    item "/item/:id"
      deeper "/item/:id/deeper"
    sheet "/sheet"
component App
  state value = ""
  action open(id: string)
    nav = top(nav).name == "item" ? replace(nav, path("item", id)) : push(nav, path("item", id))
  action deeper
    nav = push(nav, "/item/a/deeper")
  action sheet
    nav = push(nav, "/sheet")
  action back
    nav = back(nav)
  action follow(url: string)
    nav = push(nav, url)
  action change(value_: string)
    value = value_
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationPresentation=(e.name == "sheet" ? "modal" : "push") navigationSplit=(e.name == "home" ? "SPLIT" : "auto") position="absolute" inset=0 testId=`route-${e.name}`
          if e.name == "home"
            list testId="rows"
              link href=path("item", "linked") height=48 testId="linked"
                text "Linked"
                image "symbol:forward-chevron" width=16 height=16 testId="chevron"
              button press=open("a") height=48 testId="a"
                text "Alpha"
              button press=open("b") height=48 testId="b"
                text "Beta"
            box navigationSplit="placeholder" testId="empty"
              text "Choose an item"
          if e.name == "item" || e.name == "deeper"
            header
              button id="back" press=back testId=`back-${e.name}`
                text "Back"
              h1 "Item"
            button press=deeper height=48 testId=`deeper-${e.name}`
              text "More"
            button press=sheet height=48 testId=`sheet-${e.name}`
              text "Sheet"
            input value=value change=change testId=`field-${e.name}`
          if e.name == "sheet"
            button press=back testId="close"
              text "Close"
"#;

fn boot(width: f32, split: &str) -> Presenter<()> {
    boot_source(width, &APP.replace("SPLIT", split))
}
fn boot_source(width: f32, source: &str) -> Presenter<()> {
    let (p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (width, 800.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}
fn id<D: DataSource>(p: &Presenter<D>, name: &str) -> ViewId {
    let k = p.host.kernel();
    k.node_by_key(
        *k.find_by_test_id(name)
            .first()
            .unwrap_or_else(|| panic!("missing {name}")),
    )
    .unwrap()
    .id
}
fn tap<D: DataSource>(p: &mut Presenter<D>, name: &str) {
    let target = id(p, name);
    let b = p
        .box_of(target)
        .unwrap_or_else(|| panic!("unpainted {name}"));
    let (x, y) = b.center();
    let hit = p
        .hit(x, y)
        .unwrap_or_else(|| panic!("cannot hit {name} at {x},{y}"));
    assert!(
        within(p.host.kernel(), hit, target),
        "hit {hit}, wanted {target}"
    );
    let now = p.host.now() + 1.;
    p.pointer_down(x, y, now).unwrap();
    p.pointer_up(x, y, now + 1.).unwrap();
}
fn state(p: &mut Presenter<()>) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(&crate::agent::handle(p, r#"{"op":"state"}"#))
        .unwrap()["split"]
        .clone()
}
fn split<D: DataSource>(p: &Presenter<D>) -> &Split {
    p.host.navigation.split.active.as_ref().unwrap()
}
fn cover(p: &Presenter<()>, view: ViewId) -> Option<HostCover> {
    let k = p.host.kernel();
    k.arena().slot_of(view).and_then(|s| k.arena().cover(s))
}
fn back_key(p: &mut Presenter<()>) {
    p.held = 1 << 4;
    assert!(p.back_key("ArrowLeft"));
    p.back_key_down("ArrowLeft", "ArrowLeft", false, p.host.now() + 1.);
    p.held = 0;
}

#[test]
fn root_column_and_authored_placeholder() {
    let mut p = boot(1200., "auto");
    let home = id(&p, "route-home");
    assert_eq!(p.host.kernel().host_block(home), None);
    let rows = id(&p, "rows");
    assert_eq!(p.host.kernel().node(rows).unwrap().frame.width, 320.);
    let empty = id(&p, "empty");
    assert_eq!(p.box_of(empty).unwrap().rect, (321., 0., 879., 800.));
    assert_eq!(state(&mut p)["empty"], "placeholder");
    assert_eq!(state(&mut p)["list"], home);
    let chevron = id(&p, "chevron");
    assert!(p.box_of(chevron).is_none());
}

#[test]
fn action_rows_replace_the_detail_and_stay_live() {
    let mut p = boot(1200., "auto");
    tap(&mut p, "a");
    let a = id(&p, "a");
    let item = id(&p, "route-item");
    assert_eq!(split(&p).row, Some(a));
    assert_eq!(split(&p).base, Some(item));
    assert_eq!(p.host.route_visibility(a), (false, false));
    assert!(!p.host.runner().inactive(a));
    assert_eq!(p.box_of(item).unwrap().rect, (321., 0., 879., 800.));
    let back = id(&p, "back-item");
    assert_eq!(cover(&p, back), Some(HostCover::Whole));
    assert!(p.box_of(back).is_none());
    let b = id(&p, "b");
    tap(&mut p, "b");
    assert_eq!(split(&p).row, Some(b));
    assert_eq!(id(&p, "route-item"), item, "replace retains the visit");
    assert_eq!(state(&mut p)["row"], b);
    assert_eq!(state(&mut p)["empty"], serde_json::Value::Null);
    let empty = id(&p, "empty");
    assert_eq!(p.host.kernel().host_block(empty), None);
    assert!(p.box_of(empty).is_none());
}

#[test]
fn link_location_deeper_back_and_return_to_placeholder() {
    let mut p = boot(1200., "auto");
    tap(&mut p, "linked");
    assert_eq!(split(&p).row, Some(id(&p, "linked")));
    let base = id(&p, "route-item");
    tap(&mut p, "deeper-item");
    assert_eq!(split(&p).base, Some(base));
    assert_eq!(split(&p).routes.len(), 2);
    let back = id(&p, "back-deeper");
    assert!(p.box_of(back).is_some());
    back_key(&mut p);
    assert_eq!(split(&p).detail, Some(base));
    assert!(p.system_back(p.host.now() + 1.));
    assert_eq!(state(&mut p)["empty"], "placeholder");
}

#[cfg(not(windows))]
#[test]
fn resize_releases_blocks_covers_lifts_and_placements() {
    let mut p = boot(800., "auto");
    assert!(state(&mut p).is_null());
    tap(&mut p, "a");
    let home = id(&p, "route-home");
    let item = id(&p, "route-item");
    let back = id(&p, "back-item");
    assert_eq!(p.host.route_visibility(home), (true, true));
    assert_eq!(p.host.kernel().host_block(item), None);
    assert_eq!(cover(&p, home), None);
    assert_eq!(p.resize(1200., 800.), None);
    assert_eq!(split(&p).row, Some(id(&p, "a")));
    assert_eq!(p.box_of(item).unwrap().rect.0, 321.);
    assert_eq!(p.resize(800., 800.), None);
    assert!(state(&mut p).is_null());
    assert_eq!(p.host.kernel().host_block(item), None);
    assert_eq!(cover(&p, home), None);
    assert_eq!(cover(&p, back), None);
    assert!(!p.host.navigation.lifted.contains(&back));
    assert_eq!(p.box_of(item).unwrap().rect.0, 0.);
    assert!(p.box_of(back).is_some());
    assert!(p.host.runner().inactive(id(&p, "a")));
    assert!(p.system_back(p.host.now() + 1.));
    assert_eq!(p.resize(1200., 800.), None);
    assert_eq!(state(&mut p)["empty"], "placeholder");
}

#[test]
fn a_list_chevron_takes_no_room_beside_the_detail() {
    let source = APP.replace("SPLIT", "auto").replace(
        "text \"Linked\"",
        "text \"Linked\" flex=1 testId=\"linked-name\"",
    );
    let mut p = boot_source(1200., &source);
    let (row, chevron) = (id(&p, "linked"), id(&p, "chevron"));
    let end = |p: &Presenter<()>, id: ViewId| {
        let f = p.host.kernel().node(id).unwrap().frame;
        f.x + f.width
    };
    assert_eq!(cover(&p, chevron), Some(HostCover::Whole));
    let k = p.host.kernel();
    let node = k.node(row).unwrap();
    let (_, _, right, _) = k.layout_padding(node.key).unwrap();
    assert_eq!(end(&p, id(&p, "linked-name")), node.frame.width - right);
    assert_eq!(p.resize(600., 800.), None);
    assert_eq!(cover(&p, chevron), None);
    assert_eq!(p.host.kernel().node(chevron).unwrap().frame.width, 16.);
}

#[test]
fn author_opt_out_width_and_minimum_detail() {
    let mut p = boot(1200., "none");
    tap(&mut p, "a");
    assert!(state(&mut p).is_null());
    assert_eq!(p.host.kernel().host_block(id(&p, "route-item")), None);
    let mut p = boot(1200., "400");
    assert_eq!(state(&mut p)["listWidth"], 400.);
    assert_eq!(state(&mut p)["authoredWidth"], 400.);
    let empty = id(&p, "empty");
    assert_eq!(p.box_of(empty).unwrap().rect.0, 401.);
    let mut p = boot(1200., "1000");
    assert_eq!(state(&mut p)["listWidth"], 879.);
    assert_eq!(state(&mut p)["detailWidth"], 320.);
}

#[test]
fn a_detail_no_row_pushed_marks_the_row_the_app_says_is_current() {
    let source = APP
        .replace("SPLIT", "auto")
        .replace(
            "button press=open(\"b\") height=48 testId=\"b\"",
            // A hidden copy of the row before it is no row of the list.
            "button press=open(\"b\") display=\"none\" aria-current=\"page\"\n                text \"Beta\"\n              button press=open(\"b\") height=48 testId=\"b\" aria-current=(top(nav).name == \"item\" ? \"page\" : \"false\")",
        )
        .replace(
            "            box navigationSplit=\"placeholder\"",
            "            button press=open(\"b\") testId=\"elsewhere\"\n              text \"Open Beta\"\n            box navigationSplit=\"placeholder\"",
        );
    let mut p = boot_source(1200., &source);
    assert_eq!(split(&p).row, None);
    tap(&mut p, "elsewhere");
    assert_eq!(split(&p).row, Some(id(&p, "b")));
    // A row's press names its detail's row, whatever the app says is current.
    tap(&mut p, "a");
    assert_eq!(split(&p).row, Some(id(&p, "a")));
}

#[test]
fn modal_keeps_list_visible_inert_and_uses_window() {
    let mut p = boot(1200., "auto");
    tap(&mut p, "a");
    let detail = split(&p).detail;
    tap(&mut p, "sheet-item");
    assert_eq!(split(&p).detail, detail);
    assert_eq!(p.host.route_visibility(id(&p, "a")), (false, true));
    assert!(p.host.runner().inactive(id(&p, "a")));
    let sheet = id(&p, "route-sheet");
    assert_eq!(p.host.kernel().host_block(sheet), None);
    assert_eq!(p.box_of(sheet).unwrap().rect, (0., 0., 1200., 800.));
    tap(&mut p, "close");
    assert_eq!(p.host.route_visibility(id(&p, "a")), (false, false));
}

/// Runs the calling test again in a child process that shows the neutral
/// look, which is the whole process's: true in that child.
fn in_neutral_child() -> bool {
    const CHILD: &str = "EXACT_SPLIT_NEUTRAL_TEST";
    if std::env::var_os(CHILD).is_some() {
        crate::app::show_neutral_look();
        return true;
    }
    let name = std::thread::current().name().unwrap().to_owned();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &name, "--nocapture"])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    false
}

#[test]
fn selected_fill_and_authored_background_win_over_default_paint() {
    if !in_neutral_child() {
        return;
    }
    for name in ["linked", "a"] {
        for authored in [false, true] {
            let source = APP
                .replace("SPLIT", "auto")
                .replace(
                    "link href=",
                    if authored {
                        "link background-color=\"#dd2244\" href="
                    } else {
                        "link href="
                    },
                )
                .replace(
                    "button press=open(\"a\")",
                    if authored {
                        "button background-color=\"#dd2244\" press=open(\"a\")"
                    } else {
                        "button press=open(\"a\")"
                    },
                );
            let mut p = boot_source(1200., &source);
            let row = id(&p, name);
            let rect = p.box_of(row).unwrap().rect;
            let pixel = |p: &mut Presenter<()>| {
                let frame = p.frame();
                let at = (((rect.1 + rect.3 / 2.) as usize) * frame.width() as usize
                    + (rect.0 + rect.2 - 8.) as usize)
                    * 4;
                frame.data()[at..at + 4].to_vec()
            };
            let before = pixel(&mut p);
            tap(&mut p, name);
            p.pointer_move(1190., 790., p.host.now() + 1.).unwrap();
            let after = pixel(&mut p);
            if authored {
                assert_eq!(before, after);
                assert_eq!(&after[..3], &[221, 34, 68]);
            } else {
                assert_ne!(before, after, "the selected row's fill is in the picture");
            }
        }
    }
}

#[test]
fn native_button_face_omits_the_list_chevron() {
    let source = APP.replace("SPLIT", "auto").replace(
        "text \"Alpha\"",
        "text \"Alpha\"\n                image \"symbol:sf/chevron.right\" -exact-tint-color=\"#ff0000\" testId=\"native-chevron\"",
    );
    let mut p = boot_source(1200., &source);
    let row = id(&p, "a");
    assert_eq!(
        p.host.kernel().node(row).unwrap().node_type,
        NodeType::Control
    );
    let chevron = id(&p, "native-chevron");
    assert!(split(&p).chevrons.contains(&chevron));
    assert!(p.host.kernel().press_face(row).unwrap().symbol.is_some());
    let (x, y, w, h) = p.box_of(row).unwrap().rect;
    let frame = p.frame();
    let mut red = 0;
    for y in (y as u32 + 4)..((y + h) as u32 - 4) {
        for x in (x as u32 + 4)..((x + w) as u32 - 4) {
            let pixel = frame.pixel(x, y).unwrap();
            red += usize::from(pixel.red() > 160 && pixel.green() < 100 && pixel.blue() < 100);
        }
    }
    assert_eq!(
        red, 0,
        "the native face must omit the red chevron while the image stays suppressed"
    );
}

#[test]
fn pane_perspective_uses_the_navigation_roots_origin() {
    let source = APP
        .replace("SPLIT", "auto")
        .replace("main navigationKey", "main perspective=500 navigationKey")
        .replace(
            "column navigationKey",
            "column translate=(e.name == \"item\" ? \"0px 0px 20px\" : \"0px\") navigationKey",
        );
    let mut p = boot_source(1200., &source);
    tap(&mut p, "a");
    let b = p.box_of(id(&p, "route-item")).unwrap();
    // At z = 20 under perspective 500, scale is 500 / 480 about x = 600.
    assert!((b.rect.0 - 309.375).abs() < 0.01, "{:?}", b.rect);
    assert!((b.rect.2 - 915.625).abs() < 0.01, "{:?}", b.rect);
    tap(&mut p, "field-item");
    assert_eq!(p.focus(), Some(id(&p, "field-item")));
}

#[test]
fn list_scrollbar_uses_column_edge_and_detail_scroll_stays_in_pane() {
    let source = APP
        .replace("SPLIT", "auto")
        .replace(
            "position=\"absolute\" inset=0",
            "position=\"absolute\" inset=0 overflow-y=\"auto\"",
        )
        .replace("list testId=", "list min-height=1200 flex-shrink=0 testId=")
        .replace(
            "h1 \"Item\"",
            "h1 \"Item\"\n            box height=1200 flex-shrink=0 testId=\"tall\"",
        );
    let mut p = boot_source(1200., &source);
    p.set_scrollbars(false);
    let home = id(&p, "route-home");
    p.wheel_at(600., 100., 0., 100.);
    assert_eq!(
        p.scroll_of(home).1,
        0.,
        "the placeholder is outside the list's scrollport"
    );
    tap(&mut p, "a");
    p.frame();
    let item = id(&p, "route-item");
    let bar = p
        .brush
        .scrollbars
        .bars
        .iter()
        .find(|b| b.id == home && b.vertical)
        .unwrap();
    assert_eq!(bar.track.0 + bar.track.2, 320.);
    let bar = p
        .brush
        .scrollbars
        .bars
        .iter()
        .find(|b| b.id == item && b.vertical)
        .unwrap();
    assert_eq!(bar.track.0 + bar.track.2, 1200.);
    p.wheel_at(600., 100., 0., 100.);
    assert!(p.scroll_of(item).1 > 0.);
    assert_eq!(p.scroll_of(home).1, 0.);
    p.wheel_at(200., 100., 0., 100.);
    assert!(p.scroll_of(home).1 > 0.);
}

#[test]
fn runner_geometry_agent_tree_and_fields_use_the_placed_block() {
    let source = APP.replace("SPLIT", "auto")
        .replace("  state value", "  state bounds = []\n  state found = \"\"\n  action probe(x: number, y: number)\n    bounds = [frame(\"item\").x, frame(\"item\").y, frame(\"item\").width, frame(\"item\").height]\n    found = match elementFromPoint(x, y) { case some(id) => id, case none => \"none\" }\n  state value")
        .replace("column navigationKey", "column id=e.name navigationKey")
        .replace("input value=", "input id=\"editor\" value=");
    let mut p = boot_source(1200., &source);
    tap(&mut p, "a");
    let field = id(&p, "field-item");
    let b = p.box_of(field).unwrap();
    let (x, y) = b.center();
    p.host
        .runner_mut()
        .act(
            "probe",
            vec![
                exact_runner::Value::Number(x as f64),
                exact_runner::Value::Number(y as f64),
            ],
        )
        .unwrap();
    assert_eq!(
        p.host.runner().slot("bounds"),
        Some(&exact_runner::Value::list(
            vec![321., 0., 879., 800.]
                .into_iter()
                .map(exact_runner::Value::Number)
                .collect()
        ))
    );
    assert_eq!(
        p.host.runner().slot("found"),
        Some(&exact_runner::Value::str("editor"))
    );
    let tree: serde_json::Value =
        serde_json::from_str(&crate::agent::handle(&mut p, r#"{"op":"tree"}"#)).unwrap();
    let nodes = tree["nodes"].as_array().unwrap();
    assert!(nodes.iter().any(|n| n["id"] == id(&p, "a")));
    assert!(nodes.iter().any(|n| n["id"] == field));
    let inspected: serde_json::Value = serde_json::from_str(&crate::agent::handle(
        &mut p,
        &format!(r#"{{"op":"node","id":{field}}}"#),
    ))
    .unwrap();
    assert_eq!(inspected["absolute"]["x"], 321.);
    let typed = crate::agent::handle(
        &mut p,
        &format!(r#"{{"op":"type","id":{field},"text":"hello"}}"#),
    );
    assert!(!typed.contains("\"error\""), "{typed}");
    assert_eq!(p.focus(), Some(field));
    assert_eq!(p.field_text(field), "hello");
    assert!(p.box_of(field).unwrap().rect.0 >= 321.);
}

#[test]
fn placeholder_restyles_again_and_split_survives_desktop_overlays() {
    let mut p = boot(1200., "auto");
    p.serve_overlays();
    assert_eq!(p.after_commit(), None);
    assert_eq!(state(&mut p)["empty"], "placeholder");
    let empty = id(&p, "empty");
    let patch = exact_kernel::StyleProps {
        display: exact_kernel::Display::None,
        mask: exact_kernel::StyleMask::of(exact_kernel::StyleId::Display),
        ..Default::default()
    };
    p.host
        .runner_mut()
        .kernel_mut()
        .apply(
            0,
            0,
            &[exact_kernel::Op::SetStyle {
                id: empty,
                patch: Box::new(patch),
            }],
        )
        .unwrap();
    assert_eq!(p.after_commit(), None);
    assert_eq!(
        p.host.kernel().node(empty).unwrap().style.display,
        exact_kernel::Display::Flex
    );
    assert_eq!(p.box_of(empty).unwrap().rect.0, 321.);
}

#[test]
fn the_columns_are_divided_in_the_separators_colour() {
    // The role table's own column here, the neutral one Windows and the
    // Linux desktop paint in the child.
    in_neutral_child();
    let mut p = boot(1200., "auto");
    let frame = p.frame();
    let x = (split(&p).pane().0 - DIVIDER) as u32;
    let line = crate::paint::look::role("-exact-separator", false);
    let fill = crate::paint::page_color(false);
    let ink: [u8; 3] = std::array::from_fn(|i| {
        ((u32::from(line[i]) * u32::from(line[3])
            + u32::from(fill[i]) * (255 - u32::from(line[3]))
            + 127)
            / 255) as u8
    });
    let pixel = frame.pixel(x, 400).unwrap();
    let rgb = [pixel.red(), pixel.green(), pixel.blue()];
    assert!(
        rgb.iter().zip(ink).all(|(a, b)| a.abs_diff(b) <= 2),
        "{rgb:?} is not the separator {ink:?}"
    );
}

#[test]
fn empty_pane_without_placeholder_has_platform_default() {
    let source = APP.replace("SPLIT", "auto").replace("            box navigationSplit=\"placeholder\" testId=\"empty\"\n              text \"Choose an item\"\n", "");
    let mut p = boot_source(1200., &source);
    assert_eq!(
        state(&mut p)["empty"],
        if cfg!(windows) {
            serde_json::Value::Null
        } else {
            serde_json::json!("No Selection")
        }
    );
    let frame = p.frame();
    assert!(split(&p).placeholder.is_none());
    let fill = crate::paint::page_color(false);
    let secondary = crate::paint::look::role("-exact-secondary-label", false);
    let mut text_pixels = 0;
    for y in 370..430 {
        for x in 600..920 {
            let pixel = frame.pixel(x, y).unwrap();
            let rgb = [pixel.red(), pixel.green(), pixel.blue()];
            if cfg!(windows) {
                assert_eq!(rgb, fill[..3]);
            } else {
                let ink: [u8; 3] = std::array::from_fn(|i| {
                    ((u32::from(secondary[i]) * u32::from(secondary[3])
                        + u32::from(fill[i]) * (255 - u32::from(secondary[3]))
                        + 127)
                        / 255) as u8
                });
                if rgb.iter().zip(ink).all(|(a, b)| a.abs_diff(b) <= 2) {
                    text_pixels += 1;
                }
            }
        }
    }
    if !cfg!(windows) {
        assert!(
            text_pixels > 20,
            "No Selection must be painted in secondary-label ink"
        );
    }
}

#[test]
fn selected_tab_splits_inside_its_panel_and_releases_previous_tab() {
    let source = r#"routes nav
  tab home "/"
    item "/item"
  tab other "/other"
component App
  action tab(name: string)
    nav = select(nav, name)
  action open
    nav = push(nav, "/item")
  action follow(url: string)
    nav = push(nav, url)
  action back
    nav = back(nav)
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow width="100%" height="100%" display="flex" flex-direction="column"
      column flex=1 min-height=0 position="relative"
        for t in nav.tabs key=t.name
          column role="tabpanel" id=`panel-${t.name}` position="absolute" inset=0
            for e in t.stack key=e.id
              column navigationKey=`${e.id}` position="absolute" inset=0 testId=`route-${e.name}`
                if e.name == "home"
                  list
                    link href="/item" height=48 testId="linked"
                      text "Item"
                  box navigationSplit="placeholder" testId="empty"
                    text "Choose"
                if e.name == "item"
                  button id="back" press=back testId="back-item"
                    text "Back"
                  box width="50vw" height=20 testId="viewport-units"
                if e.name == "other"
                  text "Other"
      row role="tablist" height=60 flex-shrink=0 testId="tabs"
        for t in nav.tabs key=t.name
          button role="tab" aria-controls=`panel-${t.name}` aria-selected=(nav.tab == t.name) press=tab(t.name) testId=`tab-${t.name}`
            text t.name
"#;
    let source = source.replace("component App\n", "component App\n  state hasRows = true\n  action empty\n    hasRows = false\n")
        .replace("                    link href=\"/item\" height=48 testId=\"linked\"\n                      text \"Item\"", "                    if hasRows\n                      link href=\"/item\" height=48 testId=\"linked\"\n                        text \"Item\"");
    for row in ["link href=\"/item\"", "button press=open"] {
        let source = source.replace("link href=\"/item\"", row);
        let mut p = boot_source(1200., &source);
        let home = id(&p, "route-home");
        for _ in 0..4 {
            assert_eq!(p.after_commit(), None);
            assert_eq!(
                p.host.kernel().node(home).unwrap().frame.width,
                1200.,
                "cover {:?}",
                cover(&p, home)
            );
        }
        tap(&mut p, "linked");
        let item = id(&p, "route-item");
        assert_eq!(p.box_of(item).unwrap().rect, (321., 0., 879., 740.));
        let tabs = id(&p, "tabs");
        assert_eq!(p.box_of(tabs).unwrap().rect, (0., 740., 1200., 60.));
        let vw = id(&p, "viewport-units");
        assert_eq!(p.box_of(vw).unwrap().rect.2, 600.);
        tap(&mut p, "tab-other");
        assert!(state(&mut p).is_null());
        assert_eq!(p.host.kernel().host_block(item), None);
        assert_eq!(cover(&p, home), None);
        tap(&mut p, "tab-home");
        assert_eq!(split(&p).detail, Some(item));
        assert_eq!(split(&p).row, Some(id(&p, "linked")));
        assert!(p.system_back(p.host.now() + 1.));
        regression::act(&mut p, "empty", vec![]);
        assert_eq!(split(&p).root, home);
        tap(&mut p, "tab-other");
        assert!(state(&mut p).is_null());
        tap(&mut p, "tab-home");
        assert_eq!(split(&p).root, home);
        assert_eq!(p.host.kernel().node(home).unwrap().frame.width, 1200.);
        assert_eq!(cover(&p, home), Some(HostCover::Edges([0., 880., 0., 0.])));
    }
}

#[test]
fn replacing_from_inside_detail_drops_the_action_row_association() {
    let source = APP
        .replace("SPLIT", "auto")
        .replace("button press=deeper", "button press=open(\"elsewhere\")");
    let mut p = boot_source(1200., &source);
    tap(&mut p, "a");
    assert_eq!(split(&p).row, Some(id(&p, "a")));
    tap(&mut p, "deeper-item");
    assert_eq!(split(&p).row, None);
    assert_eq!(split(&p).base, split(&p).detail);
}

#[test]
fn a_shell_bar_room_takes_back_the_split() {
    let mut p = boot(1200., "auto");
    tap(&mut p, "a");
    let item = id(&p, "route-item");
    p.host.set_bar_room(Some((60., 0.))).unwrap();
    assert_eq!(p.after_commit(), None);
    assert!(state(&mut p).is_null());
    assert_eq!(p.host.kernel().host_block(item), None);
    assert!(p.host.runner().inactive(id(&p, "a")));
}

#[test]
fn split_cover_and_content_stay_stable_across_sync_and_resize() {
    let mut p = boot(1200., "auto");
    let home = id(&p, "route-home");
    for width in [1200., 1400., 1000.] {
        assert_eq!(p.resize(width, 800.), None);
        for _ in 0..4 {
            let stamp = p.host.kernel().layout_stamp();
            p.sync_split();
            assert_eq!(
                p.host.kernel().layout_stamp(),
                stamp,
                "an unchanged split must not lay out again"
            );
            let node = p.host.kernel().node(home).unwrap();
            assert_eq!(node.frame.width, width, "cover {:?}", cover(&p, home));
            assert_eq!(
                cover(&p, home),
                Some(HostCover::Edges([0., width - 320., 0., 0.]))
            );
            assert_eq!(
                p.host.kernel().node(id(&p, "rows")).unwrap().frame.width,
                320.
            );
        }
    }
}

#[path = "split_regression_tests.rs"]
mod regression;
