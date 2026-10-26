use super::*;

fn guests(app: &mut App, id: &str) -> Vec<(String, String)> {
    let value = app.call("calendarGuests", vec![Value::str(id), num(0)]);
    list(&fields(&value)[0])
        .iter()
        .map(|g| {
            let f = fields(g);
            (f[0].as_str().unwrap().into(), f[1].as_str().unwrap().into())
        })
        .collect()
}

#[test]
fn guests_are_kept_in_order_across_a_restart_and_need_no_schema_change() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.load();
    assert!(guests(&mut app, "sample-2026-09-0").is_empty());
    let added = app.call(
        "addGuests",
        vec![
            Value::str("sample-2026-09-0"),
            Value::str("Ben Okafor\nEli Navarro"),
            Value::str("ben@example.com\neli@example.com"),
        ],
    );
    assert_eq!(fields(&added)[0], num(2), "{added:?}");
    let more = app.call(
        "addGuests",
        vec![
            Value::str("sample-2026-09-0"),
            Value::str("\nAna Ruiz"),
            Value::str("solo@example.com\n"),
        ],
    );
    assert_eq!(fields(&more)[0], num(2), "{more:?}");
    let gone = app.call(
        "addGuests",
        vec![Value::str("missing"), Value::str("A"), Value::str("")],
    );
    assert_eq!(fields(&gone)[0], num(0));
    drop(app);
    let mut app = App::open(&root);
    app.load();
    let want = [
        ("Ben Okafor", "ben@example.com"),
        ("Eli Navarro", "eli@example.com"),
        ("", "solo@example.com"),
        ("Ana Ruiz", ""),
    ];
    let got = guests(&mut app, "sample-2026-09-0");
    assert_eq!(
        got,
        want.map(|(n, e)| (n.to_owned(), e.to_owned())).to_vec()
    );
    assert!(guests(&mut app, "sample-2026-09-1").is_empty());
}

#[test]
fn an_item_exports_as_a_local_start_and_a_duration() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.load();
    // 2026-09-25 09:30 to 2026-09-30 17:15: five days and 7 h 45 min.
    let id = success(&app.save("", "Offsite", "export-offsite")).to_owned();
    let value = app.call("calendarExport", vec![Value::str(&id), num(0)]);
    let f = fields(&value);
    assert_eq!(f[0].as_str(), Some("2026-09-25T09:30:00"), "{value:?}");
    assert_eq!(f[1].as_str(), Some("PT127H45M"));
    assert_eq!(f[2], Value::Bool(false));
    let missing = app.call("calendarExport", vec![Value::str("missing"), num(0)]);
    assert_eq!(fields(&missing)[0].as_str(), Some(""));
}
