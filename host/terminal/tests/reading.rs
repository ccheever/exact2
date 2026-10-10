//! What a reader needs from the terminal host (the LLP reader, LLP 1101):
//! `scrollIntoView` by an element's `id`, the page's keys scrolling when
//! nothing is focused, a shortcut that acts without taking the focus, and a
//! data module's continuation run to its answer.

use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, Outcome, Request, Response, Store};
use exact_terminal::host::{Host, Key, Mode};
use exact_terminal::vt::Vt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Forty numbered lines in a scroller under a one-row header, a status
/// line with two shortcuts, and actions that bring a line into view.
const PAGE: &str = r#"component App
  state pressed = 0
  action toLine(id: string)
    scrollIntoView(id, block="start")
  action centre(id: string)
    scrollIntoView(id, block="center")
  action bump
    pressed = pressed + 1
  action by(rows: number)
    scrollBy("page", 0, rows * 16)
  view
    column width="100%" height="100%"
      text `header ${pressed}`
      scroll id="page" flex=1 min-height=0 testId="page"
        each i in LINES key=i
          text `line ${i}` id=`l${i}`
      row
        button appearance="none" press=toLine("l30") aria-keyshortcuts="j" testId="jump"
          text "j jump"
        button appearance="none" press=centre("l20") aria-keyshortcuts="c" testId="centre"
          text "c centre"
        button appearance="none" press=bump aria-keyshortcuts="b" testId="bump"
          text "b bump"
        button appearance="none" press=by(1) aria-keyshortcuts="d" testId="down"
          text "d"
        button appearance="none" press=by(-3) aria-keyshortcuts="u" testId="up"
          text "u"
"#;

fn boot() -> (Host<()>, Vt) {
    let lines: Vec<String> = (0..40).map(|i| i.to_string()).collect();
    let src = PAGE.replace("LINES", &format!("[{}]", lines.join(", ")));
    let plan = contract::compile(&src).expect("compiles");
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 30, 12).expect("boots");
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    (host, vt)
}

/// The first `line N` on the screen.
fn top(vt: &Vt) -> usize {
    vt.text(false)
        .lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix("line ")?
                .trim_end_matches(['┃', '│', ' '])
                .parse()
                .ok()
        })
        .expect("a line on the screen")
}

fn keys(host: &mut Host<()>, vt: &mut Vt, keys: &[Key]) {
    host.read_at = host.frames;
    for k in keys {
        host.key(k.clone());
    }
    vt.render(host);
}

#[test]
fn scroll_into_view_brings_an_element_to_the_start_or_the_centre() {
    let (mut host, mut vt) = boot();
    assert_eq!(top(&vt), 0);
    keys(&mut host, &mut vt, &[Key::Char('j')]);
    assert_eq!(top(&vt), 30, "{}", vt.text(false));
    // Ten rows of page: line 20 in the middle has line 15 or 16 at the top.
    keys(&mut host, &mut vt, &[Key::Char('c')]);
    let at = top(&vt);
    assert!((15..=16).contains(&at), "{at}:\n{}", vt.text(false));
}

#[test]
fn with_nothing_focused_the_page_keys_scroll() {
    let (mut host, mut vt) = boot();
    keys(
        &mut host,
        &mut vt,
        &[Key::Named("ArrowDown"), Key::Named("ArrowDown")],
    );
    assert_eq!(top(&vt), 2);
    keys(&mut host, &mut vt, &[Key::Char(' ')]);
    assert_eq!(top(&vt), 11, "a page is the scroller's height less a row");
    keys(&mut host, &mut vt, &[Key::Named("End")]);
    assert_eq!(top(&vt), 30);
    keys(&mut host, &mut vt, &[Key::Named("Home")]);
    assert_eq!(top(&vt), 0);
}

#[test]
fn a_shortcut_acts_without_taking_the_focus() {
    let (mut host, mut vt) = boot();
    keys(&mut host, &mut vt, &[Key::Char('b')]);
    assert!(vt.text(false).contains("header 1"), "{}", vt.text(false));
    // The arrows still scroll the page: nothing took the focus.
    keys(&mut host, &mut vt, &[Key::Named("ArrowDown")]);
    assert_eq!(top(&vt), 1);
}

/// A source whose answer comes from work run off the runner's thread.
#[derive(Default)]
struct Slow {
    done: Arc<AtomicBool>,
}

impl DataSource for Slow {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::record(vec![Value::str("loading")]))
    }

    fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
        Ok(if self.done.load(Ordering::SeqCst) {
            Answer::Now(Value::record(vec![Value::str("read")]))
        } else {
            Answer::Later(Request::continuation(7))
        })
    }

    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        assert_eq!(token, 7);
        let done = self.done.clone();
        Some(Box::new(move || {
            std::thread::sleep(Duration::from_millis(20));
            done.store(true, Ordering::SeqCst);
            Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: Vec::new(),
            })
        }))
    }

    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        self.answer(store, source, args)
    }
}

#[test]
fn a_continuation_runs_and_its_answer_lands() {
    let src = "shape Word\n  text: string\ncomponent App\n  resource word = word(\"x\") as shape Word\n  view\n    text `word ${word.text}`\n";
    let plan = contract::compile(src).expect("compiles");
    let mut host = Host::boot(plan, Slow::default(), Mode::Fullscreen, 30, 4).expect("boots");
    let woken = Arc::new(AtomicBool::new(false));
    let flag = woken.clone();
    host.listen(Arc::new(move || flag.store(true, Ordering::SeqCst)));
    let start = Instant::now();
    loop {
        if woken.swap(false, Ordering::SeqCst) {
            host.announced();
        }
        if host.frame().grid.text().contains("word read") {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "{}",
            host.frame().grid.text()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn scroll_by_moves_a_scroller_by_rows() {
    let (mut host, mut vt) = boot();
    keys(
        &mut host,
        &mut vt,
        &[
            Key::Char('d'),
            Key::Char('d'),
            Key::Char('d'),
            Key::Char('d'),
        ],
    );
    assert_eq!(top(&vt), 4, "{}", vt.text(false));
    keys(&mut host, &mut vt, &[Key::Char('u')]);
    assert_eq!(top(&vt), 1);
    // Past the start it stops there.
    keys(&mut host, &mut vt, &[Key::Char('u')]);
    assert_eq!(top(&vt), 0);
}

#[test]
fn n_and_shift_n_are_two_shortcuts() {
    let src = "component App\n  state said = \"\"\n  action say(w: string)\n    said = `${said}${w}`\n  view\n    column\n      text `said ${said}`\n      button appearance=\"none\" press=say(\"n\") aria-keyshortcuts=\"n\"\n        text \"next\"\n      button appearance=\"none\" press=say(\"N\") aria-keyshortcuts=\"Shift+N\"\n        text \"prev\"\n";
    let plan = contract::compile(src).expect("compiles");
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 30, 4).expect("boots");
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    host.read_at = host.frames;
    for c in ['n', 'N', 'n'] {
        host.key(Key::Char(c));
    }
    vt.render(&mut host);
    assert!(vt.text(false).contains("said nNn"), "{}", vt.text(false));
}

/// A paragraph with a run that has a handler and one that only links out.
const RUNS: &str = r#"component App
  state n = 0
  action hit
    n = n + 1
  view
    column
      text `n ${n}`
      text
        text "see "
        text "here" press=hit
        text " or "
        text "the web" href="https://example.com/a"
"#;

fn at(vt: &Vt, word: &str) -> (i32, i32) {
    let screen = vt.text(false);
    let (y, line) = screen
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains(word))
        .expect(word);
    (line.find(word).unwrap() as i32, y as i32)
}

#[test]
fn a_click_on_an_inline_run_presses_it_or_follows_its_link() {
    let plan = contract::compile(RUNS).expect("compiles");
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 40, 4).expect("boots");
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    let (x, y) = at(&vt, "here");
    host.click(x + 1, y);
    vt.render(&mut host);
    assert!(vt.text(false).contains("n 1"), "{}", vt.text(false));
    // A run that only links goes out of the app.
    let (x, y) = at(&vt, "the web");
    host.click(x + 4, y);
    assert_eq!(host.outbound, vec!["https://example.com/a".to_string()]);
    // Between the runs, nothing.
    host.outbound.clear();
    let (x, y) = at(&vt, " or ");
    host.click(x + 1, y);
    vt.render(&mut host);
    assert!(host.outbound.is_empty() && vt.text(false).contains("n 1"));
}

#[test]
fn a_link_to_a_route_navigates() {
    let src = r##"routes nav
  home "/"
  note "/note/:note"
  notfound

component App
  derive current = top(nav)
  action follow(url: string)
    nav = go(nav, url)
  view
    main navigationKey=`${current.id}` navigationBack="back" navigate=follow width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` position="absolute" inset=0 background-color="#000000"
          text `at ${e.name}`
          text
            text "go to "
            text "note three" href="/note/3"
"##;
    let plan = contract::compile(src).expect("compiles");
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 40, 4).expect("boots");
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    assert!(vt.text(false).contains("at home"), "{}", vt.text(false));
    let (x, y) = at(&vt, "note three");
    host.click(x, y);
    vt.render(&mut host);
    assert!(vt.text(false).contains("at note"), "{}", vt.text(false));
    assert!(host.outbound.is_empty());
}

#[test]
fn compile_terminal_applies_the_terminal_profile() {
    let cells = "component App\n  view\n    column width=\"10ch\"\n      text \"ok\"\n";
    contract::compile_terminal(cells).expect("cells are a terminal's lengths");
    let pixels = "component App\n  view\n    column width=80\n      text \"no\"\n";
    let refused = contract::compile_terminal(pixels).expect_err("pixels are refused");
    assert!(refused.message.contains("ch"), "{refused}");
    // The same source compiles for every other surface.
    contract::compile(pixels).expect("a pixel width elsewhere");
}
