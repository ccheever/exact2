//! Text fields the shell hosts (LLP 1115.001 D8), against a stand-in for
//! Android's reader: a shell on the test's own thread whose field sets a
//! character half its font's size wide and a line one and a quarter of it
//! tall. What is held here is the presenter's side of the boundary: a field
//! measured by the shell during layout and no longer painted, what the
//! reader is told, what its edits, its selection, its focus and its action
//! key do, a field's identity, and that a host with no such shell (desktop
//! Linux) is as it was.
use super::*;
use crate::fields::{FieldFace, FieldMetrics};
use std::sync::atomic::{AtomicUsize, Ordering};

/// The reader's field, stood in for, and how often layout asked it.
#[derive(Default)]
struct Shell {
    asked: AtomicUsize,
}

impl FieldMetrics for Shell {
    fn measure(&self, face: &FieldFace, text: &str, width: Option<f32>) -> [f32; 3] {
        self.asked.fetch_add(1, Ordering::Relaxed);
        let (advance, line) = (face.size / 2.0, face.size * 1.25);
        let (mut rows, mut widest) = (0.0f32, 0.0f32);
        for row in text.split('\n') {
            let wide = row.chars().count() as f32 * advance;
            rows += match width {
                Some(most) if face.multiline && wide > most => (wide / most).ceil(),
                _ => 1.0,
            };
            widest = widest.max(wide);
        }
        [width.unwrap_or(widest), rows * line, face.size]
    }
}

/// A shell that hosts this thread's fields until the guard drops.
fn shell() -> (&'static Shell, impl Drop) {
    let shell: &'static Shell = Box::leak(Box::default());
    (shell, crate::fields::host_here(shell, 1.0))
}

/// A form: a field the app hears every edit, selection, change and submit
/// of, an email, a field the app rewrites in lower case, a numeric
/// password of four, a textarea, and a button that sets a selection.
const FORM: &str = r##"component Form
  state name = ""
  state low = ""
  state sent = ""
  state heard = ""
  action rename(value: string, e: InputEvent)
    name = value
    heard = `${heard}[${value} ${e.selectionStart}-${e.selectionEnd}]`
  action committed(value: string)
    heard = `${heard}{${value}}`
  action picked(e: InputEvent)
    heard = `${heard}<${e.selectionStart}-${e.selectionEnd}>`
  action hush(value: string)
    low = value.toLowerCase()
  action send
    sent = `sent ${name}`
  action jump
    setSelectionRange("name", 1, 2)
  view
    column width=320 height=480 background-color="#ffffff" align-items="flex-start"
      input id="name" testId="name" value=name input=rename change=committed select=picked submit=send enterkeyhint="next" placeholder="Name"
      input testId="email" type="email" autocomplete="email" enterkeyhint="done" submit=send
      input testId="low" value=low input=hush
      input testId="pin" type="password" inputmode="numeric" maxlength=4
      textarea testId="notes" rows=3
      button "Jump" press=jump testId="jump" height=30
      text `${name}|${low}|${sent}|${heard}` testId="log" height=20
"##;

/// Twelve fields with nothing bound to them, in a scroller that shows six.
const LONG: &str = r#"component Long
  derive rows = "0 1 2 3 4 5 6 7 8 9 10 11".split(" ")
  view
    scroll testId="list" width=300 height=200
      for n in rows key=n
        input testId=`f${n}` height=20
"#;

fn boot(source: &str) -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (320., 480.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    settle(&mut p);
    p
}

fn settle(p: &mut Presenter<()>) {
    for _ in 0..8 {
        assert!(p.pump(p.host.now()).is_none());
        if p.dirty() {
            let _ = p.frame();
        }
    }
}

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

fn log(p: &Presenter<()>) -> String {
    let k = p.host().kernel();
    let node = k.node_by_key(k.find_by_test_id("log")[0]).unwrap();
    node.props.str(PropId::Text).unwrap().to_string()
}

fn tap(p: &mut Presenter<()>, name: &str) {
    let (x, y, w, h) = p.rect_of(id(p, name)).unwrap();
    p.press_at(x + w / 2.0, y + h / 2.0, p.host.now());
    settle(p);
}

/// One field of a statement.
#[derive(Debug)]
struct Told {
    id: ViewId,
    generation: i32,
    flags: i32,
    rect: [f32; 4],
    clip: [f32; 4],
    seq: i32,
    value: String,
    look: Vec<i32>,
    strings: [String; 3],
}

fn string(words: &[i32], at: usize) -> (String, usize) {
    let n = words[at] as usize;
    let bytes: Vec<u8> = (0..n)
        .map(|k| (words[at + 1 + k / 4] as u32 >> (8 * (k % 4))) as u8)
        .collect();
    (String::from_utf8(bytes).unwrap(), at + 1 + n.div_ceil(4))
}

/// A statement's words, read as the reader reads them.
fn read(words: &[i32]) -> Vec<Told> {
    let px =
        |at: usize| -> [f32; 4] { std::array::from_fn(|i| f32::from_bits(words[at + i] as u32)) };
    let mut out = Vec::new();
    let mut at = 1;
    for _ in 0..words[0] {
        let (value, look) = string(words, at + 13);
        let (placeholder, next) = string(words, look + 18);
        let (autocomplete, next) = string(words, next);
        let (family, end) = string(words, next);
        out.push(Told {
            id: words[at] as ViewId,
            generation: words[at + 1],
            flags: words[at + 2],
            rect: px(at + 4),
            clip: px(at + 8),
            seq: words[at + 12],
            value,
            look: words[look..look + 18].to_vec(),
            strings: [placeholder, autocomplete, family],
        });
        at = end;
    }
    assert_eq!(at, words.len(), "the statement is read to its end");
    out
}

/// The statement as it now stands, whether or not it was told before.
fn told(p: &mut Presenter<()>) -> Vec<Told> {
    p.hosted.last.clear();
    read(&p.reader_fields(1.0).expect("a statement"))
}

/// The keyboard the shell was last asked for: its state and its field.
fn keyboard(p: &mut Presenter<()>) -> Option<(i32, i32)> {
    p.tell_shell();
    let words = p.take_requests()?;
    let (mut at, mut last) = (0, None);
    while at + 3 <= words.len() {
        if words[at] == 6 {
            last = Some((words[at + 3], words[at + 4]));
        }
        at += 3 + words[at + 2] as usize;
    }
    last
}

/// Whether anything but the field's fill shows inside `name`'s content box:
/// the fill is the container's at its start, beside the text.
fn inked(p: &mut Presenter<()>, name: &str) -> bool {
    let (x, y, w, h) = p.rect_of(id(p, name)).unwrap();
    let pixmap = p.frame();
    let fill = pixmap
        .pixel((x + 8.0) as u32, (y + h / 2.0) as u32)
        .unwrap();
    let mut inked = false;
    for row in (y + 16.0) as u32..(y + h - 16.0) as u32 {
        for col in (x + 16.0) as u32..(x + w - 16.0) as u32 {
            inked |= pixmap.pixel(col, row).unwrap() != fill;
        }
    }
    inked
}

#[test]
fn a_hosted_field_is_measured_by_the_shell_during_layout() {
    let (shell, _hosting) = shell();
    let mut p = boot(FORM);
    assert!(
        shell.asked.load(Ordering::Relaxed) > 0,
        "the boot layout asked the shell's field"
    );
    // Twenty characters of 8 px and a line of 20, in Material's filled
    // field (16 in on every side, 56 tall at least); a textarea's three rows.
    let (_, _, w, h) = p.rect_of(id(&p, "name")).unwrap();
    assert_eq!((w, h), (160.0 + 32.0, 56.0));
    let (_, _, w, h) = p.rect_of(id(&p, "notes")).unwrap();
    assert_eq!((w, h), (160.0 + 32.0, 60.0 + 32.0));
    // The device's font scale is the field's own font's too: 16 by 1.5.
    let device = Device {
        night: false,
        font_scale: 1.5,
        locale: "en-US".into(),
        rtl: false,
        hour24: false,
        reduced_motion: false,
        bold_text: false,
        high_contrast: false,
        touch: true,
        touch_slop: 8.0,
        dates: String::new(),
    };
    assert_eq!(p.set_device(&device), None);
    settle(&mut p);
    let (_, _, w, h) = p.rect_of(id(&p, "name")).unwrap();
    assert_eq!((w, h), (240.0 + 32.0, 30.0 + 32.0));
    assert_eq!(told(&mut p)[0].look[11], 24f32.to_bits() as i32);
}

#[test]
fn the_reader_is_told_each_field_its_box_and_its_keyboard() {
    let (_shell, _hosting) = shell();
    let mut p = boot(FORM);
    let words = p.reader_fields(1.0).expect("the first statement is whole");
    assert_eq!(
        p.reader_fields(1.0),
        None,
        "told once, while nothing changes"
    );
    let fields = read(&words);
    let names = ["name", "email", "low", "pin", "notes"];
    let ids: Vec<ViewId> = names.iter().map(|n| id(&p, n)).collect();
    assert_eq!(fields.iter().map(|f| f.id).collect::<Vec<_>>(), ids);
    // Its content box, inside the container this host paints, unclipped.
    let (x, y, _, _) = p.rect_of(ids[0]).unwrap();
    let name = &fields[0];
    assert_eq!((name.generation, name.flags, name.seq), (0, 1, 0));
    assert_eq!(name.rect, [x + 16.0, y + 16.0, 160.0, 24.0]);
    assert!(name.clip[2] < 0.0, "no clip: {:?}", name.clip);
    assert_eq!(name.value, "");
    // Enter submits and another field follows; the key says Next.
    assert_eq!(&name.look[..6], &[128 | 256, 0, 0, 4, -1, 0]);
    assert_eq!(name.look[11], 16f32.to_bits() as i32, "its font, pixels");
    assert_eq!(name.look[12], 400);
    assert_eq!(
        name.strings,
        ["Name".to_string(), String::new(), String::new()]
    );
    // An email's keyboard, Done, and autofill's hint.
    assert_eq!(&fields[1].look[..4], &[128 | 256, 1, 0, 2]);
    assert_eq!(fields[1].strings[1], "email");
    // A password of four digits; a textarea, which nothing follows.
    assert_eq!(&fields[3].look[..5], &[2 | 256, 4, 0, 0, 4]);
    assert_eq!(fields[4].look[0], 1);
}

#[test]
fn a_hosted_fields_text_is_the_shells_to_draw() {
    let (_shell, _hosting) = shell();
    let mut p = boot(FORM);
    p.reader_fields(1.0);
    let name = id(&p, "name");
    p.reader_field_edited(name, (0, 1), "hello", (5, 5), 1.0);
    assert_eq!(p.field_text(name), "hello");
    assert!(
        !inked(&mut p, "name"),
        "nothing of the text is painted here"
    );
    assert!(!inked(&mut p, "email"), "nor a placeholder's");
}

#[test]
fn an_edit_is_the_fields_text_and_the_app_answers_it_by_its_number() {
    let (_shell, _hosting) = shell();
    let mut p = boot(FORM);
    p.serve_requests();
    p.reader_fields(1.0);
    let (name, low) = (id(&p, "name"), id(&p, "low"));
    // A finger tapped the field: the presenter hears of the focus.
    p.reader_field_focused(name, 0, true, 1.0);
    assert_eq!(p.focus(), Some(name));
    assert_eq!(
        keyboard(&mut p),
        Some((1, name as i32)),
        "that field's keyboard"
    );
    // The person types (the keyboard still composing it): `input` fires as
    // the text composes, with the caret.
    p.reader_field_edited(name, (0, 1), "ab", (2, 2), 2.0);
    assert_eq!(log(&p), "ab|||[ab 2-2]");
    // The app's commit set the value the field holds. The statement
    // answers edit 1 with that same text, which the reader leaves alone:
    // its composition and its selection stand.
    let after = told(&mut p);
    assert_eq!((after[0].seq, after[0].value.as_str()), (1, "ab"));
    // The same text with the caret elsewhere is no edit to the app.
    p.reader_field_edited(name, (0, 2), "ab", (1, 1), 3.0);
    assert_eq!(log(&p), "ab|||[ab 2-2]");
    assert_eq!(told(&mut p)[0].seq, 2);
    // A range the person selects fires `select`; a caret does not.
    p.reader_field_selected(name, 0, (0, 2), 4.0);
    p.reader_field_selected(name, 0, (2, 2), 5.0);
    assert_eq!(log(&p), "ab|||[ab 2-2]<0-2>");
    // A field whose value the app rewrites: the statement answers the
    // edit with another text, which the reader's field takes.
    p.reader_field_edited(low, (0, 1), "AB", (2, 2), 6.0);
    let after = told(&mut p);
    assert_eq!((after[2].seq, after[2].value.as_str()), (1, "ab"));
    p.reader_field_edited(low, (0, 2), "abC", (3, 3), 7.0);
    let after = told(&mut p);
    assert_eq!((after[2].seq, after[2].value.as_str()), (2, "abc"));
    // A command's selection is told by its revision, once.
    assert_eq!(&after[0].look[15..], &[0, 0, 0]);
    tap(&mut p, "jump");
    p.run_commands(<()>::default);
    assert_eq!(&told(&mut p)[0].look[15..], &[1, 1, 2]);
    // What a reader says of a generation the presenter moved on from is
    // dropped, and the field starts over.
    p.hosted_renewed(&[name]);
    p.reader_field_edited(name, (0, 3), "stale", (5, 5), 8.0);
    let after = told(&mut p);
    assert_eq!((after[0].generation, after[0].seq), (1, 0));
    let heard = "ab|abc||[ab 2-2]<0-2><1-2>";
    assert_eq!(log(&p), heard, "the command's `select`, and no `input`");
}

/// LLP 1115.000 §5, ruling 6: the app hears `input` at every change, the
/// composing text included, and a value it writes meanwhile is held, the
/// last one, until the composition ends.
#[test]
fn a_value_the_app_writes_while_a_field_composes_is_held_until_it_ends() {
    let (_shell, _hosting) = shell();
    let mut p = boot(FORM);
    p.serve_requests();
    p.reader_fields(1.0);
    let low = id(&p, "low");
    p.reader_field_focused(low, 0, true, 1.0);
    // The keyboard marks "A": the reader says the field composes, then
    // the edit. The app hears it and writes "a", which is not told: the
    // statement answers the edit with the field's own text, so the reader
    // replaces nothing and the composition stands.
    p.reader_field_composing(low, 0, true, "A");
    p.reader_field_edited(low, (0, 1), "A", (1, 1), 2.0);
    assert!(log(&p).starts_with("|a|"), "the app heard it: {}", log(&p));
    let after = told(&mut p);
    assert_eq!((after[2].seq, after[2].value.as_str()), (1, "A"));
    // Each change of the marked text is heard, and each value held.
    p.reader_field_edited(low, (0, 2), "AB", (2, 2), 3.0);
    assert!(log(&p).starts_with("|ab|"), "{}", log(&p));
    let after = told(&mut p);
    assert_eq!((after[2].seq, after[2].value.as_str()), (2, "AB"));
    // The keyboard commits the word with a space: the composition ended,
    // said before the edit that ended it. The app hears that text, and
    // the statement then says the value the app shows, the last it wrote.
    p.reader_field_composing(low, 0, false, "AB ");
    p.reader_field_edited(low, (0, 3), "AB ", (3, 3), 4.0);
    assert!(log(&p).starts_with("|ab |"), "{}", log(&p));
    let after = told(&mut p);
    assert_eq!((after[2].seq, after[2].value.as_str()), (3, "ab "));
    // The next word composes, and its composition is cancelled as it
    // stands (the keyboard put away): no edit follows, and the held value
    // is told all the same.
    p.reader_field_composing(low, 0, true, "ab C");
    p.reader_field_edited(low, (0, 4), "ab C", (4, 4), 5.0);
    let after = told(&mut p);
    assert_eq!((after[2].seq, after[2].value.as_str()), (4, "ab C"));
    p.reader_field_composing(low, 0, false, "ab C");
    let after = told(&mut p);
    assert_eq!((after[2].seq, after[2].value.as_str()), (4, "ab c"));
    // A field renewed for another item composes nothing, and what the
    // reader says of the generation before is dropped.
    p.reader_field_composing(low, 0, true, "ab cD");
    p.hosted_renewed(&[low]);
    p.reader_field_composing(low, 0, true, "stale");
    let after = told(&mut p);
    assert_eq!((after[2].generation, after[2].value.as_str()), (1, "ab c"));
}

#[test]
fn the_action_key_does_what_it_says() {
    let (_shell, _hosting) = shell();
    let mut p = boot(FORM);
    p.serve_requests();
    p.reader_fields(1.0);
    let (name, email, low) = (id(&p, "name"), id(&p, "email"), id(&p, "low"));
    p.reader_field_focused(name, 0, true, 1.0);
    p.reader_field_edited(name, (0, 1), "Ann", (3, 3), 2.0);
    // Next: Enter commits and submits, then the next field has the focus
    // and the keyboard.
    p.reader_field_action(name, 0, field_action::NEXT, 3.0);
    assert_eq!(log(&p), "Ann||sent Ann|[Ann 3-3]{Ann}");
    assert_eq!(p.focus(), Some(email));
    assert_eq!(keyboard(&mut p), Some((1, email as i32)));
    // Done: submits and leaves the field, which puts the keyboard away.
    p.reader_field_action(email, 0, field_action::DONE, 4.0);
    assert_eq!(p.focus(), None);
    assert_eq!(keyboard(&mut p), Some((0, 0)));
    // Tab is the page's own: the next stop, and back.
    p.reader_field_focused(email, 0, true, 5.0);
    p.reader_field_action(email, 0, field_action::TAB, 6.0);
    assert_eq!(p.focus(), Some(low));
    p.reader_field_action(low, 0, field_action::BACKTAB, 7.0);
    assert_eq!(p.focus(), Some(email));
    // A hardware keyboard's Enter with no action of its own leaves the
    // focus where it is.
    p.reader_field_action(email, 0, field_action::ENTER, 8.0);
    assert_eq!(p.focus(), Some(email));
}

#[test]
fn a_touch_in_a_hosted_field_is_the_fields() {
    let (_shell, _hosting) = shell();
    let mut p = boot(FORM);
    let name = id(&p, "name");
    let (x, y, w, h) = p.rect_of(name).unwrap();
    assert_eq!(
        p.reader_under(x + w / 2.0, y + h / 2.0, 1.0)[2],
        name as i32
    );
    let (x, y, w, h) = p.rect_of(id(&p, "jump")).unwrap();
    assert_eq!(
        p.reader_under(x + w / 2.0, y + h / 2.0, 1.0)[2],
        0,
        "a button"
    );
}

#[test]
fn a_field_keeps_its_identity_while_its_node_lives() {
    let (_shell, _hosting) = shell();
    let mut p = boot(LONG);
    let (list, first) = (id(&p, "list"), id(&p, "f0"));
    let before = told(&mut p);
    assert_eq!(before.len(), 12);
    assert_eq!((before[0].id, before[0].flags & 1), (first, 1));
    assert_eq!(
        before[0].clip,
        [0.0, 0.0, 300.0, 200.0],
        "its scroller's port"
    );
    let at = before[0].rect;
    p.reader_field_edited(first, (0, 1), "kept", (2, 2), 1.0);
    // Its row scrolls out of the port: the field is still the reader's,
    // the same one, with the text typed into it.
    assert_eq!(p.scroll_to(list, (0.0, 150.0)), Some((0.0, 150.0)));
    settle(&mut p);
    let out = told(&mut p);
    assert_eq!((out[0].id, out[0].generation, out[0].seq), (first, 0, 1));
    assert_eq!(out[0].value, "kept");
    if out[0].flags & 1 != 0 {
        assert_eq!(out[0].rect[1], at[1] - 150.0, "above the port");
    }
    // And back: where it was.
    p.scroll_to(list, (0.0, 0.0));
    settle(&mut p);
    let back = told(&mut p);
    assert_eq!((back[0].generation, back[0].seq), (0, 1));
    assert_eq!((back[0].rect, back[0].value.as_str()), (at, "kept"));
    assert_eq!(p.field_selection(first).start, 2, "and its caret");
    // Its node renewed for another row's item (LLP 1078): another field.
    p.hosted_renewed(&[first]);
    let renewed = told(&mut p);
    assert_eq!((renewed[0].generation, renewed[0].seq), (1, 0));
    assert_eq!(renewed[0].value, "", "nothing of the last item's text");
    // A field whose node is gone leaves the statement.
    assert_eq!(renewed.len(), 12);
}

#[test]
fn desktop_linux_hosts_nothing_and_paints_its_fields() {
    // No shell on this thread: the host every other test of this crate runs.
    assert!(crate::fields::hosting().is_none());
    let mut p = boot(FORM);
    p.serve_requests();
    let name = id(&p, "name");
    assert_eq!(p.reader_fields(1.0), None, "no field is told");
    let (x, y, w, h) = p.rect_of(name).unwrap();
    assert_eq!(p.reader_under(x + w / 2.0, y + h / 2.0, 1.0)[2], 0);
    // A press focuses it, and the keyboard asked is this host's own.
    tap(&mut p, "name");
    assert_eq!(p.focus(), Some(name));
    assert_eq!(keyboard(&mut p), Some((1, 0)));
    // What a reader would say of a field is nobody's here.
    p.reader_field_edited(name, (0, 1), "nobody", (6, 6), 1.0);
    assert_eq!(p.field_text(name), "");
    // Keys type into it, one at a time, and its text is painted.
    p.key(Some('h'), false, 2.0);
    p.key(Some('i'), false, 3.0);
    assert_eq!(log(&p), "hi|||[h 1-1][hi 2-2]");
    assert!(inked(&mut p, "name"), "the painter draws the field's text");
}

#[test]
#[cfg(not(windows))]
fn scroll_end_distinguishes_field_navigation_from_keyboard_insets() {
    let (_shell, _hosting) = shell();
    for displayed in [false, true] {
        let mut p = boot(
            r#"component App
  state order = ""
  action moved(x: number, y: number)
    order = `${order}s`
  action ended
    order = `${order}e`
  view
    main viewport-fit="cover" width="100%" height="100%" display="flex" flex-direction="column"
      scroll testId="port" flex=1 width="100%" scroll=moved scrollend=ended
        input testId="first" height=40
        box height=600
        input testId="last" height=40
"#,
        );
        let window = Window {
            size: (320., 480.),
            ..Window::default()
        };
        assert_eq!(p.set_window(window), None);
        let acknowledge = |p: &mut Presenter<()>| {
            if displayed {
                let frame = p.display_frame().unwrap();
                assert!(p.display_complete(&frame));
            }
        };
        acknowledge(&mut p);
        p.reader_fields(1.);
        let (port, first, last) = (id(&p, "port"), id(&p, "first"), id(&p, "last"));
        p.reader_field_focused(first, 0, true, 0.);
        p.reader_field_action(first, 0, field_action::NEXT, 0.);
        assert_eq!(p.focus(), Some(last));
        assert!(p.scroll_of(port).1 > 0.);
        acknowledge(&mut p);
        p.pump(0.);
        assert_eq!(
            p.host.runner().slot("order"),
            Some(&exact_runner::Value::str("se"))
        );
        let before = p.scroll_of(port);
        let typing = Window {
            keyboard: [0., 0., 200., 0.],
            ..window
        };
        assert_eq!(p.set_window(typing), None);
        acknowledge(&mut p);
        acknowledge(&mut p);
        assert!(p.scroll_of(port).1 > before.1);
        p.pump(0.);
        assert_eq!(
            p.host.runner().slot("order"),
            Some(&exact_runner::Value::str("ses"))
        );
        assert_eq!(p.timer_due_ms(), None);
        p.reader_field_action(last, 0, field_action::PREVIOUS, 0.);
        assert_eq!(p.focus(), Some(first));
        assert_eq!(p.scroll_of(port).1, 0.);
        acknowledge(&mut p);
        p.pump(0.);
        assert_eq!(
            p.host.runner().slot("order"),
            Some(&exact_runner::Value::str("sesse"))
        );
        assert_eq!(p.window(), Some(&typing));
    }
}
