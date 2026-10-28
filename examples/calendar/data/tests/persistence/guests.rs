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
            Value::str("+1 415 555 0102\n"),
        ],
    );
    assert_eq!(fields(&added)[0], num(2), "{added:?}");
    let more = app.call(
        "addGuests",
        vec![
            Value::str("sample-2026-09-0"),
            Value::str("\nAna Ruiz"),
            Value::str("solo@example.com\n"),
            Value::str("\n+34 91 555 0105"),
        ],
    );
    assert_eq!(fields(&more)[0], num(2), "{more:?}");
    let gone = app.call(
        "addGuests",
        vec![
            Value::str("missing"),
            Value::str("A"),
            Value::str(""),
            Value::str(""),
        ],
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
    // The phones, kept beside the guests in the order added (LLP 1133.024 D16).
    let phones = app.call(
        "calendarGuestPhones",
        vec![Value::str("sample-2026-09-0"), num(0)],
    );
    let tels: Vec<&str> = list(&fields(&phones)[0])
        .iter()
        .map(|t| t.as_str().unwrap())
        .collect();
    assert_eq!(tels, ["+1 415 555 0102", "+34 91 555 0105"]);
    let none = app.call(
        "calendarGuestPhones",
        vec![Value::str("sample-2026-09-1"), num(0)],
    );
    assert!(list(&fields(&none)[0]).is_empty());
}

#[test]
fn an_invitation_is_the_item_as_an_icalendar_event_in_app_tmp() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.load();
    let id = success(&app.save("", "Offsite; day one", "invite-offsite")).to_owned();
    // 2026-10-28T00:00:05Z.
    let written = app.call(
        "writeInvitation",
        vec![
            Value::str(&id),
            Value::str("Offsite; day one"),
            Value::str(""),
            Value::Number(1_793_145_605_000.0),
        ],
    );
    let f = fields(&written);
    let path = f[0].as_str().unwrap();
    assert!(
        path.starts_with("app:/tmp/invite-") && path.ends_with(".ics"),
        "{written:?}"
    );
    let ics = std::fs::read_to_string(
        root.0
            .join("temporary")
            .join(path.trim_start_matches("app:/tmp/")),
    )
    .unwrap();
    for line in [
        "BEGIN:VEVENT",
        "DTSTAMP:20261028T000005Z",
        "DTSTART:20260925T093000",
        "DTEND:20260930T171500",
        "SUMMARY:Offsite\\; day one",
    ] {
        assert!(ics.contains(&format!("{line}\r\n")), "{line} in {ics}");
    }
    let missing = app.call(
        "writeInvitation",
        vec![
            Value::str("missing"),
            Value::str(""),
            Value::str(""),
            num(0),
        ],
    );
    assert_eq!(fields(&missing)[0].as_str(), Some(""), "{missing:?}");
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
