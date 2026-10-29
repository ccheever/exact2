//! `Mixed::keep_sent` with a JavaScript child in an ordered set (LLP 1146 D2).

use super::{Proxy, Source};
use crate::Mixed;
use exact_runner::{DataSource, InFlight, Store, Target};

/// A refused commit puts back nothing in a JavaScript child in an ordered set
/// (LLP 1146 D2): its calls take their turns in the set, outside any commit,
/// so it hears no let-go request and no refusal, while the Rust child hears its
/// own request and the refusal.
#[test]
fn a_refused_commit_restores_nothing_in_an_ordered_javascript_child() {
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
    assert_eq!(*js.kept.borrow(), [(Vec::<String>::new(), false)]);
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
