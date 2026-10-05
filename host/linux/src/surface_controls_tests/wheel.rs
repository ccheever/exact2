use super::*;

fn count(p: &Presenter<NoData>) -> u32 {
    unsafe { p.surfaces.abis[""].symbol::<unsafe extern "C" fn() -> u32>(b"test_input_count")() }
}

#[test]
fn game_wheel_is_one_relative_sample_without_focus_or_contact_changes() {
    let (mut p, path) = fixture();
    let raw = find(&p, "raw");
    let editor = find(&p, "editor");
    let (x, y, _, _) = p.rect_of(raw).unwrap();
    p.focus = Some(editor);
    p.advance(25.);
    p.wheel_at(x + 25., y + 75., -2.5, 17.25);
    assert_eq!(
        last_input(&p),
        (
            3,
            1,
            json!({"t":"wheel","dx":-2.5,"dy":17.25,"x":25.,"y":75.,"at":25.})
        )
    );
    assert_eq!(p.focus, Some(editor));
    assert_eq!(p.page(), (0., 0.));
    // The canvas's own `wheel` is still heard beside the module's input
    // (review b5-b 2), as the web's element hears it.
    assert_eq!(
        p.host().runner().slot("spins"),
        Some(&exact_plan::Value::Number(1.0))
    );
    assert!(p.pointer_down(x + 20., y + 70., 30.).unwrap());
    p.pointer_aux(2, true, x + 20., y + 70., 31.);
    let before = count(&p);
    p.wheel_at(x + 25., y + 75., 0., 40.);
    assert_eq!(count(&p), before + 1);
    assert_eq!(p.contact_canvas(), Some(raw));
    assert_eq!(p.surfaces.aux, 2);
    // Capture does not redirect wheel over the covered HUD to the game, and
    // scrolling there must not cancel the game's primary/secondary contact.
    p.wheel_at(x + 25., y + 15., 0., 40.);
    assert_eq!(count(&p), before + 1);
    assert_eq!(p.contact_canvas(), Some(raw));
    assert_eq!(p.surfaces.aux, 2);
    p.pointer_up(x + 20., y + 70., 35.).unwrap();
    p.pointer_aux(2, false, x + 20., y + 70., 36.);
    assert_eq!(last_input(&p).2["phase"], "up");
    let button = find(&p, "a-jump");
    let a = find(&p, "a");
    assert!(p.control_input(button, "down", 10., 10., 7, 37.));
    p.wheel_at(x + 25., y + 75., 0., 4.);
    assert!(p.control_bindings.contains_key(&(a, 7)));
    assert!(p.control_input(button, "up", 10., 10., 7, 38.));
    done(p, path);
}

#[test]
fn invalid_and_zero_wheel_samples_cannot_activate_or_cancel() {
    let (mut p, path) = fixture();
    let raw = find(&p, "raw");
    let (x, y, _, _) = p.rect_of(raw).unwrap();
    p.pointer_down(x + 20., y + 70., 1.).unwrap();
    let before = count(&p);
    for (x, y, dx, dy) in [
        (x, y, 0., 0.),
        (x, y, f32::NAN, 1.),
        (x, y, 1., f32::INFINITY),
        (f32::NAN, y, 1., 1.),
        (x, f32::INFINITY, 1., 1.),
        (-1., y, 1., 1.),
        (x, 500., 1., 1.),
    ] {
        p.wheel_at(x, y, dx, dy);
        assert_eq!(count(&p), before);
        assert_eq!(p.contact_canvas(), Some(raw));
    }
    for wheel in ["null", "[]", "[1]", "[1,2,3]", r#"["1",2]"#, "[1,1e100]"] {
        let reply: Value = serde_json::from_str(&crate::agent::answer(
            &mut p,
            &format!(r#"{{"op":"tap","id":{raw},"wheel":{wheel}}}"#),
        ))
        .unwrap();
        assert!(
            reply["error"].as_str().unwrap().contains("wheel"),
            "{reply}"
        );
        assert_eq!(count(&p), before);
        assert_eq!(p.contact_canvas(), Some(raw));
    }
    p.pointer_lost(2.).unwrap();
    done(p, path);
}

const SOURCE: &str = r#"component Wheels
  view
    column width=200 height=500
      canvas testId="raw" width=100 height=100
        box testId="hud" width=100 height=30 overflow-y="scroll"
          box width=100 height=200
      canvas testId="other" width=100 height=100
      box width=100 height=300
"#;

fn scroll_fixture(source: &str) -> (Presenter<NoData>, PathBuf) {
    let (path, compat) = super::super::tests::fixture();
    let plan = contract::compile(source).unwrap();
    let (mut p, _) = Presenter::boot(
        &plan.encode(),
        NoData,
        (200., 200.),
        1.,
        path.parent().unwrap().into(),
    )
    .unwrap();
    p.surfaces
        .abis
        .insert(String::new(), Abi::open_path(&path, &compat, "").unwrap());
    for (i, name) in ["raw", "other"].iter().enumerate() {
        let view = find(&p, name);
        p.surfaces.canvases.insert(
            view,
            Canvas {
                id: i as u32 + 1,
                name: (*name).into(),
                artifact: String::new(),
                owner: true,
                since: 0,
                held: Default::default(),
                restored_controls: None,
                restore_error: None,
                restore_input: false,
                restore_bytes: None,
                restore_logged: false,
            },
        );
    }
    p.boxes();
    (p, path)
}

#[test]
fn painted_hit_selects_surface_and_hud_scroll_never_leaks() {
    let (mut p, path) = scroll_fixture(SOURCE);
    let other = find(&p, "other");
    let hud = find(&p, "hud");
    p.focus = Some(other);
    p.wheel_at(25., 75., 0., 12.);
    assert_eq!(last_input(&p).0, 1);
    p.wheel_at(25., 175., 0., 24.);
    assert_eq!(last_input(&p).0, 2);
    assert_eq!(count(&p), 2);
    p.wheel_at(25., 15., 0., 20.);
    assert_eq!(count(&p), 2);
    assert_eq!(p.scroll_of(hud), (0., 20.));
    assert_eq!(p.page(), (0., 0.));
    done(p, path);
}

#[test]
fn no_input_falls_back_but_abi_refusal_consumes_and_reports() {
    let (mut p, path) = scroll_fixture(SOURCE);
    let raw = find(&p, "raw");
    p.surfaces.canvases.get_mut(&raw).unwrap().id = 97;
    p.wheel_at(25., 75., 0., 20.);
    assert_eq!(count(&p), 1);
    assert_eq!(p.page(), (0., 0.));
    assert_eq!(
        p.surfaces.error.as_deref(),
        Some("GPU surface refused wheel input")
    );
    p.surfaces.canvases.get_mut(&raw).unwrap().id = 98;
    p.wheel_at(25., 75., 0., 20.);
    assert_eq!(count(&p), 1);
    assert_eq!(p.page(), (0., 20.));
    done(p, path);
}

#[test]
fn pointer_events_none_simple_overlay_falls_through() {
    let source = SOURCE
        .replace("testId=\"hud\"", "testId=\"hud\" pointer-events=\"none\"")
        .replace("          box width=100 height=200\n", "");
    let (mut p, path) = scroll_fixture(&source);
    p.wheel_at(25., 15., 0., 20.);
    let delivered = count(&p);
    done(p, path);
    assert_eq!(delivered, 1);
}

#[test]
fn pointer_events_none_passes_through_but_hidden_and_inert_canvases_do_not() {
    let source = SOURCE.replace("testId=\"hud\"", "testId=\"hud\" pointer-events=\"none\"");
    let (mut p, path) = scroll_fixture(&source);
    p.wheel_at(25., 15., 0., 20.);
    assert_eq!(count(&p), 1);
    assert_eq!(last_input(&p).0, 1);
    assert_eq!(p.page(), (0., 0.));
    done(p, path);
    for attr in ["display=\"none\"", "inert=true"] {
        let source = SOURCE.replace("testId=\"raw\"", &format!("testId=\"raw\" {attr}"));
        let (mut p, path) = scroll_fixture(&source);
        let raw = find(&p, "raw");
        assert!(p.wheel(raw, 0., 20.).is_err(), "{attr}");
        assert_eq!(count(&p), 0, "{attr}");
        done(p, path);
    }
}

#[test]
fn child_auto_reenables_ordinary_and_svg_hits_below_none_parent() {
    let source = SOURCE
        .replace("testId=\"hud\"", "testId=\"hud\" pointer-events=\"none\"")
        .replace(
            "box width=100 height=200",
            "box testId=\"child\" width=100 height=200 pointer-events=\"auto\"",
        );
    let (mut p, path) = scroll_fixture(&source);
    let child = find(&p, "child");
    let hud = find(&p, "hud");
    assert_eq!(p.hit(25., 15.), Some(child));
    p.wheel_at(25., 15., 0., 20.);
    assert_eq!(count(&p), 0);
    assert_eq!(p.scroll_of(hud), (0., 20.));
    done(p, path);

    let source = SOURCE.replace("box testId=\"hud\" width=100 height=30 overflow-y=\"scroll\"\n          box width=100 height=200", "svg testId=\"drawing\" width=100 height=30 viewBox=\"0 0 100 30\" pointer-events=\"none\"\n          rect testId=\"shape\" width=40 height=30 fill=\"#f00\" pointer-events=\"auto\"");
    let (mut p, path) = scroll_fixture(&source);
    let raw = find(&p, "raw");
    let shape = find(&p, "shape");
    assert_eq!(p.hit(25., 15.), Some(shape));
    assert_eq!(p.hit(75., 15.), Some(raw));
    p.wheel_at(75., 15., 0., 20.);
    assert_eq!(count(&p), 1);
    p.wheel_at(25., 15., 0., 20.);
    assert_eq!(count(&p), 1);
    done(p, path);
}
