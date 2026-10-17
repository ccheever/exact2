//! The channel out (LLP 1115.001 D5): what a commit asks of the platform is
//! queued for the shell, taken once and answered by its id; a host with no
//! shell keeps what it did.
use super::*;
use exact_runner::{DataError, Value};

/// A source that answers nothing, and is granted notifications.
#[derive(Default)]
struct Nothing;
impl DataSource for Nothing {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
    fn grants(&self) -> &str {
        "device.notifications notify-reason"
    }
}

/// Every command the channel carries, a link of each kind that leaves the
/// app, a pressable with a press haptic, two fields and a second route.
const APP: &str = r#"routes nav
  home "/"
    detail "/detail"
component App
  state name = ""
  action buzz
    haptic("success")
  action purr
    haptic("purr")
  action copy
    copyText("Copied from a test")
  action web
    openURL("https://example.com/a?b=c")
  action file
    openURL("file:///etc/passwd")
  action send
    share(title="A post", text="Read this", url="https://example.com/post/1")
  action words
    share(text="héllo")
  action note
    showNotification(title="Stretch")
  action open
    nav = push(nav, "/detail")
  action rename(value)
    name = value
  view
    main navigationKey=`${top(nav).id}` width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` position="absolute" inset=0 align-items="flex-start"
          if e.name == "home"
            button press=buzz testId="buzz" height=30
              text "Buzz"
            button press=purr testId="purr" height=30
              text "Purr"
            button press=copy testId="copy" height=30
              text "Copy"
            button press=web testId="web" height=30
              text "Web"
            button press=file testId="file" height=30
              text "File"
            button press=send testId="send" height=30
              text "Send"
            button press=words testId="words" height=30
              text "Words"
            button press=note testId="note" height=30
              text "Note"
            button press=open testId="open" height=30
              text "Open"
            link href="mailto:someone@example.com" testId="mail" height=30
              text "Mail"
            link href="tel:+15550100" testId="phone" height=30
              text "Phone"
            button press=buzz -exact-press-haptic="impact-light" testId="thump" height=30
              text "Thump"
            input value=name input=rename testId="field" width=200 height=30
            textarea value=name input=rename testId="area" width=200 rows=2
          if e.name == "detail"
            text "Detail" testId="detail"
"#;

fn boot() -> Presenter<Nothing> {
    let (p, error) = Presenter::boot_with(
        &crate::test_plan(APP),
        Nothing,
        (400., 800.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}

fn id(p: &Presenter<Nothing>, name: &str) -> ViewId {
    let kernel = p.host().kernel();
    kernel
        .node_by_key(kernel.find_by_test_id(name)[0])
        .unwrap()
        .id
}

/// Tap `name` and run the commands its action issued, with no frame.
fn tap(p: &mut Presenter<Nothing>, name: &str) {
    let view = id(p, name);
    p.tap(view).unwrap();
    p.run_commands(|| Nothing);
}

/// The shell's reading of the words: each request's kind, id and body.
fn decode(words: Option<Vec<i32>>) -> Vec<(i32, u32, Vec<i32>)> {
    let words = words.unwrap_or_default();
    let (mut out, mut at) = (Vec::new(), 0);
    while at < words.len() {
        let n = words[at + 2] as usize;
        out.push((
            words[at],
            words[at + 1] as u32,
            words[at + 3..at + 3 + n].to_vec(),
        ));
        at += 3 + n;
    }
    out
}

/// The strings in a request's body from word `at` on, as the shell reads
/// them: a byte length, then UTF-8 padded to whole words.
fn strings(body: &[i32], mut at: usize) -> Vec<String> {
    let mut out = Vec::new();
    while at < body.len() {
        let n = body[at] as usize;
        let bytes: Vec<u8> = body[at + 1..at + 1 + n.div_ceil(4)]
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .take(n)
            .collect();
        out.push(String::from_utf8(bytes).unwrap());
        at += 1 + n.div_ceil(4);
    }
    out
}

fn journal(p: &Presenter<Nothing>) -> Vec<String> {
    p.host().runner().journal().map(str::to_owned).collect()
}

fn says(p: &Presenter<Nothing>, what: &str) -> usize {
    journal(p).iter().filter(|l| l.contains(what)).count()
}

#[test]
fn desktop_linux_has_no_shell_and_keeps_what_each_command_did() {
    let mut p = boot();
    for name in [
        "buzz", "purr", "copy", "web", "send", "note", "mail", "thump",
    ] {
        tap(&mut p, name);
    }
    p.tell_shell();
    assert_eq!(p.take_requests(), None, "nothing is queued without a shell");
    assert_eq!(
        says(&p, "share: refused: unavailable"),
        1,
        "{:?}",
        journal(&p)
    );
    assert_eq!(
        says(
            &p,
            "openURL mailto:someone@example.com: the Linux host has no browser"
        ),
        1
    );
    assert_eq!(says(&p, "showNotification: refused: unavailable"), 1);
    assert_eq!(says(&p, "does not carry it yet"), 0);
    assert_eq!(
        says(&p, "is not one this platform plays"),
        0,
        "a haptic is nothing there, whatever its name: {:?}",
        journal(&p)
    );
}

#[test]
fn each_request_is_queued_taken_once_and_answered_by_its_id() {
    let mut p = boot();
    p.serve_requests();
    // A haptic from an action, by Exact's name; one no host plays is
    // journaled and asks nothing.
    tap(&mut p, "buzz");
    tap(&mut p, "purr");
    let taken = decode(p.take_requests());
    assert_eq!(taken, [(2, 1, vec![Haptic::Success as i32])]);
    assert_eq!(p.take_requests(), None, "a request is taken once");
    assert_eq!(
        says(&p, "haptic \"purr\" is not one this platform plays"),
        1
    );
    // The clipboard.
    tap(&mut p, "copy");
    let taken = decode(p.take_requests());
    assert_eq!((taken[0].0, taken[0].1), (3, 2), "ids go up by one");
    assert_eq!(strings(&taken[0].2, 0), ["Copied from a test"]);
    // A link out of the app: a command's, a `mailto` link's and a `tel`
    // link's, each as written; a scheme that does not leave is refused.
    tap(&mut p, "web");
    tap(&mut p, "file");
    tap(&mut p, "mail");
    tap(&mut p, "phone");
    let taken = decode(p.take_requests());
    assert_eq!(taken.iter().map(|r| r.0).collect::<Vec<_>>(), [1, 1, 1]);
    let urls: Vec<String> = taken.iter().flat_map(|r| strings(&r.2, 0)).collect();
    assert_eq!(
        urls,
        [
            "https://example.com/a?b=c",
            "mailto:someone@example.com",
            "tel:+15550100"
        ]
    );
    assert_eq!(says(&p, "openURL refused: \"file:///etc/passwd\""), 1);
    assert_eq!(says(&p, "no browser"), 0, "nothing is logged as unopenable");
    // The share sheet: what the app gave, and which of the three it gave.
    tap(&mut p, "send");
    tap(&mut p, "words");
    let taken = decode(p.take_requests());
    let (first, second) = (taken[0].1, taken[1].1);
    assert_eq!((taken[0].0, taken[0].2[0]), (4, 0b111));
    assert_eq!(
        strings(&taken[0].2, 1),
        ["A post", "Read this", "https://example.com/post/1"]
    );
    assert_eq!((taken[1].0, taken[1].2[0]), (4, 0b010));
    assert_eq!(strings(&taken[1].2, 1), ["", "héllo", ""]);
    assert_eq!(says(&p, "share:"), 0, "no outcome before the shell answers");
    // Answered by id, in any order, once each; a cancellation is a dismissal.
    assert!(p.answer_request(second, Some("shared")));
    assert_eq!(says(&p, "share: shared"), 1);
    assert!(
        !p.answer_request(second, Some("shared")),
        "answered already"
    );
    assert!(
        !p.answer_request(1, Some("shared")),
        "a haptic waits for none"
    );
    assert!(p.answer_request(first, None));
    assert_eq!(
        (says(&p, "share: shared"), says(&p, "share: dismissed")),
        (1, 1)
    );
    tap(&mut p, "send");
    let again = decode(p.take_requests())[0].1;
    assert!(p.answer_request(again, Some("no app can take it")));
    assert_eq!(says(&p, "share: refused: no app can take it"), 1);
    // A press haptic plays at the press, before any release or commit.
    let thump = id(&p, "thump");
    let (x, y, w, h) = p.rect_of(thump).unwrap();
    p.pointer_down(x + w / 2., y + h / 2., 100.).unwrap();
    let taken = decode(p.take_requests());
    assert_eq!(
        taken.iter().map(|r| (r.0, r.2.clone())).collect::<Vec<_>>(),
        [(2, vec![Haptic::ImpactLight as i32])]
    );
    p.pointer_cancel(110.).unwrap();
    assert_eq!(p.take_requests(), None);
    // What the channel does not carry yet says so beside the refusal.
    tap(&mut p, "note");
    assert_eq!(says(&p, "showNotification: refused: unavailable"), 1);
    assert_eq!(
        says(
            &p,
            "showNotification: the host's channel to the platform (LLP 1115.001 D5) does not carry it yet"
        ),
        1
    );
    assert_eq!(p.take_requests(), None);
}

/// Whether the executor's wake is readable within `ms`.
#[cfg(unix)]
#[allow(unsafe_code)] // poll(2) on the wake, as the shell's loop waits on it
fn woken(p: &Presenter<Nothing>, ms: i32) -> bool {
    let mut fds = libc::pollfd {
        fd: p.executor_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: one valid pollfd for the duration of the call.
    unsafe { libc::poll(&mut fds, 1, ms) > 0 }
}

/// A haptic changes no pixel, so no frame follows it: the shell is woken by
/// the descriptor it already waits on, and finds the request there.
#[test]
#[cfg(unix)]
fn a_request_made_with_no_frame_still_wakes_the_shell() {
    let mut p = boot();
    p.serve_requests();
    p.frame();
    // Quiet first: whatever boot left on the wake is drained.
    for _ in 0..64 {
        if !woken(&p, 0) {
            break;
        }
        p.pump(p.host().now());
    }
    assert!(!woken(&p, 0), "the wake never went quiet");
    assert!(!p.dirty(), "nothing to paint");
    p.haptic("success");
    assert!(!p.dirty(), "a haptic asks for no frame");
    assert!(woken(&p, 2000), "the request did not wake the shell");
    // The shell's turn: it drains the wake and takes what is queued.
    p.pump(p.host().now());
    assert_eq!(decode(p.take_requests()).len(), 1);
    assert!(!woken(&p, 0), "taken: the wake is quiet again");
}

#[test]
fn the_shell_is_told_when_back_has_somewhere_to_go_and_a_field_holds_the_focus() {
    let mut p = boot();
    p.serve_requests();
    let told = |p: &mut Presenter<Nothing>| -> Vec<(i32, i32)> {
        p.tell_shell();
        decode(p.take_requests())
            .iter()
            .map(|r| (r.0, r.2[0]))
            .collect()
    };
    // At the first screen Back has nowhere to go, and no field is focused.
    assert_eq!(told(&mut p), [(5, 0), (6, 0)]);
    assert_eq!(told(&mut p), [], "told once, while nothing changes");
    tap(&mut p, "open");
    assert_eq!(told(&mut p), [(5, 1)], "a route was pushed");
    assert!(p.system_back(10.));
    assert_eq!(told(&mut p), [(5, 0)], "and popped");
    // A field takes the focus: the keyboard, for an input and a textarea.
    tap(&mut p, "field");
    assert_eq!(told(&mut p), [(6, 1)]);
    tap(&mut p, "area");
    assert_eq!(told(&mut p), [(6, 3)]);
    tap(&mut p, "buzz");
    assert_eq!(told(&mut p), [(2, Haptic::Success as i32), (6, 0)]);
    // A shell made again is told the state anew, and what the last one was
    // asked and never answered is cancelled.
    tap(&mut p, "send");
    tap(&mut p, "field");
    let asked = decode(p.take_requests());
    let share = asked.iter().find(|r| r.0 == 4).unwrap().1;
    p.requests_reset();
    assert_eq!(says(&p, "share: dismissed"), 1);
    assert!(!p.answer_request(share, Some("shared")));
    assert_eq!(told(&mut p), [(5, 0), (6, 1)]);
}

/// A date control and a time control, bound.
const DATES: &str = r##"component App
  state day = "2026-10-16"
  state at = ""
  action setDay(value)
    day = value
  action setAt(value)
    at = value
  view
    column
      input type="date" value=day change=setDay min="2026-01-01" max="2026-12-31" testId="day" height=30
      input type="time" value=at change=setAt step="60" accent-color="#ff0000" testId="at" height=30
      input type="date" value=day change=setDay readonly=true testId="fixed" height=30
"##;

#[test]
fn a_date_or_time_controls_press_asks_the_shell_for_its_picker_and_its_answer_is_the_value() {
    let boot = || {
        Presenter::boot_with(
            &crate::test_plan(DATES),
            Nothing,
            (400., 800.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap()
        .0
    };
    let value = |p: &Presenter<Nothing>, name: &str| {
        let node = id(p, name);
        p.host()
            .kernel()
            .node(node)
            .unwrap()
            .props
            .str(PropId::Value)
            .unwrap_or("")
            .to_owned()
    };
    // With no shell nothing is asked.
    let mut p = boot();
    tap(&mut p, "day");
    assert_eq!(p.take_requests(), None);
    let mut p = boot();
    p.serve_requests();
    tap(&mut p, "day");
    let asked = decode(p.take_requests());
    assert_eq!(asked.len(), 1);
    let (kind, request, body) = &asked[0];
    assert_eq!((*kind, body[0] as u32), (9, id(&p, "day")));
    // Five strings, then the accent: none set.
    assert_eq!(
        strings(&body[..body.len() - 1], 1),
        ["date", "2026-10-16", "2026-01-01", "2026-12-31", ""]
    );
    assert_eq!(body.last(), Some(&0));
    // The answer is the person's choice: `change` runs, and the bound value moves.
    assert!(p.answer_request(*request, Some("2026-11-02")));
    p.run_commands(|| Nothing);
    assert_eq!(value(&p, "day"), "2026-11-02");
    tap(&mut p, "at");
    let asked = decode(p.take_requests());
    let body = &asked[0].2;
    assert_eq!(
        strings(&body[..body.len() - 1], 1),
        ["time", "", "", "", "60"]
    );
    assert_eq!(
        *body.last().unwrap() as u32,
        0xffff_0000,
        "its accent-color"
    );
    // A dismissal changes nothing, and is journaled.
    assert!(p.answer_request(asked[0].1, None));
    assert_eq!(value(&p, "at"), "");
    assert_eq!(says(&p, "picker: dismissed"), 1);
    // A read-only control opens none.
    tap(&mut p, "fixed");
    assert_eq!(p.take_requests(), None);
    // While a control's picker is up, a press opens no second; a reader
    // made again over the same activity leaves it up, and it still answers.
    tap(&mut p, "day");
    let asked = decode(p.take_requests());
    tap(&mut p, "day");
    assert_eq!(p.take_requests(), None);
    p.reader_replaced(Box::new(crate::raster::Raster::new()));
    let _ = p.take_requests();
    assert!(p.answer_request(asked[0].1, Some("2026-12-24")));
    p.run_commands(|| Nothing);
    assert_eq!(value(&p, "day"), "2026-12-24");
    // A view made again while its picker is up drops the answer.
    tap(&mut p, "day");
    let asked = decode(p.take_requests());
    p.picks_renewed(&[id(&p, "day")]);
    assert!(!p.answer_request(asked[0].1, Some("2026-12-25")));
    assert_eq!(value(&p, "day"), "2026-12-24");
}
