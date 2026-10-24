use super::*;
use exact_runner::{DataError, Value};

fn offset(p: &Presenter<()>, port: ViewId) -> (f32, f32) {
    if port == PAGE {
        p.page()
    } else {
        p.scroll_of(port)
    }
}

fn attach(p: &mut Presenter<()>) {
    let frame = p.display_frame().unwrap();
    assert!(p.display_complete(&frame));
}

#[test]
fn a_reader_sample_can_fold_the_header_without_invalidating_the_next_sample() {
    let source = source(false)
        .replace(
            "  action fold",
            "  action moved(x: number, y: number)\n    if y > 400\n      header = 0\n  action fold",
        )
        .replace("testId=\"port\"", "testId=\"port\" scroll=moved");
    for attached in [false, true] {
        let mut p = boot_source(&source);
        let port = named(&p, "port");
        p.scroll_to(port, (0., 400.)).unwrap();
        if attached {
            attach(&mut p);
        }
        let rev = p.reader_under(200., 60., 1.)[5] as u32;
        assert_eq!(p.reader_scrolled(port, (0., 10.), rev), Some((0., 10.)));
        if attached {
            attach(&mut p);
        }
        assert_eq!(p.scroll_of(port), (0., 362.));
        assert_eq!(p.reader_scrolled(port, (0., 10.), rev), Some((0., 10.)));
        assert_eq!(p.scroll_of(port), (0., 372.));
    }
}

#[test]
fn queued_corrections_precede_absolute_seeks() {
    for page in [false, true] {
        for attached in [false, true] {
            let source =
                source(page).replace("height=header", "height=\"env(safe-area-inset-top)\"");
            let mut p = boot_source(&source);
            p.host.set_safe_area([48., 0., 0., 0.]).unwrap();
            assert_eq!(p.after_commit(), None);
            let port = if page { PAGE } else { named(&p, "port") };
            p.scroll_to(port, (0., 400.)).unwrap();
            if attached {
                attach(&mut p);
            }
            p.host.set_safe_area([0.; 4]).unwrap();
            assert_eq!(p.scroll_to(port, (0., 200.)), Some((0., 200.)));
            if attached {
                attach(&mut p);
            }
            assert_eq!(
                offset(&p, port),
                (0., 200.),
                "page={page} attached={attached}"
            );
        }
    }
}

#[test]
fn pending_corrections_do_not_override_absolute_seeks() {
    for page in [false, true] {
        for target in [10., 200., 600.] {
            let mut p = boot(page);
            let port = if page { PAGE } else { named(&p, "port") };
            p.scroll_to(port, (0., 400.)).unwrap();
            attach(&mut p);
            fold(&mut p);
            let stale = p.display_frame().unwrap();
            p.scroll_to(port, (0., target)).unwrap();
            assert!(p.display_complete(&stale));
            attach(&mut p);
            assert_eq!(offset(&p, port), (0., target), "page={page}");
        }
    }
}

#[test]
fn reader_corrections_clamp_to_both_boundaries_before_paint() {
    for page in [false, true] {
        for (header, by, expected) in [(0, -390., 0.), (96, 340., 744.)] {
            let mut source = source(page).replace("header = 0", &format!("header = {header}"));
            source.push_str("      box height=144-header\n");
            let mut p = boot_source(&source);
            let port = if page { PAGE } else { named(&p, "port") };
            p.scroll_to(port, (0., 400.)).unwrap();
            attach(&mut p);
            let rev = p.reader_under(200., 60., 1.)[5] as u32;
            fold(&mut p);
            let stale = p.display_frame().unwrap();
            p.reader_scrolled(port, (0., by), rev).unwrap();
            assert!(p.display_complete(&stale));
            let next = p.display_frame().unwrap();
            let model = if page {
                p.model_page()
            } else {
                p.collection_paint_scroll()
                    .as_ref()
                    .unwrap_or(&p.scroll)
                    .get(&port)
                    .copied()
                    .unwrap()
            };
            assert_eq!(model.1, expected, "page={page} header={header}");
            assert!(p.display_complete(&next));
            assert_eq!(offset(&p, port).1, expected);
            assert_eq!(p.reader_scrollers(&[], 1.).unwrap()[2] as u32, rev);
        }
    }
}

#[test]
fn scaled_content_anchors_in_scrollport_coordinates() {
    for page in [false, true] {
        let source = source(page).replace("      box", "        box").replace(
            "        box height=header",
            "      box scale=2 transform-origin=\"0 0\"\n        box height=header",
        );
        let mut p = boot_source(&source);
        let port = if page { PAGE } else { named(&p, "port") };
        p.scroll_to(port, (0., 400.)).unwrap();
        let row = named(&p, "row10");
        let before = p.boxes().iter().find(|b| b.id == row).unwrap().rect.1;
        assert_eq!(before, 96.);
        fold(&mut p);
        assert_eq!(offset(&p, port), (0., 304.));
        assert_eq!(
            p.boxes().iter().find(|b| b.id == row).unwrap().rect.1,
            before
        );
    }
}

#[test]
fn clipped_wrappers_do_not_stop_anchor_descent() {
    let source = source(false).replace("      box", "        box").replace(
        "        box height=header",
        "      box overflow=\"clip\"\n        box height=header",
    );
    let mut p = boot_source(&source);
    let port = named(&p, "port");
    p.scroll_to(port, (0., 400.)).unwrap();
    fold(&mut p);
    assert_eq!(p.scroll_of(port), (0., 352.));
    assert_eq!(row_y(&mut p), 48.);
}

#[test]
fn a_box_hidden_by_the_border_is_not_an_anchor() {
    let source = source(false)
        .replace("state header = 48", "state header = 38")
        .replace("header = 0", "header = 0\n    last = 21")
        .replace("  action fold", "  state last = 20\n  action fold")
        .replace(
            "testId=\"port\"",
            "testId=\"port\" box-sizing=\"border-box\" border-top-width=10 border-style=\"solid\"",
        )
        .replace(
            "testId=\"row17\" width=400 height=20",
            "testId=\"row17\" width=400 height=last",
        );
    let mut p = boot_source(&source);
    let port = named(&p, "port");
    p.scroll_to(port, (0., 400.)).unwrap();
    let before = row_y(&mut p);
    fold(&mut p);
    assert_eq!(p.scroll_of(port), (0., 363.));
    assert_eq!(row_y(&mut p), before);
}

#[test]
fn a_page_absolute_matches_chromes_document_anchor() {
    let source = source(true).replace(
        "      box height=header",
        "      box position=\"absolute\" top=420 width=20 height=20\n      box height=header",
    );
    let mut p = boot_source(&source);
    p.scroll_to(PAGE, (0., 400.)).unwrap();
    let before = row_y(&mut p);
    fold(&mut p);
    assert_eq!(p.page(), (0., 400.));
    assert_eq!(row_y(&mut p), before - 48.);
}

struct Rows;
impl DataSource for Rows {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::list(
            (0..200).map(|i| Value::Number(i as f64)).collect(),
        ))
    }
}

fn row_name<D: DataSource>(p: &Presenter<D>, name: &str) -> ViewId {
    let k = p.host.kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

#[test]
fn acknowledgement_after_scroller_unmount_does_not_restore_its_offset() {
    for list in [false, true] {
        let source = format!(
            r#"component App
  resource rows = rows() as shape number[]
  state top = 0
  state visible = true
  state header = 48
  action fold
    header = 0
    top = 352
  action remove
    visible = false
  view
    column width=400
      button "remove" testId="remove" press=remove
      button "fold" testId="fold" press=fold
      if visible
        {} testId="port" width=400 height=400 {}
{}          for x in rows key=x
            box height=20
"#,
            if list {
                "list virtualized=true"
            } else {
                "scroll"
            },
            if list { "scrollTop=top" } else { "" },
            if list {
                ""
            } else {
                "          box height=header\n"
            }
        );
        let (mut p, error) = Presenter::boot_with(
            &crate::test_plan(&source),
            Rows,
            (400., 450.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert_eq!(error, None);
        assert_eq!(p.settle_collections(), None);
        let port = row_name(&p, "port");
        p.scroll_to(port, (0., 400.)).unwrap();
        let first = p.display_frame().unwrap();
        assert!(p.display_complete(&first));
        let fold = row_name(&p, "fold");
        assert_eq!(p.host.dispatch_at(fold, Event::Press, p.host.now()), None);
        assert_eq!(p.after_commit(), None);
        assert!(p.pending_model_scroll(port).is_some());
        let pending = p.display_frame().unwrap();
        let remove = row_name(&p, "remove");
        assert_eq!(p.host.dispatch_at(remove, Event::Press, p.host.now()), None);
        assert_eq!(p.after_commit(), None);
        assert!(p.host.kernel().node(port).is_none());
        assert!(p.display_complete(&pending));
        assert_eq!(p.scroll_of(port), (0., 0.));
    }
}

#[test]
fn recycled_scrollers_forget_corrections_waiting_for_acknowledgement() {
    let source = r#"component App
  resource rows = rows() as shape number[]
  state header = 48
  action fold
    header = 0
  view
    list virtualized=true testId="list" width=400 height=300
      for x in rows key=x
        scroll testId=`row-${x}` width=400 height=50
          box height=header
          box height=500 press=fold testId=`fold-${x}`
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        Rows,
        (400., 400.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert_eq!(error, None);
    assert_eq!(p.settle_collections(), None);
    p.host.runner_mut().set_row_reuse(true);
    let mut old = Vec::new();
    for i in 0..6 {
        let id = row_name(&p, &format!("row-{i}"));
        p.scroll_to(id, (0., 100.)).unwrap();
        old.push((
            id,
            p.host.kernel().node(id).unwrap().key,
            format!("row-{i}"),
        ));
    }
    let first = p.display_frame().unwrap();
    assert!(p.display_complete(&first));
    let fold = row_name(&p, "fold-0");
    assert_eq!(p.host.dispatch_at(fold, Event::Press, p.host.now()), None);
    assert_eq!(p.after_commit(), None);
    assert!(old
        .iter()
        .all(|(id, _, _)| p.pending_model_scroll(*id).is_some()));
    let pending = p.display_frame().unwrap();
    let list = row_name(&p, "list");
    for top in [1000., 2000.] {
        p.scroll_to(list, (0., top)).unwrap();
        assert_eq!(p.settle_collections(), None);
    }
    let rebound: Vec<_> = old
        .iter()
        .filter(|(id, key, name)| {
            p.host.kernel().node(*id).is_some_and(|n| {
                n.key == *key && n.props.str(PropId::TestId) != Some(name.as_str())
            })
        })
        .collect();
    assert!(!rebound.is_empty());
    assert!(p.display_complete(&pending));
    for (id, _, _) in rebound {
        assert_eq!(p.scroll_of(*id), (0., 0.));
        assert!(p.pending_model_scroll(*id).is_none());
    }
}

#[test]
fn an_invalid_seek_keeps_the_pending_anchor_correction() {
    for page in [false, true] {
        let mut p = boot(page);
        let port = if page { PAGE } else { named(&p, "port") };
        p.scroll_to(port, (0., 400.)).unwrap();
        attach(&mut p);
        fold(&mut p);
        let pending = p.display_frame().unwrap();
        assert_eq!(p.scroll_to(port, (0., f32::NAN)), None);
        assert!(p.display_complete(&pending));
        assert_eq!(offset(&p, port), (0., 352.));
    }
}

#[test]
fn spatial_rotation_keeps_the_visible_anchor_still() {
    for page in [false, true] {
        let source = source(page).replace("      box", "        box").replace(
            "        box height=header",
            "      box rotate=\"y 60deg\" transform-origin=\"0 0\"\n        box height=header",
        );
        let mut p = boot_source(&source);
        let port = if page { PAGE } else { named(&p, "port") };
        p.scroll_to(port, (0., 400.)).unwrap();
        let before = row_y(&mut p);
        assert_eq!(before, 48.);
        fold(&mut p);
        assert!((offset(&p, port).1 - 352.).abs() < 0.001);
        assert!((row_y(&mut p) - before).abs() < 0.001);
    }
}

#[test]
fn clipped_candidates_match_chrome_with_and_without_suppression() {
    for margin in [false, true] {
        let source = format!(
            r#"component App
  state header = 40
  state margin = 0
  action change
    {} = {}
  view
    scroll testId="port" width=400 height=400
      box height=380
      box testId="clip" overflow="clip" height=30
        box width=0 height=header
        box height=20 margin-top=margin
      box testId="row20" height=1000 press=change
"#,
            if margin { "margin" } else { "header" },
            if margin { 20 } else { 60 }
        );
        let mut p = boot_source(&source);
        let port = named(&p, "port");
        p.scroll_to(port, (0., 400.)).unwrap();
        assert_eq!(row_y(&mut p), 10.);
        fold(&mut p);
        assert_eq!(offset(&p, port).1, if margin { 400. } else { 420. });
        assert_eq!(row_y(&mut p), if margin { 10. } else { -10. });
    }
}

#[test]
fn a_later_layout_clamps_pending_anchors_even_without_displacement() {
    for page in [false, true] {
        for suppress in [false, true] {
            let mut source = source(page)
                .replace("  action fold", "  state footer = 400\n  state width = 400\n  action shrink\n    footer = 124\n    width = 399\n  action fold")
                .replace("header = 0", "header = 96")
                .replace("testId=\"row20\"", "testId=\"row20\" dblclick=shrink");
            for i in 30..50 {
                source = source.replace(
                    &format!("      box testId=\"row{i}\" width=400 height=20 press=fold\n"),
                    "",
                );
            }
            source.push_str("      box height=footer\n");
            if suppress {
                source =
                    source.replace("testId=\"row17\" width=400", "testId=\"row17\" width=width");
            }
            let mut p = boot_source(&source);
            let port = if page { PAGE } else { named(&p, "port") };
            p.scroll_to(port, (0., 400.)).unwrap();
            attach(&mut p);
            let rev = p.reader_under(200., 60., 1.)[5] as u32;
            fold(&mut p);
            assert_eq!(p.live_scroll_max(port), 696.);
            let row = named(&p, "row20");
            assert_eq!(p.host.dispatch_at(row, Event::Dblclick, p.host.now()), None);
            assert_eq!(p.after_commit(), None);
            assert_eq!(p.live_scroll_max(port), 420.);
            let frame = p.display_frame().unwrap();
            let model = if page {
                p.model_page().1
            } else {
                p.collection_paint_scroll().unwrap()[&port].1
            };
            assert_eq!(model, 420., "page={page} suppress={suppress}");
            assert!(p.display_complete(&frame));
            assert_eq!(offset(&p, port).1, 420.);
            assert_eq!(p.reader_scrolled(port, (0., -10.), rev), Some((0., -10.)));
        }
    }
}

#[test]
fn input_clamps_the_sum_of_the_pending_correction_and_the_whole_delta() {
    for page in [false, true] {
        for wheel in [false, true] {
            for (first, by, expected) in [(10., 250., 600.), (200., 10., 562.)] {
                let mut p = boot(page);
                let port = if page { PAGE } else { named(&p, "port") };
                p.scroll_to(port, (0., 400.)).unwrap();
                attach(&mut p);
                let rev = p.reader_under(200., 60., 1.)[5] as u32;
                fold(&mut p);
                let frame = p.display_frame().unwrap();
                p.reader_scrolled(port, (0., first), rev).unwrap();
                assert!(p.display_complete(&frame));
                if wheel {
                    p.wheel_at(200., 200., 0., by);
                } else {
                    let expected_delta = if first == 10. { 190. } else { 0. };
                    assert_eq!(
                        p.reader_scrolled(port, (0., by), rev),
                        Some((0., expected_delta))
                    );
                }
                attach(&mut p);
                assert_eq!(
                    offset(&p, port).1,
                    expected,
                    "page={page} wheel={wheel} first={first}"
                );
                if !wheel {
                    assert_eq!(p.reader_scrolled(port, (0., -10.), rev), Some((0., -10.)));
                }
            }
        }
    }
}

#[cfg(not(windows))]
#[test]
fn bound_scrollers_remount_without_completing_a_scroll() {
    let source = r#"component App
  resource rows = rows() as shape number[]
  state header = 48
  state ends = 0
  action ended
    ends = ends + 1
  action fold
    header = 0
  view
    list virtualized=true testId="list" width=400 height=300
      for x in rows key=x
        scroll testId=`row-${x}` width=400 height=50 scrollTop=100 scrollend=ended
          box height=header
          box height=500 press=fold testId=`fold-${x}`
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        Rows,
        (400., 400.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert_eq!(error, None);
    assert_eq!(p.settle_collections(), None);
    assert_eq!(p.pump(p.host.now()), None);
    assert_eq!(p.host.runner().slot("ends"), Some(&Value::Number(0.)));
    p.host.runner_mut().set_row_reuse(true);
    let old: Vec<_> = (0..6)
        .map(|i| {
            let name = format!("row-{i}");
            let id = row_name(&p, &name);
            (id, p.host.kernel().node(id).unwrap().key, name)
        })
        .collect();
    let fold = row_name(&p, "fold-0");
    assert_eq!(p.host.dispatch_at(fold, Event::Press, p.host.now()), None);
    assert_eq!(p.after_commit(), None);
    assert_eq!(p.pump(p.host.now()), None);
    assert_eq!(p.host.runner().slot("ends"), Some(&Value::Number(0.)));
    let list = row_name(&p, "list");
    for top in [1000., 2000.] {
        p.scroll_to(list, (0., top)).unwrap();
        assert_eq!(p.settle_collections(), None);
    }
    assert!(old
        .iter()
        .all(|(id, _, _)| p.host.kernel().node(*id).is_none()));
    assert_eq!(p.pump(p.host.now()), None);
    assert_eq!(p.host.runner().slot("ends"), Some(&Value::Number(0.)));
}

#[test]
fn other_relative_inputs_keep_the_whole_delta_with_a_pending_anchor() {
    for authored in [false, true] {
        let mut p =
            boot_source(&source(false).replace("testId=\"port\"", "testId=\"port\" id=\"port\""));
        let port = named(&p, "port");
        p.scroll_to(port, (0., 400.)).unwrap();
        attach(&mut p);
        let rev = p.reader_under(200., 60., 1.)[5] as u32;
        fold(&mut p);
        let frame = p.display_frame().unwrap();
        p.reader_scrolled(port, (0., 10.), rev).unwrap();
        assert!(p.display_complete(&frame));
        if authored {
            p.scroll_element_by(&[Value::str("port"), Value::Number(0.), Value::Number(250.)]);
        } else {
            assert!(p.scroll_by(port, 250.));
        }
        attach(&mut p);
        assert_eq!(offset(&p, port).1, 600., "authored={authored}");
    }
}

#[test]
fn acknowledging_a_smaller_range_keeps_the_pending_reader_base() {
    for page in [false, true] {
        let mut p = boot(page);
        let port = if page { PAGE } else { named(&p, "port") };
        p.scroll_to(port, (0., 400.)).unwrap();
        attach(&mut p);
        let rev = p.reader_under(200., 60., 1.)[5] as u32;
        fold(&mut p);
        let frame = p.display_frame().unwrap();
        p.reader_scrolled(port, (0., 230.), rev).unwrap();
        assert!(p.display_complete(&frame));
        assert_eq!(p.reader_scrolled(port, (0., 10.), rev), Some((0., 0.)));
        attach(&mut p);
        assert_eq!(offset(&p, port).1, 592.);
        assert_eq!(p.reader_scrolled(port, (0., -10.), rev), Some((0., -10.)));
    }
}

fn pending_authored_seek() -> (Presenter<()>, ViewId, u32) {
    let source = source(false)
        .replace("  action fold", "  state top = 0\n  state ends = 0\n  action ended\n    ends = ends + 1\n  action seek\n    top = 600\n  action fold")
        .replace("testId=\"port\"", "testId=\"port\" id=\"port\" scrollTop=top scrollend=ended")
        .replace("testId=\"row20\"", "testId=\"row20\" dblclick=seek");
    let mut p = boot_source(&source);
    let port = named(&p, "port");
    p.scroll_to(port, (0., 400.)).unwrap();
    attach(&mut p);
    fold(&mut p);
    attach(&mut p);
    p.scroll_to(port, (0., 400.)).unwrap();
    attach(&mut p);
    assert_eq!(p.pump(p.host.now()), None);
    let rev = p.reader_under(200., 60., 1.)[5] as u32;
    let row = named(&p, "row20");
    assert_eq!(p.host.dispatch_at(row, Event::Dblclick, p.host.now()), None);
    assert_eq!(p.after_commit(), None);
    assert_eq!(p.scroll_of(port).1, 400.);
    assert_eq!(p.collection_paint_scroll().unwrap()[&port].1, 600.);
    (p, port, rev)
}

#[test]
fn no_op_relative_inputs_preserve_a_pending_authored_seek() {
    for input in ["command", "reader", "feed"] {
        let (mut p, port, rev) = pending_authored_seek();
        let ends = p.host.runner().slot("ends").cloned();
        let pending = p.pending_model_scroll(port);
        match input {
            "command" => {
                p.scroll_element_by(&[Value::str("port"), Value::Number(0.), Value::Number(0.)])
            }
            "reader" => assert_eq!(p.reader_scrolled(port, (0., 0.), rev), Some((0., 0.))),
            _ => assert!(!p.scroll_by(port, 0.)),
        }
        assert!(p.pending_model_scroll(port) == pending, "input={input}");
        assert_eq!(p.pump(p.host.now()), None);
        assert_eq!(p.host.runner().slot("ends"), ends.as_ref());
        attach(&mut p);
        assert_eq!(p.scroll_of(port).1, 600.);
    }
}

#[test]
fn reader_movement_does_not_include_the_cancelled_authored_seek() {
    let (mut p, port, rev) = pending_authored_seek();
    assert_eq!(p.reader_scrolled(port, (0., 10.), rev), Some((0., 10.)));
    assert_eq!(p.scroll_of(port).1, 410.);
    attach(&mut p);
    assert_eq!(p.scroll_of(port).1, 410.);
    assert_eq!(p.reader_scrolled(port, (0., 10.), rev), Some((0., 10.)));
}

#[test]
fn changing_only_the_rotation_axis_suppresses_anchoring() {
    for page in [false, true] {
        let source = source(page)
            .replace("  state header = 48", "  state header = 48\n  state rotation = \"y 60deg\"")
            .replace("header = 0", "rotation = \"x 60deg\"")
            .replace("      box", "        box")
            .replace("        box height=header", "      box testId=\"wrapper\" rotate=rotation transform-origin=\"0 0\"\n        box height=header");
        let mut p = boot_source(&source);
        let port = if page { PAGE } else { named(&p, "port") };
        p.scroll_to(port, (0., 400.)).unwrap();
        let wrapper = named(&p, "wrapper");
        assert_eq!(
            p.host.kernel().node(wrapper).unwrap().style.rotate_axis.0,
            [0., 1., 0.]
        );
        fold(&mut p);
        assert_eq!(
            p.host.kernel().node(wrapper).unwrap().style.rotate_axis.0,
            [1., 0., 0.]
        );
        assert_eq!(offset(&p, port).1, 400., "page={page}");
    }
}

#[cfg(not(windows))]
#[test]
fn input_that_only_moves_the_pending_model_completes_after_acknowledgement() {
    for (input, deferred) in [
        ("wheel", None),
        ("command", None),
        ("reader", None),
        ("feed", None),
        ("wheel", Some(false)),
        ("wheel", Some(true)),
    ] {
        let mut source = source(false)
            .replace("  action fold", "  state footer = 0\n  state order = \"\"\n  action moved(x: number, y: number)\n    order = `${order}s${y};`\n  action ended\n    order = `${order}e;`\n  action fold")
            .replace("header = 0", "header = 0\n    footer = 248\n    order = \"\"")
            .replace("testId=\"port\"", "testId=\"port\" id=\"port\" scroll=moved scrollend=ended");
        source.push_str("      box height=footer\n");
        let mut p = boot_source(&source);
        let port = named(&p, "port");
        p.scroll_to(port, (0., 648.)).unwrap();
        attach(&mut p);
        assert_eq!(p.pump(p.host.now()), None);
        let rev = p.reader_under(200., 60., 1.)[5] as u32;
        fold(&mut p);
        assert_eq!(p.live_scroll_max(port), 848.);
        p.set_deferred_collections(deferred.is_some());
        match input {
            "wheel" => p.wheel_at(200., 200., 0., 10.),
            "command" => {
                p.scroll_element_by(&[Value::str("port"), Value::Number(0.), Value::Number(10.)])
            }
            "reader" => {
                p.reader_scrolled(port, (0., 10.), rev).unwrap();
            }
            _ => assert!(p.scroll_by(port, 10.)),
        }
        assert_eq!(p.scroll_of(port).1, 648.);
        assert_eq!(p.collection_paint_scroll().unwrap()[&port].1, 610.);
        if deferred == Some(false) {
            assert!(p.run_deferred_collections());
        }
        assert_eq!(p.clock(p.host.now() + 100.).1, None);
        let order = p.host.runner().slot("order").unwrap().as_str().unwrap();
        assert_eq!(order, "", "input={input} deferred={deferred:?}");
        attach(&mut p);
        assert_eq!(p.scroll_of(port).1, 610.);
        if deferred == Some(true) {
            assert!(p.run_deferred_collections());
        }
        assert_eq!(p.clock(p.host.now() + 100.).1, None);
        let order = p.host.runner().slot("order").unwrap().as_str().unwrap();
        assert_eq!(order, "s610;e;", "input={input} deferred={deferred:?}");
    }
}
