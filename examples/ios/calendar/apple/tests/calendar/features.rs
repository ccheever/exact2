//! Creation types, compact labels and per-day stickers through the real app.

use super::*;

fn todo(app: &App, title: &str) -> Value {
    items(app.runner.resource("todos").unwrap())
        .iter()
        .find(|item| fields(item)[1].as_str() == Some(title))
        .unwrap_or_else(|| panic!("missing Todo {title}"))
        .clone()
}

fn assert_no_success_notice(app: &App) {
    assert!(app
        .runner
        .kernel()
        .find_by_test_id("calendar-notice")
        .is_empty());
    assert!(app
        .runner
        .kernel()
        .find_by_test_id("popup-status")
        .is_empty());
}

fn save_sticker(app: &mut App, date: &str, sticker: &str) {
    app.create("sticker");
    app.input("sticker-date", date);
    app.press(&format!("sticker-{sticker}"));
    app.press("save-sticker");
    app.finish_motion();
    app.assert_creation_closed();
}

fn sticker_on(app: &mut App, day: i32) -> String {
    let revision = app.derived("revision").clone();
    let agenda = app
        .runner
        .data()
        .query(
            "calendarDay",
            &[
                Value::Number(f64::from(day)),
                revision,
                Value::Number(f64::from(TODAY)),
            ],
        )
        .unwrap();
    fields(&agenda)[4].as_str().unwrap().to_owned()
}

fn landing_frame(app: &App) -> Frame {
    let root = app.frame("calendar");
    Frame {
        x: root.x + app.state("ghostX").as_number().unwrap() as f32,
        y: root.y + app.state("ghostY").as_number().unwrap() as f32,
        width: app.state("landingWidth").as_number().unwrap() as f32,
        height: app.state("landingHeight").as_number().unwrap() as f32,
    }
}

fn assert_sticker_landed_at(app: &App, date: &str, expected: Frame) {
    let actual = app.frame(&format!("sticker-2026-09-{date}"));
    assert_eq!((expected.width, expected.height), (32.0, 32.0));
    for (name, actual, expected) in [
        ("x", actual.x, expected.x),
        ("y", actual.y, expected.y),
        ("width", actual.width, expected.width),
        ("height", actual.height, expected.height),
    ] {
        assert!(
            (actual - expected).abs() < 0.01,
            "sticker landing {name}: preview {expected}, committed {actual}"
        );
    }
}

#[test]
fn the_four_creation_types_form_a_two_by_two_picker_and_cancel_slides_out() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("add-schedule");
    let event = app.frame("picker-event");
    let plan = app.frame("picker-plan");
    let task = app.frame("picker-todo");
    let sticker = app.frame("picker-sticker");
    assert_eq!(event.y, plan.y);
    assert_eq!(task.y, sticker.y);
    assert!(event.x < plan.x && task.x < sticker.x && task.y > event.y);
    let picker = app.key("type-picker");
    app.press("cancel-type-picker");
    assert_eq!(app.key("type-picker"), picker);
    assert!(app.translation_y("type-picker") > 0.0);
    app.presented_frame(100.0);
    assert_eq!(app.key("type-picker"), picker);
    app.finish_motion();
    assert!(app
        .runner
        .kernel()
        .find_by_test_id("type-picker")
        .is_empty());
    assert_eq!(app.state("popupOpen"), &Value::Bool(false));
}

#[test]
fn the_pickup_keeps_one_contact_and_source_mounted_for_the_300_ms_morph() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-25");
    let id = app.agenda_id("Summer in Seoul");
    let source = app.key(&format!("agenda-item-{id}"));
    let input = app.key("calendar-input");
    let popup = app.key("date-popup");
    let contact = Contact::new(&app, &id, 1);
    app.event(
        "calendar-input",
        contact.event(1, "begin", contact.origin(), ""),
    );
    let background = app.key("calendar-drag-background");
    assert_eq!(app.state("liftPrepared"), &Value::Bool(false));
    let source_frame = app.frame("calendar-drag-background");
    assert_eq!(
        source_frame.width,
        app.frame(&format!("agenda-item-{id}")).width
    );
    assert_eq!(
        source_frame.height,
        app.frame(&format!("agenda-item-{id}")).height
    );
    let preparation_ms = app.state("liftReadyAt").as_number().unwrap() - app.runner.now_ms();
    assert!(preparation_ms > 0.0 && preparation_ms <= 100.0);
    app.presented_frame(preparation_ms + 1.0);
    assert_eq!(app.state("liftPrepared"), &Value::Bool(true));
    assert_eq!(
        app.runner
            .kernel()
            .node_by_key(background)
            .unwrap()
            .style
            .layout_transition
            .0[0]
            .duration,
        0.3
    );
    let chip_frame = app.frame("calendar-drag-background");
    assert_eq!((chip_frame.width, chip_frame.height), (220.0, 36.0));
    let point = app.center("date-2026-09-2026-09-19");
    app.event("calendar-input", contact.event(2, "move", point, ""));
    for elapsed in [100.0, 100.0, 99.0] {
        app.presented_frame(elapsed);
        assert_eq!(app.state("dragPhase").as_str(), Some("held"));
        assert_eq!(app.state("dragSerial"), &Value::Number(1.0));
        assert_eq!(app.state("contactAck"), &Value::Number(2.0));
        assert_eq!(app.key(&format!("agenda-item-{id}")), source);
        assert_eq!(app.key("calendar-input"), input);
        assert_eq!(app.key("date-popup"), popup);
        assert_eq!(app.key("calendar-drag-background"), background);
        assert!(app.frame("date-popup").y + app.translation_y("date-popup") >= HEIGHT);
    }
    app.event(
        "calendar-input",
        contact.event(3, "cancel", point, "interrupted"),
    );
    app.finish_motion();
    assert_eq!(app.derived("revision"), &Value::Number(1.0));
    assert_eq!(app.key("date-popup"), popup);
    assert_eq!(app.translation_y("date-popup"), 0.0);
}

#[test]
fn cancelling_an_editor_keeps_it_mounted_until_the_slide_out_finishes() {
    for kind in ["event", "plan", "todo", "sticker"] {
        let root = Root::new();
        let mut app = App::open(&root);
        app.create(kind);
        app.finish_motion();
        let sheet = match kind {
            "todo" => "todo-editor",
            "sticker" => "sticker-editor",
            _ => "schedule-editor",
        };
        let key = app.key(sheet);
        app.press(if kind == "sticker" {
            "cancel-sticker"
        } else {
            "cancel-editor"
        });
        assert_eq!(app.key(sheet), key);
        assert!(app.translation_y(sheet) > 0.0);
        app.presented_frame(100.0);
        assert_eq!(app.key(sheet), key);
        app.finish_motion();
        assert!(app.runner.kernel().find_by_test_id(sheet).is_empty());
        assert_eq!(app.state("popupOpen"), &Value::Bool(false));
        assert_eq!(app.derived("revision"), &Value::Number(1.0));
    }
}

#[test]
fn closing_the_todo_list_keeps_it_mounted_until_the_slide_out_finishes() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("todos-button");
    app.finish_motion();
    let key = app.key("todo-sheet");
    app.press("close-todos");
    assert_eq!(app.key("todo-sheet"), key);
    assert!(app.translation_y("todo-sheet") > 0.0);
    app.presented_frame(100.0);
    assert_eq!(app.key("todo-sheet"), key);
    app.finish_motion();
    assert!(app.runner.kernel().find_by_test_id("todo-sheet").is_empty());
    assert_eq!(app.derived("revision"), &Value::Number(1.0));
}

#[test]
fn a_plan_created_from_global_plus_stays_a_plan_and_does_not_open_the_date_sheet() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.create("plan");
    app.input("schedule-title", "A saved plan");
    app.input("schedule-notes", "Plan notes survive restart.");
    app.press("color-Mint");
    app.input("start-date", "2026-09-22");
    app.input("end-date", "2026-09-24");
    app.press("save-schedule");
    app.finish_motion();
    assert_eq!(app.state("editorOpen"), &Value::Bool(false));
    assert_eq!(app.state("popupOpen"), &Value::Bool(false));
    app.assert_creation_closed();
    assert!(app.runner.kernel().find_by_test_id("date-popup").is_empty());
    assert_no_success_notice(&app);
    app.press("date-2026-09-2026-09-22");
    let id = app.agenda_id("A saved plan");
    let plan = items(&fields(app.runner.resource("agenda").unwrap())[3])
        .iter()
        .find(|item| fields(item)[0].as_str() == Some(&id))
        .unwrap();
    assert_eq!(fields(plan)[12].as_str(), Some("plan"));
    assert_no_success_notice(&app);
    drop(app);

    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-22");
    assert_eq!(app.agenda_id("A saved plan"), id);
    app.press(&format!("agenda-item-{id}"));
    assert_eq!(
        app.derived("notes").as_str(),
        Some("Plan notes survive restart.")
    );
    assert_eq!(app.derived("color").as_str(), Some("#70B8A2"));
    assert_eq!(app.derived("startDate").as_str(), Some("2026-09-22"));
    assert_eq!(app.derived("endDate").as_str(), Some("2026-09-24"));
    app.input("schedule-title", "Updated plan");
    app.press("save-schedule");
    app.finish_motion();
    drop(app);

    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-22");
    assert_eq!(app.agenda_id("Updated plan"), id);
    let plan = items(&fields(app.runner.resource("agenda").unwrap())[3])
        .iter()
        .find(|item| fields(item)[0].as_str() == Some(&id))
        .unwrap();
    assert_eq!(
        fields(plan)[12].as_str(),
        Some("plan"),
        "editing a Plan must keep its type"
    );
}

#[test]
fn undated_todos_persist_completion_cancel_without_writes_and_convert_once_on_drop() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.create("todo");
    assert!(!app
        .runner
        .kernel()
        .find_by_test_id("todo-editor")
        .is_empty());
    assert!(app.runner.kernel().find_by_test_id("start-date").is_empty());
    assert!(app.runner.kernel().find_by_test_id("end-date").is_empty());
    app.input("schedule-title", "Initial undated task");
    app.press("color-Blue");
    app.press("save-schedule");
    app.finish_motion();
    assert_eq!(app.state("popupOpen"), &Value::Bool(false));
    app.assert_creation_closed();
    let created = todo(&app, "Initial undated task");
    let id = fields(&created)[0].as_str().unwrap().to_owned();
    assert_eq!(fields(&created)[2].as_str(), Some("#747AFF"));
    assert_eq!(fields(&created)[3], Value::Bool(false));
    app.press("todos-button");
    app.press(&format!("todo-item-{id}"));
    assert!(!app
        .runner
        .kernel()
        .find_by_test_id("todo-editor")
        .is_empty());
    assert!(app.runner.kernel().find_by_test_id("start-date").is_empty());
    app.input("schedule-title", "Undated task");
    app.press("save-schedule");
    app.finish_motion();
    assert_eq!(
        fields(&todo(&app, "Undated task"))[0].as_str(),
        Some(id.as_str())
    );
    app.press(&format!("todo-check-{id}"));
    let complete = todo(&app, "Undated task");
    assert_eq!(fields(&complete)[3], Value::Bool(true));
    assert!(fields(&complete)[5].as_number().unwrap() >= fields(&complete)[4].as_number().unwrap());
    drop(app);

    let mut app = App::open(&root);
    assert_eq!(fields(&todo(&app, "Undated task"))[3], Value::Bool(true));
    app.press("todos-button");
    app.press(&format!("todo-check-{id}"));
    let wire_id = format!("todo:{id}");
    let contact = Contact::from_source(&app, &wire_id, 1, &format!("todo-item-{id}"));
    let revision = app.derived("revision").clone();
    let original_sheet = app.key("todo-sheet");
    app.event(
        "calendar-input",
        contact.event(1, "begin", contact.origin(), ""),
    );
    let cancel = app.center("cancel-zone");
    app.event("calendar-input", contact.event(2, "end", cancel, ""));
    app.finish_motion();
    assert_eq!(app.derived("revision"), &revision);
    assert_eq!(app.key("todo-sheet"), original_sheet);
    assert_eq!(app.translation_y("todo-sheet"), 0.0);
    assert_eq!(fields(&todo(&app, "Undated task"))[3], Value::Bool(false));

    let contact = Contact::from_source(&app, &wire_id, 2, &format!("todo-item-{id}"));
    app.event(
        "calendar-input",
        contact.event(3, "begin", contact.origin(), ""),
    );
    let destination = app.center("date-2026-09-2026-09-20");
    app.event("calendar-input", contact.event(4, "end", destination, ""));
    app.event("calendar-input", contact.event(4, "end", destination, ""));
    app.finish_motion();
    assert_eq!(
        app.derived("revision").as_number().unwrap(),
        revision.as_number().unwrap() + 1.0
    );
    assert!(app.runner.kernel().find_by_test_id("todo-sheet").is_empty());
    assert!(app.runner.kernel().find_by_test_id("date-popup").is_empty());
    assert!(items(app.runner.resource("todos").unwrap())
        .iter()
        .all(|item| fields(item)[0].as_str() != Some(&id)));
    assert_no_success_notice(&app);
    drop(app);

    let mut app = App::open(&root);
    assert!(items(app.runner.resource("todos").unwrap())
        .iter()
        .all(|item| fields(item)[0].as_str() != Some(&id)));
    app.press("date-2026-09-2026-09-20");
    let converted: Vec<_> = items(&fields(app.runner.resource("agenda").unwrap())[3])
        .iter()
        .filter(|item| fields(item)[1].as_str() == Some("Undated task"))
        .collect();
    assert_eq!(converted.len(), 1);
    assert_eq!(fields(converted[0])[3].as_str(), Some("#747AFF"));
    assert_eq!(fields(converted[0])[7].as_str(), Some("2026-09-20"));
    assert_eq!(fields(converted[0])[8].as_str(), Some("2026-09-20"));
    assert_eq!(fields(converted[0])[12].as_str(), Some("event"));
}

#[test]
fn todo_recovery_and_failed_write_retry_keep_one_original_timestamp() {
    for committed in [false, true] {
        let root = Root::new();
        let mut app = App::open(&root);
        app.create("todo");
        app.input("schedule-title", "Todo reply lost");
        app.press("color-Mint");
        let original_epoch = app.derived("saveEpoch").clone();
        if committed {
            app.lose_write_reply_after = Some(2);
        } else {
            app.fail_write_reply_after = Some(2);
        }
        app.press("save-schedule");
        app.finish_motion();
        assert!(app.lose_write_reply_after.is_none() && app.fail_write_reply_after.is_none());
        if !committed {
            assert_eq!(app.state("editorOpen"), &Value::Bool(true));
            assert!(!app.state("error").as_str().unwrap().is_empty());
            assert_eq!(app.derived("title").as_str(), Some("Todo reply lost"));
            assert_eq!(app.state("retryEpoch"), &original_epoch);
            app.presented_frame(1_000.0);

            app.press("save-schedule");
            app.finish_motion();
        }
        app.assert_creation_closed();
        assert_eq!(app.derived("revision"), &Value::Number(2.0));
        assert!(app.state("error").as_str().unwrap().is_empty());
        let saved = todo(&app, "Todo reply lost");
        let id = fields(&saved)[0].clone();
        assert_eq!(fields(&saved)[4], original_epoch);
        assert_eq!(fields(&saved)[2].as_str(), Some("#70B8A2"));
        drop(app);

        let app = App::open(&root);
        let matching: Vec<_> = items(app.runner.resource("todos").unwrap())
            .iter()
            .filter(|item| fields(item)[1].as_str() == Some("Todo reply lost"))
            .collect();
        assert_eq!(matching.len(), 1);
        assert_eq!(fields(matching[0])[0], id);
        assert_eq!(fields(matching[0])[4], original_epoch);
    }
}

#[test]
fn one_sticker_per_day_can_be_chosen_replaced_and_removed_across_restarts() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.create("sticker");
    app.input("sticker-date", "2026-09-22");
    for id in [
        "sunshine", "coffee", "cake", "heart", "sparkle", "flower", "book", "workout", "travel",
        "rest",
    ] {
        assert_eq!(
            app.runner
                .kernel()
                .find_by_test_id(&format!("sticker-{id}"))
                .len(),
            1
        );
    }
    app.press("sticker-coffee");
    app.press("save-sticker");
    app.finish_motion();
    assert_eq!(app.state("popupOpen"), &Value::Bool(false));
    app.assert_creation_closed();
    app.press("date-2026-09-2026-09-22");
    assert_eq!(
        fields(app.runner.resource("agenda").unwrap())[4].as_str(),
        Some("coffee")
    );
    let cell = app.frame("date-2026-09-2026-09-22");
    let sticker = app.frame("sticker-2026-09-2026-09-22");
    assert_eq!((sticker.width, sticker.height), (32.0, 32.0));
    assert!((cell.x + cell.width - sticker.x - sticker.width - 4.0).abs() < 0.01);
    assert!((cell.y + cell.height - sticker.y - sticker.height - 4.0).abs() < 0.01);
    drop(app);

    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-22");
    assert_eq!(
        fields(app.runner.resource("agenda").unwrap())[4].as_str(),
        Some("coffee")
    );
    app.press("edit-sticker");
    app.press("sticker-flower");
    app.press("save-sticker");
    app.finish_motion();
    assert_eq!(
        fields(app.runner.resource("agenda").unwrap())[4].as_str(),
        Some("flower")
    );
    assert_eq!(
        app.runner
            .kernel()
            .find_by_test_id("sticker-2026-09-2026-09-22")
            .len(),
        1
    );
    drop(app);

    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-22");
    assert_eq!(
        fields(app.runner.resource("agenda").unwrap())[4].as_str(),
        Some("flower")
    );
    app.press("edit-sticker");
    app.press("remove-sticker");
    app.finish_motion();
    assert!(app
        .runner
        .kernel()
        .find_by_test_id("sticker-2026-09-2026-09-22")
        .is_empty());
    drop(app);

    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-22");
    assert_eq!(
        fields(app.runner.resource("agenda").unwrap())[4].as_str(),
        Some("")
    );
    assert!(app
        .runner
        .kernel()
        .find_by_test_id("sticker-2026-09-2026-09-22")
        .is_empty());
}

#[test]
fn a_picker_sticker_drops_on_the_final_date_once_and_lands_on_its_image() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.create("sticker");
    let picker = app.key("sticker-editor");
    let source = app.key("sticker-pick-coffee");
    let input = app.key("calendar-input");
    let contact = Contact::from_source(&app, "sticker-pick:coffee", 1, "sticker-pick-coffee");
    app.event(
        "calendar-input",
        contact.event(1, "begin", contact.origin(), ""),
    );
    assert_eq!(app.state("dragPhase").as_str(), Some("held"));
    assert_eq!(app.state("dragKind").as_str(), Some("sticker"));
    assert_eq!(app.state("dragSource").as_str(), Some("sticker-picker"));
    assert_eq!(app.key("sticker-editor"), picker);
    assert_eq!(app.key("sticker-pick-coffee"), source);
    assert!(app.frame("sticker-editor").y + app.translation_y("sticker-editor") >= HEIGHT);
    let ghost = app.key("calendar-drag-sticker");
    app.presented_frame(20.0);
    let previous = app.center("date-2026-09-2026-09-19");
    app.event("calendar-input", contact.event(2, "move", previous, ""));
    // The temporary held-only scroll room brings the last week's date center
    // above the cancel strip, then disappears when the drop lands.
    let unscrolled = app.center("date-2026-09-2026-09-30");
    let cancel_top = f64::from(app.frame("cancel-zone").y);
    let edge = (unscrolled.0, cancel_top - 8.0);
    app.event("calendar-input", contact.event(3, "move", edge, ""));
    for _ in 0..120 {
        app.presented_frame(1000.0 / 120.0);
        let requested = app.state("calendarScrollRequest").as_number().unwrap();
        app.event("month-scroll-2026-09", Event::Scroll(0.0, requested));
        if unscrolled.1 - requested < cancel_top - 16.0 {
            break;
        }
    }
    let scrolled = app.state("calendarScrollTop").as_number().unwrap();
    assert!(scrolled > 0.0);
    let destination = (unscrolled.0, unscrolled.1 - scrolled);
    assert!(destination.1 < cancel_top - 16.0);
    app.event("calendar-input", contact.event(4, "end", destination, ""));
    assert_eq!(app.state("dragPhase").as_str(), Some("landing"));
    assert_eq!(app.derived("revision"), &Value::Number(1.0));
    assert_eq!(app.key("calendar-drag-sticker"), ghost);
    let landing = landing_frame(&app);
    let ghost_frame = app.frame("calendar-drag-sticker");
    let scale = app.runner.kernel().node_by_key(ghost).unwrap().style.scale;
    assert!((ghost_frame.width * scale - 32.0).abs() < 0.01);
    assert!((ghost_frame.height * scale - 32.0).abs() < 0.01);
    app.event("calendar-input", contact.event(4, "end", destination, ""));
    app.event("calendar-input", contact.event(5, "end", destination, ""));
    assert_eq!(app.state("calendarScrollRequest"), &Value::Number(0.0));
    app.event("month-scroll-2026-09", Event::Scroll(0.0, 0.0));
    app.finish_motion();
    app.assert_creation_closed();
    assert_no_success_notice(&app);
    assert_eq!(app.key("calendar-input"), input);
    assert_eq!(app.derived("revision"), &Value::Number(2.0));
    assert_eq!(sticker_on(&mut app, TODAY + 29), "coffee");
    assert_eq!(sticker_on(&mut app, TODAY + 18), "");
    assert_sticker_landed_at(&app, "2026-09-30", landing);
    drop(app);

    let mut app = App::open(&root);
    assert_eq!(app.derived("revision"), &Value::Number(2.0));
    assert_eq!(sticker_on(&mut app, TODAY + 29), "coffee");
    assert_eq!(sticker_on(&mut app, TODAY + 18), "");
}

#[test]
fn dragging_a_cell_sticker_atomically_clears_its_source_and_replaces_the_target() {
    let root = Root::new();
    let mut app = App::open(&root);
    save_sticker(&mut app, "2026-09-20", "coffee");
    save_sticker(&mut app, "2026-09-22", "flower");
    let revision = app.derived("revision").as_number().unwrap();
    let source_id = "sticker-cell-2026-09-2026-09-20";
    let source = app.key(source_id);
    let contact = Contact::from_source(
        &app,
        &format!("sticker-day:{}:coffee", TODAY + 19),
        1,
        source_id,
    );
    app.event(
        "calendar-input",
        contact.event(1, "begin", contact.origin(), ""),
    );
    assert_eq!(app.state("dragSource").as_str(), Some("sticker-cell"));
    let destination = app.center("date-2026-09-2026-09-22");
    app.event("calendar-input", contact.event(2, "end", destination, ""));
    assert_eq!(app.state("dragPhase").as_str(), Some("landing"));
    assert_eq!(app.derived("revision").as_number(), Some(revision));
    assert_eq!(app.key(source_id), source);
    let landing = landing_frame(&app);
    app.event("calendar-input", contact.event(2, "end", destination, ""));
    app.finish_motion();
    app.assert_creation_closed();
    assert_eq!(app.derived("revision").as_number(), Some(revision + 1.0));
    assert_eq!(sticker_on(&mut app, TODAY + 19), "");
    assert_eq!(sticker_on(&mut app, TODAY + 21), "coffee");
    assert!(app.runner.kernel().find_by_test_id(source_id).is_empty());
    assert_eq!(
        app.runner
            .kernel()
            .find_by_test_id("sticker-2026-09-2026-09-22")
            .len(),
        1
    );
    assert_sticker_landed_at(&app, "2026-09-22", landing);
    drop(app);

    let mut app = App::open(&root);
    assert_eq!(app.derived("revision").as_number(), Some(revision + 1.0));
    assert_eq!(sticker_on(&mut app, TODAY + 19), "");
    assert_eq!(sticker_on(&mut app, TODAY + 21), "coffee");
}

#[test]
fn a_sticker_dragged_from_the_date_sheet_moves_and_keeps_the_sheet_closed() {
    let root = Root::new();
    let mut app = App::open(&root);
    // Force the target week above its minimum height so adding its first
    // sticker grows the footer and changes the actual landing position.
    for n in 0..4 {
        app.create("event");
        app.input("schedule-title", &format!("Sticker target lane {n}"));
        app.input("start-date", "2026-09-18");
        app.input("end-date", "2026-09-18");
        app.press("save-schedule");
        app.finish_motion();
    }
    save_sticker(&mut app, "2026-09-22", "coffee");
    let revision = app.derived("revision").as_number().unwrap();
    let original_target = app.frame("date-2026-09-2026-09-18");
    app.press("date-2026-09-2026-09-22");
    let sheet = app.key("date-popup");
    let source_id = "sticker-agenda-2026-09-22";
    let source = app.key(source_id);
    let contact = Contact::from_source(
        &app,
        &format!("sticker-day:{}:coffee", TODAY + 21),
        1,
        source_id,
    );
    app.event(
        "calendar-input",
        contact.event(1, "begin", contact.origin(), ""),
    );
    assert_eq!(app.state("dragSource").as_str(), Some("sticker-date"));
    assert_eq!(app.key("date-popup"), sheet);
    assert_eq!(app.key(source_id), source);
    assert!(app.frame("date-popup").y + app.translation_y("date-popup") >= HEIGHT);
    let destination = app.center("date-2026-09-2026-09-18");
    app.event("calendar-input", contact.event(2, "end", destination, ""));
    assert_eq!(app.state("dragPhase").as_str(), Some("landing"));
    let landing = landing_frame(&app);
    app.finish_motion();
    app.assert_creation_closed();
    assert_eq!(app.derived("revision").as_number(), Some(revision + 1.0));
    assert_eq!(sticker_on(&mut app, TODAY + 21), "");
    assert_eq!(sticker_on(&mut app, TODAY + 17), "coffee");
    assert_eq!(
        app.frame("date-2026-09-2026-09-18").height,
        original_target.height + 26.0
    );
    assert_sticker_landed_at(&app, "2026-09-18", landing);
}

#[test]
fn cancelled_sticker_drags_restore_their_source_without_writing_storage() {
    for (origin, ending) in [
        ("picker", "zone"),
        ("date", "zone"),
        ("cell", "zone"),
        ("cell", "same-day"),
        ("picker", "cancel"),
        ("cell", "outside"),
    ] {
        let root = Root::new();
        let mut app = App::open(&root);
        let (wire_id, source_id, sheet_id) = if origin == "picker" {
            app.create("sticker");
            app.input("sticker-date", "2026-09-22");
            app.press("sticker-coffee");
            (
                "sticker-pick:coffee".to_owned(),
                "sticker-pick-coffee",
                Some("sticker-editor"),
            )
        } else {
            save_sticker(&mut app, "2026-09-22", "coffee");
            if origin == "date" {
                app.press("date-2026-09-2026-09-22");
            }
            (
                format!("sticker-day:{}:coffee", TODAY + 21),
                if origin == "date" {
                    "sticker-agenda-2026-09-22"
                } else {
                    "sticker-cell-2026-09-2026-09-22"
                },
                (origin == "date").then_some("date-popup"),
            )
        };
        let revision = app.derived("revision").clone();
        let source = app.key(source_id);
        let sheet = sheet_id.map(|id| app.key(id));
        let contact = Contact::from_source(&app, &wire_id, 1, source_id);
        app.event(
            "calendar-input",
            contact.event(1, "begin", contact.origin(), ""),
        );
        assert_eq!(app.state("dragPhase").as_str(), Some("held"));
        let point = match ending {
            "zone" => app.center("cancel-zone"),
            "outside" => (-10.0, 180.0),
            _ => app.center("date-2026-09-2026-09-22"),
        };
        let (phase, reason) = if ending == "cancel" {
            ("cancel", "interrupted")
        } else {
            ("end", "")
        };
        app.event("calendar-input", contact.event(2, phase, point, reason));
        assert_eq!(app.state("dragPhase").as_str(), Some("returning"));
        app.finish_motion();
        assert_eq!(app.derived("revision"), &revision, "{origin}/{ending}");
        assert_eq!(app.key(source_id), source, "{origin}/{ending}");
        if let Some(sheet_id) = sheet_id {
            assert_eq!(Some(app.key(sheet_id)), sheet);
            assert_eq!(app.translation_y(sheet_id), 0.0);
        } else {
            app.assert_creation_closed();
        }
        if origin == "picker" {
            assert_eq!(app.derived("stickerSelection").as_str(), Some("coffee"));
        }
        drop(app);

        let mut app = App::open(&root);
        assert_eq!(app.derived("revision"), &revision, "{origin}/{ending}");
        assert_eq!(
            sticker_on(&mut app, TODAY + 21),
            if origin == "picker" { "" } else { "coffee" },
            "{origin}/{ending}"
        );
    }
}

#[test]
fn a_committed_sticker_move_with_a_lost_reply_and_replayed_end_is_atomic_once() {
    let root = Root::new();
    let mut app = App::open(&root);
    save_sticker(&mut app, "2026-09-20", "coffee");
    save_sticker(&mut app, "2026-09-22", "flower");
    let revision = app.derived("revision").as_number().unwrap();
    let operation = app.state("operationSequence").as_number().unwrap();
    let contact = Contact::from_source(
        &app,
        &format!("sticker-day:{}:coffee", TODAY + 19),
        1,
        "sticker-cell-2026-09-2026-09-20",
    );
    app.event(
        "calendar-input",
        contact.event(1, "begin", contact.origin(), ""),
    );
    let destination = app.center("date-2026-09-2026-09-22");
    app.lose_write_reply_after = Some(2);
    app.event("calendar-input", contact.event(2, "end", destination, ""));
    assert!(app.lose_write_reply_after.is_none());
    assert_eq!(app.state("dragPhase").as_str(), Some("landing"));
    assert!(app.state("error").as_str().unwrap().is_empty());
    app.event("calendar-input", contact.event(2, "end", destination, ""));
    app.event("calendar-input", contact.event(3, "end", destination, ""));
    app.finish_motion();
    assert_eq!(
        app.state("operationSequence").as_number(),
        Some(operation + 1.0)
    );
    assert_eq!(app.derived("revision").as_number(), Some(revision + 1.0));
    app.assert_creation_closed();
    assert_eq!(app.state("dragId").as_str(), Some(""));
    drop(app);

    let mut app = App::open(&root);
    assert_eq!(app.derived("revision").as_number(), Some(revision + 1.0));
    assert_eq!(sticker_on(&mut app, TODAY + 19), "");
    assert_eq!(sticker_on(&mut app, TODAY + 21), "coffee");
}

#[test]
fn date_and_page_labels_only_include_a_different_year() {
    let root = Root::new();
    let mut app = App::open(&root);
    assert_eq!(app.text("month-label"), "Sep");
    assert!(app.runner.kernel().find_by_test_id("month-year").is_empty());
    app.press("date-2026-09-2026-09-25");
    assert_eq!(app.text("popup-date"), "Fri, Sep 25");
    app.press("close-popup");
    app.finish_motion();
    for _ in 0..13 {
        app.press("previous-month");
        app.finish_motion();
    }
    app.finish_motion();
    assert_eq!(app.text("month-label"), "Aug, 2025");
    app.press("date-2025-08-2025-08-31");
    assert_eq!(app.text("popup-date"), "Sun, Aug 31, 2025");
}
