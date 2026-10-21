use super::*;

#[test]
fn a_dragged_schedule_is_read_without_a_storage_request() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.load();
    let id = success(&app.save("", "Desk review", "desk-review")).to_owned();
    let requests = app.requests;
    let value = app.call("calendarItem", vec![Value::str(&id), num(1)]);
    assert_eq!(fields(&value)[0], Value::Bool(true), "{value:?}");
    let item = fields(&fields(&value)[1]);
    assert_eq!(item[0].as_str(), Some(id.as_str()));
    assert_eq!(item[1].as_str(), Some("Desk review"));
    assert_eq!(item[2].as_str(), Some(""), "notes stay on their own read");
    assert_eq!(fields(&value)[2].as_str(), Some(""));
    assert_eq!(
        app.requests, requests,
        "a drag's first packet must not wait for storage"
    );
    for missing in ["missing", ""] {
        let value = app.call("calendarItem", vec![Value::str(missing), num(1)]);
        assert_eq!(fields(&value)[0], Value::Bool(false), "{missing}");
    }
}

#[test]
fn month_pages_carry_their_first_and_last_in_month_days() {
    let root = Root::new();
    let mut app = App::open(&root);
    let september = 2026 * 12 + 8;
    let page = app.call("calendarMonth", vec![num(september)]);
    assert_eq!(fields(&page)[4], num(TODAY));
    assert_eq!(fields(&page)[5], num(TODAY + 29));
    let first = list(&fields(&page)[3])
        .iter()
        .flat_map(|week| list(&fields(week)[1]))
        .find(|cell| fields(cell)[4] == Value::Bool(true))
        .unwrap();
    assert_eq!(fields(first)[1], num(TODAY));
    for (month, days) in [
        (2027 * 12 + 1, 28.0),
        (2028 * 12 + 1, 29.0),
        (2100 * 12 + 11, 31.0),
    ] {
        let page = app.call("calendarMonth", vec![num(month)]);
        let first = fields(&page)[4].as_number().unwrap();
        let last = fields(&page)[5].as_number().unwrap();
        assert_eq!(last - first + 1.0, days, "month {month}");
    }
}
