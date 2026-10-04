use super::*;

pub(super) fn without_starter_stickers(root: &Root) -> App {
    let mut app = App::open(root);
    assert_eq!(app.load(), 1);
    drop(app);
    sql(
        root,
        true,
        json!([{"kind":"execute","sql":"DELETE FROM stickers","params":[]} ]),
    );
    let mut app = App::open(root);
    assert_eq!(app.load(), 1);
    app
}

#[test]
fn themes_survive_restart_without_changing_schedules_or_stickers() {
    let root = Root::new();
    let mut app = without_starter_stickers(&root);
    assert_eq!(app.call("calendarTheme", vec![num(1)]), num(0));
    set(&mut app, TODAY, "bunny", "new-sticker");
    let selected = app.call("setTheme", vec![num(3), Value::str("meadow-theme")]);
    success(&selected);
    assert_eq!(app.call("calendarTheme", vec![num(3)]), num(3));
    let invalid = app.call("setTheme", vec![num(7), Value::str("invalid-theme")]);
    assert_eq!(fields(&invalid)[1], Value::Bool(false));
    assert_eq!(at(&mut app, TODAY), "bunny");
    drop(app);
    let mut reopened = App::open(&root);
    assert_eq!(reopened.load(), 3);
    assert_eq!(reopened.call("calendarTheme", vec![num(3)]), num(3));
    assert_eq!(at(&mut reopened, TODAY), "bunny");
    success(&reopened.call("setTheme", vec![num(0), Value::str("default-theme")]));
    drop(reopened);
    let mut restored = App::open(&root);
    assert_eq!(restored.load(), 4);
    assert_eq!(restored.call("calendarTheme", vec![num(4)]), num(0));
    success(&restored.call("setTheme", vec![num(6), Value::str("san-francisco-theme")]));
    assert_eq!(restored.call("calendarTheme", vec![num(5)]), num(6));
    drop(restored);
    let mut golden = App::open(&root);
    assert_eq!(golden.load(), 5);
    assert_eq!(golden.call("calendarTheme", vec![num(5)]), num(6));
    assert_eq!(at(&mut golden, TODAY), "bunny");
}

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
    let mut app = without_starter_stickers(&root);
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
    let mut first = without_starter_stickers(&root);
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
    let mut first = without_starter_stickers(&root);
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
    let mut app = without_starter_stickers(&root);
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
    let mut app = without_starter_stickers(&root);
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
    drop(without_starter_stickers(&root));
    // Seed content must not supply lanes to this geometry fixture. Keep the
    // initialized database and revision, then rebuild the index from our rows.
    sql(
        &root,
        true,
        json!([{"kind":"execute","sql":"DELETE FROM schedules","params":[]}]),
    );
    let mut app = without_starter_stickers(&root);
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

fn version_two_samples() -> Json {
    serde_json::from_str(r##"[
        ["sample-2026-09-0","Flight to San Francisco","Check in online and leave time for the airport.","#747AFF",0,20704,20704,600,1080,"event",0],
        ["sample-2026-09-1","Ferry Building","Coffee, the farmers market, and a walk along the Embarcadero.","#F39B65",0,20706,20706,600,720,"event",0],
        ["sample-2026-09-2","Book Alcatraz tickets","Reserve the morning ferry for September 18.","#70B8A2",1,20699,20699,0,0,"todo",1],
        ["sample-2026-09-3","Pack for San Francisco","Layers for the fog, comfortable shoes, and a travel adapter.","#EE8192",1,20702,20702,0,0,"todo",0],
        ["sample-2026-09-4","Dinner in North Beach","A table for two after exploring the neighborhood.","#F39B65",0,20709,20709,1140,1230,"event",0],
        ["sample-2026-09-5","Golden Gate walk","Start at Crissy Field and walk toward the bridge.","#70B8A2",0,20712,20712,600,720,"event",0],
        ["sample-2026-09-6","San Francisco","Time to explore San Francisco, from arrival through the flight home.","#70B8A2",1,20704,20716,0,0,"plan",0],
        ["sample-2026-09-7","Download boarding pass","Keep an offline copy for the flight.","#747AFF",1,20703,20703,0,0,"todo",1],
        ["sample-2026-09-8","Alcatraz morning","Ferry from Pier 33, then lunch by the waterfront.","#747AFF",0,20714,20714,540,720,"event",0],
        ["sample-2026-09-9","Choose Japan hotels","Compare stays near the station in Tokyo and Kyoto.","#EE8192",1,20725,20725,0,0,"todo",0],
        ["sample-2026-09-10","Travel journal","A few quiet evenings to write down favorite places and sort photos.","#F39B65",1,20718,20721,0,0,"plan",0],
        ["sample-2026-09-11","Photo evening","Share favorite San Francisco photos over dinner.","#747AFF",0,20723,20723,1080,1200,"event",0],
        ["sample-2026-10-0","Japan trip","Tokyo, Kyoto, and a final evening in Osaka.","#EE8192",1,20736,20748,0,0,"plan",0],
        ["sample-2026-10-1","Flight to Tokyo","Passport, boarding pass, and the hotel address are ready.","#747AFF",0,20736,20736,540,900,"event",0],
        ["sample-2026-10-2","Reserve Kyoto stay","Booked a small hotel near Kyoto Station.","#EE8192",1,20728,20728,0,0,"todo",1],
        ["sample-2026-10-3","Set up Japan eSIM","Install before departure and enable data on arrival.","#70B8A2",1,20734,20734,0,0,"todo",0],
        ["sample-2026-10-4","Tokyo neighborhoods","Asakusa, quiet coffee stops, and an evening in Shinjuku.","#747AFF",1,20737,20741,0,0,"plan",0],
        ["sample-2026-10-5","Shinkansen to Kyoto","Pick up breakfast before boarding.","#F39B65",0,20742,20742,540,675,"event",0],
        ["sample-2026-10-6","Kyoto stay","An early start at Fushimi Inari, then a slow walk through Higashiyama.","#70B8A2",1,20742,20746,0,0,"plan",0],
        ["sample-2026-10-7","Reserve teamLab tickets","The afternoon entry time is confirmed.","#747AFF",1,20731,20731,0,0,"todo",1],
        ["sample-2026-10-8","Osaka food walk","Try takoyaki and explore Dotonbori after sunset.","#F39B65",0,20747,20747,1020,1200,"event",0],
        ["sample-2026-10-9","Send Japan postcards","Find stamps and write a note home.","#EE8192",1,20744,20744,0,0,"todo",0],
        ["sample-2026-10-10","Japan trip review","Share the best moments and save favorite places for next time.","#70B8A2",0,20752,20752,1080,1140,"event",0],
        ["sample-2026-10-11","Print favorite photos","Choose a small set for the travel album.","#F39B65",1,20756,20756,0,0,"todo",0]
    ]"##).unwrap()
}

#[test]
fn version_two_samples_upgrade_once_without_overwriting_edits_deletions_or_journal() {
    let root = Root::new();
    let fingerprint = legacy_database(&root);
    let mut commands = vec![
        json!({"kind":"execute","sql":"ALTER TABLE schedules ADD COLUMN kind TEXT NOT NULL DEFAULT 'event'","params":[]}),
        json!({"kind":"execute","sql":"ALTER TABLE schedules ADD COLUMN completed INTEGER NOT NULL DEFAULT 0","params":[]}),
        json!({"kind":"execute","sql":"CREATE TABLE stickers(day INTEGER PRIMARY KEY NOT NULL,sticker_id TEXT NOT NULL)","params":[]}),
        json!({"kind":"execute","sql":"PRAGMA user_version=2","params":[]}),
    ];
    for row in version_two_samples().as_array().unwrap() {
        commands.push(json!({"kind":"execute","sql":"INSERT INTO schedules VALUES(?,?,?,?,?,?,?,?,?,?,?)","params":row}));
    }
    commands.extend([
        json!({"kind":"execute","sql":"UPDATE schedules SET notes='My flight notes' WHERE id='sample-2026-09-0'","params":[]}),
        json!({"kind":"execute","sql":"UPDATE schedules SET completed=0 WHERE id='sample-2026-09-2'","params":[]}),
        json!({"kind":"execute","sql":"DELETE FROM schedules WHERE id='sample-2026-09-3'","params":[]})
    ]);
    sql(&root, true, Json::Array(commands));
    let mut app = App::open(&root);
    assert_eq!(app.load(), 13);
    let edited = app.event("sample-2026-09-0");
    assert_eq!(fields(&edited)[1].as_str(), Some("Flight to San Francisco"));
    assert_eq!(fields(&edited)[2].as_str(), Some("My flight notes"));
    assert_eq!(
        fields(&edited)[5],
        fields(&edited)[6],
        "a user's single-day edit remains valid"
    );
    let reopened_todo = app.event("sample-2026-09-2");
    assert_eq!(
        fields(&reopened_todo)[1].as_str(),
        Some("Book Alcatraz tickets")
    );
    assert_eq!(fields(&reopened_todo)[13], Value::Bool(false));
    assert_eq!(fields(&reopened_todo)[5], fields(&reopened_todo)[6]);
    assert_eq!(
        fields(&app.call(
            "calendarEvent",
            vec![Value::str("sample-2026-09-3"), num(13)]
        ))[0],
        Value::Bool(false)
    );
    let trip = app.event("sample-2026-09-6");
    assert_eq!(fields(&trip)[8].as_str(), Some("2026-09-19"));
    let rows = sql(
        &root,
        false,
        json!([
            {"kind":"query","sql":"SELECT id,title,notes,color,all_day,start_day,end_day,start_minute,end_minute,kind,completed FROM schedules ORDER BY id","params":[]},
            {"kind":"query","sql":"SELECT fingerprint,revision FROM calendar_operations WHERE token='legacy-create'","params":[]},
            {"kind":"query","sql":"PRAGMA user_version","params":[]}
        ]),
    );
    let stored = rows[0]["rows"].as_array().unwrap();
    assert_eq!(
        stored.len(),
        48,
        "47 starter rows plus the untouched user schedule"
    );
    for row in stored.iter().filter(|row| {
        row[0].as_str().unwrap().starts_with("sample-")
            && row[0] != "sample-2026-09-0"
            && row[0] != "sample-2026-09-2"
    }) {
        let start = row[5]["integer"].as_str().unwrap().parse::<i32>().unwrap();
        let end = row[6]["integer"].as_str().unwrap().parse::<i32>().unwrap();
        if row[9] == "todo" {
            assert_eq!(end, start);
        } else {
            assert!(
                end > start,
                "unchanged Event/Plan was not refreshed: {}",
                row[0]
            );
        }
    }
    assert_eq!(rows[1]["rows"][0][0], fingerprint);
    assert_eq!(rows[1]["rows"][0][1]["integer"], "12");
    assert_eq!(rows[2]["rows"][0][0]["integer"], "5");
    assert_eq!(
        fields(&app.event("event-legacy-create"))[1].as_str(),
        Some("Legacy schedule")
    );
    drop(app);
    let mut reopened = App::open(&root);
    assert_eq!(reopened.load(), 13);
    let after = sql(
        &root,
        false,
        json!([{"kind":"query","sql":"SELECT id,title,notes,color,all_day,start_day,end_day,start_minute,end_minute,kind,completed FROM schedules ORDER BY id","params":[]}]),
    );
    assert_eq!(after[0], rows[0]);
}

fn version_four_samples() -> Json {
    serde_json::from_str(r##"[
        ["sample-2026-09-0","Fly to SF","An overnight flight, with time to settle in on arrival.","#747AFF",0,20704,20705,1380,900,"event",0],
        ["sample-2026-09-1","Ferry market","Two days of market stops, coffee, and waterfront exploring.","#F39B65",1,20706,20707,0,0,"event",0],
        ["sample-2026-09-2","Book tickets","Reserve the morning ferry for September 18.","#70B8A2",1,20699,20700,0,0,"todo",1],
        ["sample-2026-09-3","Pack bags","Layers for the fog, comfortable shoes, and a travel adapter.","#EE8192",1,20702,20703,0,0,"todo",0],
        ["sample-2026-09-4","Food tour","A two-day neighborhood food tour, with a table booked each evening.","#F39B65",1,20709,20710,0,0,"event",0],
        ["sample-2026-09-5","Coast hike","An overnight coastal hike, with a slow morning by the water.","#70B8A2",1,20712,20713,0,0,"event",0],
        ["sample-2026-09-6","San Francisco","Time to explore San Francisco, from arrival through the flight home.","#70B8A2",1,20704,20715,0,0,"plan",0],
        ["sample-2026-09-7","Check in","Keep an offline copy for the flight.","#747AFF",1,20703,20704,0,0,"todo",1],
        ["sample-2026-09-8","Photo walk","A two-day photography walk through favorite San Francisco neighborhoods.","#747AFF",1,20714,20715,0,0,"event",0],
        ["sample-2026-09-9","Book hotels","Compare stays near the station in Tokyo and Kyoto.","#EE8192",1,20725,20726,0,0,"todo",0],
        ["sample-2026-09-10","Travel journal","A few quiet evenings to write down favorite places and sort photos.","#F39B65",1,20718,20721,0,0,"plan",0],
        ["sample-2026-09-11","Photo class","A weekend photography class, using pictures from the trip.","#747AFF",1,20723,20724,0,0,"event",0],
        ["sample-2026-10-0","Japan trip","Tokyo, Kyoto, and a final evening in Osaka.","#EE8192",1,20735,20748,0,0,"plan",0],
        ["sample-2026-10-1","Fly to Tokyo","An overnight flight, with time to settle in on arrival.","#747AFF",0,20735,20736,1380,900,"event",0],
        ["sample-2026-10-2","Kyoto hotel","Booked a small hotel near Kyoto Station.","#EE8192",1,20728,20729,0,0,"todo",1],
        ["sample-2026-10-3","Set up eSIM","Install before departure and enable data on arrival.","#70B8A2",1,20734,20735,0,0,"todo",0],
        ["sample-2026-10-4","Tokyo stay","Asakusa, quiet coffee stops, and an evening in Shinjuku.","#747AFF",1,20737,20741,0,0,"plan",0],
        ["sample-2026-10-5","Kyoto arrival","The train to Kyoto, hotel check-in, and a first day exploring.","#F39B65",1,20742,20743,0,0,"event",0],
        ["sample-2026-10-6","Kyoto stay","An early start at Fushimi Inari, then a slow walk through Higashiyama.","#70B8A2",1,20742,20746,0,0,"plan",0],
        ["sample-2026-10-7","Book teamLab","The afternoon entry time is confirmed.","#747AFF",1,20731,20732,0,0,"todo",1],
        ["sample-2026-10-8","Osaka tour","Two days tasting local favorites and exploring Osaka.","#F39B65",1,20747,20748,0,0,"event",0],
        ["sample-2026-10-9","Postcards","Find stamps and write a note home.","#EE8192",1,20744,20745,0,0,"todo",0],
        ["sample-2026-10-10","Travel class","A two-day travel workshop to turn favorite places into the next itinerary.","#70B8A2",1,20752,20753,0,0,"event",0],
        ["sample-2026-10-11","Print photos","Choose a small set for the travel album.","#F39B65",1,20756,20757,0,0,"todo",0]
    ]"##).unwrap()
}

#[test]
fn version_four_upgrade_collapses_only_todo_dates_and_preserves_user_fields_and_stickers() {
    let root = Root::new();
    let fingerprint = legacy_database(&root);
    let mut commands = vec![
        json!({"kind":"execute","sql":"ALTER TABLE schedules ADD COLUMN kind TEXT NOT NULL DEFAULT 'event'","params":[]}),
        json!({"kind":"execute","sql":"ALTER TABLE schedules ADD COLUMN completed INTEGER NOT NULL DEFAULT 0","params":[]}),
        json!({"kind":"execute","sql":"CREATE TABLE stickers(day INTEGER PRIMARY KEY NOT NULL,sticker_id TEXT NOT NULL)","params":[]}),
        json!({"kind":"execute","sql":"PRAGMA user_version=4","params":[]}),
    ];
    for row in version_four_samples().as_array().unwrap() {
        commands.push(json!({"kind":"execute","sql":"INSERT INTO schedules VALUES(?,?,?,?,?,?,?,?,?,?,?)","params":row}));
    }
    commands.extend([
        json!({"kind":"execute","sql":"UPDATE schedules SET title='My travel checklist',notes='Keep this note',completed=0 WHERE id='sample-2026-09-2'","params":[]}),
        json!({"kind":"execute","sql":"DELETE FROM schedules WHERE id='sample-2026-09-3'","params":[]}),
        json!({"kind":"execute","sql":"INSERT INTO schedules VALUES(?,?,?,?,?,?,?,?,?,?,?),(?,?,?,?,?,?,?,?,?,?,?)","params":["todo-user-range","Call the hotel","Private hotel notes","#EE8192",0,TODAY+9,TODAY+14,600,660,"todo",1,"todo-user-overnight","Night transfer","Keep the overnight clocks","#F39B65",0,TODAY+2,TODAY+3,1380,900,"todo",1]}),
        json!({"kind":"execute","sql":"INSERT INTO stickers VALUES(?,?)","params":[TODAY+7,"book"]})
    ]);
    sql(&root, true, Json::Array(commands));
    let mut app = App::open(&root);
    assert_eq!(app.load(), 13);
    let edited = app.event("sample-2026-09-2");
    assert_eq!(fields(&edited)[1].as_str(), Some("My travel checklist"));
    assert_eq!(fields(&edited)[2].as_str(), Some("Keep this note"));
    assert_eq!(fields(&edited)[13], Value::Bool(false));
    for (id, title, notes, color, start, start_time, end_time) in [
        (
            "todo-user-range",
            "Call the hotel",
            "Private hotel notes",
            "#EE8192",
            TODAY + 9,
            "10:00",
            "11:00",
        ),
        (
            "todo-user-overnight",
            "Night transfer",
            "Keep the overnight clocks",
            "#F39B65",
            TODAY + 2,
            "23:00",
            "15:00",
        ),
    ] {
        let event = app.event(id);
        assert_eq!(fields(&event)[1].as_str(), Some(title));
        assert_eq!(fields(&event)[2].as_str(), Some(notes));
        assert_eq!(fields(&event)[3].as_str(), Some(color));
        assert_eq!(fields(&event)[4], Value::Bool(false));
        assert_eq!(&fields(&event)[5..7], &[num(start), num(start)]);
        assert_eq!(fields(&event)[9].as_str(), Some(start_time));
        assert_eq!(fields(&event)[10].as_str(), Some(end_time));
        assert_eq!(fields(&event)[13], Value::Bool(true));
    }
    assert_eq!(
        fields(&app.call(
            "calendarEvent",
            vec![Value::str("sample-2026-09-3"), num(13)]
        ))[0],
        Value::Bool(false)
    );
    assert_eq!(
        fields(&app.event("event-legacy-create"))[8].as_str(),
        Some("2026-09-30"),
        "user Event range is unchanged"
    );
    assert_eq!(
        at(&mut app, TODAY + 7),
        "book",
        "starter art cannot replace a user's sticker"
    );
    let rows = sql(
        &root,
        false,
        json!([
            {"kind":"query","sql":"SELECT * FROM schedules ORDER BY id","params":[]},
            {"kind":"query","sql":"SELECT * FROM calendar_operations ORDER BY token","params":[]},
            {"kind":"query","sql":"SELECT day,sticker_id FROM stickers ORDER BY day","params":[]},
            {"kind":"query","sql":"PRAGMA user_version","params":[]}
        ]),
    );
    assert_eq!(rows[0]["rows"].as_array().unwrap().len(), 50);
    for row in rows[0]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row[9] == "todo")
    {
        assert_eq!(row[5], row[6]);
    }
    assert_eq!(rows[1]["rows"][0][1], fingerprint);
    assert_eq!(rows[1]["rows"][0][2]["integer"], "12");
    assert_eq!(rows[2]["rows"].as_array().unwrap().len(), 12);
    assert_eq!(rows[3]["rows"][0][0]["integer"], "5");
    drop(app);
    let mut reopened = App::open(&root);
    assert_eq!(reopened.load(), 13);
    let after = sql(
        &root,
        false,
        json!([
            {"kind":"query","sql":"SELECT * FROM schedules ORDER BY id","params":[]},
            {"kind":"query","sql":"SELECT * FROM calendar_operations ORDER BY token","params":[]},
            {"kind":"query","sql":"SELECT day,sticker_id FROM stickers ORDER BY day","params":[]},
            {"kind":"query","sql":"PRAGMA user_version","params":[]}
        ]),
    );
    assert_eq!(after, rows);
}
