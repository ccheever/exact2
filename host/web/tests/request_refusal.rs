//! Unwrapped Rust sources must not turn an invalid HTTP annotation into an effect.
use exact_runner::{
    Answer, DataError, DataSource, Event, FailureKind, Outcome, Request, RequestOut, Value,
};
use exact_web::{batch::Batch, Host};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

struct Direct {
    request: Request,
    converted: Arc<AtomicUsize>,
}
impl DataSource for Direct {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!()
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[Value],
    ) -> Result<Answer, DataError> {
        Ok(Answer::Later(self.request.clone()))
    }
    fn dispatch(&mut self, _: u64, _: &exact_runner::Store) -> exact_runner::Dispatch {
        self.converted.fetch_add(1, Ordering::SeqCst);
        exact_runner::Dispatch::Missing // Refusal must happen before dispatch.
    }
    fn parse(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        assert!(
            matches!(
                outcome,
                Outcome::Failed {
                    kind: FailureKind::Refused,
                    ..
                }
            ),
            "{outcome:?}"
        );
        Ok(Answer::Now(Value::record(vec![Value::Bool(true)])))
    }
}
fn invalid_requests() -> Vec<Request> {
    let mut continuation = Request::get("").independent_http(100);
    continuation.continuation = Some(7);
    vec![
        Request::storage(b"{\"op\":\"write\"}".to_vec()).independent_http(100),
        continuation,
        Request::get("https://example.test").independent_http(0),
        Request::get("https://example.test").independent_http(64 * 1024 * 1024 + 1),
    ]
}
fn refused(batch: &str) {
    assert!(batch.contains("\"op\":\"refuse\""), "{batch}");
    for effect in ["storage", "continue", "request"] {
        assert!(!batch.contains(&format!("\"op\":\"{effect}\"")), "{batch}");
    }
}
#[test]
fn batch_refuses_invalid_annotations_before_serializing_any_effect() {
    for request in invalid_requests() {
        let mut batch = Batch::new();
        batch.request(&RequestOut {
            ticket: 8,
            target: "reply".into(),
            request,
            forced: false,
        });
        refused(&batch.finish(false, 0.0, None));
    }
    for limit in [1, 64 * 1024 * 1024] {
        let mut batch = Batch::new();
        batch.request(&RequestOut {
            ticket: 9,
            target: "reply".into(),
            request: Request::get("https://example.test").independent_http(limit),
            forced: false,
        });
        assert!(batch
            .finish(false, 0.0, None)
            .contains("\"op\":\"request\""));
    }
}
#[test]
fn direct_sources_refuse_at_boot_and_dispatch_without_converting_tokens_and_settle_terminally() {
    for at_boot in [true, false] {
        let source = if at_boot {
            "shape Reply\n  refused: bool\ncomponent App\n  resource reply = bad(true) as shape Reply\n  view\n    text \"boot\"\n"
        } else {
            "shape Reply\n  refused: bool\ncomponent App\n  mutation reply as shape Reply\n  action fire writes reply\n    send reply = bad()\n  view\n    button press=fire testId=\"fire\"\n      text \"Fire\"\n"
        };
        let plan = contract::compile(source).unwrap().encode();
        for request in invalid_requests() {
            let converted = Arc::new(AtomicUsize::new(0));
            let carried = exact_runner::Carried {
                resources: vec![(
                    "reply".into(),
                    vec![Value::Bool(false)],
                    Value::record(vec![Value::Bool(false)]),
                )],
                ..Default::default()
            };
            let (mut host, boot) = Host::boot_with(
                &plan,
                Direct {
                    request,
                    converted: converted.clone(),
                },
                at_boot.then_some(&carried),
                Default::default(),
                "/",
            )
            .unwrap();
            let batch = if at_boot {
                boot
            } else {
                let kernel = host.runner().kernel();
                let id = kernel
                    .node_by_key(kernel.find_by_test_id("fire")[0])
                    .unwrap()
                    .id;
                host.dispatch(id, Event::Press)
            };
            refused(&batch);
            assert_eq!(converted.load(Ordering::SeqCst), 0);
            let pending = host.runner().pending();
            assert_eq!(pending.len(), 1);
            let result = host.fulfill_at(
                pending[0].1,
                2,
                0,
                "",
                b"invalid HTTP annotation".to_vec(),
                1.0,
            );
            assert!(result.contains("\"error\":null"), "{result}");
            assert!(
                host.runner().pending().is_empty(),
                "refusal must settle the current ticket"
            );
        }
    }
}
