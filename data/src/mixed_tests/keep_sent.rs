//! `Mixed::keep_sent` with a JavaScript child in an ordered set (LLP 1146 D2).

use super::{Proxy, Source};
use crate::Mixed;
use exact_runner::{DataSource, InFlight, Store, Target};

/// An ordered JavaScript child receives its sent effects, whose replies take
/// another ordered turn. Its earlier turns happened outside the refused
/// commit, so it must not restore them; the Rust child hears the refusal.
#[test]
fn an_ordered_child_keeps_sent_replies_without_restoring_refused_turns() {
    let js = Source::new("js");
    let rust = Source::new("rust");
    let mut mixed = Mixed::new(
        Proxy::new(js.clone()),
        rust.clone(),
        &["jssecret", "js"],
        &["rustsecret", "rust"],
    )
    .unwrap();
    let store = Store::new(
        "secret.keep js\nsecret.keep rust\nsecret.keep shared",
        [("shared".to_string(), "before".into())],
    );
    let let_go = [
        InFlight {
            target: Target::Mutation(0),
            source: "jssecret",
            args: &[],
            continuation: Some(101),
        },
        InFlight {
            target: Target::Mutation(1),
            source: "rustsecret",
            args: &[],
            continuation: None,
        },
    ];
    assert_eq!(mixed.keep_sent(&store, &let_go, true), [None, None]);
    assert_eq!(*js.kept.borrow(), [(vec!["jssecret".to_string()], false)]);
    assert_eq!(
        *rust.kept.borrow(),
        [(vec!["rustsecret".to_string()], true)]
    );
}

/// Module work lands in the JavaScript child under its own grants (LLP 1146
/// D2), as its answers run: it keeps its own secret and the shared one, never
/// the Rust child's.
#[test]
fn module_work_lands_under_the_javascript_child_s_grants() {
    let mut mixed = super::pair();
    let mut store = Store::new(mixed.grants(), [("rust".to_string(), "before".into())]);
    mixed.module_landed(&mut store, 7, super::failed()).unwrap();
    assert_eq!(store.get("js"), Some("landed"));
    assert_eq!(store.get("shared"), Some("landed"));
    assert_eq!(store.get("rust"), Some("before"));
}

#[test]
fn disjoint_workers_dispatch_without_waiting_for_each_other() {
    use crate::Placed;
    use exact_runner::{DataError, Dispatch, Placement, Reply, Value, Work};
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::time::Duration;
    struct Waiting {
        grants: &'static str,
        entered: Sender<String>,
        release: Receiver<()>,
    }
    impl DataSource for Waiting {
        fn app_id(&self) -> &str {
            "test.disjoint"
        }
        fn grants(&self) -> &str {
            self.grants
        }
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            self.entered.send(source.into()).unwrap();
            self.release
                .recv()
                .map_err(|e| DataError::Unavailable(e.to_string()))?;
            Ok(Value::Unit)
        }
    }
    let (entered, started) = channel();
    let (release_a, a) = channel();
    let (release_b, b) = channel();
    let a = Placed::new(
        Waiting {
            grants: "secret.keep a",
            entered: entered.clone(),
            release: a,
        },
        Placement::Worker,
    );
    let b = Placed::new(
        Waiting {
            grants: "secret.keep b",
            entered,
            release: b,
        },
        Placement::Worker,
    );
    let mut mixed = Mixed::new(a, b, &["a"], &["b"]).unwrap();
    mixed.activate().unwrap();
    let mut store = Store::new(mixed.grants(), []);
    let a = super::token(mixed.answer(&mut store, "a", &[]).unwrap());
    let b = super::token(mixed.answer(&mut store, "b", &[]).unwrap());
    let (replied, replies) = channel();
    for (token, source) in [(a, "a"), (b, "b")] {
        let Dispatch::Run(Work::Later(work)) = mixed.dispatch(token, &store) else {
            panic!("disjoint worker held")
        };
        let replied = replied.clone();
        work(Reply::new(move |outcome| {
            let _ = replied.send((source, outcome));
        }));
    }
    let mut names: Vec<_> = (0..2)
        .map(|_| {
            started
                .recv_timeout(Duration::from_secs(5))
                .expect("both owners entered before either was released")
        })
        .collect();
    names.sort();
    assert_eq!(names, ["a", "b"]);
    release_a.send(()).unwrap();
    release_b.send(()).unwrap();
    for _ in 0..2 {
        let (source, outcome) = replies.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(
            super::value_of(mixed.parse(&mut store, source, &[], outcome)),
            Value::Unit
        );
    }
}
