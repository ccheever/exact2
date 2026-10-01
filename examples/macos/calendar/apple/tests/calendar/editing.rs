use super::*;

#[test]
fn new_opens_a_centered_dialog_that_switches_kinds_and_saves_on_enter() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-25");
    app.press("new-item");
    assert_eq!(app.state("editorOpen"), &Value::Bool(true));
    assert_eq!(app.text("editor-title"), "New Event");
    assert_eq!(app.derived("startDate").as_str(), Some("2026-09-25"));
    let dialog = app.frame("schedule-editor");
    assert_eq!(dialog.width, 480.0);
    assert!(
        (dialog.x + dialog.width / 2.0 - WIDTH / 2.0).abs() < 0.5,
        "the dialog is centered in the window"
    );
    assert!(dialog.y > 0.0 && dialog.y + dialog.height < HEIGHT);

    app.press("kind-todo");
    assert!(app.mounted("todo-editor"));
    assert!(!app.mounted("end-date"));
    assert_eq!(app.text("editor-title"), "New Todo");
    app.press("kind-plan");
    assert_eq!(app.state("editorKind").as_str(), Some("plan"));
    app.input("schedule-title", "Offsite");
    app.input("end-date", "2026-09-27");
    app.event("schedule-title", Event::Submit);
    app.finish_motion();
    assert_eq!(app.state("editorOpen"), &Value::Bool(false));
    let id = app.agenda_id("Offsite");
    let _ = bar_handle(&app, "2026-09", &id);
    drop(app);

    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-26");
    app.press(&format!("agenda-item-{id}"));
    app.finish_motion();
    assert_eq!(app.text("editor-title"), "Edit Plan");
    assert!(!app.mounted("kind-tabs"), "an item keeps its kind");
    assert_eq!(app.derived("endDate").as_str(), Some("2026-09-27"));
}

#[test]
fn escape_and_cancel_close_without_writing_and_the_backdrop_keeps_the_dialog() {
    let root = Root::new();
    let mut app = App::open(&root);
    let revision = app.derived("revision").clone();
    app.press("date-2026-09-2026-09-21");
    app.event("date-2026-09-2026-09-21", Event::Dblclick);
    assert_eq!(app.state("editorOpen"), &Value::Bool(true));
    assert_eq!(app.derived("startDate").as_str(), Some("2026-09-21"));
    assert_eq!(app.derived("day"), &day(20));
    app.input("schedule-title", "Unsaved");
    app.press("editor-backdrop");
    app.press("schedule-editor");
    assert_eq!(app.state("editorOpen"), &Value::Bool(true));
    assert_eq!(app.derived("title").as_str(), Some("Unsaved"));
    app.key_press("schedule-title", "Enter");
    app.key_press("schedule-title", "Escape");
    assert_eq!(app.state("editorClosing"), &Value::Bool(true));
    app.finish_motion();
    assert_eq!(app.state("editorOpen"), &Value::Bool(false));
    assert!(!app.mounted("editor-backdrop"));

    app.event("date-2026-09-2026-09-21", Event::Dblclick);
    app.press("cancel-editor");
    app.finish_motion();
    assert_eq!(app.state("editorOpen"), &Value::Bool(false));
    assert_eq!(app.derived("revision"), &revision);
}

#[test]
fn the_context_menu_edits_toggles_and_asks_before_deleting() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-01");
    let id = app.agenda_id("Check passport");
    let completed = |app: &App| {
        items(&app.agenda()[3])
            .iter()
            .find(|item| fields(item)[0].as_str() == Some(&*id))
            .map(|item| fields(item)[13].clone())
            .unwrap()
    };
    let before = completed(&app);
    app.event(
        &format!("agenda-item-{id}"),
        Event::Message("toggle".into()),
    );
    assert_ne!(completed(&app), before);
    app.press(&format!("todo-check-{id}"));
    assert_eq!(completed(&app), before);

    app.event(&format!("agenda-item-{id}"), Event::Message("edit".into()));
    assert_eq!(app.state("editorOpen"), &Value::Bool(true));
    assert_eq!(app.state("editId").as_str(), Some(&*id));
    assert_eq!(app.text("editor-title"), "Edit Todo");
    assert_eq!(app.state("confirmDelete"), &Value::Bool(false));
    app.finish_motion();
    assert_eq!(
        app.state("titleFocusPending"),
        &Value::Bool(false),
        "the title is focused once its notes arrive"
    );
    app.press("cancel-editor");
    app.finish_motion();

    app.event(
        &format!("agenda-item-{id}"),
        Event::Message("delete".into()),
    );
    assert_eq!(app.state("editorOpen"), &Value::Bool(true));
    assert_eq!(app.state("confirmDelete"), &Value::Bool(true));
    assert!(app.mounted("confirm-delete"));
    assert!(!app.mounted("cancel-editor"), "one Escape button at a time");
    app.press("keep-schedule");
    assert_eq!(app.state("confirmDelete"), &Value::Bool(false));
    assert!(app.mounted("cancel-editor"));
    app.finish_motion();
    app.press("delete-schedule");
    app.press("confirm-delete");
    app.finish_motion();
    assert_eq!(app.state("editorOpen"), &Value::Bool(false));
    assert!(!app.agenda_ids().contains(&id));
    drop(app);

    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-01");
    assert!(!app.agenda_ids().contains(&id));
}

#[test]
fn a_failed_save_keeps_the_draft_and_its_retry_writes_once() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-21");
    app.press("new-item");
    app.input("schedule-title", "Retry me");
    app.fail_write_reply_after = Some(1);
    app.press("save-schedule");
    app.finish_motion();
    assert_eq!(app.state("editorOpen"), &Value::Bool(true));
    assert!(app.mounted("editor-error"));
    assert_eq!(app.derived("title").as_str(), Some("Retry me"));
    let token = app.state("retryToken").clone();
    assert_ne!(token, Value::str(""));
    app.press("save-schedule");
    app.finish_motion();
    assert_eq!(app.state("editorOpen"), &Value::Bool(false));
    let saved = items(&app.agenda()[3])
        .iter()
        .filter(|item| fields(item)[1].as_str() == Some("Retry me"))
        .count();
    assert_eq!(saved, 1);
}

#[test]
fn the_theme_panel_hangs_under_its_button_and_the_choice_survives_restart() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("theme-button");
    assert_eq!(app.state("themeOpen"), &Value::Bool(true));
    let panel = app.frame("theme-popup");
    let button = app.frame("theme-button");
    assert!(
        (panel.x + panel.width - (button.x + button.width)).abs() < 0.5,
        "the panel's right edge meets its button's"
    );
    assert!(panel.y > button.y + button.height);
    app.press("theme-popup");
    assert_eq!(
        app.state("themeOpen"),
        &Value::Bool(true),
        "its own surface keeps it"
    );
    app.press("theme-backdrop");
    app.finish_motion();
    assert!(!app.mounted("theme-popup"));

    app.press("theme-button");
    app.press("theme-3");
    app.finish_motion();
    assert!(!app.mounted("theme-popup"));
    assert_eq!(app.runner.resource("theme").unwrap(), &Value::Number(3.0));
    assert!(app.mounted("theme-wallpaper"));
    drop(app);

    let mut app = App::open(&root);
    assert_eq!(app.runner.resource("theme").unwrap(), &Value::Number(3.0));
    app.press("theme-button");
    app.press("close-themes");
    app.finish_motion();
    assert!(!app.mounted("theme-popup"));
}

#[test]
fn the_san_francisco_theme_is_offered_and_survives_restart() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("theme-button");
    assert!(app.mounted("theme-6"));
    app.press("theme-6");
    app.finish_motion();
    assert!(!app.mounted("theme-popup"));
    assert_eq!(app.runner.resource("theme").unwrap(), &Value::Number(6.0));
    assert!(app.mounted("theme-wallpaper"));
    drop(app);

    let app = App::open(&root);
    assert_eq!(app.runner.resource("theme").unwrap(), &Value::Number(6.0));
    assert!(app.mounted("theme-wallpaper"));
}

#[test]
fn a_hovered_row_shows_a_delete_button_that_deletes_at_once() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-01");
    let id = app.agenda_id("City workshop");
    let delete = format!("delete-{id}");
    assert!(!app.mounted(&delete), "the button shows only on hover");
    app.event(&format!("agenda-row-{id}"), Event::Hover(true));
    assert!(app.mounted(&delete));
    let button = app.frame(&delete);
    let row = app.frame(&format!("agenda-row-{id}"));
    assert!(button.x + button.width <= row.x + row.width && button.x > row.x + row.width / 2.0);
    app.press(&delete);
    assert_eq!(
        app.state("editorOpen"),
        &Value::Bool(false),
        "no dialog asks first"
    );
    assert!(!app.agenda_ids().contains(&id));
    drop(app);

    let app = App::open(&root);
    assert!(!app.agenda_ids().contains(&id));
}

#[test]
fn a_trackpad_swipe_over_the_month_turns_one_page_unless_a_dialog_is_open() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.event("calendar-input", Event::Message("swipe:next".into()));
    assert_eq!(app.derived("month"), &Value::Number(SEPTEMBER + 1.0));
    assert_eq!(app.text("month-label"), "October");
    app.event("calendar-input", Event::Message("swipe:previous".into()));
    app.event("calendar-input", Event::Message("swipe:previous".into()));
    assert_eq!(app.text("month-label"), "August");
    app.press("new-item");
    app.event("calendar-input", Event::Message("swipe:next".into()));
    assert_eq!(
        app.text("month-label"),
        "August",
        "a swipe behind the dialog turns nothing"
    );
}
