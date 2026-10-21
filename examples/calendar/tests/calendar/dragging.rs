use super::*;

#[test]
fn an_inspector_row_drops_on_a_date_once_and_the_inspector_follows_it() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-16");
    let id = app.agenda_id("San Francisco");
    let contact = Contact::new(&app, &id, 1);
    app.dispatch(
        "calendar-input",
        contact.event(1, "begin", contact.origin(), ""),
    );
    assert_eq!(
        app.state("contactAck"),
        &Value::Number(0.0),
        "the module must wait for then"
    );
    app.settle();
    assert_eq!(app.state("contactAck"), &Value::Number(1.0));
    assert_eq!(app.state("dragPhase").as_str(), Some("held"));
    assert_eq!(app.state("dragSource").as_str(), Some("row"));
    assert_eq!(app.state("dragId").as_str(), Some(id.as_str()));
    assert_eq!(app.state("dragSpan"), &Value::Number(11.0));
    assert_eq!(
        app.text("inspector-date"),
        "Wed, Sep 16",
        "no hint replaces the date"
    );
    assert!(app.mounted("calendar-drag-title"));
    assert!(
        app.mounted(&format!("agenda-item-{id}")),
        "the source stays mounted while it is carried"
    );
    // The ghost begins exactly on the row: the trip is a plan, so its line
    // starts on the row's arrow mark and its title on the row's title.
    let row = app.frame(&format!("agenda-item-{id}"));
    let mark = app.frame(&format!("row-mark-{id}"));
    let heading = app.frame(&format!("row-title-{id}"));
    let close = |a: f32, b: f32| (a - b).abs() < 0.01;
    assert!(close(
        app.state("markX").as_number().unwrap() as f32,
        mark.x - row.x
    ));
    assert!(close(
        app.state("titleY").as_number().unwrap() as f32,
        heading.y - row.y
    ));
    let line = app.frame("calendar-drag-plan-line");
    assert!(
        close(line.x, mark.x - row.x + 2.0) && close(line.width, 16.0),
        "{line:?}"
    );
    let title = app.frame("calendar-drag-title");
    assert!(close(title.x, heading.x - row.x) && close(title.y, heading.y - row.y));
    app.finish_motion();
    // Lifted: the arrow has stretched into a 194 px line above the title, with
    // the chip's left edge 20 px before the pointer.
    let line = app.frame("calendar-drag-plan-line");
    let origin = contact.origin();
    // The chip keeps inside the window: 200 px plus an 8 px margin.
    let chip_x = (origin.0 - 20.0).min(f64::from(WIDTH) - 208.0)
        - f64::from(row.x - app.frame("calendar").x);
    assert!(
        (f64::from(line.x) - (chip_x + 3.0)).abs() < 0.01,
        "{line:?}"
    );
    assert!(close(line.width, 194.0));
    assert!(app.frame("calendar-drag-title").y > line.y);

    let target = app.center("date-2026-09-2026-09-20");
    app.event("calendar-input", contact.event(2, "move", target, ""));
    assert_eq!(app.derived("hoverDay"), &day(19));
    app.event("calendar-input", contact.event(3, "end", target, ""));
    assert_eq!(app.state("dragPhase").as_str(), Some("landing"));
    app.event("calendar-input", contact.event(3, "end", target, ""));
    app.event("calendar-input", contact.event(4, "end", target, ""));
    app.finish_motion();
    assert_eq!(
        app.derived("revision"),
        &Value::Number(2.0),
        "terminal packets cannot replay storage"
    );
    assert_eq!(app.state("dragId").as_str(), Some(""));
    assert_eq!(app.derived("day"), &day(19), "the drop date is selected");
    assert!(app.agenda_ids().contains(&id));
    assert!(!app.mounted("calendar-drag-title"));
    drop(app);

    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-20");
    app.press(&format!("agenda-item-{id}"));
    app.finish_motion();
    assert_eq!(app.derived("startDate").as_str(), Some("2026-09-20"));
    assert_eq!(app.derived("endDate").as_str(), Some("2026-10-01"));
}

#[test]
fn month_bars_and_stickers_only_select_their_date_and_never_start_a_drag() {
    let root = Root::new();
    let mut app = App::open(&root);
    let id = "sample-2026-09-6";
    // The trip's first bar has no handle: the date button above it takes the click.
    assert!(!app.mounted(&bar_handle(&app, "2026-09", id)));
    assert!(!app.mounted("sticker-cell-2026-09-2026-09-14"));
    assert!(app.mounted("sticker-2026-09-2026-09-14"));
    let bar = app.frame("bar-2026-09-sample-2026-09-6-20702");
    let button = app.frame("date-2026-09-2026-09-09");
    assert!(
        button.x <= bar.x + 130.0
            && bar.y >= button.y
            && bar.y + bar.height <= button.y + button.height
    );
    app.press("date-2026-09-2026-09-09");
    assert_eq!(app.derived("day"), &day(8));
    assert_eq!(app.state("editorOpen"), &Value::Bool(false));
    // A packet naming the bar is not an inspector row's: nothing lifts.
    let contact = Contact::from_source(&app, &format!("bar:{id}"), 1, "date-2026-09-2026-09-09");
    app.event(
        "calendar-input",
        contact.event(1, "begin", contact.origin(), ""),
    );
    assert_eq!(app.state("dragPhase").as_str(), Some("idle"));
}

#[test]
fn cancelled_same_date_and_off_grid_drops_write_nothing() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-16");
    let id = app.agenda_id("San Francisco");
    let revision = app.derived("revision").clone();

    let first = Contact::new(&app, &id, 1);
    let near = app.center("date-2026-09-2026-09-18");
    app.event(
        "calendar-input",
        first.event(1, "begin", first.origin(), ""),
    );
    app.event("calendar-input", first.event(2, "move", near, ""));
    app.event(
        "calendar-input",
        first.event(3, "cancel", near, "cancelled"),
    );
    assert_eq!(app.state("dragPhase").as_str(), Some("returning"));
    app.finish_motion();
    assert_eq!(app.state("dragId").as_str(), Some(""));

    let same = Contact::new(&app, &id, 2);
    // The trip starts on September 8; a drop there is no move.
    let start = app.center("date-2026-09-2026-09-08");
    same.drag_to(&mut app, 4, start);
    assert_eq!(app.state("dragPhase").as_str(), Some("returning"));
    app.finish_motion();

    let outside = Contact::new(&app, &id, 3);
    let inspector = app.frame("inspector");
    let over = (
        f64::from(inspector.x + inspector.width / 2.0),
        f64::from(inspector.y + inspector.height / 2.0),
    );
    outside.drag_to(&mut app, 7, over);
    assert_eq!(app.state("dragPhase").as_str(), Some("returning"));
    app.finish_motion();
    assert_eq!(app.state("dragId").as_str(), Some(""));
    assert_eq!(app.derived("revision"), &revision);
    assert_eq!(app.derived("day"), &day(15));
    assert!(app.agenda_ids().contains(&id));
}

#[test]
fn the_palette_toggles_and_its_stickers_and_the_inspector_row_drag_onto_dates() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-10");
    assert_eq!(app.sticker(), "");
    app.press("sticker-pick-cake");
    assert_eq!(app.sticker(), "cake");
    assert!(app.mounted("sticker-2026-09-2026-09-10"));
    assert!(app.mounted("sticker-row"));
    assert!(
        !app.mounted("remove-sticker"),
        "no remove button: the palette toggles"
    );
    app.press("sticker-pick-cake");
    assert_eq!(
        app.sticker(),
        "",
        "the selected sticker comes off when clicked again"
    );
    app.press("sticker-pick-cake");
    app.press("sticker-pick-heart");
    assert_eq!(app.sticker(), "heart", "another sticker replaces it");

    let palette = Contact::from_source(&app, "sticker-pick:bunny", 1, "sticker-pick-bunny");
    let to = app.center("date-2026-09-2026-09-12");
    app.event(
        "calendar-input",
        palette.event(1, "begin", palette.origin(), ""),
    );
    assert_eq!(app.state("dragSource").as_str(), Some("sticker-picker"));
    app.event("calendar-input", palette.event(2, "move", to, ""));
    app.event("calendar-input", palette.event(3, "end", to, ""));
    app.finish_motion();
    assert_eq!(app.derived("day"), &day(11));
    assert_eq!(app.sticker(), "bunny");

    // The inspector's sticker row moves it.
    let row = Contact::from_source(
        &app,
        &format!("sticker-day:{}:bunny", TODAY + 11),
        2,
        "sticker-agenda-2026-09-12",
    );
    let to = app.center("date-2026-09-2026-09-20");
    app.event("calendar-input", row.event(4, "begin", row.origin(), ""));
    assert_eq!(app.state("dragSource").as_str(), Some("sticker-date"));
    app.event("calendar-input", row.event(5, "move", to, ""));
    app.event("calendar-input", row.event(6, "end", to, ""));
    app.finish_motion();
    assert_eq!(app.derived("day"), &day(19));
    assert_eq!(app.sticker(), "bunny");
    assert!(!app.mounted("sticker-2026-09-2026-09-12"));

    // Hovering the row shows its delete button; it removes the sticker at once.
    assert!(!app.mounted("delete-sticker"));
    app.event("sticker-row", Event::Hover(true));
    app.press("delete-sticker");
    assert_eq!(app.sticker(), "");
}

#[test]
fn a_closed_palette_refuses_its_stickers_and_the_sticker_row_reopens_it() {
    let root = Root::new();
    let mut app = App::open(&root);
    let palette = Contact::from_source(&app, "sticker-pick:moon", 1, "sticker-pick-moon");
    app.press("sticker-palette-toggle");
    assert_eq!(app.state("stickerPaletteOpen"), &Value::Bool(false));
    assert!(!app.mounted("sticker-pick-moon"));
    app.event(
        "calendar-input",
        palette.event(1, "begin", palette.origin(), ""),
    );
    assert_eq!(app.state("dragPhase").as_str(), Some("idle"));
    assert!(!app.state("error").as_str().unwrap().is_empty());
    // September 2 holds a starter sticker; its row opens the palette.
    app.press("date-2026-09-2026-09-02");
    assert_eq!(app.sticker(), "coffee");
    app.press("sticker-agenda-2026-09-02");
    assert_eq!(app.state("stickerPaletteOpen"), &Value::Bool(true));
}

#[test]
fn holding_at_the_month_edge_pages_once_and_the_drop_lands_in_the_new_month() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-16");
    let id = app.agenda_id("San Francisco");
    let contact = Contact::new(&app, &id, 1);
    app.event(
        "calendar-input",
        contact.event(1, "begin", contact.origin(), ""),
    );
    let stage = app.frame("calendar-stage");
    let edge = (
        f64::from(stage.x + stage.width - 8.0),
        f64::from(stage.y + 200.0),
    );
    app.event("calendar-input", contact.event(2, "move", edge, ""));
    for _ in 0..45 {
        app.presented_frame(1000.0 / 60.0);
    }
    assert_eq!(app.derived("month"), &Value::Number(SEPTEMBER + 1.0));
    let to = app.center("date-2026-10-2026-10-05");
    app.event("calendar-input", contact.event(3, "move", to, ""));
    for _ in 0..30 {
        app.presented_frame(1000.0 / 60.0);
    }
    let to = app.center("date-2026-10-2026-10-05");
    app.event("calendar-input", contact.event(4, "move", to, ""));
    assert_eq!(app.derived("hoverDay"), &day(34));
    app.event("calendar-input", contact.event(5, "end", to, ""));
    app.finish_motion();
    assert_eq!(app.derived("month"), &Value::Number(SEPTEMBER + 1.0));
    assert_eq!(app.derived("day"), &day(34));
    assert!(app.agenda_ids().contains(&id));
}

#[test]
fn crossing_the_layout_breakpoint_cancels_a_held_drag_without_writing() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-16");
    let id = app.agenda_id("San Francisco");
    let contact = Contact::new(&app, &id, 1);
    app.event(
        "calendar-input",
        contact.event(1, "begin", contact.origin(), ""),
    );
    assert_eq!(app.state("dragPhase").as_str(), Some("held"));
    app.resize(402.0, 874.0);
    assert_eq!(app.state("dragPhase").as_str(), Some("idle"));
    assert_eq!(app.state("dragId").as_str(), Some(""));
    let target = app.center("date-2026-09-2026-09-20");
    app.event("calendar-input", contact.event(2, "end", target, ""));
    app.finish_motion();
    assert_eq!(app.derived("revision"), &Value::Number(1.0));
    assert_eq!(app.derived("day"), &day(15));
}
