//! Inline mode as a person sees it: the host's frames written into a
//! terminal emulator and read back from its screen (LLP 1101.002 §0 P1).
//! Each test is a bug the harness found by hand.

use exact_terminal::host::{Host, Key, Mode};
use exact_terminal::vt::Vt;

/// A log the app can retire once printed, a line under it, a button that
/// opens a dialog of five choices in a list three rows tall, one that opens
/// a dialog with a field, and an action that starts a new transcript.
const APP: &str = r#"component App
  state picked = "none"
  state picks = 0
  state epoch = 0
  state retired = false
  state draft = ""
  state saved = ""
  action pick(v: string)
    picked = v
    picks = picks + 1
  action retire
    retired = true
  action clear
    epoch = epoch + 1
    retired = false
  action write(value: string)
    draft = value
  action save
    saved = draft
  view
    column
      each e in [epoch] key=e
        column role="log"
          when not retired
            text `banner ${e}` id=`banner-${e}`
            text "line one" id=`a-${e}`
            text "line two" id=`b-${e}`
      text `picked ${picked} ×${picks}`
      button commandfor="d" command="show-modal" testId="open"
        text "open"
      button commandfor="k" command="show-modal" testId="openk"
        text "key"
      text `saved [${saved}]`
      dialog id="k" closedby="any" testId="key-dialog"
        input value=draft input=write submit=save autofocus=true appearance="none" placeholder="type" testId="field"
      dialog id="d" closedby="any" testId="dialog"
        scroll max-height="3lh" testId="list"
          each v in ["alpha", "beta", "gamma", "delta", "epsilon"] key=v
            button press=pick(v) commandfor="d" command="close" text-align="start" testId=v
              text v
"#;

fn boot(cols: usize, rows: usize) -> (Host<()>, Vt) {
    let plan = contract::compile(APP).expect("compiles");
    let mut host = Host::boot(plan, (), Mode::Inline, cols, rows).expect("boots");
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    (host, vt)
}

/// One read's keys, against what the screen showed when it was read.
fn read(host: &mut Host<()>, vt: &mut Vt, keys: &[Key]) {
    host.read_at = host.frames;
    for k in keys {
        host.key(k.clone());
    }
    vt.render(host);
    assert_eq!(vt.unanswered(), 0, "a position query went unanswered");
}

fn press(host: &mut Host<()>, vt: &mut Vt, id: &str) {
    host.read_at = host.frames;
    let v = host.by_test_id(id).expect(id);
    assert!(
        host.on_screen(v),
        "#{id} is not on the screen:\n{}",
        vt.text(false)
    );
    host.press(v);
    vt.render(host);
}

/// The row and column of text on the visible screen.
fn find(vt: &Vt, text: &str) -> (i32, i32) {
    let screen = vt.text(false);
    let (y, line) = screen
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains(text))
        .unwrap_or_else(|| panic!("{text:?} is not on the screen:\n{screen}"));
    (line.find(text).unwrap() as i32, y as i32)
}

#[test]
fn the_wheel_in_a_dialog_list_moves_the_list() {
    let (mut host, mut vt) = boot(30, 12);
    press(&mut host, &mut vt, "open");
    let screen = vt.text(false);
    assert!(
        screen.contains("alpha") && !screen.contains("delta"),
        "{screen}"
    );
    // A real wheel, at the list's cells on the screen.
    let (x, y) = find(&vt, "beta");
    host.wheel(x, y, 2);
    vt.render(&mut host);
    let screen = vt.text(false);
    assert!(
        screen.contains("delta") && !screen.contains("alpha"),
        "{screen}"
    );
    // And a click on what the wheel brought into view picks it.
    let (x, y) = find(&vt, "delta");
    host.click(x, y);
    vt.render(&mut host);
    assert!(
        vt.text(false).contains("picked delta"),
        "{}",
        vt.text(false)
    );
}

#[test]
fn a_key_typed_ahead_does_not_reach_a_dialog_it_opened() {
    let (mut host, mut vt) = boot(30, 12);
    let open = host.by_test_id("open").unwrap();
    host.focus(Some(open));
    vt.render(&mut host);
    // Enter opens the dialog; the second Enter, in the same read, was typed
    // before the dialog was on the screen.
    read(
        &mut host,
        &mut vt,
        &[Key::Named("Enter"), Key::Named("Enter")],
    );
    let screen = vt.text(false);
    assert!(screen.contains("alpha"), "the dialog opened:\n{screen}");
    assert!(screen.contains("picked none"), "{screen}");
    // Once it shows, Enter picks its first choice.
    read(&mut host, &mut vt, &[Key::Named("Enter")]);
    assert!(
        vt.text(false).contains("picked alpha"),
        "{}",
        vt.text(false)
    );
}

#[test]
fn clear_twice_shows_only_the_newest_transcript() {
    let (mut host, mut vt) = boot(30, 12);
    for _ in 0..2 {
        // Printed, then retired: the log has no children left when it is
        // replaced, and its rows are still on the screen to clear.
        host.act("retire");
        vt.render(&mut host);
        assert!(vt.text(false).contains("line one"), "{}", vt.text(false));
        host.act("clear");
        vt.render(&mut host);
    }
    let screen = vt.text(false);
    assert!(screen.contains("banner 2"), "{screen}");
    assert!(
        !screen.contains("banner 0") && !screen.contains("banner 1"),
        "{screen}"
    );
    assert_eq!(screen.matches("line one").count(), 1, "{screen}");
    assert_eq!(screen.matches("picked none").count(), 1, "{screen}");
}

#[test]
fn a_resize_reanchors_the_pointer() {
    let (mut host, mut vt) = boot(40, 12);
    press(&mut host, &mut vt, "open");
    host.resize(30, 10);
    vt.render(&mut host);
    assert_eq!(vt.unanswered(), 0);
    let (x, y) = find(&vt, "gamma");
    host.click(x, y);
    vt.render(&mut host);
    assert!(
        vt.text(false).contains("picked gamma"),
        "{}",
        vt.text(false)
    );
}

#[test]
fn tap_refuses_what_is_not_on_the_screen() {
    let (mut host, mut vt) = boot(30, 12);
    press(&mut host, &mut vt, "open");
    // Below the list's three rows.
    let hidden = host.by_test_id("epsilon").unwrap();
    assert!(!host.on_screen(hidden), "{}", vt.text(false));
    let shown = host.by_test_id("alpha").unwrap();
    assert!(host.on_screen(shown));
}

/// A log that grows, under a busy tail that ticks.
const GROWING: &str = r#"component App
  state items = ["one", "two"]
  state busy = true
  state tick = 0
  action add
    items = concat(items, [`item ${length(items)}`])
  action spin
    tick = tick + 1
  action settle
    busy = false
  view
    column
      column role="log"
        each it, i in items key=i
          text it id=`e${i}`
        when busy
          text `working ${tick}` aria-busy=true id="tail"
      text "prompt"
"#;

#[test]
fn settled_rows_are_written_once_and_a_tick_costs_bytes() {
    let plan = contract::compile(GROWING).expect("compiles");
    let mut host = Host::boot(plan, (), Mode::Inline, 30, 4).expect("boots");
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    for _ in 0..6 {
        host.act("add");
        vt.render(&mut host);
    }
    // A spinner tick changes one cell of the live region.
    host.act("spin");
    let tick = vt.render(&mut host);
    assert!(
        tick.len() < 120,
        "a tick wrote {} bytes: {tick:?}",
        tick.len()
    );
    assert!(
        !tick.contains("item"),
        "a tick re-sent settled rows: {tick:?}"
    );
    host.act("settle");
    vt.render(&mut host);
    let all = vt.text(true);
    for item in ["one", "two", "item 2", "item 3", "item 7"] {
        let count = all.lines().filter(|l| l.trim() == item).count();
        assert_eq!(count, 1, "{item:?} appears {count} times in:\n{all}");
    }
    assert!(
        !all.contains("working"),
        "the busy tail went away when it settled:\n{all}"
    );
}

#[test]
fn a_click_lands_on_the_item_under_it_below_history() {
    let (mut host, mut vt) = boot(30, 12);
    // History first, so the region is not at the screen's top.
    for _ in 0..3 {
        host.act("clear");
        vt.render(&mut host);
    }
    press(&mut host, &mut vt, "open");
    let (x, y) = find(&vt, "gamma");
    assert!(y > 0);
    host.click(x, y);
    vt.render(&mut host);
    assert!(
        vt.text(false).contains("picked gamma"),
        "{}",
        vt.text(false)
    );
}

#[test]
fn a_burst_taller_than_the_screen_reaches_the_scrollback_whole() {
    // Twenty settled rows printed in one frame on a four-row screen.
    let mut src = String::from("component App\n  view\n    column\n      column role=\"log\"\n");
    for i in 0..20 {
        src.push_str(&format!("        text \"row {i}\" id=\"r{i}\"\n"));
    }
    src.push_str("      text \"prompt\"\n");
    let plan = contract::compile(&src).expect("compiles");
    let mut host = Host::boot(plan, (), Mode::Inline, 30, 4).expect("boots");
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    let all = vt.text(true);
    for i in 0..20 {
        let row = format!("row {i}");
        assert_eq!(
            all.lines().filter(|l| l.trim() == row).count(),
            1,
            "{row}:\n{all}"
        );
    }
}

#[test]
fn typed_ahead_text_does_not_reach_a_field_it_opened() {
    let (mut host, mut vt) = boot(30, 12);
    let open = host.by_test_id("openk").unwrap();
    host.focus(Some(open));
    vt.render(&mut host);
    // One read: Enter opens the dialog and focuses its field; the rest was
    // typed before the field was on the screen.
    let keys = [Key::Named("Enter"), Key::Char('x'), Key::Named("Enter")];
    read(&mut host, &mut vt, &keys);
    let screen = vt.text(false);
    assert!(screen.contains("saved []"), "{screen}");
    assert!(!screen.contains('x'), "{screen}");
    // Once it shows, typing reaches it.
    read(&mut host, &mut vt, &[Key::Char('y'), Key::Named("Enter")]);
    assert!(vt.text(false).contains("saved [y]"), "{}", vt.text(false));
}

#[test]
fn a_dialog_reopened_in_one_read_is_unseen_again() {
    let (mut host, mut vt) = boot(30, 12);
    let open = host.by_test_id("open").unwrap();
    host.focus(Some(open));
    vt.render(&mut host);
    read(&mut host, &mut vt, &[Key::Named("Enter")]);
    // Escape closes it with no commit; the opener has focus back and is
    // armed (it was on the screen all along).
    read(&mut host, &mut vt, &[Key::Named("Escape")]);
    assert!(!vt.text(false).contains("alpha"));
    // Reopened and answered in one read: the answer was typed blind.
    read(
        &mut host,
        &mut vt,
        &[Key::Named("Enter"), Key::Named("Enter")],
    );
    let screen = vt.text(false);
    assert!(screen.contains("alpha"), "it reopened:\n{screen}");
    assert!(screen.contains("picked none ×0"), "{screen}");
}

#[test]
fn a_second_click_in_one_read_does_not_reach_a_closed_dialog() {
    let (mut host, mut vt) = boot(30, 12);
    press(&mut host, &mut vt, "open");
    let (x, y) = find(&vt, "gamma");
    // Both clicks in one read: the first closes the dialog.
    host.click(x, y);
    host.click(x, y);
    vt.render(&mut host);
    assert!(
        vt.text(false).contains("picked gamma ×1"),
        "{}",
        vt.text(false)
    );
}

#[test]
fn an_entry_that_settles_under_an_open_dialog_is_printed_without_it() {
    // GROWING with an action that opens a dialog, and the dialog.
    let (head, view) = GROWING.split_at(GROWING.find("  view\n").unwrap());
    let src = format!(
        "{head}  action open\n    showModal(\"d\")\n{view}      dialog id=\"d\"\n        text \"DIALOG\"\n"
    );
    let plan = contract::compile(&src).expect("compiles");
    let mut host = Host::boot(plan, (), Mode::Inline, 30, 8).expect("boots");
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    host.act("open");
    vt.render(&mut host);
    for _ in 0..6 {
        host.act("add");
        vt.render(&mut host);
    }
    let all = vt.text(true);
    assert_eq!(
        all.matches("DIALOG").count(),
        1,
        "the dialog was printed:\n{all}"
    );
}

#[test]
fn closed_dialogs_take_no_rows() {
    let (mut host, _) = boot(30, 12);
    let bare = APP[..APP.find("      dialog ").unwrap()].to_string();
    let plan = contract::compile(&bare).expect("compiles");
    let without = Host::boot(plan, (), Mode::Inline, 30, 12).expect("boots");
    assert_eq!(host.document_rows(), without.document_rows());
    host.act("retire");
    assert!(host.document_rows() > 0);
}

#[test]
fn a_control_a_dialog_covered_is_armed_when_it_closes() {
    let (mut host, mut vt) = boot(30, 12);
    let open = host.by_test_id("openk").unwrap();
    host.focus(Some(open));
    vt.render(&mut host);
    read(&mut host, &mut vt, &[Key::Named("Enter")]);
    // Typing in the dialog commits while the opener is covered.
    read(&mut host, &mut vt, &[Key::Char('y')]);
    // Escape closes it without a commit; Enter on the opener opens it again.
    read(&mut host, &mut vt, &[Key::Named("Escape")]);
    assert!(!host.has_layer());
    read(&mut host, &mut vt, &[Key::Named("Enter")]);
    assert!(
        host.has_layer(),
        "the opener ignored Enter:\n{}",
        vt.text(false)
    );
}

#[test]
fn native_buttons_measure_and_paint_faces_and_activate_once_per_key() {
    use exact_kernel::style::cells::{COLUMN, ROW};
    let plan = contract::compile(
        r#"component App
  state n = 0
  action go
    n = n + 1
  view
    column align-items="flex-start"
      text `pressed ${n}`
      button appearance="auto" press=go testId="native"
        image "symbol:send"
        text "Send 界"
      button appearance="auto" aria-label="Close" testId="symbol"
        image "symbol:sf/xmark"
      button appearance="auto" disabled=true press=go testId="disabled"
        text "Off"
      button appearance="none" press=go testId="bare"
        text "Bare"
"#,
    )
    .unwrap();
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 30, 12).unwrap();
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    let native = host.by_test_id("native").unwrap();
    let symbol = host.by_test_id("symbol").unwrap();
    let disabled = host.by_test_id("disabled").unwrap();
    for (id, width) in [(native, 9.), (symbol, 7.), (disabled, 5.)] {
        let f = host.kernel().node(id).unwrap().frame;
        assert_eq!((f.width, f.height), (width * COLUMN, ROW));
    }
    assert_eq!(host.kernel().provisional_layouts(), 0);
    let screen = vt.text(false);
    assert!(
        screen.contains(" Send 界") && screen.contains(" Close"),
        "{screen}"
    );
    let (x, y) = find(&vt, "Send");
    let style = host.frame().grid.cell(x as usize, y as usize).style;
    assert!(style.reverse && !style.bold && !style.faint);
    // The wide glyph keeps its continuation cell; both padding cells reverse.
    assert!(
        host.frame()
            .grid
            .cell(x as usize - 1, y as usize)
            .style
            .reverse
    );
    assert!(
        host.frame()
            .grid
            .cell(x as usize + 7, y as usize)
            .style
            .reverse
    );
    host.focus(Some(native));
    vt.render(&mut host);
    assert!(host.frame().grid.cell(x as usize, y as usize).style.bold);
    read(&mut host, &mut vt, &[Key::Named("Enter")]);
    assert!(vt.text(false).contains("pressed 1"));
    read(&mut host, &mut vt, &[Key::Char(' ')]);
    assert!(vt.text(false).contains("pressed 2"));
    let (x, y) = find(&vt, "Off");
    assert!(host.frame().grid.cell(x as usize, y as usize).style.faint);
    // Disabled buttons are skipped in keyboard traversal.
    read(
        &mut host,
        &mut vt,
        &[Key::Named("Tab"), Key::Named("Tab"), Key::Named("Enter")],
    );
    assert!(vt.text(false).contains("pressed 3"));
    let (x, y) = find(&vt, "Bare");
    assert!(host.frame().grid.cell(x as usize, y as usize).style.reverse);
}

#[test]
fn native_button_wrap_and_bound_title_remeasure_the_painted_face() {
    use exact_kernel::style::cells::{COLUMN, ROW};
    let plan = contract::compile(
        r#"component App
  state title = "First title"
  action change
    title = "界"
  view
    column align-items="flex-start"
      button appearance="auto" width="7ch" press=change testId="button"
        text title
"#,
    )
    .unwrap();
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 30, 12).unwrap();
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    let id = host.by_test_id("button").unwrap();
    assert_eq!(host.kernel().node(id).unwrap().frame.height, 2. * ROW);
    assert!(vt.text(false).contains("First") && vt.text(false).contains("title"));
    host.act("change");
    vt.render(&mut host);
    let f = host.kernel().node(id).unwrap().frame;
    assert_eq!((f.width, f.height), (7. * COLUMN, ROW));
    assert!(vt.text(false).contains('界') && !vt.text(false).contains("First"));
}
