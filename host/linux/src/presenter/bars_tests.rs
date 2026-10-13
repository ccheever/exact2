//! A shell's bars (LLP 1115.001 D11): what each shows for Shelf's routes,
//! the room they take of the page, what a touch in one does, how a driver
//! addresses what they stand for, and a host with no such shell unchanged.
use super::look::{nav_height, TOP_HEIGHT};
use super::*;
use crate::presenter::top::SurfacePainter;
use exact_runner::{DataError, Value};
use std::cell::RefCell;
use std::rc::Rc;

/// Shelf's data, as much as its screens read: a library of two books, no
/// works for a subject, and the clock. The fixture below asks for none.
#[derive(Default)]
struct Nothing;
impl DataSource for Nothing {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        let book = |id: &str, title: &str| {
            let text = |s: &str| Value::str(s);
            Value::record(vec![
                text(id),
                text(title),
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
                Value::list(vec![book("seed-1", "Piranesi"), book("seed-2", "Emma")]),
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
            _ => Err(DataError::UnknownSource(name.into())),
        }
    }
}

/// Two tabs. Home: a level-1 heading, a button that opens a menu, and a
/// list taller than the screen. Detail, pushed: the Back control, a
/// heading, a text button. Bare, pushed: no header. Quiet, pushed: a header
/// its route says is no bar's. Other: a heading alone.
const APP: &str = r#"routes nav
  tab home "/"
    detail "/detail"
    bare "/bare"
    quiet "/quiet"
  tab other "/other"
component App
  state said = "nothing"
  action pick(name: string)
    nav = select(nav, name)
  action open(url: string)
    nav = push(nav, url)
  action back
    nav = back(nav)
  action edit
    said = "edited"
  action share
    said = "shared"
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" width="100%" height="100%" display="flex" flex-direction="column"
      column flex=1 min-height=0 position="relative"
        each t in nav.tabs key = t.name
          column role="tabpanel" id=`panel-${t.name}` position="absolute" inset=0
            each e in t.stack key = e.id
              when e.name == "home"
                column navigationKey=`${e.id}` testId="route-home" position="absolute" inset=0 display="flex" flex-direction="column"
                  header display="flex" padding="8px 16px" testId="home-header"
                    text "Home" role="heading" aria-level=1 flex=1 testId="home-title"
                    button popovertarget="more" aria-label="More" testId="more"
                      image "symbol:more"
                  scroll testId="list" flex=1 min-height=0
                    button press=open("/detail") testId="open" height=40
                      text "Open"
                    button press=open("/bare") testId="open-bare" height=40
                      text "Bare"
                    button press=open("/quiet") testId="open-quiet" height=40
                      text "Quiet"
                    column height=2000
                      text "Tall"
              when e.name == "detail"
                column navigationKey=`${e.id}` testId="route-detail" position="absolute" inset=0 display="flex" flex-direction="column"
                  header display="flex" testId="detail-header"
                    button id="back" press=back testId="back"
                      image "symbol:back"
                    text "Detail" role="heading"
                    button press=edit testId="edit"
                      text "Edit"
                  text "Body" testId="body"
              when e.name == "bare"
                column navigationKey=`${e.id}` testId="route-bare" position="absolute" inset=0
                  text "No header here" testId="bare-body"
              when e.name == "quiet"
                column navigationKey=`${e.id}` navigationBar="none" testId="route-quiet" position="absolute" inset=0
                  header testId="quiet-header"
                    text "Quiet" role="heading" testId="quiet-title"
              when e.name == "other"
                column navigationKey=`${e.id}` testId="route-other" position="absolute" inset=0 display="flex" flex-direction="column"
                  header
                    text "Other" role="heading" aria-level=1
                  text "Other body" testId="other-body"
      row role="tablist" testId="tabs"
        button role="tab" aria-controls="panel-home" aria-selected=(nav.tab == "home") press=pick("home") testId="tab-home" flex=1
          image "symbol:home"
          text "Home"
        button role="tab" aria-controls="panel-other" aria-selected=(nav.tab == "other") press=pick("other") testId="tab-other" flex=1
          image "symbol:star"
          text "Other"
      text said testId="said" display="none"
      column id="more" popover="auto" role="menu" aria-label="More"
        button press=share popovertarget="more" popovertargetaction="hide" testId="menu-share"
          text "Share"
"#;

const SHELF: &str = include_str!("../../../../apps/shelf/app.contract");

/// The CPU painter standing in for a shell's: it keeps each picture.
struct Pixels(Rc<RefCell<Vec<Pixmap>>>);
impl SurfacePainter for Pixels {
    fn record(
        &mut self,
        size: (f32, f32),
        scale: f32,
        paint: &mut dyn FnMut(&mut dyn Backend),
    ) -> Vec<u32> {
        let mut raster = crate::raster::Raster::transparent();
        raster.begin(size.0, size.1, scale);
        paint(&mut raster);
        let pixels = raster.finish().unwrap();
        let words = vec![pixels.width(), pixels.height()];
        self.0.borrow_mut().push(pixels);
        words
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

fn boot(source: &str) -> Presenter<Nothing> {
    let (p, error) = Presenter::boot_with(
        &contract::compile(source).unwrap().encode(),
        Nothing,
        (412., 840.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}

/// A presenter whose shell shows bars, in a phone's window, and the
/// pictures it recorded.
fn barred(source: &str) -> (Presenter<Nothing>, Rc<RefCell<Vec<Pixmap>>>) {
    let mut p = boot(source);
    let pictures = Rc::new(RefCell::new(Vec::new()));
    p.serve_requests();
    p.serve_windows(Box::new(Pixels(Rc::new(RefCell::new(Vec::new())))));
    assert_eq!(p.serve_bars(Box::new(Pixels(pictures.clone()))), None);
    assert_eq!(p.set_window(WINDOW), None);
    (p, pictures)
}

fn id(p: &Presenter<Nothing>, name: &str) -> ViewId {
    let kernel = p.host().kernel();
    let found = kernel.find_by_test_id(name);
    assert!(!found.is_empty(), "no node has the test id {name}");
    kernel.node_by_key(found[0]).unwrap().id
}

fn text(p: &Presenter<Nothing>, name: &str) -> String {
    let node = p.host().kernel().node(id(p, name)).unwrap();
    node.text_runs().iter().map(|r| &*r.text).collect()
}

fn tap(p: &mut Presenter<Nothing>, name: &str) -> Result<String, String> {
    let view = id(p, name);
    let result = p.tap(view);
    p.run_commands(|| Nothing);
    result
}

/// A finger down and up in the middle of the bar's target that stands for
/// the node `name`, or for the Up arrow where `name` is `None`.
fn touch(p: &mut Presenter<Nothing>, which: i32, name: Option<&str>) {
    let node = name.map(|name| id(p, name));
    let (top, bottom) = p.bars();
    let bar = if which == TOP { top } else { bottom }.expect("the bar shows");
    let target = (bar.targets.iter())
        .find(|t| match (t.what, node) {
            (What::Up, None) => true,
            (What::Item(n) | What::Tab(n), Some(node)) => n == node,
            _ => false,
        })
        .expect("a target for the node");
    let (x, y, w, h) = target.rect;
    for action in [0, 1] {
        p.bar_event(which, action, x + w / 2.0, y + h / 2.0, 0.0);
    }
    p.run_commands(|| Nothing);
}

fn top(p: &mut Presenter<Nothing>) -> Top {
    p.bar_plans().0.cloned().expect("a top bar")
}

fn titles(buttons: &[Button]) -> Vec<(&str, Option<&str>)> {
    (buttons.iter())
        .map(|b| (b.title.as_str(), b.symbol.as_deref()))
        .collect()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

#[test]
fn shelfs_library_is_a_top_app_bar_and_a_navigation_bar() {
    let (mut p, _) = barred(SHELF);
    let library = top(&mut p);
    assert_eq!(library.title, "Library");
    assert!(!library.up, "the first route of its stack has no Up arrow");
    assert_eq!(titles(&library.leading), []);
    assert_eq!(titles(&library.trailing), [("Add Book", Some("add"))]);
    let tabs = p.bar_plans().1.cloned().expect("a navigation bar");
    let said: Vec<_> = (tabs.tabs.iter())
        .map(|t| (t.label.as_str(), t.symbol.as_deref(), t.selected))
        .collect();
    assert_eq!(
        said,
        [
            ("Library", Some("book"), true),
            ("Discover", Some("sparkles"), false),
            ("Stats", Some("chart"), false),
        ]
    );
    // Where each is: the top bar under the status bar, 64 tall; the
    // navigation bar over the gesture bar, 80 tall; both the window's width.
    let (above, below) = p.bars();
    assert_eq!(above.unwrap().frame, [0.0, 0.0, 412.0, 52.0 + 64.0]);
    assert_eq!(
        below.unwrap().frame,
        [0.0, 916.0 - 104.0, 412.0, 80.0 + 24.0]
    );
    // Out of the page: the heading, the button and the tablist take no box.
    // The search field stays, under the bar; the list ends at the
    // navigation bar. The page is the window less the system's bars.
    assert_eq!(p.viewport(), (412.0, 916.0 - 52.0 - 24.0));
    for gone in ["add-book", "tabs", "tab-library"] {
        let view = id(&p, gone);
        assert_eq!(p.rect_of(view), None, "{gone} is in a bar");
    }
    let search = id(&p, "search");
    let field = p.rect_of(search).expect("the search field is in the page");
    assert!(near(field.1, 64.0 + 8.0), "under the bar: {field:?}");
    let list = id(&p, "library-list");
    let list = p.rect_of(list).unwrap();
    assert!(near(list.1 + list.3, 840.0 - 80.0), "{list:?}");
}

#[test]
fn shelfs_add_form_and_its_other_tabs() {
    let (mut p, _) = barred(SHELF);
    // The add form, presented: its Back control is a leading item and Save
    // a trailing one, with no Up arrow. Its header is the bar, so the
    // library's heading no longer shows through it.
    assert!(tap(&mut p, "add-book").unwrap().contains("\"bar\":\"top\""));
    let add = top(&mut p);
    assert_eq!(add.title, "New Book");
    assert!(!add.up);
    assert_eq!(titles(&add.leading), [("Cancel", None)]);
    assert_eq!(titles(&add.trailing), [("Save", None)]);
    let kernel = p.host().kernel();
    let form = kernel.find_by_id("add-form")[0];
    let form = kernel.node_by_key(form).unwrap().id;
    let form = p.rect_of(form).unwrap();
    assert!(
        near(form.1, 64.0),
        "the form starts under the bar: {form:?}"
    );
    touch(&mut p, TOP, Some("add-cancel"));
    assert_eq!(top(&mut p).title, "Library");
    // Stats and Discover, each chosen in the navigation bar.
    touch(&mut p, BOTTOM, Some("tab-stats"));
    assert_eq!(top(&mut p).title, "Stats");
    let tabs = p.bar_plans().1.cloned().unwrap();
    let selected: Vec<_> = tabs.tabs.iter().map(|t| t.selected).collect();
    assert_eq!(selected, [false, false, true]);
    assert!(tap(&mut p, "tab-discover")
        .unwrap()
        .contains("\"bar\":\"bottom\""));
    assert_eq!(top(&mut p).title, "Discover");
    assert_eq!(titles(&top(&mut p).trailing), []);
}

#[test]
fn a_pushed_route_has_the_up_arrow_which_goes_back() {
    let (mut p, _) = barred(APP);
    assert_eq!(top(&mut p).title, "Home");
    assert_eq!(titles(&top(&mut p).trailing), [("More", Some("more"))]);
    tap(&mut p, "open").unwrap();
    let detail = top(&mut p);
    assert_eq!(detail.title, "Detail");
    assert!(detail.up, "pushed: the Up arrow");
    // The Back control is the arrow's, not an item; Edit is a text button.
    assert_eq!(titles(&detail.leading), []);
    assert_eq!(titles(&detail.trailing), [("Edit", None)]);
    touch(&mut p, TOP, Some("edit"));
    assert_eq!(text(&p, "said"), "edited");
    // The arrow goes Back through its order: the route's authored control.
    touch(&mut p, TOP, None);
    assert_eq!(top(&mut p).title, "Home");
    // A driver names the Back control, which the arrow stands for.
    tap(&mut p, "open").unwrap();
    assert!(tap(&mut p, "back").unwrap().contains("\"bar\":\"top\""));
    assert_eq!(top(&mut p).title, "Home");
    // System Back does the same.
    tap(&mut p, "open").unwrap();
    assert!(p.back_goes());
    assert!(p.system_back(0.0));
    assert_eq!(top(&mut p).title, "Home");
}

#[test]
fn a_route_with_no_header_or_that_says_no_bar_has_none() {
    let (mut p, _) = barred(APP);
    // Pushed with no header: no bar. Android's Back is the system's.
    tap(&mut p, "open-bare").unwrap();
    assert!(p.bar_plans().0.is_none());
    let body = id(&p, "bare-body");
    let body = p.rect_of(body).unwrap();
    assert!(near(body.1, 0.0), "nothing is taken of its top: {body:?}");
    assert!(p.bar_plans().1.is_some(), "the navigation bar stays");
    assert!(p.system_back(0.0));
    // `navigationBar="none"`: its header paints in the page, as written.
    tap(&mut p, "open-quiet").unwrap();
    assert!(p.bar_plans().0.is_none());
    let title = id(&p, "quiet-title");
    assert!(p.rect_of(title).is_some(), "its header is the page's");
}

/// `appearance="none"` written on a header or the root's tablist keeps the
/// author's bar (LLP 1075.003 §9.10, #393): no Material bar is made of
/// either, and both paint in the page.
#[test]
fn a_header_or_tablist_that_writes_appearance_none_stays_the_pages() {
    let source = APP
        .replace(
            r#"header display="flex" padding="8px 16px" testId="home-header""#,
            r#"header display="flex" padding="8px 16px" testId="home-header" appearance="none""#,
        )
        .replace(
            r#"row role="tablist" testId="tabs""#,
            r#"row role="tablist" testId="tabs" appearance="none""#,
        );
    assert_ne!(source, APP);
    let (mut p, _) = barred(&source);
    assert!(p.bar_plans().0.is_none(), "no top app bar");
    assert!(p.bar_plans().1.is_none(), "no navigation bar");
    for name in ["home-title", "tabs", "tab-home"] {
        let view = id(&p, name);
        assert!(p.rect_of(view).is_some(), "{name} paints in the page");
    }
}

#[test]
fn a_touch_lights_its_target_and_the_shell_is_told_each_change_once() {
    let (mut p, pictures) = barred(APP);
    let words = p.reader_bars(2.0).expect("the bars are told");
    // A count, then for each bar which it is, its frame in pixels, and its
    // stream (the test's painter writes two words).
    assert_eq!(words[0], 2);
    assert_eq!(&words[1..6], &[TOP, 0, 0, 824, 232]);
    assert_eq!(words[6], 2);
    assert_eq!(&words[9..14], &[BOTTOM, 0, 1624, 824, 208]);
    assert_eq!(words.len(), 17);
    assert_eq!(p.reader_bars(2.0), None, "told once");
    // A finger down on a destination lights it; lifting chooses it.
    let other = id(&p, "tab-other");
    let (_, bottom) = p.bars();
    let target = bottom.unwrap().targets[1];
    assert_eq!(target.what, What::Tab(other));
    let (x, y, w, h) = target.rect;
    let before = pictures.borrow().len();
    p.bar_event(BOTTOM, 0, x + w / 2.0, y + h / 2.0, 0.0);
    assert!(p.reader_bars(2.0).is_some(), "lit");
    assert_eq!(pictures.borrow().len(), before + 2);
    // A finger that leaves its target lets go of it, and chooses nothing.
    p.bar_event(BOTTOM, 2, 10.0, 10.0, 0.0);
    p.bar_event(BOTTOM, 1, 10.0, 10.0, 0.0);
    p.run_commands(|| Nothing);
    assert_eq!(top(&mut p).title, "Home");
    assert!(p.reader_bars(2.0).is_some(), "unlit");
    touch(&mut p, BOTTOM, Some("tab-other"));
    assert_eq!(top(&mut p).title, "Other");
    // A reader made again is told its bars anew.
    assert!(p.reader_bars(2.0).is_some());
    assert_eq!(p.reader_bars(2.0), None);
    p.bars_reset();
    assert!(p.reader_bars(2.0).is_some());
}

/// The colour in the middle of the status bar's area of the last top bar.
fn bar_colour(pictures: &Rc<RefCell<Vec<Pixmap>>>) -> [u8; 4] {
    let all = pictures.borrow();
    let top = (all.iter().rev())
        .find(|p| p.height() == 116)
        .expect("a top bar was painted");
    let c = top.pixel(206, 20).unwrap();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

#[test]
fn the_top_bar_takes_its_on_scroll_colour_and_a_retapped_tab_scrolls_to_the_top() {
    let (mut p, pictures) = barred(APP);
    // The dark scheme: with no device palette told, the table's light pair
    // has one white for both colours.
    p.set_system_scheme(true);
    p.reader_bars(1.0).unwrap();
    let rest = bar_colour(&pictures);
    assert!(!top(&mut p).scrolled);
    let list = id(&p, "list");
    assert_eq!(p.scroll_to(list, (0.0, 300.0)), Some((0.0, 300.0)));
    assert!(top(&mut p).scrolled);
    p.reader_bars(1.0).expect("told its new colour");
    let scrolled = bar_colour(&pictures);
    // Sampled in the status bar's area, which is the bar's own colour.
    assert_ne!(rest, scrolled, "surface, then surfaceContainer");
    assert_eq!(p.scroll_of(list), (0.0, 300.0));
    // The selected destination tapped again, its stack at its root: to the top.
    touch(&mut p, BOTTOM, Some("tab-home"));
    assert_eq!(p.scroll_of(list), (0.0, 0.0));
    assert!(!top(&mut p).scrolled);
    p.reader_bars(1.0).unwrap();
    assert_eq!(bar_colour(&pictures), rest);
    // Above its root the tap is the app's own (it selects the tab): the
    // list beneath keeps its place.
    tap(&mut p, "open").unwrap();
    p.scroll_to(list, (0.0, 300.0));
    touch(&mut p, BOTTOM, Some("tab-home"));
    assert_eq!(p.scroll_of(list), (0.0, 300.0));
}

#[test]
fn a_keyboard_covers_the_navigation_bar_and_the_page_ends_at_the_keyboard() {
    let (mut p, _) = barred(APP);
    let list = id(&p, "list");
    let at_rest = p.rect_of(list).unwrap();
    assert!(near(at_rest.1 + at_rest.3, 840.0 - 80.0), "{at_rest:?}");
    let mut window = WINDOW;
    window.keyboard = [0.0, 0.0, 320.0, 0.0];
    assert_eq!(p.set_window(window), None);
    // The page ends at the keyboard's top; the navigation bar stays where
    // it was, under the keyboard, and takes nothing of the page.
    assert_eq!(p.viewport(), (412.0, 916.0 - 52.0 - 320.0));
    let (_, bottom) = p.bars();
    assert_eq!(bottom.unwrap().frame, [0.0, 812.0, 412.0, 104.0]);
    let list = p.rect_of(list).unwrap();
    assert!(near(list.1 + list.3, 544.0), "{list:?}");
    assert!(near(list.1, 64.0), "the top bar's room stands: {list:?}");
}

#[test]
fn the_largest_font_scale_clips_neither_bar() {
    let (mut p, _) = barred(APP);
    let device = Device {
        night: false,
        font_scale: 2.0,
        locale: "en-US".into(),
        rtl: false,
        hour24: false,
        reduced_motion: false,
        bold_text: false,
        high_contrast: false,
        touch: true,
        touch_slop: 8.0,
    };
    assert_eq!(p.set_device(&device), None);
    assert_eq!(p.fit_window(), None);
    assert_eq!(nav_height(2.0), 80.0, "Material's 80 holds a doubled label");
    assert!(nav_height(3.0) > 80.0, "and grows past it");
    let (above, below) = p.bars();
    assert_eq!(above.unwrap().frame[3], 52.0 + TOP_HEIGHT);
    assert_eq!(below.unwrap().frame[3], 24.0 + 80.0);
}

#[test]
fn a_bars_button_hangs_its_menu_from_the_bar() {
    let (mut p, _) = barred(APP);
    assert!(tap(&mut p, "more").unwrap().contains("\"bar\":\"top\""));
    let surface = p.surface().expect("its menu opens");
    assert_eq!(surface.kind, crate::presenter::top::SurfaceKind::Menu);
    p.tell_shell();
    let words = p.take_requests().unwrap();
    // The surface's statement: kind 7; its frame's y is under the button's
    // 48 target, which is in the top bar (52 of status bar, then 8).
    let at = words.iter().position(|w| *w == 7).unwrap();
    let frame = &words[at + 6..at + 10];
    assert_eq!(frame[1], 52 + 8 + 48, "{frame:?}");
    assert!(frame[0] + frame[2] <= 412, "inside the window: {frame:?}");
}

#[test]
fn a_driver_reads_the_bars_and_what_each_target_stands_for() {
    let (mut p, _) = barred(APP);
    tap(&mut p, "open").unwrap();
    let state = p.bars_json();
    assert_eq!(state["top"]["title"], "Detail");
    assert_eq!(state["top"]["up"], true);
    let kinds: Vec<_> = (state["top"]["targets"].as_array().unwrap().iter())
        .map(|t| (t["kind"].as_str().unwrap().to_string(), t["id"].clone()))
        .collect();
    let edit = id(&p, "edit");
    assert_eq!(
        kinds,
        [
            ("up".to_string(), serde_json::Value::Null),
            ("item".to_string(), edit.into())
        ]
    );
    // The arrow: 4 from the edge, in the bar under the status bar.
    let up = &state["top"]["targets"][0];
    assert_eq!(
        (up["x"].as_f64(), up["y"].as_f64()),
        (Some(4.0), Some(60.0))
    );
    assert_eq!(
        (up["w"].as_f64(), up["h"].as_f64()),
        (Some(48.0), Some(48.0))
    );
    assert_eq!(state["bottom"]["tabs"][0]["label"], "Home");
    assert_eq!(state["bottom"]["targets"][1]["kind"], "tab");
    // What a bar stands for needs no scrolling into view, and a node of the
    // header the bar does not show is not in the page either.
    assert!(p.reveal(edit).unwrap().contains("\"scrolled\":false"));
    let header = id(&p, "detail-header");
    assert!(p.tap(header).is_err());
}

#[test]
fn a_host_with_no_such_shell_paints_the_header_and_the_tablist_in_the_page() {
    // Desktop Linux, a headless run, the agent: nothing serves bars.
    let mut p = boot(SHELF);
    assert!(!p.has_bars());
    assert!(p.bar_plans().0.is_none() && p.bar_plans().1.is_none());
    assert_eq!(p.reader_bars(1.0), None);
    assert!(p.host().bars().routes.is_empty());
    for shown in ["add-book", "tabs", "tab-library", "search"] {
        let view = id(&p, shown);
        assert!(p.rect_of(view).is_some(), "{shown} is in the page");
    }
    // Its buttons are pressed in the page, as they were.
    let result = tap(&mut p, "add-book").unwrap();
    assert!(!result.contains("bar"), "{result}");
    assert_eq!(p.viewport(), (412.0, 840.0));
    // With a window told and no bars, the page is the window less the
    // system's bars, and nothing more is taken of it.
    let mut p = boot(APP);
    assert_eq!(p.set_window(WINDOW), None);
    let list = id(&p, "list");
    let header = id(&p, "home-header");
    let (header, list) = (p.rect_of(header).unwrap(), p.rect_of(list).unwrap());
    assert!(near(header.1, 0.0) && near(list.1, header.3));
}
