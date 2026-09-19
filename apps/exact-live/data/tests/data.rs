//! Source-first app composition proofs. Authored before Live; execution held.
use exact_live_data::Live;
use exact_plan::Value;
use exact_runner::{Answer, DataSource, Outcome, Response, Store};
use messages_stress_data::ReusableMessagesStress;
use std::rc::Rc;

fn fields(v: &Value) -> &Rc<Vec<Value>> {
    let Value::Record(v) = v else {
        panic!("record")
    };
    v
}
fn rows(v: &Value) -> &Rc<Vec<Value>> {
    let Value::List(v) = &fields(v)[0] else {
        panic!("rows")
    };
    v
}
fn history(count: f64, revision: f64) -> Vec<Value> {
    vec![
        Value::Number(count),
        Value::Number(revision),
        Value::Number(32.),
        Value::str(""),
        Value::Number(0.),
        Value::Bool(true),
    ]
}

#[test]
fn curated_default_has_real_content_without_network() {
    let mut data = Live::default();
    let crew = data.query("history", &history(0., 0.)).unwrap();
    assert_eq!(rows(&crew).len(), 8);
    assert!(fields(&rows(&crew)[0])[2]
        .as_str()
        .unwrap()
        .contains("North shore"));
    let gallery = data.query("gallery", &[]).unwrap();
    assert_eq!(fields(&gallery)[1], Value::Number(6.));
    let book = data.query("runbook", &[Value::Number(0.)]).unwrap();
    let Value::List(blocks) = &fields(&book)[2] else {
        panic!("blocks")
    };
    assert!(blocks.len() > 8);
    assert!(matches!(
        data.answer(
            &mut Store::default(),
            "completion",
            &[
                Value::Number(0.),
                Value::Number(0.),
                Value::Number(0.),
                Value::Bool(false)
            ]
        )
        .unwrap(),
        Answer::Now(_)
    ));
}

#[test]
fn curated_scene_notice_does_not_leak_initial_trimming_but_keeps_real_deletes() {
    let mut data = Live::default();
    let initial = data.query("gallery", &[]).unwrap();
    assert!(fields(&initial)[19]
        .as_str()
        .unwrap()
        .starts_with("Open a study"));
    let removed = data
        .query(
            "galleryAction",
            &[
                Value::str("delete"),
                Value::str("photo-00000"),
                Value::Number(0.),
            ],
        )
        .unwrap();
    assert!(fields(&removed)[19]
        .as_str()
        .unwrap()
        .contains("Removed photo-00000"));
    let reset = data
        .query(
            "galleryAction",
            &[Value::str("reset"), Value::str(""), Value::Number(0.)],
        )
        .unwrap();
    assert!(fields(&reset)[19]
        .as_str()
        .unwrap()
        .starts_with("Open a study"));
}

#[test]
fn full_stress_is_exact_existing_reusable_source_and_reuses_unchanged_rows() {
    let mut data = Live::default();
    let mut reference = ReusableMessagesStress::default();
    let a = data.query("history", &history(10_000., 0.)).unwrap();
    assert_eq!(
        a,
        reference.query("history", &history(10_000., 0.)).unwrap()
    );
    let b = data.query("history", &history(10_000., 1.)).unwrap();
    assert_eq!(
        b,
        reference.query("history", &history(10_000., 1.)).unwrap()
    );
    assert_eq!(rows(&b).len(), 10_000);
    assert_eq!(fields(&b)[3], Value::Number(32.));
    for i in 0..9968 {
        assert!(Rc::ptr_eq(fields(&rows(&a)[i]), fields(&rows(&b)[i])));
    }
}

#[test]
fn unrelated_queries_preserve_canonical_history_and_runbook() {
    let mut data = Live::default();
    let a = data.query("history", &history(10_000., 1.)).unwrap();
    let book = data.query("runbook", &[Value::Number(0.)]).unwrap();
    data.query("gallery", &[]).unwrap();
    data.query("theme", &[]).unwrap();
    let b = data.query("history", &history(10_000., 1.)).unwrap();
    assert!(Rc::ptr_eq(fields(&a), fields(&b)));
    assert!(Rc::ptr_eq(
        fields(&book),
        fields(&data.query("runbook", &[Value::Number(0.)]).unwrap())
    ));
}

#[test]
fn curated_input_is_bounded_and_preserves_one_local_echo() {
    let mut data = Live::default();
    let mut args = history(0., 0.);
    args[3] = Value::str("Ready for the coast — 朝");
    let value = data.query("history", &args).unwrap();
    assert_eq!(rows(&value).len(), 9);
    assert_eq!(
        fields(rows(&value).last().unwrap())[0],
        Value::str("local-echo")
    );
    args[3] = Value::str(&"x".repeat(513));
    assert!(data.query("history", &args).is_err());
    for bad in [f64::NAN, -1., 7., 0.5] {
        let mut args = history(bad, 0.);
        assert!(data.query("history", &args).is_err());
        args[0] = Value::Number(0.);
        args[1] = Value::Number(bad);
        assert!(data.query("history", &args).is_err());
    }
}

#[test]
fn gallery_manual_move_and_reset_use_existing_identity_rules() {
    let mut data = Live::default();
    let action = |op: &str, id: &str, n: f64| [Value::str(op), Value::str(id), Value::Number(n)];
    let initial = data.query("gallery", &[]).unwrap();
    let original = data
        .query(
            "galleryRows",
            &[
                fields(&initial)[7].clone(),
                Value::Number(0.),
                Value::Bool(true),
            ],
        )
        .unwrap();
    let lifted = data
        .query("galleryAction", &action("lift", "photo-00000", 0.))
        .unwrap();
    let token = fields(&lifted)[15].as_number().unwrap();
    data.query("galleryAction", &action("later", "", token))
        .unwrap();
    let placed = data
        .query("galleryAction", &action("place", "", token))
        .unwrap();
    assert_eq!(fields(&placed)[1], Value::Number(6.));
    let reordered = data
        .query(
            "galleryRows",
            &[
                fields(&placed)[7].clone(),
                Value::Number(0.),
                Value::Bool(true),
            ],
        )
        .unwrap();
    let (Value::List(original), Value::List(reordered)) = (&original, &reordered) else {
        panic!("full ordered rows")
    };
    assert_eq!(fields(&original[0])[0], Value::str("photo-00000"));
    assert_eq!(fields(&reordered[0])[0], Value::str("photo-00001"));
    assert_eq!(fields(&reordered[1])[0], Value::str("photo-00000"));
    let reset = data
        .query("galleryAction", &action("reset", "", 0.))
        .unwrap();
    assert_eq!(fields(&reset)[1], Value::Number(6.));
    let restored = data
        .query(
            "galleryRows",
            &[
                fields(&reset)[7].clone(),
                Value::Number(0.),
                Value::Bool(true),
            ],
        )
        .unwrap();
    assert_eq!(restored, Value::List(original.clone()));
}

#[test]
fn scene_reset_does_not_reuse_old_viewer_tokens() {
    let mut data = Live::default();
    let open = [
        Value::str("open"),
        Value::str("photo-00000"),
        Value::Number(0.),
    ];
    let old = data.query("galleryAction", &open).unwrap();
    let old_token = fields(&old)[11].clone();
    data.query(
        "galleryAction",
        &[Value::str("reset"), Value::str(""), Value::Number(0.)],
    )
    .unwrap();
    let fresh = data.query("galleryAction", &open).unwrap();
    assert_ne!(fields(&fresh)[11], old_token);
    let stale = data
        .query(
            "galleryAction",
            &[Value::str("close"), Value::str(""), old_token],
        )
        .unwrap();
    assert_eq!(
        stale, fresh,
        "old callback must not close the replacement viewer"
    );
}

#[test]
fn jobs_are_requests_and_failure_is_never_a_completed_job() {
    let mut data = Live::default();
    let mut store = Store::default();
    let args = [Value::Number(128.), Value::Number(0.)];
    let Answer::Later(request) = data.answer(&mut store, "openWave", &args).unwrap() else {
        panic!("real HTTP required")
    };
    assert_eq!(request.method, "POST");
    assert!(request.url.contains("/api/open?count=128"));
    let Answer::Now(result) = data
        .parse(
            &mut store,
            "openWave",
            &args,
            Outcome::Response(Response {
                status: 503,
                headers: vec![],
                body: vec![],
            }),
        )
        .unwrap()
    else {
        panic!("settled error")
    };
    assert_eq!(fields(&result)[0], Value::Number(0.));
    assert!(fields(&result)[2].as_str().unwrap().contains("503"));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_full_one_and_four_mib_runbooks_use_real_continuations_without_splitting() {
    let mut data = Live::default();
    let mut store = Store::default();
    for bytes in [1_048_576., 4_194_304.] {
        let args = [Value::Number(bytes)];
        let Answer::Later(request) = data.answer(&mut store, "runbook", &args).unwrap() else {
            panic!("cold native parse must yield")
        };
        assert!(request.is_ordered());
        let work = data.continuation(request.continuation.unwrap()).unwrap();
        let outcome = std::thread::spawn(work).join().unwrap();
        let Answer::Now(actual) = data.parse(&mut store, "runbook", &args, outcome).unwrap() else {
            panic!("finished document")
        };
        let control = markdown_stress_data::MarkdownStress::default()
            .query(
                "document",
                &[
                    Value::str("paragraph"),
                    Value::Number(bytes),
                    Value::Number(0.),
                    Value::Number(0.),
                    Value::Bool(true),
                ],
            )
            .unwrap();
        assert_eq!(
            actual,
            Value::record(vec![
                fields(&control)[0].clone(),
                fields(&control)[1].clone(),
                fields(&control)[9].clone()
            ])
        );
        let Value::List(blocks) = &fields(&actual)[2] else {
            panic!("blocks")
        };
        let paragraphs = blocks
            .iter()
            .filter(|b| fields(b)[1].as_str() == Some("paragraph"))
            .collect::<Vec<_>>();
        assert_eq!(
            paragraphs.len(),
            1,
            "existing pathological paragraph is never split"
        );
        let Value::List(runs) = &fields(paragraphs[0])[7] else {
            panic!("paragraph runs")
        };
        let text_bytes: usize = runs
            .iter()
            .map(|run| fields(run)[1].as_str().unwrap().len())
            .sum();
        assert!(
            text_bytes > (bytes as usize) * 9 / 10,
            "full giant paragraph runs retained: {text_bytes}"
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn returning_to_curated_retires_unclaimed_document_work() {
    let mut data = Live::default();
    let mut store = Store::default();
    let Answer::Later(request) = data
        .answer(&mut store, "runbook", &[Value::Number(1_048_576.)])
        .unwrap()
    else {
        panic!("cold")
    };
    assert!(matches!(
        data.answer(&mut store, "runbook", &[Value::Number(0.)])
            .unwrap(),
        Answer::Now(_)
    ));
    assert!(data.continuation(request.continuation.unwrap()).is_none());
}
