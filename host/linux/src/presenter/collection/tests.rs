//! The cursor's corrections and the refinement queue.
use super::*;
use exact_runner::AnchorCorrection;

fn snapshot() -> CollectionSnapshot {
    CollectionSnapshot {
        view: 1,
        axis: exact_runner::ListAxis::Vertical,
        parent: None,
        restored: false,
        seeking: false,
        waiting: false,
        request: 0,
        revision: 4,
        scroll_sequence: 2,
        count: 0,
        total_extent: 0.,
        rows: vec![],
        correction: Some(AnchorCorrection {
            scroll_sequence: 2,
            offset: 200.,
            from: None,
            smooth: false,
        }),
        pending: false,
    }
}
#[test]
fn corrections_are_consumed_once_and_user_scroll_or_resize_wins() {
    let s = snapshot();
    let mut cursor = Cursor {
        sequence: 2,
        ..Cursor::default()
    };
    assert_eq!(cursor.correction(&s, None), Some(200.));
    assert_eq!(cursor.correction(&s, None), None);
    let mut cursor = Cursor {
        sequence: 2,
        ..Cursor::default()
    };
    cursor.advance();
    assert_eq!(cursor.correction(&s, None), None);
    let mut cursor = Cursor {
        sequence: 2,
        ..Cursor::default()
    };
    cursor.geometry((200., 300., 180., [10., 0.]));
    cursor.geometry((190., 300., 170., [10., 0.]));
    assert_eq!(cursor.correction(&s, None), None);
    let mut cursor = Cursor {
        sequence: 2,
        ..Cursor::default()
    };
    cursor.geometry((200., 300., 180., [10., 0.]));
    cursor.geometry((200., 300., 180., [20., 0.]));
    assert_eq!(
        cursor.correction(&s, None),
        None,
        "origin changes also invalidate old corrections"
    );
    // A resize in the pass that brings the correction (feed F14) is
    // not the reader: the correction planned before it lands.
    let mut cursor = Cursor {
        sequence: 2,
        ..Cursor::default()
    };
    cursor.geometry((200., 300., 180., [10., 0.]));
    cursor.geometry((200., 348., 180., [10., 0.]));
    assert_eq!(cursor.correction(&s, Some(2)), Some(200.));
    let mut cursor = Cursor {
        sequence: u64::MAX,
        ..Cursor::default()
    };
    let mut overflow = s;
    overflow.correction.as_mut().unwrap().scroll_sequence = u64::MAX;
    assert_eq!(cursor.correction(&overflow, None), None);
    // A request's move lands though the reader has scrolled since the
    // report it was planned on; an anchor's relative one does not.
    let mut seeking = snapshot();
    seeking.seeking = true;
    let mut cursor = Cursor {
        sequence: 2,
        ..Cursor::default()
    };
    cursor.advance();
    assert_eq!(cursor.correction(&seeking, None), Some(200.));
    seeking.revision += 1;
    seeking.correction.as_mut().unwrap().from = Some(150.);
    assert_eq!(cursor.correction(&seeking, None), None);
    // Sent again after the reader moved the port (a data update before
    // the next report), it does not land; a newer request's move does.
    seeking.revision += 1;
    seeking.correction.as_mut().unwrap().from = None;
    cursor.reader_moved = true;
    cursor.advance();
    assert_eq!(cursor.correction(&seeking, None), None);
    seeking.revision += 1;
    seeking.request += 1;
    assert_eq!(cursor.correction(&seeking, None), Some(200.));
}

#[test]
fn refinement_queue_deduplicates_and_forgets_unmounted_collections() {
    let mut state = State::default();
    for _ in 0..1000 {
        state.schedule(&[snapshot()]);
    }
    assert_eq!(state.queue.len(), 1);
    assert_eq!(state.cursors.len(), 1);
    state.schedule(&[]);
    assert!(!state.pending());
    assert!(state.cursors.is_empty());
}

#[test]
fn empty_collection_padding_resolves_lengths_and_percentages() {
    struct Empty;
    impl DataSource for Empty {
        fn query(
            &mut self,
            _: &str,
            _: &[exact_runner::Value],
        ) -> Result<exact_runner::Value, exact_runner::DataError> {
            Ok(exact_runner::Value::list(vec![]))
        }
    }
    for (padding, cross) in [
        ("20px", 60.),
        ("10%", 60.),
        ("calc(10% + 3px)", 54.),
        ("min(10%, 12px)", 76.),
        ("clamp(10px, 10%, 30px)", 60.),
    ] {
        let source = format!("component App\n  resource rows = rows() as shape number[]\n  view\n    column width=200\n      list virtualized=true width=\"50%\" height=100 padding-left=\"{padding}\" padding-right=\"{padding}\" box-sizing=\"border-box\"\n        for x in rows key=x\n          text `${{x}}`\n");
        let plan = contract::compile(&source).unwrap();
        let (host, error) = Host::boot(
            &plan.encode(),
            Empty,
            Box::new(exact_kernel::MonospaceMeasurer::default()),
            400.,
            500.,
        )
        .unwrap();
        assert!(error.is_none());
        let facts = geometry(host.kernel(), &host.collections()[0], 400.).unwrap();
        assert_eq!(facts.width, 100., "{padding}");
        assert_eq!(facts.cross, cross, "{padding}");
        assert_eq!(facts.origin, 0., "{padding}");
    }
}

/// A padding whose `calc()` resolves below zero is zero, CSS's range for
/// padding (LLP 1010 §6.9): the origin, the range and the reported
/// padding all take 0, on either axis, and the runner accepts the report.
#[test]
fn a_negative_resolved_padding_is_zero_on_either_axis() {
    struct Rows;
    impl DataSource for Rows {
        fn query(
            &mut self,
            _: &str,
            _: &[exact_runner::Value],
        ) -> Result<exact_runner::Value, exact_runner::DataError> {
            Ok(exact_runner::Value::list(
                (0..50)
                    .map(|i| exact_runner::Value::Number(i as f64))
                    .collect(),
            ))
        }
    }
    let below = "\"calc(env(safe-area-inset-bottom) - 12px)\"";
    for list in [
        format!("list virtualized=true width=200 height=100 overflow-x=\"hidden\" padding-top={below} padding-bottom={below}"),
        format!("list virtualized=true display=\"flex\" width=200 height=100 overflow-x=\"scroll\" overflow-y=\"hidden\" padding-left={below} padding-right={below}"),
    ] {
        let source = format!("component App\n  resource rows = rows() as shape number[]\n  view\n    column width=200\n      {list}\n        for x in rows key=x\n          text `${{x}}` width=40 height=20\n");
        let plan = contract::compile(&source).unwrap();
        let (mut host, error) = Host::boot(
            &plan.encode(),
            Rows,
            Box::new(exact_kernel::MonospaceMeasurer::default()),
            400.,
            500.,
        )
        .unwrap();
        assert!(error.is_none());
        let snapshot = host.collections()[0].clone();
        let facts = geometry(host.kernel(), &snapshot, 400.).unwrap();
        assert_eq!((facts.origin, facts.padding), (0., [0.; 2]), "{list}");
        let feedback = CollectionFeedback {
            view: snapshot.view,
            revision: snapshot.revision,
            scroll_sequence: 1,
            offset: 0.,
            port_main: facts.main(),
            port_cross: facts.port_cross(),
            cross: facts.cross,
            padding: facts.padding,
            measurements: vec![],
            focus_view: None,
            interaction_view: None,
        };
        host.collection_feedback_filled(feedback, CollectionFill::default())
            .unwrap_or_else(|e| panic!("{list}: {e:?}"));
    }
}
