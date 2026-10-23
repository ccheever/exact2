use super::*;

fn source(page: bool) -> String {
    let mut source =
        String::from("component App\n  state header = 48\n  action fold\n    header = 0\n  view\n");
    source.push_str(if page {
        "    column width=400\n"
    } else {
        "    scroll testId=\"port\" width=400 height=400\n"
    });
    source.push_str("      box height=header\n");
    for row in 0..50 {
        source.push_str(&format!(
            "      box testId=\"row{row}\" width=400 height=20 press=fold\n"
        ));
    }
    source
}

fn boot(page: bool) -> Presenter<()> {
    boot_source(&source(page))
}

fn boot_source(source: &str) -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 400.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert_eq!(error, None);
    p.frame();
    p
}

fn fold(p: &mut Presenter<()>) {
    let row = named(p, "row20");
    assert_eq!(p.host.dispatch_at(row, Event::Press, p.host.now()), None);
    assert_eq!(p.after_commit(), None);
}

fn named(p: &Presenter<()>, name: &str) -> ViewId {
    let kernel = p.host.kernel();
    kernel
        .node_by_key(kernel.find_by_test_id(name)[0])
        .unwrap()
        .id
}

fn row_y(p: &mut Presenter<()>) -> f32 {
    named_y(p, "row20")
}

fn named_y(p: &mut Presenter<()>, name: &str) -> f32 {
    let row = named(p, name);
    p.boxes().iter().find(|b| b.id == row).unwrap().rect.1
}

#[test]
fn folding_header_keeps_row20_still_in_plain_scroll() {
    let mut p = boot(false);
    let port = named(&p, "port");
    assert_eq!(p.scroll_to(port, (0., 400.)), Some((0., 400.)));
    let before = row_y(&mut p);
    assert_eq!(before, 48.);
    p.tap(named(&p, "row20")).unwrap();
    assert_eq!(row_y(&mut p), before);
    assert_eq!(p.scroll_of(port), (0., 352.));
}

#[test]
fn folding_header_keeps_row20_still_in_document() {
    let mut p = boot(true);
    p.scroll_to(PAGE, (0., 400.)).unwrap();
    let before = row_y(&mut p);
    fold(&mut p);
    assert_eq!(row_y(&mut p), before);
    assert_eq!(p.page(), (0., 352.));
}

#[test]
fn anchoring_waits_for_the_picture_and_rebases_the_reader() {
    for page in [false, true] {
        let mut p = boot(page);
        let port = if page { PAGE } else { named(&p, "port") };
        p.scroll_to(port, (0., 400.)).unwrap();
        let a = p.display_frame().unwrap();
        assert!(p.display_complete(&a));
        let before = row_y(&mut p);
        let told = p.reader_under(200., 60., 1.);
        let rev = told[5] as u32;
        fold(&mut p);
        let offset = |p: &Presenter<()>| if page { p.page() } else { p.scroll_of(port) };
        assert_eq!(offset(&p), (0., 400.));
        let b = p.display_frame().unwrap();
        assert_eq!(offset(&p), (0., 400.));
        assert!(p.display_complete(&b));
        assert_eq!(offset(&p), (0., 352.));
        assert_eq!(row_y(&mut p), before);
        let told = p.reader_scrollers(&[], 1.).unwrap();
        assert_eq!(told[2] as u32, rev);
        assert_eq!(p.reader_scrolled(port, (0., 10.), rev), Some((0., 10.)));
        assert_eq!(offset(&p), (0., 362.));
    }
}

#[test]
fn reader_input_during_submission_keeps_the_correction() {
    for page in [false, true] {
        let mut p = boot(page);
        let port = if page { PAGE } else { named(&p, "port") };
        p.scroll_to(port, (0., 400.)).unwrap();
        let a = p.display_frame().unwrap();
        assert!(p.display_complete(&a));
        let rev = p.reader_under(200., 60., 1.)[5] as u32;
        fold(&mut p);
        let b = p.display_frame().unwrap();
        assert_eq!(p.reader_scrolled(port, (0., 10.), rev), Some((0., 10.)));
        assert!(p.display_complete(&b));
        let c = p.display_frame().unwrap();
        assert!(p.display_complete(&c));
        assert_eq!(if page { p.page() } else { p.scroll_of(port) }, (0., 362.));
        assert_eq!(p.reader_scrollers(&[], 1.).unwrap()[2] as u32, rev);
    }
}

#[test]
fn anchoring_sends_scroll_without_completing_a_scroll() {
    for attached in [false, true] {
        let source = source(false)
            .replace("  action fold", "  state moves = 0\n  state ends = 0\n  action moved(x: number, y: number)\n    moves = moves + 1\n  action ended\n    ends = ends + 1\n  action fold")
            .replace("testId=\"port\"", "testId=\"port\" scroll=moved scrollend=ended");
        let mut p = boot_source(&source);
        let port = named(&p, "port");
        p.scroll_to(port, (0., 400.)).unwrap();
        assert_eq!(p.pump(p.host.now()), None);
        if attached {
            let a = p.display_frame().unwrap();
            assert!(p.display_complete(&a));
        }
        let moves = p.host.runner().slot("moves").cloned();
        let ends = p.host.runner().slot("ends").cloned();
        fold(&mut p);
        if attached {
            let b = p.display_frame().unwrap();
            assert!(p.display_complete(&b));
        }
        assert_eq!(p.pump(p.host.now()), None);
        assert_ne!(p.host.runner().slot("moves"), moves.as_ref());
        assert_eq!(p.host.runner().slot("ends"), ends.as_ref());
    }
}

fn assert_fold_anchors(source: &str, top: f32, shift: f32) {
    let mut p = boot_source(source);
    let port = named(&p, "port");
    p.scroll_to(port, (0., top)).unwrap();
    let before = row_y(&mut p);
    fold(&mut p);
    assert_eq!(p.scroll_of(port), (0., top - shift));
    assert_eq!(row_y(&mut p), before);
}

#[test]
fn zero_offset_and_explicit_scroll_requests_win() {
    assert_fold_anchors(&source(false), 400., 48.);
    let mut p = boot(false);
    fold(&mut p);
    assert_eq!(p.scroll_of(named(&p, "port")), (0., 0.));
    let source = source(false)
        .replace("  action fold", "  state top = 0\n  action fold")
        .replace("    header = 0", "    header = 0\n    top = 200")
        .replace("testId=\"port\"", "testId=\"port\" scrollTop=top");
    assert_fold_anchors(&source.replace("\n    top = 200", ""), 400., 48.);
    let mut p = boot_source(&source);
    let port = named(&p, "port");
    p.scroll_to(port, (0., 400.)).unwrap();
    fold(&mut p);
    assert_eq!(p.scroll_of(port), (0., 200.));
}

#[test]
fn anchor_chain_layout_changes_and_out_of_flow_changes_suppress() {
    // Row 17 spans y=-12..8: the partly visible leaf is the anchor.
    for change in [
        "height",
        "padding-top",
        "margin-top",
        "translate",
        "position",
    ] {
        let value = match change {
            "translate" => "\"0 1px\"",
            "position" => "\"absolute\"",
            _ => "21",
        };
        let initial = match change {
            "height" => "20",
            "translate" => "\"0 0\"",
            "position" => "\"static\"",
            _ => "0",
        };
        let source = source(false)
            .replace(
                "  action fold",
                &format!("  state changed = {initial}\n  action fold"),
            )
            .replace(
                "    header = 0",
                &format!("    header = 0\n    changed = {value}"),
            )
            .replace(
                "testId=\"row17\" width=400 height=20",
                &format!(
                    "testId=\"row17\" width=400 {} {change}=changed",
                    if change == "height" { "" } else { "height=20" }
                ),
            );
        assert_fold_anchors(
            &source.replace(&format!("\n    changed = {value}"), ""),
            400.,
            48.,
        );
        let mut p = boot_source(&source);
        let port = named(&p, "port");
        p.scroll_to(port, (0., 400.)).unwrap();
        fold(&mut p);
        assert_eq!(p.scroll_of(port), (0., 400.), "{change}");
    }
}

#[test]
fn consecutive_commits_keep_each_pictures_correction() {
    let source = source(false).replace("header = 0", "header = header - 24");
    let mut p = boot_source(&source);
    let port = named(&p, "port");
    p.scroll_to(port, (0., 400.)).unwrap();
    let a = p.display_frame().unwrap();
    p.display_complete(&a);
    let rev = p.reader_under(200., 60., 1.)[5] as u32;
    fold(&mut p);
    let b = p.display_frame().unwrap();
    fold(&mut p);
    p.display_complete(&b);
    assert_eq!(p.scroll_of(port), (0., 376.));
    assert_eq!(row_y(&mut p), 48.);
    let c = p.display_frame().unwrap();
    p.display_complete(&c);
    assert_eq!(p.scroll_of(port), (0., 352.));
    assert_eq!(row_y(&mut p), 48.);
    assert_eq!(p.reader_scrollers(&[], 1.).unwrap()[2] as u32, rev);
}

#[test]
fn an_environment_relayout_anchors_before_clamping() {
    let source = source(false).replace("height=header", "height=\"env(safe-area-inset-top)\"");
    for top in [400., 648.] {
        let mut p = boot_source(&source);
        assert!(p.host.set_safe_area([48., 0., 0., 0.]).unwrap());
        assert_eq!(p.after_commit(), None);
        let port = named(&p, "port");
        p.scroll_to(port, (0., top)).unwrap();
        let anchor = if top == 648. { "row30" } else { "row17" };
        let before = named_y(&mut p, anchor);
        assert!(p.host.set_safe_area([0.; 4]).unwrap());
        p.dirty = true;
        p.clamp_scroll();
        assert_eq!(p.scroll_of(port), (0., top - 48.));
        assert_eq!(named_y(&mut p, anchor), before);
    }
}

#[cfg(not(windows))]
#[test]
fn a_corrected_authored_request_still_completes_after_its_picture() {
    let source = source(false)
        .replace("  action fold", "  state top = 0\n  state ends = 0\n  action jump\n    top = 500\n  action ended\n    ends = ends + 1\n  action fold")
        .replace("testId=\"port\"", "testId=\"port\" scrollTop=top scrollend=ended")
        .replace("testId=\"row21\" width=400 height=20 press=fold", "testId=\"row21\" width=400 height=20 press=jump");
    let mut p = boot_source(&source);
    let port = named(&p, "port");
    p.scroll_to(port, (0., 400.)).unwrap();
    assert_eq!(p.pump(p.host.now()), None);
    let a = p.display_frame().unwrap();
    assert!(p.display_complete(&a));
    assert_eq!(
        p.host.runner().slot("ends"),
        Some(&exact_runner::Value::Number(1.))
    );
    let jump = named(&p, "row21");
    assert_eq!(p.host.dispatch_at(jump, Event::Press, p.host.now()), None);
    assert_eq!(p.after_commit(), None);
    fold(&mut p);
    let b = p.display_frame().unwrap();
    assert!(p.display_complete(&b));
    assert_eq!(p.pump(p.host.now()), None);
    assert_eq!(p.scroll_of(port), (0., 452.));
    assert_eq!(
        p.host.runner().slot("ends"),
        Some(&exact_runner::Value::Number(2.))
    );
}

#[test]
fn an_out_of_flow_change_elsewhere_suppresses() {
    let source = source(false)
        .replace(
            "  action fold",
            "  state position = \"static\"\n  action fold",
        )
        .replace(
            "    header = 0",
            "    header = 0\n    position = \"absolute\"",
        )
        .replace("testId=\"row0\"", "testId=\"row0\" position=position");
    assert_fold_anchors(
        &source.replace("\n    position = \"absolute\"", ""),
        400.,
        48.,
    );
    let mut p = boot_source(&source);
    let port = named(&p, "port");
    p.scroll_to(port, (0., 400.)).unwrap();
    fold(&mut p);
    assert_eq!(p.scroll_of(port), (0., 400.));
}

#[test]
fn selection_descends_into_partly_visible_wrappers_and_clamps_at_the_end() {
    let source = source(false)
        .replace("      box testId=", "        box testId=")
        .replace(
            "      box height=header\n",
            "      box height=header\n      box width=400\n",
        );
    let mut p = boot_source(&source);
    let port = named(&p, "port");
    p.scroll_to(port, (0., 400.)).unwrap();
    fold(&mut p);
    assert_eq!(p.scroll_of(port), (0., 352.));
    assert_eq!(row_y(&mut p), 48.);
    let end = self::source(false)
        .replace("  action fold", "  state bottom = 20\n  action fold")
        .replace("    header = 0", "    header = 0\n    bottom = 0")
        .replace(
            "testId=\"row49\" width=400 height=20",
            "testId=\"row49\" width=400 height=bottom",
        );
    assert_fold_anchors(&end, 610., 48.);
    let mut p = boot_source(&end);
    let port = named(&p, "port");
    p.scroll_to(port, (0., 648.)).unwrap();
    assert_eq!(named_y(&mut p, "row30"), 0.);
    fold(&mut p);
    assert_eq!(p.scroll_of(port), (0., 580.));
    assert_eq!(named_y(&mut p, "row30"), 20.);
}

#[test]
fn excluded_boxes_are_not_selected_and_removing_the_anchor_does_not_scroll() {
    for style in [
        "display=\"none\"",
        "width=0",
        "position=\"absolute\" top=408",
    ] {
        let source = source(false).replace(
            "      box height=header\n",
            &format!("      box height=header\n      box {style} height=header\n"),
        );
        let mut p = boot_source(&source);
        let port = named(&p, "port");
        p.scroll_to(port, (0., 400.)).unwrap();
        let before = row_y(&mut p);
        fold(&mut p);
        assert_eq!(row_y(&mut p), before, "{style}");
    }
    let source = source(false).replace(
        "      box testId=\"row17\"",
        "      if header > 0\n        box testId=\"row17\"",
    );
    assert_fold_anchors(&source.replace("if header > 0", "if true"), 400., 48.);
    let mut p = boot_source(&source);
    let port = named(&p, "port");
    p.scroll_to(port, (0., 400.)).unwrap();
    fold(&mut p);
    assert_eq!(p.scroll_of(port), (0., 400.));
}

mod review_tests;

/// Stage 14 B: with no shell to cover it, a popover is laid out in the
/// viewport's block, whose space the document doesn't scroll. Its box is
/// no anchor: the row the page was showing keeps its place.
#[test]
fn a_popover_in_its_block_is_no_scroll_anchor() {
    let source = source(true).replace(
        "    column width=400\n",
        "    column width=400\n      box testId=\"pop\" popover=\"manual\" width=400 height=600\n",
    );
    let mut p = boot_source(&source);
    assert!(p.host.kernel().host_block(named(&p, "pop")).is_some());
    p.scroll_to(PAGE, (0., 400.)).unwrap();
    let before = row_y(&mut p);
    fold(&mut p);
    assert_eq!(row_y(&mut p), before);
}
