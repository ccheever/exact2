//! @ref LLP 1075.003 §3.7 — a tab's routes are built the first time it is
//! selected, then kept.
use exact_kernel::Kernel;
use exact_runner::{agent, Answer, DataError, DataSource, Runner, Value};

const SOURCE: &str = r#"
routes nav
  tab home "/"
    item "/item"
  tab saved "/saved"
    note "/note"

component Tabs
  state start = 1
  resource feed = feed() as shape Feed
  action pick(name: string)
    nav = select(nav, name)
  action follow(url: string)
    nav = go(nav, url)
  action back
    nav = back(nav)
  action later
    start = 5
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow testId="navigation"
      column
        each t in nav.tabs key=t.name
          column role="tabpanel" id=`panel-${t.name}` testId=`panel-${t.name}`
            each e in t.stack key=e.id
              column navigationKey=`${e.id}` testId=`route-${e.name}`
                when e.name == "home"
                  text "Home" testId="home-title"
                when e.name == "item"
                  text "Item" testId="item-title"
                when e.name == "saved"
                  text feed.title testId="saved-title"
                  Counter(start=start)
                when e.name == "note"
                  text "Note" testId="note-title"
      row role="tablist"
        button role="tab" aria-controls="panel-home" press=pick("home") testId="tab-home"
          text "Home"
        button role="tab" aria-controls="panel-saved" press=pick("saved") testId="tab-saved"
          text "Saved"

component Counter
  props
    start: number
  state c = start
  action bump
    c = c + 1
  view
    button press=bump testId="bump"
      text `${c}` testId="count"

shape Feed
  title: string
"#;

/// `feed` answers later.
struct Feed;
impl DataSource for Feed {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::Unavailable(format!("{source} answers later")))
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[Value],
    ) -> Result<Answer, DataError> {
        Ok(Answer::Later(exact_runner::Request::get(
            "https://feed.test/",
        )))
    }
    fn parse(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[Value],
        _: exact_runner::Outcome,
    ) -> Result<Answer, DataError> {
        Ok(Answer::Now(Value::record(vec![Value::str("news")])))
    }
}

fn boot(launch: &str) -> Runner<Feed> {
    let plan = contract::compile(SOURCE).unwrap_or_else(|e| panic!("{e:?}"));
    Runner::boot(
        plan,
        Feed,
        Kernel::with_monospace(),
        Default::default(),
        launch,
    )
    .unwrap()
}

/// Whether a live node carries `test_id`.
fn built(r: &Runner<Feed>, test_id: &str) -> bool {
    agent::tree(r).contains(&format!("\"testId\":\"{test_id}\""))
}

fn press(r: &mut Runner<Feed>, test_id: &str) {
    let tree: serde_json::Value = serde_json::from_str(&agent::tree(r)).unwrap();
    let id = tree["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["props"]["testId"] == test_id)
        .unwrap_or_else(|| panic!("{test_id} is not built"))["id"]
        .as_u64()
        .unwrap();
    r.dispatch(id as u32, exact_runner::Event::Press).unwrap();
}

#[test]
fn only_the_selected_tab_is_built_at_launch() {
    let r = boot("/");
    assert!(built(&r, "home-title"));
    assert!(
        built(&r, "panel-saved") && built(&r, "route-saved"),
        "the panel and its route are"
    );
    assert!(
        !built(&r, "saved-title") && !built(&r, "count"),
        "its content is not"
    );
}

#[test]
fn a_tab_is_built_by_its_first_selection_and_kept() {
    let mut r = boot("/");
    r.act("later", vec![]).unwrap();
    press(&mut r, "tab-saved");
    assert!(built(&r, "saved-title"), "built in the selecting commit");
    // A child in a `when` of the screen starts its state when the screen is built.
    press(&mut r, "bump");
    let count = |r: &Runner<Feed>| {
        let tree: serde_json::Value = serde_json::from_str(&agent::tree(r)).unwrap();
        tree["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["props"]["testId"] == "count")
            .map(|n| n["props"]["text"].as_str().unwrap().to_owned())
    };
    assert_eq!(count(&r).as_deref(), Some("6"));
    press(&mut r, "tab-home");
    assert!(
        built(&r, "saved-title") && built(&r, "home-title"),
        "both stay built"
    );
    press(&mut r, "tab-saved");
    assert_eq!(
        count(&r).as_deref(),
        Some("6"),
        "its state survived the visit"
    );
}

#[test]
fn a_launch_location_and_a_typed_one_build_their_tab() {
    let r = boot("/note");
    assert!(
        built(&r, "saved-title") && built(&r, "note-title"),
        "every route of the tab"
    );
    assert!(!built(&r, "home-title"));
    let mut r = boot("/");
    r.act("follow", vec![Value::str("/saved")]).unwrap();
    assert!(built(&r, "saved-title"));
}

#[test]
fn tabs_in_a_virtualized_row_are_built_at_once() {
    // A collection builds its own rows, so routes in one are not deferred.
    let (head, rest) = SOURCE.split_once("  view\n").unwrap();
    let (view, tail) = rest.split_once("\ncomponent Counter").unwrap();
    let view: String = view
        .replace(r#"navigationBack="back" navigate=follow "#, "")
        .lines()
        .map(|l| format!("    {l}\n"))
        .collect();
    let source = format!(
        "{head}  view\n    list virtualized=true height=800 testId=\"list\"\n      each x in [1] key=x\n{view}\ncomponent Counter{tail}"
    );
    let plan = contract::compile(&source).unwrap_or_else(|e| panic!("{e:?}"));
    let r = Runner::boot(
        plan,
        Feed,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(built(&r, "home-title") && built(&r, "saved-title"));
}
