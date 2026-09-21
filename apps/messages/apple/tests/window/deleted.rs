//! Recently Deleted pagination through the real native app and SQLite.
use super::*;

#[test]
fn contract_deleted_pages_preserve_selection_and_prepare_global_recovery() {
    let mut model = Model::new();
    for i in 0..201 {
        let id = format!("address:deleted-actions-{i:03}%40example.test");
        model.send(&id, "Archived message", "", 0.);
        model.call(
            "deleteConversation",
            vec![Value::str(&id), Value::Number(0.)],
        );
    }
    let first = model.call(
        "recentlyDeleted",
        vec![
            Value::str(""),
            Value::Number(0.),
            Value::Number(0.),
            Value::str(""),
        ],
    );
    let first_id = text(&first["people"][0], "id");
    let last_id = "address:deleted-actions-000%40example.test";
    let cursor = text(&first, "later");
    let mut runner = Runner::boot(
        model.plan,
        model.module,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let deleted = |runner: &Runner<Module>| {
        let state: Json = serde_json::from_str(&exact_runner::agent::state(runner)).unwrap();
        state["resources"]["deletedInbox"].clone()
    };
    runner
        .act("chooseDeleted", vec![Value::str(first_id)])
        .unwrap();
    runner.act("pageDeleted", vec![Value::str(cursor)]).unwrap();
    assert_eq!(runner.slot("deletedSelection"), Some(&Value::str(first_id)));
    assert_eq!(runner.slot("deletedCursor"), Some(&Value::str(cursor)));
    assert_eq!(deleted(&runner)["people"].as_array().unwrap().len(), 1);
    let selection = format!("{first_id}|{last_id}");
    runner
        .act("chooseDeleted", vec![Value::str(&selection)])
        .unwrap();
    assert_eq!(runner.slot("deletedCursor"), Some(&Value::str(cursor)));
    runner.act("prepareRecovery", vec![]).unwrap();
    assert_eq!(
        runner.slot("recoveryTargets"),
        Some(&Value::str(&selection))
    );
    assert_eq!(runner.slot("recoveryCount"), Some(&Value::Number(2.)));
    runner.act("recoverDeleted", vec![]).unwrap();
    assert_eq!(deleted(&runner)["count"], 199.);
    assert_eq!(runner.slot("deletedSelection"), Some(&Value::str("")));
    runner
        .act("filterInbox", vec![Value::str("messages")])
        .unwrap();
    assert_eq!(runner.slot("deletedCursor"), Some(&Value::str("")));
    assert_eq!(runner.slot("deletedShift"), Some(&Value::Number(2.)));
}

#[test]
fn deleted_pages_keep_global_selection_and_atomic_recovery() {
    let root = Directory(
        std::env::temp_dir().join(format!("messages-deleted-pages-{}", std::process::id())),
    );
    std::fs::create_dir(&root.0).unwrap();
    let mut model = open(&root);
    for i in 0..401 {
        let id = format!("address:deleted-page-{i:03}%40example.test");
        model.send(&id, "Archived message", "", 0.);
        model.call(
            "deleteConversation",
            vec![Value::str(&id), Value::Number(0.)],
        );
    }
    let args = |selection: &str, now: f64, cursor: &str| {
        vec![
            Value::str(selection),
            Value::Number(0.),
            Value::Number(now),
            Value::str(cursor),
        ]
    };
    let first = model.call("recentlyDeleted", args("", 0., ""));
    let second = model.call("recentlyDeleted", args("", 0., text(&first, "later")));
    let last = model.call("recentlyDeleted", args("", 0., text(&second, "later")));
    let ids = |page: &Json| {
        page["people"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| text(row, "id").to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        [ids(&first).len(), ids(&second).len(), ids(&last).len()],
        [200, 200, 1]
    );
    let all: Vec<_> = (0..401)
        .rev()
        .map(|i| format!("address:deleted-page-{i:03}%40example.test"))
        .collect();
    assert_eq!([ids(&first), ids(&second), ids(&last)].concat(), all);
    for page in [&first, &second, &last] {
        assert_eq!(page["count"], 401.);
        assert_eq!(text(page, "targets"), all.join("|"));
    }
    assert_eq!(
        model.call("recentlyDeleted", args("", 0., text(&second, "earlier"))),
        first
    );
    let selection = format!("{}|{}", all[0], all[400]);
    let selected = model.call(
        "recentlyDeleted",
        args(&selection, 0., text(&first, "later")),
    );
    assert_eq!(selected["count"], 2.);
    assert_eq!(text(&selected, "targets"), selection);
    assert!(selected["people"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["chosen"] == false));
    // Recover All remains one atomic edit, never a silent first-page edit.
    let refused = model
        .try_call(
            "recoverConversations",
            vec![Value::str(&all.join("|")), Value::Number(0.)],
        )
        .unwrap_err();
    assert!(format!("{refused:?}").contains("512"));
    assert_eq!(model.call("recentlyDeleted", args("", 0., "")), first);
    let refused = model
        .try_call("recentlyDeleted", args("", 30. * 86400000., "bad"))
        .unwrap_err();
    assert!(matches!(refused, DataError::BadArguments(_)));
    assert_eq!(model.call("recentlyDeleted", args("", 0., "")), first);
    model.call(
        "recoverConversations",
        vec![Value::str(&selection), Value::Number(0.)],
    );
    let cursor = text(&first, "later");
    model.call(
        "purgeConversations",
        vec![Value::str(&all[200]), Value::Number(0.)],
    );
    let shifted = model.call("recentlyDeleted", args("", 0., cursor));
    assert_eq!(ids(&shifted)[0], all[201]);
    assert_eq!(shifted["count"], 398.);
    drop(model);
    let mut model = open(&root);
    assert_eq!(model.call("recentlyDeleted", args("", 0., cursor)), shifted);
    let expired = model.call("recentlyDeleted", args(&selection, 30. * 86400000., cursor));
    assert!(ids(&expired).is_empty());
    assert_eq!(expired["count"], 0.);
    assert_eq!(text(&expired, "targets"), "");
    assert_eq!(text(&expired, "earlier"), "");
    assert_eq!(text(&expired, "later"), "");
    drop(model);
    assert_eq!(
        open(&root).call("recentlyDeleted", args(&selection, 0., "")),
        expired
    );
}
