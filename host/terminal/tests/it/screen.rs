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
      for e in [epoch] key=e
        column role="log"
          if ! retired
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
          for v in ["alpha", "beta", "gamma", "delta", "epsilon"] key=v
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

#[test]
fn an_image_can_appear_disappear_and_reappear_after_image_free_commits() {
    let path = std::env::temp_dir().join(format!("terminal-late-image-{}.png", std::process::id()));
    let file = std::fs::File::create(&path).unwrap();
    let mut encoder = png::Encoder::new(file, 1, 1);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&[20, 40, 60])
        .unwrap();
    let source = format!(
        "component App\n  state shown = false\n  action show\n    shown = true\n  action hide\n    shown = false\n  view\n    column\n      text \"anchor\"\n      if shown\n        image \"{}\" width=\"2ch\" height=\"1lh\" testId=\"picture\"\n",
        path.display()
    );
    let plan = contract::compile_terminal(&source).unwrap();
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 20, 6).unwrap();
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    assert!(host.images.nodes.is_empty());
    assert!(!vt.text(false).contains('▀'));
    for pass in 0..2 {
        host.act("show");
        vt.render(&mut host);
        let id = host.by_test_id("picture").unwrap();
        assert_eq!(host.images.nodes.len(), 1);
        assert_eq!(host.images.of(id).unwrap().rgba, [20, 40, 60, 255]);
        assert!(vt.text(false).contains('▀'));
        if pass == 0 {
            // A remount reuses the source decoded on its first appearance.
            std::fs::remove_file(&path).unwrap();
        }
        host.act("hide");
        vt.render(&mut host);
        assert!(host.images.nodes.is_empty());
        assert!(host.images.of(id).is_none());
        assert!(!vt.text(false).contains('▀'));
    }
}

#[test]
fn a_large_settled_burst_keeps_every_row_across_paint_strips() {
    let mut source = String::from("component App\n  view\n    column\n      column role=\"log\"\n");
    // One multiline child crosses several paint strips; the next child
    // must still start on its own row. Wide glyphs retain their columns.
    let lines: Vec<String> = (0..300).map(|n| format!("row {n:03} 界 é")).collect();
    source.push_str(&format!(
        "        text \"{}\" white-space=\"pre\" id=\"long\"\n        text \"last entry\" id=\"last\"\n      text \"live tail\"\n",
        lines.join("\\n")
    ));
    let plan = contract::compile_terminal(&source).unwrap();
    let mut host = Host::boot(plan, (), Mode::Inline, 30, 12).unwrap();
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    let expected = format!("{}\nlast entry\nlive tail", lines.join("\n"));
    assert_eq!(vt.text(true).trim_end(), expected);
    // Painting another live frame must not print the settled burst again.
    host.resize(31, 12);
    vt.render(&mut host);
    assert_eq!(vt.text(true).matches("row 000").count(), 1);
    assert_eq!(vt.text(true).matches("last entry").count(), 1);
}

#[test]
fn retiring_a_large_tree_returns_slots_and_keeps_keys_and_frames_correct() {
    let items = (0..500)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        "component App\n  state filled = false\n  action fill\n    filled = true\n  action retire\n    filled = false\n  view\n    column\n      text \"anchor\" testId=\"anchor\"\n      if filled\n        for n in [{items}] key=n\n          text `row ${{n}}` testId=`row-${{n}}`\n"
    );
    for mode in [Mode::Inline, Mode::Fullscreen] {
        let plan = contract::compile_terminal(&source).unwrap();
        let mut host = Host::boot(plan, (), mode, 30, 12).unwrap();
        let anchor = host.by_test_id("anchor").unwrap();
        let key = host.kernel().node(anchor).unwrap().key;
        let initial = host.frame().grid.clone();
        let slots = host.kernel().arena().slot_count();
        let mut previous = None;
        for _ in 0..3 {
            host.act("fill");
            let leaf = host.by_test_id("row-499").unwrap();
            let retired = host.kernel().node(leaf).unwrap().key;
            assert_ne!(previous, Some(retired), "reused slots get fresh identities");
            assert!(host.kernel().arena().slot_count() >= slots + 500);
            host.act("retire");
            assert_eq!(host.kernel().arena().slot_count(), slots);
            assert!(host.kernel().node_by_key(retired).is_none());
            assert_eq!(host.kernel().node(anchor).unwrap().key, key);
            assert_eq!(host.frame().grid, initial);
            previous = Some(retired);
        }
    }
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
    items = items.concat([`item ${items.length}`])
  action spin
    tick = tick + 1
  action settle
    busy = false
  view
    column
      column role="log"
        for it, i in items key=i
          text it id=`e${i}`
        if busy
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

#[test]
fn live_time_ticks_in_the_terminal_without_an_app_task() {
    let now = 1_767_225_600_000.;
    let source = format!(
        "component App\n  view\n    time datetime={} format=\"relative\" testId=\"live-time\"\n",
        now - 120_000.
    );
    let plan = contract::compile(&source).unwrap();
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 40, 4).unwrap();
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    assert!(vt.text(false).contains("2 min. ago"));
    host.tick(60_000.);
    vt.render(&mut host);
    assert!(vt.text(false).contains("3 min. ago"));
}

/// LLP 1084 D7: a grouped row's separator is no border. It is drawn on the
/// row's last line from its content edge, though no row has a bottom border
/// width; not under the card's last row, which follows a row that comes and
/// goes; and a colour the author gives the bottom edge alone recolours
/// nothing.
#[test]
fn a_grouped_rows_separator_is_drawn_without_a_border_and_not_under_the_last_row() {
    let plan = contract::compile(
        r##"component App
  state show = true
  action toggle
    show = ! show
  view
    list appearance="auto" width=320 height=320
      section
        row press=toggle testId="first" border-bottom-color="#0000ff"
          text "First"
        if show
          row testId="last"
            text "Last"
"##,
    )
    .unwrap();
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 40, 20).unwrap();
    let id = host.by_test_id("first").unwrap();
    let row = host.kernel().node(id).unwrap();
    assert_eq!(row.style.border_widths_in(&host.kernel().env())[2], 0.0);
    let f = row.frame;
    let x = ((f.x + 16.0) / 8.0).round() as usize;
    let right = ((f.x + f.width) / 8.0).round() as usize;
    let y = ((f.y + f.height) / 16.0).round() as usize - 1;
    let frame = host.frame();
    assert_ne!(frame.grid.cell(x - 1, y).text, "─", "not in the inset");
    for col in x..right {
        assert_eq!(frame.grid.cell(col, y).text, "─", "column {col}");
        assert_ne!(
            frame.grid.cell(col, y).style.fg,
            Some(exact_terminal::grid::Rgb(0, 0, 255)),
            "the platform's ink"
        );
    }
    host.press(id);
    assert_ne!(
        host.frame().grid.cell(x, y).text,
        "─",
        "the card's only row has none"
    );
    host.press(id);
    assert_eq!(host.frame().grid.cell(x, y).text, "─");
}

/// LLP 1084 D7: a native button row as wide as its row stays in its card:
/// its inset is its margin and its widest the card less its insets.
#[test]
fn a_full_width_native_button_row_stays_in_its_card() {
    let plan = contract::compile(
        r#"component App
  view
    list appearance="auto" width=320 height=320
      section
        button appearance="auto" width="100%" testId="native"
          text "Native"
        row
          text "Last"
"#,
    )
    .unwrap();
    let host = Host::boot(plan, (), Mode::Fullscreen, 40, 20).unwrap();
    let row = host
        .kernel()
        .node(host.by_test_id("native").unwrap())
        .unwrap();
    let card = host.kernel().node(row.parent.unwrap()).unwrap();
    assert_eq!(row.frame.x - card.frame.x, 16.0, "its inset");
    assert!(
        row.frame.x + row.frame.width <= card.frame.x + card.frame.width,
        "inside the card: {:?} in {:?}",
        row.frame,
        card.frame
    );
}

/// A `press` on a box (a card, a pressable column) is on the screen and
/// taps as a control does, and a click on its text reaches it; a button
/// inside a pressable column is the click's target there, its own press
/// alone (QUEUE: "the terminal never taps a pressable box", iosfix's repro).
const PRESSABLE: &str = r#"component App
  state solo = 0
  state card = 0
  state inner = 0
  action tapSolo
    solo = solo + 1
  action tapCard
    card = card + 1
  action tapInner
    inner = inner + 1
  view
    column padding=16
      box testId="solo" press=tapSolo width=48 height=32
        text "tap"
      column testId="card" press=tapCard
        text "card text"
        button testId="inner" press=tapInner
          text "inner"
      Card(go=tapCard)
        box testId="filled" press=tapInner width=24 height=16
          text "fill"
      text `solo ${solo} card ${card} inner ${inner}`

component Card
  props
    go: action
  slot
  view
    column press=go testId="slotted"
      text "slotted"
      children
"#;

#[test]
fn a_pressable_box_taps_and_clicks_as_a_control_does() {
    let plan = contract::compile(PRESSABLE).expect("compiles");
    // Full screen, whose painting starts at row 0, so a click's cell needs
    // no position report.
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 40, 20).expect("boots");
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    press(&mut host, &mut vt, "solo");
    press(&mut host, &mut vt, "card");
    press(&mut host, &mut vt, "inner");
    // A pressable box filling a pressable component's slot (library-gate's
    // marker in a PostCard), and the component's own surface.
    press(&mut host, &mut vt, "filled");
    press(&mut host, &mut vt, "slotted");
    assert!(
        vt.text(false).contains("solo 1 card 2 inner 2"),
        "{}",
        vt.text(false)
    );
    let (x, y) = find(&vt, "card text");
    host.read_at = host.frames;
    host.click(x, y);
    vt.render(&mut host);
    let (x, y) = find(&vt, "tap");
    host.read_at = host.frames;
    host.click(x, y);
    vt.render(&mut host);
    let (x, y) = find(&vt, "inner");
    host.read_at = host.frames;
    host.click(x, y);
    vt.render(&mut host);
    let (x, y) = find(&vt, "fil");
    host.read_at = host.frames;
    host.click(x, y);
    vt.render(&mut host);
    let (x, y) = find(&vt, "slotted");
    host.read_at = host.frames;
    host.click(x, y);
    vt.render(&mut host);
    assert!(
        vt.text(false).contains("solo 2 card 4 inner 4"),
        "{}",
        vt.text(false)
    );
}

/// What a pressable box must not lend its cells to (Astra's round 1): a
/// link-like run in its text is the click's target, and a button scrolled
/// out of a list it holds, or a hidden box inside it, is not on the screen.
const COVERED: &str = r#"component App
  state outer = 0
  state run = 0
  state far = 0
  action tapOuter
    outer = outer + 1
  action tapRun
    run = run + 1
  action tapFar
    far = far + 1
  view
    column padding=16
      column testId="outer" press=tapOuter
        text
          text "see "
          text "this" press=tapRun testId="run"
          text " here"
        box testId="ghost" press=tapFar visibility="hidden" height=16
          text "ghost"
        scroll max-height="2lh" testId="list"
          for v in ["a1", "a2", "a3", "a4", "a5"] key=v
            button press=tapFar testId=v
              text v
      text `outer ${outer} run ${run} far ${far}`
"#;

#[test]
fn a_pressable_box_lends_no_cells_to_what_it_hides_or_holds_inline() {
    let plan = contract::compile(COVERED).expect("compiles");
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 40, 20).expect("boots");
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    let (x, y) = find(&vt, "this");
    host.read_at = host.frames;
    host.click(x, y);
    vt.render(&mut host);
    let (x, y) = find(&vt, "see");
    host.read_at = host.frames;
    host.click(x, y);
    vt.render(&mut host);
    assert!(
        vt.text(false).contains("outer 1 run 1 far 0"),
        "{}",
        vt.text(false)
    );
    for hidden in ["ghost", "a5"] {
        let v = host.by_test_id(hidden).unwrap();
        assert!(!host.on_screen(v), "#{hidden}:\n{}", vt.text(false));
    }
    let shown = host.by_test_id("a1").unwrap();
    assert!(host.on_screen(shown));
}

/// A press on a box that can't hold the focus moves the focus away (the
/// field blurs), as a click on a `div` does, and scrolls nothing into view
/// (Grok's round 1): a card taller than its scroller stays where it is.
const BLURS: &str = r#"component App
  state blurs = 0
  state presses = 0
  action blurred
    blurs = blurs + 1
  action pressed
    presses = presses + 1
  view
    column
      input testId="field" blur=blurred appearance="none" placeholder="type"
      scroll max-height="4lh" testId="port"
        column testId="card" press=pressed
          for v in ["c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8"] key=v
            text v
      text `blurs ${blurs} presses ${presses}`
"#;

#[test]
fn a_press_on_a_box_blurs_the_field_and_scrolls_nothing() {
    let plan = contract::compile(BLURS).expect("compiles");
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 40, 20).expect("boots");
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    press(&mut host, &mut vt, "field");
    let (x, y) = find(&vt, "c1");
    host.read_at = host.frames;
    host.click(x, y);
    vt.render(&mut host);
    let screen = vt.text(false);
    assert!(screen.contains("blurs 1 presses 1"), "{screen}");
    assert!(
        screen.contains("c1") && !screen.contains("c8"),
        "not scrolled: {screen}"
    );
}

/// A disabled pressable box, or one inside an `inert` subtree, swallows a
/// press: neither its own `press` nor an enclosing card's runs, by a tap,
/// a click, or a click on a styled run of its text; a plain styled label
/// inside a button leaves the click the button's: its own press runs and
/// its `commandfor` opens the dialog (Astra's round 2 and its re-check).
const INELIGIBLE: &str = r#"component App
  state presses = 0
  state outer = 0
  action pressed
    presses = presses + 1
  action around
    outer = outer + 1
  view
    column
      column testId="card" press=around
        column testId="off" press=pressed disabled=true
          text
            text "disabled "
            text "card" font-weight=700
      column inert=true
        box testId="numb" press=pressed
          text "inert box"
      button press=pressed commandfor="d" command="show-modal" appearance="none" testId="labelled"
        text
          text "open "
          text "the dialog" font-weight=700
      text `presses ${presses} outer ${outer}`
      dialog id="d" testId="dialog"
        text "opened"
"#;

#[test]
fn a_disabled_or_inert_box_takes_no_press_and_a_buttons_label_is_its_own() {
    let plan = contract::compile(INELIGIBLE).expect("compiles");
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 40, 20).expect("boots");
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    for id in ["off", "numb"] {
        press(&mut host, &mut vt, id);
    }
    for text in ["disabled", "card", "inert box"] {
        let (x, y) = find(&vt, text);
        host.read_at = host.frames;
        host.click(x, y);
        vt.render(&mut host);
    }
    assert!(
        vt.text(false).contains("presses 0 outer 0"),
        "{}",
        vt.text(false)
    );
    let (x, y) = find(&vt, "the dialog");
    host.read_at = host.frames;
    host.click(x, y);
    vt.render(&mut host);
    let screen = vt.text(false);
    assert!(
        screen.contains("opened") && screen.contains("presses 1"),
        "{screen}"
    );
}

/// An `autofocus` control takes the focus once, as it arrives with nothing
/// focused (HTML's candidates): a press on a box that can't hold the focus
/// blurs it and the focus stays empty, so a key types nowhere; one that
/// arrives later, with nothing focused, takes it (QUEUE: "the terminal
/// refills autofocus whenever the focus is empty").
const AUTOFOCUS: &str = r#"component App
  state draft = ""
  state other = ""
  state shown = false
  state presses = 0
  state more = false
  state third = ""
  action showMore
    more = true
  action writeThird(v: string)
    third = v
  action write(v: string)
    draft = v
  action writeOther(v: string)
    other = v
  action pressed
    presses = presses + 1
    shown = true
  view
    column
      input testId="field" value=draft input=write autofocus=true appearance="none" placeholder="type"
      box testId="card" press=pressed
        text "card"
      if shown
        input testId="late" value=other input=writeOther autofocus=true appearance="none" placeholder="late"
      button testId="more" press=showMore
        text "more"
      if more
        input testId="third" value=third input=writeThird autofocus=true appearance="none" placeholder="third"
      text `draft [${draft}] other [${other}] third [${third}] presses ${presses}`
"#;

#[test]
fn autofocus_takes_the_focus_once_and_never_refills_it() {
    let plan = contract::compile(AUTOFOCUS).expect("compiles");
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 40, 20).expect("boots");
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    read(&mut host, &mut vt, &[Key::Char('a')]);
    assert!(vt.text(false).contains("draft [a]"), "{}", vt.text(false));
    // The press blurs the field and shows `late`, which arrives with
    // nothing focused and takes the focus: the next key is its.
    let (x, y) = find(&vt, "card");
    host.read_at = host.frames;
    host.click(x, y);
    vt.render(&mut host);
    read(&mut host, &mut vt, &[Key::Char('b')]);
    assert!(
        vt.text(false)
            .contains("draft [a] other [b] third [] presses 1"),
        "{}",
        vt.text(false)
    );
    // Pressed again: nothing new arrives, so the focus stays empty.
    let (x, y) = find(&vt, "card");
    host.read_at = host.frames;
    host.click(x, y);
    vt.render(&mut host);
    read(&mut host, &mut vt, &[Key::Char('c')]);
    assert!(
        vt.text(false)
            .contains("draft [a] other [b] third [] presses 2"),
        "{}",
        vt.text(false)
    );
    // One that arrives while something holds the focus (the button just
    // pressed) is spent: after a blur, a key still types nowhere.
    press(&mut host, &mut vt, "more");
    let (x, y) = find(&vt, "card");
    host.read_at = host.frames;
    host.click(x, y);
    vt.render(&mut host);
    read(&mut host, &mut vt, &[Key::Char('d')]);
    assert!(
        vt.text(false)
            .contains("draft [a] other [b] third [] presses 3"),
        "{}",
        vt.text(false)
    );
}

/// With nothing focused the root's `key` handler hears every key, a
/// printable one and one that would scroll, as the web's `window` listener
/// hears one at `<body>`.
#[test]
fn with_nothing_focused_the_root_hears_keys() {
    const ROOT_KEYS: &str = r#"component App
  state heard = ""
  action keyed(k: string)
    heard = `${heard}${k};`
  view
    column key=keyed
      text `heard [${heard}]`
"#;
    let plan = contract::compile(ROOT_KEYS).expect("compiles");
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 40, 10).expect("boots");
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    read(&mut host, &mut vt, &[Key::Char('a'), Key::Named("End")]);
    assert!(
        vt.text(false).contains("heard [a;End;]"),
        "{}",
        vt.text(false)
    );
}
