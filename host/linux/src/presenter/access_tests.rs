//! The accessibility tree a shell is told (LLP 1115.001 D13), against a
//! stand-in for Android's reader: what Shelf's screens are in the order a
//! person reads, what a node's identity outlives, what an open surface
//! leaves of the page, a hosted field's place, what a statement says
//! changed, what an action does, and that a host whose shell never asks
//! (desktop Linux) builds nothing.
use super::*;
use crate::fields::{FieldFace, FieldMetrics};
use crate::presenter::top::SurfacePainter;
use exact_runner::{DataError, Value};

/// Shelf's data: a library of `0` books, no works, the clock; and for the
/// list fixture, 400 rows.
struct Data(usize);
impl DataSource for Data {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        let book = |n: usize| {
            let text = |s: &str| Value::str(s);
            Value::record(vec![
                Value::str(&format!("seed-{n}")),
                Value::str(&format!("Book {n}")),
                text("Susanna Clarke"),
                Value::Number(272.0),
                Value::Number(120.0),
                text("reading"),
                Value::Number(0.0),
                text(""),
                Value::Number(0.0),
                text(""),
                text(""),
            ])
        };
        match name {
            "library" => Ok(Value::record(vec![
                Value::Bool(true),
                Value::list((1..=self.0).map(book).collect()),
                Value::Number(12.0),
                Value::Number(0.0),
                Value::str("—"),
            ])),
            "subjectWorks" => Ok(Value::record(vec![
                Value::Bool(true),
                Value::str(""),
                Value::list(Vec::new()),
            ])),
            "exactTime" => Ok(Value::record(vec![Value::Number(0.0)])),
            "rows" => Ok(Value::list(
                (0..400).map(|n| Value::Number(n as f64)).collect(),
            )),
            _ => Err(DataError::UnknownSource(name.into())),
        }
    }
}

const SHELF: &str = include_str!("../../../../apps/shelf/app.contract");

/// A heading, a level with no heading's role (text, LLP 1115.000 D12.5), a
/// checkbox, a switch, a slider, a toggle, a status line the button changes,
/// a link, a named image, words hidden from assistive technology, a
/// disabled button, and a field.
const CONTROLS: &str = r#"component App
  state on = false
  state loud = true
  state level = 40
  state said = "Nothing yet"
  action flip(checked: bool)
    on = checked
  action shout(checked: bool)
    loud = checked
  action set(value: number)
    level = value
  action say
    said = "Saved"
  view
    column width=320 height=600 align-items="flex-start"
      text "Settings" role="heading" aria-level=1 testId="title"
      text "On this phone" aria-level=2 testId="levelled"
      input type="checkbox" checked=on change=flip aria-label="Remember me" testId="check"
      input type="checkbox" role="switch" checked=loud change=shout aria-label="Sound" testId="switch"
      input type="range" min=0 max=100 step=10 value=level input=set aria-label="Volume" testId="slider"
      button press=say aria-pressed=`${on}` testId="toggle" height=30
        text "Bold"
      button press=say testId="save" height=30
        text "Save"
      button disabled=true testId="off" height=30
        text "Unavailable"
      text said role="status" testId="status" width=200 height=20
      link href="https://example.com/" testId="link"
        text "Read more"
      image "symbol:star" aria-label="Favourite" testId="star"
      text "decoration" aria-hidden=true
      input aria-label="Name" placeholder="Your name" testId="name"
"#;

/// A virtualized list of 400 rows, each a button, under a heading.
const ROWS: &str = r#"component App
  resource rows = rows() as shape list<number>
  state picked = -1
  action pick(n: number)
    picked = n
  view
    column width=320 height=480
      text `Picked ${picked}` testId="picked" height=20
      list virtualized=true width=320 height=400 testId="port"
        each x in rows key=x
          button press=pick(x) height=40 width=300
            text `Row ${x}`
"#;

/// A status line, and a menu whose one row changes what it says.
const MENU: &str = r#"component App
  state said = "Nothing yet"
  action say
    said = "Saved"
  view
    column width=320 height=400 align-items="flex-start"
      text said role="status" testId="status" width=200 height=20
      button popovertarget="more" aria-label="More" testId="more" height=30
        text "More"
      column id="more" popover="auto" role="menu" aria-label="More"
        button press=say popovertarget="more" popovertargetaction="hide" testId="menu-save"
          text "Save"
"#;

/// The shell's painter for a bar or a window: it draws nothing.
struct Blank;
impl SurfacePainter for Blank {
    fn record(&mut self, _: (f32, f32), _: f32, _: &mut dyn FnMut(&mut dyn Backend)) -> Vec<u32> {
        vec![1, 1]
    }
}

/// The reader's field, stood in for.
#[derive(Default)]
struct Shell;
impl FieldMetrics for Shell {
    fn measure(&self, face: &FieldFace, _: &str, width: Option<f32>) -> [f32; 3] {
        [width.unwrap_or(100.0), face.size * 1.25, face.size]
    }
}

/// A Pixel's window in points: a 52 status bar and a 24 gesture bar.
const WINDOW: Window = Window {
    size: (412.0, 916.0),
    bars: [52.0, 0.0, 24.0, 0.0],
    cutout: [0.0; 4],
    keyboard: [0.0; 4],
    gestures: [0.0; 4],
};

fn boot(source: &str, size: (f32, f32), books: usize) -> Presenter<Data> {
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(source).unwrap().encode(),
        Data(books),
        size,
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    settle(&mut p);
    p
}

/// Shelf as Android's reader shows it: bars, windows and the system's
/// requests served, in a phone's window.
fn shelf(books: usize) -> Presenter<Data> {
    let mut p = boot(SHELF, (412., 840.), books);
    p.serve_requests();
    p.serve_windows(Box::new(Blank));
    assert_eq!(p.serve_bars(Box::new(Blank)), None);
    assert_eq!(p.set_window(WINDOW), None);
    settle(&mut p);
    p
}

fn settle(p: &mut Presenter<Data>) {
    for _ in 0..12 {
        assert!(p.pump(p.host.now()).is_none());
        p.run_commands(|| Data(0));
        if p.dirty() {
            let _ = p.frame();
        }
    }
}

fn id(p: &Presenter<Data>, name: &str) -> ViewId {
    let kernel = p.host().kernel();
    let found = kernel.find_by_test_id(name);
    assert!(!found.is_empty(), "no node has the test id {name}");
    kernel.node_by_key(found[0]).unwrap().id
}

fn tap(p: &mut Presenter<Data>, name: &str) {
    let view = id(p, name);
    p.tap(view).unwrap();
    settle(p);
}

/// The tree, each node as its depth, its role, its name and its value.
fn read(p: &mut Presenter<Data>) -> Vec<Node> {
    p.access_tree().0
}

fn named<'a>(nodes: &'a [Node], name: &str) -> &'a Node {
    (nodes.iter().find(|n| n.name == name)).unwrap_or_else(|| panic!("no node is named {name}"))
}

fn names(nodes: &[Node]) -> Vec<(i32, &str)> {
    nodes.iter().map(|n| (n.role, n.name.as_str())).collect()
}

fn string(words: &[i32], at: usize) -> (String, usize) {
    let n = words[at] as usize;
    let bytes: Vec<u8> = (0..n)
        .map(|k| (words[at + 1 + k / 4] as u32 >> (8 * (k % 4))) as u8)
        .collect();
    (String::from_utf8(bytes).unwrap(), at + 1 + n.div_ceil(4))
}

/// A statement as the reader reads it.
#[derive(Debug)]
struct Told {
    host: i32,
    whole: bool,
    pane: (i32, String),
    changes: Vec<(i32, i32, f32, f32)>,
    nodes: Vec<Node>,
}

/// A statement's words, read as the reader reads them.
fn told(words: &[i32]) -> Told {
    let (host, whole, pane) = (words[1], words[2] == 1, words[3]);
    let (title, mut at) = string(words, 4);
    let float = |at: usize| f32::from_bits(words[at] as u32);
    let changes = (0..words[at] as usize)
        .map(|k| at + 1 + 4 * k)
        .map(|c| (words[c], words[c + 1], float(c + 2), float(c + 3)))
        .collect::<Vec<_>>();
    at += 1 + 4 * changes.len();
    let count = words[at] as usize;
    at += 1;
    let mut nodes = Vec::new();
    for _ in 0..count {
        let w = &words[at..at + 14];
        let (name, next) = string(words, at + 14);
        let (value, next) = string(words, next);
        let (hint, next) = string(words, next);
        nodes.push(Node {
            id: w[0],
            parent: w[1],
            view: w[2] as ViewId,
            role: w[3],
            flags: w[4],
            rect: [w[5] as f32, w[6] as f32, w[7] as f32, w[8] as f32],
            range: [float(at + 9), float(at + 10), float(at + 11)],
            set: (w[12], w[13]),
            name,
            value,
            hint,
        });
        at = next;
    }
    assert_eq!(at, words.len(), "the statement is read to its end");
    Told {
        host,
        whole,
        pane: (pane, title),
        changes,
        nodes,
    }
}

/// The reader's field hosting this thread's fields until the guard drops.
fn hosting() -> impl Drop {
    crate::fields::host_here(Box::leak(Box::<Shell>::default()), 1.0)
}

fn statement(p: &mut Presenter<Data>) -> Told {
    told(&p.reader_access(1.0, false).expect("a statement"))
}

#[test]
fn shelfs_library_is_read_bar_then_page_then_navigation_bar() {
    let _hosting = hosting();
    let mut p = shelf(12);
    let nodes = read(&mut p);
    // The top app bar first: its title a heading, its button by its name.
    assert_eq!(
        names(&nodes[..3]),
        [
            (role::HEADING, "Library"),
            (role::BUTTON, "Add Book"),
            (role::FIELD, "Search title or author"),
        ]
    );
    assert_eq!(nodes[2].value, "Search", "a hosted field's placeholder");
    // The page: the filter's tabs, which stand over the list and are no
    // row of it (LLP 1115.000 D16), then the list, a scroller that can go
    // on and holds each row that shows, a row one node named by its words,
    // which opens and has a menu.
    let filter = &nodes[3];
    assert_eq!(filter.name, "Filter");
    assert_eq!(
        (filter.role, filter.parent, filter.set),
        (role::TABLIST, 0, (0, 4))
    );
    let list = nodes
        .iter()
        .find(|n| n.view == id(&p, "library-list"))
        .expect("the list");
    assert_eq!((list.role, list.parent), (role::SCROLLER, 0));
    assert_eq!(list.flags, flag::FORWARD);
    let all = named(&nodes, "All");
    assert_eq!(
        (all.role, all.parent, all.set),
        (role::TAB, filter.id, (1, 4))
    );
    assert_eq!(all.flags, flag::CLICK | flag::SELECTED);
    assert_eq!(named(&nodes, "Want").flags, flag::CLICK);
    let row = named(&nodes, "Book 1 Susanna Clarke 44% · page 120 of 272");
    assert_eq!((row.role, row.parent), (role::LINK, list.id));
    assert_eq!(row.flags, flag::CLICK | flag::LONG_CLICK);
    assert_eq!(row.view, id(&p, "book-seed-1"));
    let meter = nodes
        .iter()
        .find(|n| n.parent == row.id)
        .expect("its meter");
    assert_eq!(
        (meter.role, meter.range),
        (role::PROGRESS, [0.0, 100.0, 44.0])
    );
    // Rows past the navigation bar are not there, and the row it cuts shows
    // only what is above it.
    assert!(nodes
        .iter()
        .all(|n| n.name != "Book 9 Susanna Clarke 44% · page 120 of 272"));
    let cut = named(&nodes, "Book 7 Susanna Clarke 44% · page 120 of 272");
    assert_eq!(cut.rect[1] + cut.rect[3], 916.0 - 104.0);
    // The navigation bar last: a tablist of three, the first selected.
    let tail = &nodes[nodes.len() - 4..];
    assert_eq!(
        names(tail),
        [
            (role::TABLIST, ""),
            (role::TAB, "Library"),
            (role::TAB, "Discover"),
            (role::TAB, "Stats"),
        ]
    );
    assert_eq!(tail[0].view, id(&p, "tabs"));
    assert_eq!(tail[1].flags, flag::CLICK | flag::SELECTED);
    assert_eq!(tail[2].flags, flag::CLICK);
    assert_eq!((tail[1].set, tail[3].set), ((1, 3), (3, 3)));
    assert!(tail[1..].iter().all(|tab| tab.parent == tail[0].id));
    // Every box is in the window, under the status bar; no identity twice.
    for node in &nodes {
        assert!(node.id > 0 && node.rect[1] >= 52.0, "{node:?}");
        assert!(node.rect[1] + node.rect[3] <= 916.0, "{node:?}");
        assert_eq!(nodes.iter().filter(|n| n.id == node.id).count(), 1);
        assert!(node.parent == 0 || nodes.iter().any(|n| n.id == node.parent));
    }
}

#[test]
fn a_control_is_its_kind_with_its_state_and_what_is_hidden_is_no_node() {
    let mut p = boot(CONTROLS, (320., 600.), 0);
    let nodes = read(&mut p);
    let said: Vec<_> = nodes
        .iter()
        .map(|n| (n.role, n.name.as_str(), n.flags))
        .collect();
    let check = flag::CLICK | flag::CHECKABLE;
    assert_eq!(
        said,
        [
            (role::HEADING, "Settings", 0),
            // A level alone is text: only `role="heading"` is a heading.
            (role::TEXT, "On this phone", 0),
            (role::CHECKBOX, "Remember me", check),
            (role::SWITCH, "Sound", check | flag::CHECKED),
            (
                role::SLIDER,
                "Volume",
                flag::RANGE | flag::FORWARD | flag::BACKWARD
            ),
            (role::TOGGLE, "Bold", check),
            (role::BUTTON, "Save", flag::CLICK),
            (role::BUTTON, "Unavailable", flag::DISABLED),
            (role::TEXT, "Nothing yet", flag::POLITE),
            (role::LINK, "Read more", flag::CLICK),
            (role::IMAGE, "Favourite", 0),
            // No shell hosts its fields here: the presenter paints it.
            (role::EDIT, "Name", flag::CLICK),
        ],
        "the words under `aria-hidden` are no node"
    );
    assert_eq!(named(&nodes, "Volume").range, [0.0, 100.0, 40.0]);
    // A shell that hosts fields has its own view there: the node says only
    // where it stands and what its author named it.
    let _hosting = hosting();
    let mut p = boot(CONTROLS, (320., 600.), 0);
    let nodes = read(&mut p);
    let field = named(&nodes, "Name");
    assert_eq!(
        (field.role, field.value.as_str()),
        (role::FIELD, "Your name")
    );
    assert_eq!(field.view, id(&p, "name"));
    assert_eq!(nodes.last().unwrap().id, field.id, "last, as it is written");
}

#[test]
fn a_nodes_identity_outlives_a_commit_a_scroll_and_a_row_that_returns() {
    let mut p = boot(ROWS, (320., 480.), 0);
    let port = id(&p, "port");
    let before = statement(&mut p).nodes;
    let (list, row) = (before[1].clone(), named(&before, "Row 3").clone());
    assert_eq!(
        (list.role, list.id, list.view),
        (role::SCROLLER, port as i32, port)
    );
    assert_eq!((row.parent, row.set), (list.id, (4, 400)));
    // A row's views may be another item's once it scrolls away, so its
    // identity is not its view's: it is the list's, the item's key and its
    // place in the row, as `identity` says of any view.
    assert!(row.id >= ROWED && row.id != row.view as i32);
    assert_eq!(identity(p.host().kernel(), row.view), row.id);
    // A commit: the row is pressed, and the text above says so.
    assert!(p.access_act(row.id, act::CLICK, 0.0, 0.0));
    settle(&mut p);
    let pressed = read(&mut p);
    assert_eq!(pressed[0].name, "Picked 3");
    assert_eq!((pressed[0].id, pressed[1].id), (before[0].id, list.id));
    assert_eq!(named(&pressed, "Row 3").id, row.id);
    // A scroll within what shows: the row is higher, and who it is stands.
    p.scroll_to(port, (0.0, 80.0)).unwrap();
    settle(&mut p);
    let moved = read(&mut p);
    let higher = named(&moved, "Row 3");
    assert_eq!((higher.id, higher.rect[1]), (row.id, row.rect[1] - 80.0));
    assert_eq!(moved[1].range, [80.0, 12600.0, 0.0]);
    assert_eq!(moved[1].flags, flag::FORWARD | flag::BACKWARD);
    // Far away: the row is gone, and no row there is taken for one that
    // was here.
    p.scroll_to(port, (0.0, 8000.0)).unwrap();
    settle(&mut p);
    let far = read(&mut p);
    assert!(far.iter().all(|n| n.name != "Row 3"));
    let others: Vec<&Node> = far.iter().filter(|n| n.role == role::BUTTON).collect();
    assert!(
        others.len() > 8 && others.iter().all(|n| n.set.0 > 100),
        "{others:?}"
    );
    for other in &others {
        assert!(before.iter().all(|n| n.id != other.id), "{other:?}");
        assert_eq!(far.iter().filter(|n| n.id == other.id).count(), 1);
    }

    // And back: the row that returns is who it was.
    p.scroll_to(port, (0.0, 0.0)).unwrap();
    settle(&mut p);
    let back = read(&mut p);
    let returned = named(&back, "Row 3");
    assert_eq!((returned.id, returned.rect), (row.id, row.rect));
}

#[test]
fn an_open_surface_is_the_whole_tree_and_the_page_under_it_is_not_in_it() {
    let _hosting = hosting();
    let mut p = shelf(3);
    // A row's menu, by a held press: its one row, in the menu's own window.
    let page = statement(&mut p);
    assert_eq!((page.host, page.whole), (0, true));
    let row = named(&page.nodes, "Book 1 Susanna Clarke 44% · page 120 of 272").id;
    assert!(p.access_act(row, act::LONG_CLICK, 0.0, 0.0));
    settle(&mut p);
    let menu = statement(&mut p);
    assert!(menu.host != 0 && menu.whole);
    assert_eq!(names(&menu.nodes), [(role::BUTTON, "Remove…")]);
    assert_eq!(menu.nodes[0].view, id(&p, "menu-remove"));
    assert!(menu.nodes[0].rect[0] >= 0.0 && menu.nodes[0].rect[1] >= 0.0);
    // Its row asks first: the alert is the tree, its label the headline,
    // its text, and its buttons as they show, the one that confirms last.
    assert!(p.access_act(menu.nodes[0].id, act::CLICK, 0.0, 0.0));
    settle(&mut p);
    let alert = statement(&mut p);
    assert!(alert.host != 0 && alert.host != menu.host && alert.whole);
    let mut said = names(&alert.nodes);
    said.sort_by_key(|(role, _)| *role);
    assert_eq!(
        said,
        [
            (role::TEXT, "“Book 1” will be removed from your library."),
            (role::HEADING, "Remove Book?"),
            (role::BUTTON, "Cancel"),
            (role::BUTTON, "Remove"),
        ]
    );
    assert_eq!(
        alert.nodes[0].role,
        role::HEADING,
        "the headline is read first"
    );
    assert!(alert.nodes.iter().all(|n| n.parent == 0));
    // Cancel closes it, and the page is the tree again.
    let cancel = named(&alert.nodes, "Cancel").id;
    assert!(p.access_act(cancel, act::CLICK, 0.0, 0.0));
    settle(&mut p);
    let again = statement(&mut p);
    assert_eq!((again.host, again.whole), (0, true));
    assert_eq!(again.nodes, page.nodes, "and each node is who it was");
}

#[test]
fn a_statement_says_what_changed_and_nothing_is_said_twice() {
    let mut p = boot(CONTROLS, (320., 600.), 0);
    let first = statement(&mut p);
    assert!(first.whole && first.changes.is_empty());
    assert_eq!(first.nodes, read(&mut p), "the words say the tree");
    assert_eq!(first.pane, (0, String::new()), "no bar, no pane");
    assert_eq!(p.reader_access(1.0, false), None, "nothing moved");
    let _ = p.frame();
    assert_eq!(p.reader_access(1.0, false), None, "a frame is no change");
    // A checkbox pressed: it and the toggle that reads the same state
    // changed their state, and nothing else is said to have.
    let (check, bold) = (
        named(&first.nodes, "Remember me").id,
        named(&first.nodes, "Bold").id,
    );
    tap(&mut p, "check");
    let checked = statement(&mut p);
    assert!(!checked.whole);
    assert_eq!(
        checked.changes,
        [
            (check, change::STATE, 0.0, 0.0),
            (bold, change::STATE, 0.0, 0.0)
        ]
    );
    assert!(named(&checked.nodes, "Remember me").flags & flag::CHECKED != 0);
    // A live region's words changed: its text, and it is spoken.
    let status = named(&first.nodes, "Nothing yet").id;
    tap(&mut p, "save");
    let saved = statement(&mut p);
    assert_eq!(
        saved.changes,
        [
            (status, change::TEXT, 0.0, 0.0),
            (status, change::LIVE, 0.0, 0.0)
        ]
    );
    assert_eq!(
        saved.nodes.iter().find(|n| n.id == status).unwrap().name,
        "Saved"
    );
    // A shell made again holds nothing: it is told the tree whole, in its
    // own pixels.
    let again = told(&p.reader_access(2.0, true).unwrap());
    assert!(again.whole && again.changes.is_empty());
    let (one, two) = (
        named(&saved.nodes, "Save").rect,
        named(&again.nodes, "Save").rect,
    );
    assert_eq!(two, one.map(|v| v * 2.0));
    // A shell that stopped asking is told everything when it asks again.
    p.access_reset();
    assert!(p.access_told().is_empty());
    assert!(statement(&mut p).whole);
}

#[test]
fn a_live_region_speaks_and_a_selection_is_told_apart_whatever_else_changed() {
    // A menu's row changes the page's status line while the menu's window
    // is the tree. The page is told whole when the menu closes, and the
    // status line is still said to speak.
    let mut p = boot(MENU, (320., 400.), 0);
    p.serve_requests();
    p.serve_windows(Box::new(Blank));
    settle(&mut p);
    let page = statement(&mut p);
    let status = named(&page.nodes, "Nothing yet").id;
    tap(&mut p, "more");
    let menu = statement(&mut p);
    assert!(menu.host != 0 && menu.whole && menu.changes.is_empty());
    assert!(p.access_act(named(&menu.nodes, "Save").id, act::CLICK, 0.0, 0.0));
    settle(&mut p);
    let after = statement(&mut p);
    assert!(after.host == 0 && after.whole);
    assert_eq!(after.changes, [(status, change::LIVE, 0.0, 0.0)]);
    assert_eq!(
        after.nodes.iter().find(|n| n.id == status).unwrap().name,
        "Saved"
    );
    // A shell made again is told no change at all.
    assert!(told(&p.reader_access(1.0, true).unwrap())
        .changes
        .is_empty());
    // Shelf's filter: the list under it empties and no longer scrolls,
    // which is one change, of everything; the two tabs whose selection
    // moved are told apart even so, and a service on one of them says it
    // is selected.
    let _hosting = hosting();
    let mut p = shelf(12);
    let library = statement(&mut p).nodes;
    let (all, want) = (named(&library, "All").id, named(&library, "Want").id);
    tap(&mut p, "filter-want");
    let filtered = statement(&mut p);
    assert!(!filtered.whole);
    assert_eq!(
        filtered.changes,
        [
            (0, change::SUBTREE, 0.0, 0.0),
            (all, change::STATE, 0.0, 0.0),
            (want, change::STATE, 0.0, 0.0)
        ]
    );
    assert_eq!(
        named(&filtered.nodes, "Want").flags,
        flag::CLICK | flag::SELECTED
    );
}

#[test]
fn a_scroll_and_a_new_screen_are_said_as_what_they_are() {
    let mut p = boot(ROWS, (320., 480.), 0);
    let port = id(&p, "port");
    let _ = statement(&mut p);
    p.scroll_to(port, (0.0, 120.0)).unwrap();
    settle(&mut p);
    let moved = statement(&mut p);
    // It moved, what is in it is elsewhere, and it can now go back.
    let what = change::SUBTREE | change::SCROLLED | change::STATE;
    assert_eq!(moved.changes, [(port as i32, what, 0.0, 120.0)]);
    // The pane is the route that shows and its title, as the bar has it.
    let _hosting = hosting();
    let mut p = shelf(3);
    let library = statement(&mut p);
    assert_eq!(library.pane.1, "Library");
    assert!(library.pane.0 != 0);
    tap(&mut p, "book-seed-2");
    let book = statement(&mut p);
    assert_eq!(book.pane.1, "Book 2");
    assert!(book.pane.0 != library.pane.0);
    assert_eq!(
        book.changes,
        [(0, change::SUBTREE, 0.0, 0.0)],
        "all of it is new"
    );
    // Its bar: the Up arrow, which the platform names, then the title.
    assert_eq!(
        names(&book.nodes[..2]),
        [(role::UP, ""), (role::HEADING, "Book 2")]
    );
    assert_eq!(book.nodes[0].flags, flag::CLICK);
    // The Up arrow pressed is the system's Back.
    assert!(p.access_act(book.nodes[0].id, act::CLICK, 0.0, 0.0));
    settle(&mut p);
    let back = statement(&mut p);
    assert_eq!(back.pane, library.pane);
    assert_eq!(back.nodes, library.nodes);
}

#[test]
fn an_action_is_the_presenters_own_press_scroll_and_step() {
    let mut p = boot(CONTROLS, (320., 600.), 0);
    let nodes = statement(&mut p).nodes;
    let of = |name: &str| named(&nodes, name).id;
    // A press is a tap at the node: the switch turns off, the app hears it.
    assert!(p.access_act(of("Sound"), act::CLICK, 0.0, 0.0));
    settle(&mut p);
    assert_eq!(
        named(&read(&mut p), "Sound").flags,
        flag::CLICK | flag::CHECKABLE
    );
    // A range steps by its step, and is set to a value.
    assert!(p.access_act(of("Volume"), act::FORWARD, 0.0, 0.0));
    settle(&mut p);
    assert_eq!(named(&statement(&mut p).nodes, "Volume").range[2], 50.0);
    assert!(p.access_act(of("Volume"), act::BACKWARD, 0.0, 0.0));
    assert!(p.access_act(of("Volume"), act::SET, 100.0, 0.0));
    settle(&mut p);
    let full = statement(&mut p);
    let volume = named(&full.nodes, "Volume");
    assert_eq!(volume.range[2], 100.0);
    assert_eq!(
        volume.flags,
        flag::RANGE | flag::BACKWARD,
        "nothing above its greatest"
    );
    assert!(!p.access_act(of("Volume"), act::FORWARD, 0.0, 0.0));
    // What takes no press takes none: a disabled button, a heading, a node
    // that is not in the tree, an action that is not the node's.
    for (id, action) in [
        (of("Unavailable"), act::CLICK),
        (of("Settings"), act::CLICK),
        (of("Save"), act::LONG_CLICK),
        (of("Save"), act::FORWARD),
        (987_654, act::CLICK),
    ] {
        assert!(!p.access_act(id, action, 0.0, 0.0), "{id} {action}");
    }
    // A scroller scrolls by what it shows, less a line kept in view, and
    // stops at its end.
    let mut p = boot(ROWS, (320., 480.), 0);
    let port = id(&p, "port");
    let list = statement(&mut p).nodes[1].id;
    assert!(
        !p.access_act(list, act::BACKWARD, 0.0, 0.0),
        "it is at its start"
    );
    assert!(p.access_act(list, act::FORWARD, 0.0, 0.0));
    settle(&mut p);
    assert_eq!(p.scroll_of(port), (0.0, 352.0));
    let _ = statement(&mut p);
    assert!(p.access_act(list, act::BACKWARD, 0.0, 0.0));
    settle(&mut p);
    assert_eq!(p.scroll_of(port), (0.0, 0.0));
    // A button of a bar is pressed by the node it stands for.
    let _hosting = hosting();
    let mut p = shelf(3);
    let add = named(&statement(&mut p).nodes, "Add Book").id;
    assert!(p.access_act(add, act::CLICK, 0.0, 0.0));
    settle(&mut p);
    let form = statement(&mut p);
    assert_eq!(form.pane.1, "New Book");
    assert_eq!(
        names(&form.nodes[..3]),
        [
            (role::BUTTON, "Cancel"),
            (role::HEADING, "New Book"),
            (role::BUTTON, "Save")
        ]
    );
    let fields: Vec<_> = (form.nodes.iter())
        .filter(|n| n.role == role::FIELD)
        .map(|n| n.name.as_str())
        .collect();
    assert_eq!(fields, ["Title", "Author", "Pages"]);
}

#[test]
fn a_host_whose_shell_never_asks_builds_nothing() {
    // Desktop Linux: frames, a press and a scroll, and no tree was made.
    let mut p = boot(ROWS, (320., 480.), 0);
    let port = id(&p, "port");
    p.scroll_to(port, (0.0, 200.0)).unwrap();
    settle(&mut p);
    assert!(p.access.key.is_none() && !p.access.told && p.access.serial == 0);
    assert!(p.access_told().is_empty());
    assert!(
        !p.access_act(port as i32, act::FORWARD, 0.0, 0.0),
        "no tree, no node"
    );
}
