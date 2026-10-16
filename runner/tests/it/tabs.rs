//! A navigation tab is built the first time it is selected (LLP 1075.003
//! §3.7): until then its panel is empty in the runner and `unopened` in the
//! agent's `tree`; once built it stays, with its state.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{agent, DataError, DataSource, Event, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("unexpected {name}")
    }
}

/// Each tab's screen holds a counter whose state is the component's own.
fn source(selected: &str) -> String {
    format!(
        "routes nav
  tab a \"/\"
  tab b \"/b\"
  tab c \"/c\"

component App
  action pick(name: string)
    nav = select(nav, name)
  view
    main navigationKey=`${{top(nav).id}}` navigationBack=\"back\"
      column
        for t in nav.tabs key=t.name
          column role=\"tabpanel\" id=`panel-${{t.name}}` testId=`panel-${{t.name}}`
            for e in t.stack key=e.id
              column navigationKey=`${{e.id}}`
                Counter(label=e.name)
      row role=\"tablist\"
        button press=pick(\"a\") testId=\"tab-a\" role=\"tab\" aria-controls=\"panel-a\"{a}
          text \"A\"
        button press=pick(\"b\") testId=\"tab-b\" role=\"tab\" aria-controls=\"panel-b\"{b}
          text \"B\"
        button press=pick(\"c\") testId=\"tab-c\" role=\"tab\" aria-controls=\"panel-c\"{c}
          text \"C\"

component Counter
  props
    label: string
  state n = 0
  action bump
    n = n + 1
  view
    button testId=`bump-${{label}}` press=bump
      text `${{label}} ${{n}}`
",
        a = selected.replace('@', "a"),
        b = selected.replace('@', "b"),
        c = selected.replace('@', "c"),
    )
}

const ARIA: &str = " aria-selected=(nav.tab == \"@\")";

fn boot(source: &str, url: &str) -> Runner<NoData> {
    let plan = contract::compile(source).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        url,
    )
    .unwrap()
}

fn nodes(tree: &str) -> Vec<&str> {
    tree.split("{\"id\":").skip(1).collect()
}

fn panel<'a>(tree: &'a str, name: &str) -> &'a str {
    let key = format!("\"testId\":\"panel-{name}\"");
    nodes(tree)
        .into_iter()
        .find(|n| n.contains(&key))
        .unwrap_or_else(|| panic!("panel-{name} in {tree}"))
}

/// Whether panel `name` is waiting: `unopened`, with no children.
fn unopened(tree: &str, name: &str) -> bool {
    let p = panel(tree, name);
    let waiting = p.contains("\"unopened\":true");
    assert_eq!(waiting, p.contains("\"children\":[]"), "{p}");
    waiting
}

fn has(tree: &str, test_id: &str) -> bool {
    tree.contains(&format!("\"testId\":\"{test_id}\""))
}

#[test]
fn a_tab_is_built_when_first_selected_and_keeps_its_state() {
    let mut r = boot(&source(ARIA), "/");
    let tree = agent::tree(&r);
    assert!(!unopened(&tree, "a"));
    assert!(unopened(&tree, "b") && unopened(&tree, "c"));
    assert!(has(&tree, "bump-a") && !has(&tree, "bump-b"));
    let live = r.kernel().live_count();

    // Selecting b builds it, in the commit that selects it.
    r.act("pick", vec![Value::str("b")]).unwrap();
    let tree = agent::tree(&r);
    assert!(!unopened(&tree, "b") && unopened(&tree, "c"));
    assert!(has(&tree, "bump-b"));
    assert!(r.kernel().live_count() > live);
    assert!(tree.contains("\"text\":\"b 0\""), "{tree}");

    // Its state starts when it is built and outlives a switch away.
    let bump = agent::handle(
        &r,
        "{\"op\":\"tree\",\"target\":\"bump-b\",\"shallow\":true}",
    );
    let id: u32 = bump
        .split("{\"id\":")
        .nth(1)
        .unwrap()
        .split(',')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    r.dispatch(id, Event::Press).unwrap();
    r.act("pick", vec![Value::str("a")]).unwrap();
    r.act("pick", vec![Value::str("b")]).unwrap();
    let tree = agent::tree(&r);
    assert!(tree.contains("\"text\":\"b 1\""), "{tree}");
    assert!(!unopened(&tree, "a") && !unopened(&tree, "b") && unopened(&tree, "c"));
    assert_eq!(r.unopened_tabs().len(), 1);
}

#[test]
fn a_launch_into_another_tab_builds_that_tab_alone() {
    let r = boot(&source(ARIA), "/c");
    let tree = agent::tree(&r);
    assert!(unopened(&tree, "a") && unopened(&tree, "b"));
    assert!(!unopened(&tree, "c") && has(&tree, "bump-c"));
}

/// With no tab marked selected, the panels are built in tab order until one
/// holds the selected route.
#[test]
fn without_aria_selected_the_panel_holding_the_route_is_found() {
    let r = boot(&source(""), "/b");
    let tree = agent::tree(&r);
    assert!(!unopened(&tree, "a") && !unopened(&tree, "b") && unopened(&tree, "c"));
    assert!(has(&tree, "bump-b"));
}

/// A tabpanel no tablist names is no navigation tab: it is built at once.
#[test]
fn a_panel_no_tab_names_is_built_at_once() {
    let source = source(ARIA).replace(" aria-controls=\"panel-c\"", "");
    let r = boot(&source, "/");
    let tree = agent::tree(&r);
    assert!(unopened(&tree, "b"));
    assert!(!unopened(&tree, "c") && has(&tree, "bump-c"));
}
