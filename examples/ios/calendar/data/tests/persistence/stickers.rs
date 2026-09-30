use super::*;

fn set(app: &mut App, day: i32, sticker: &str, token: &str) -> Value {
    let result = app.call(
        "setSticker",
        vec![num(day), Value::str(sticker), Value::str(token)],
    );
    success(&result);
    result
}

fn at(app: &mut App, day: i32) -> String {
    let agenda = app.call("calendarDay", vec![num(day), num(0), num(TODAY)]);
    fields(&agenda)[4].as_str().unwrap().to_owned()
}

fn move_args(from: i32, to: i32, sticker: &str, token: &str) -> Vec<Value> {
    vec![num(from), num(to), Value::str(sticker), Value::str(token)]
}

fn pending_write(app: &mut App, source: &str, args: &[Value]) -> Answer {
    let lookup = app.data.answer(&mut app.store, source, args).unwrap();
    let outcome = run(app.work(lookup));
    app.data
        .parse(&mut app.store, source, args, outcome)
        .unwrap()
}

fn stored(root: &Root) -> Json {
    let reply = sql(
        root,
        false,
        json!([{"kind":"query","sql":"SELECT day,sticker_id FROM stickers ORDER BY day","params":[]}]),
    );
    json!(reply[0]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let day = row[0]["integer"].as_str().unwrap().parse::<i32>().unwrap();
            json!([day, row[1]])
        })
        .collect::<Vec<_>>())
}

#[test]
fn moving_replaces_one_destination_atomically_and_replays_without_undoing_later_edits() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.load();
    set(&mut app, TODAY, "coffee", "source");
    set(&mut app, TODAY + 27, "flower", "destination");
    let args = move_args(TODAY, TODAY + 27, "coffee", "move");
    let result = app.call("moveSticker", args.clone());
    success(&result);
    assert_eq!(fields(&result)[0], num(4));
    assert_eq!(fields(&result)[4], num(TODAY + 27));
    assert_eq!(at(&mut app, TODAY), "");
    assert_eq!(at(&mut app, TODAY + 27), "coffee");
    assert_eq!(stored(&root), json!([[TODAY + 27, "coffee"]]));

    let same_day = app.call(
        "moveSticker",
        move_args(TODAY + 27, TODAY + 27, "coffee", "same-day"),
    );
    success(&same_day);
    assert_eq!(
        fields(&same_day)[0],
        num(4),
        "same date changes no revision"
    );
    set(&mut app, TODAY + 27, "heart", "later-edit");
    let replayed = app.call("moveSticker", args);
    success(&replayed);
    assert_eq!(fields(&replayed)[0], num(5));
    assert_eq!(at(&mut app, TODAY + 27), "heart");
    drop(app);
    let mut reopened = App::open(&root);
    assert_eq!(reopened.load(), 5);
    assert_eq!(at(&mut reopened, TODAY), "");
    assert_eq!(at(&mut reopened, TODAY + 27), "heart");
}

#[test]
fn changed_or_removed_drag_sources_cannot_move_a_replacement_sticker() {
    let root = Root::new();
    let mut first = App::open(&root);
    first.load();
    set(&mut first, TODAY, "coffee", "source");
    set(&mut first, TODAY + 1, "flower", "destination");
    let mut other = App::open(&root);
    other.load();
    set(&mut other, TODAY, "heart", "replacement");
    let rejected = first.call(
        "moveSticker",
        move_args(TODAY, TODAY + 1, "coffee", "stale-drag"),
    );
    assert_eq!(fields(&rejected)[1], Value::Bool(false));
    assert_eq!(fields(&rejected)[0], num(4));
    assert_eq!(at(&mut first, TODAY), "heart");
    assert_eq!(at(&mut first, TODAY + 1), "flower");
    success(&other.call("removeSticker", vec![num(TODAY), Value::str("removed")]));
    let rejected = first.call(
        "moveSticker",
        move_args(TODAY, TODAY + 1, "heart", "removed-drag"),
    );
    assert_eq!(fields(&rejected)[1], Value::Bool(false));
    assert_eq!(fields(&rejected)[0], num(5));
    assert_eq!(stored(&root), json!([[TODAY + 1, "flower"]]));
}

#[test]
fn a_journal_conflict_rolls_back_both_sticker_rows_and_the_revision() {
    let root = Root::new();
    let mut first = App::open(&root);
    first.load();
    set(&mut first, TODAY, "coffee", "source");
    set(&mut first, TODAY + 1, "flower", "destination");
    let args = move_args(TODAY, TODAY + 1, "coffee", "claimed-token");
    let write = pending_write(&mut first, "moveSticker", &args);
    let mut other = App::open(&root);
    other.load();
    let no_op = other.call(
        "removeSticker",
        vec![num(TODAY + 2), Value::str("claimed-token")],
    );
    success(&no_op);
    assert_eq!(fields(&no_op)[0], num(3));
    let outcome = run(first.work(write));
    let recovery = first
        .data
        .parse(&mut first.store, "moveSticker", &args, outcome)
        .unwrap();
    let before = first.requests;
    let failed = first.finish("moveSticker", &args, recovery);
    assert_eq!(fields(&failed)[1], Value::Bool(false));
    assert_eq!(fields(&failed)[0], num(3));
    assert_eq!(
        first.requests - before,
        1,
        "only a journal read follows rollback"
    );
    assert_eq!(at(&mut first, TODAY), "coffee");
    assert_eq!(at(&mut first, TODAY + 1), "flower");
    assert_eq!(
        stored(&root),
        json!([[TODAY, "coffee"], [TODAY + 1, "flower"]])
    );
    let mut reopened = App::open(&root);
    assert_eq!(reopened.load(), 3);
}

#[test]
fn a_committed_sticker_move_with_a_lost_reply_recovers_one_durable_result() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.load();
    set(&mut app, TODAY, "coffee", "source");
    set(&mut app, TODAY + 1, "flower", "destination");
    let args = move_args(TODAY, TODAY + 1, "coffee", "lost-reply");
    let write = pending_write(&mut app, "moveSticker", &args);
    assert!(exact_data::storage::response(run(app.work(write))).is_ok());
    let recovery = app
        .data
        .parse(&mut app.store, "moveSticker", &args, storage_error())
        .unwrap();
    let recovered = app.finish("moveSticker", &args, recovery);
    success(&recovered);
    assert_eq!(fields(&recovered)[0], num(4));
    assert_eq!(at(&mut app, TODAY), "");
    assert_eq!(at(&mut app, TODAY + 1), "coffee");
    let replayed = app.call("moveSticker", args);
    success(&replayed);
    assert_eq!(fields(&replayed)[0], num(4));
    assert_eq!(stored(&root), json!([[TODAY + 1, "coffee"]]));
}

#[test]
fn an_uncommitted_write_error_is_not_retried_and_recovery_errors_terminate_once() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.load();
    set(&mut app, TODAY, "coffee", "source");
    for fail_recovery in [false, true] {
        let args = move_args(TODAY, TODAY + 1, "coffee", "uncommitted");
        let write = pending_write(&mut app, "moveSticker", &args);
        drop(app.work(write)); // The transaction never runs.
        let recovery = app
            .data
            .parse(&mut app.store, "moveSticker", &args, storage_error())
            .unwrap();
        let requests = app.requests;
        let failed = if fail_recovery {
            drop(app.work(recovery));
            let answer = app
                .data
                .parse(&mut app.store, "moveSticker", &args, storage_error())
                .unwrap();
            assert!(
                matches!(answer, Answer::Now(_)),
                "recovery errors never recurse"
            );
            app.finish("moveSticker", &args, answer)
        } else {
            app.finish("moveSticker", &args, recovery)
        };
        assert_eq!(fields(&failed)[1], Value::Bool(false));
        assert_eq!(fields(&failed)[0], num(2));
        assert_eq!(
            fields(&failed)[2].as_str(),
            Some("Simulated storage interruption.")
        );
        assert_eq!(app.requests - requests, 1);
        assert_eq!(at(&mut app, TODAY), "coffee");
        assert_eq!(at(&mut app, TODAY + 1), "");
        assert_eq!(stored(&root), json!([[TODAY, "coffee"]]));
    }
    let retried = app.call(
        "moveSticker",
        move_args(TODAY, TODAY + 1, "coffee", "uncommitted"),
    );
    success(&retried);
    assert_eq!(fields(&retried)[0], num(3));
}

fn height(page: &Value) -> f64 {
    list(&fields(page)[3])
        .iter()
        .map(|week| {
            let week = fields(week);
            (34.0 + week[3].as_number().unwrap() * 24.0 + week[4].as_number().unwrap()).max(76.0)
        })
        .sum()
}

fn sticker_box(page: &Value, day: i32) -> (f64, f64) {
    let mut top = 0.0;
    for week in list(&fields(page)[3]) {
        let week = fields(week);
        let row_height =
            (34.0 + week[3].as_number().unwrap() * 24.0 + week[4].as_number().unwrap()).max(76.0);
        if let Some(column) = list(&week[1])
            .iter()
            .position(|cell| fields(cell)[1] == num(day))
        {
            return ((column as f64 + 1.0) * 50.0 - 36.0, top + row_height - 36.0);
        }
        top += row_height;
    }
    panic!("sticker date missing");
}

#[test]
fn sticker_preview_predicts_footer_shrink_and_final_scroll_without_mutation_or_io() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.load();
    for index in 0..2 {
        let mut args = save_args("", "Dense source week", &format!("dense-{index}"));
        args[4] = Value::Bool(true);
        args[5] = Value::str("2026-09-01");
        args[6] = Value::str("2026-09-01");
        success(&app.call("saveSchedule", args));
    }
    set(&mut app, TODAY, "coffee", "source");
    set(&mut app, TODAY + 27, "flower", "destination");
    let before = app.call("calendarMonth", vec![num(24320), num(5)]);
    let old_scroll = height(&before) - 300.0;
    let wire = format!("sticker-day:{TODAY}:coffee");
    let requests = app.requests;
    let contact = app.call("calendarStickerDrag", vec![Value::str(&wire), num(5)]);
    assert_eq!(
        fields(&contact),
        &[Value::Bool(true), Value::str("coffee"), num(TODAY)]
    );
    let preview = app.call(
        "calendarLanding",
        vec![
            Value::str(&wire),
            num(TODAY + 27),
            num(24320),
            num(5),
            num(350),
            num(76),
        ],
    );
    assert_eq!(app.requests, requests);
    assert_eq!(app.call("calendarMonth", vec![num(24320), num(5)]), before);
    assert_eq!(at(&mut app, TODAY), "coffee");
    assert_eq!(at(&mut app, TODAY + 27), "flower");
    let preview = fields(&preview);
    assert_eq!(
        &preview[..3],
        &[
            Value::Bool(true),
            Value::str("coffee"),
            Value::str("sticker")
        ]
    );
    assert_eq!(
        &preview[5..9],
        &[num(32), num(32), Value::Bool(false), Value::Bool(false)]
    );
    let new_height = preview[9].as_number().unwrap();
    assert_eq!(height(&before) - new_height, 26.0);
    let final_scroll = old_scroll.min((new_height - 300.0).max(0.0));
    assert_eq!(old_scroll - final_scroll, 26.0);

    success(&app.call(
        "moveSticker",
        move_args(TODAY, TODAY + 27, "coffee", "move"),
    ));
    let after = app.call("calendarMonth", vec![num(24320), num(6)]);
    assert_eq!(height(&after), new_height);
    let (x, y) = sticker_box(&after, TODAY + 27);
    assert_eq!(preview[3], Value::Number(x));
    assert_eq!(preview[4], Value::Number(y));
    assert_eq!(
        preview[4].as_number().unwrap() - final_scroll,
        y - (height(&after) - 300.0)
    );
    let old_contact = app.call("calendarStickerDrag", vec![Value::str(&wire), num(6)]);
    assert_eq!(fields(&old_contact)[0], Value::Bool(false));
    let picker = app.call(
        "calendarStickerDrag",
        vec![Value::str("sticker-pick:cake"), num(6)],
    );
    assert_eq!(
        fields(&picker),
        &[Value::Bool(true), Value::str("cake"), num(-1_000_000)]
    );
    let invalid = app.call(
        "calendarStickerDrag",
        vec![Value::str("sticker-pick:missing"), num(6)],
    );
    assert_eq!(fields(&invalid)[0], Value::Bool(false));
}
