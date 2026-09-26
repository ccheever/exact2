//! A testId on two mounted routes resolves on the selected one (LLP 1012):
//! the screens a stack keeps under its top are flagged `inactive` in `tree`,
//! and a covered copy is the target only when no active screen carries it.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{agent, DataError, DataSource, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("unexpected {name}")
    }
}

const SOURCE: &str = "routes nav
  tab home \"/\"
    thread \"/t/:thread\"

component App
  action back writes nav
    nav = back(nav)
  view
    main navigationKey=`${top(nav).id}` navigationBack=\"back\" testId=\"navigation\"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` testId=`route-${e.name}`
          button testId=\"dup\" press=back
            text e.name
          when e.name == \"home\"
            text \"only here\" testId=\"only-home\"
";

/// Each node of a tree reply as its JSON text from `\"id\":` on.
fn nodes(tree: &str) -> Vec<&str> {
    tree.split("{\"id\":").skip(1).collect()
}
fn field(node: &str, name: &str) -> String {
    if name == "id" {
        return node.split(',').next().unwrap().to_owned();
    }
    node.split(&format!("\"{name}\":"))
        .nth(1)
        .unwrap_or_else(|| panic!("{name} in {node}"))
        .split([',', '}'])
        .next()
        .unwrap()
        .to_owned()
}
fn by_test_id<'a>(tree: &'a str, test_id: &str) -> &'a str {
    let key = format!("\"testId\":\"{test_id}\"");
    nodes(tree)
        .into_iter()
        .find(|n| n.contains(&key))
        .unwrap_or_else(|| panic!("{test_id} in {tree}"))
}
fn target(r: &Runner<NoData>, test_id: &str) -> String {
    agent::handle(
        r,
        &format!("{{\"op\":\"tree\",\"target\":\"{test_id}\",\"shallow\":true}}"),
    )
}

#[test]
fn a_test_id_resolves_on_the_selected_route_and_covered_screens_are_flagged() {
    let plan = contract::compile(SOURCE).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/t/1",
    )
    .unwrap();
    let tree = agent::tree(&r);
    let home = by_test_id(&tree, "route-home");
    let thread = by_test_id(&tree, "route-thread");
    assert_eq!(field(home, "inactive"), "true");
    assert!(!thread.contains("\"inactive\""), "{thread}");
    let dups: Vec<&str> = nodes(&tree)
        .into_iter()
        .filter(|n| n.contains("\"testId\":\"dup\""))
        .collect();
    assert_eq!(dups.len(), 2);
    assert_eq!(field(dups[0], "parent"), field(home, "id"));
    assert_eq!(field(dups[0], "inactive"), "true");
    assert_eq!(field(dups[1], "parent"), field(thread, "id"));
    assert!(!dups[1].contains("\"inactive\""), "{}", dups[1]);

    // The target is the top screen's copy, not the first in preorder.
    let found = target(&r, "dup");
    let found = nodes(&found);
    assert_eq!(found.len(), 1);
    assert_eq!(field(found[0], "id"), field(dups[1], "id"));
    // A testId only a covered screen carries still resolves, flagged.
    let only = target(&r, "only-home");
    assert_eq!(field(nodes(&only)[0], "inactive"), "true");

    // Popping the thread makes home the selected route: nothing is covered.
    r.act("back", vec![]).unwrap();
    let tree = agent::tree(&r);
    assert!(!tree.contains("\"inactive\""), "{tree}");
    let found = target(&r, "dup");
    assert_eq!(
        field(nodes(&found)[0], "parent"),
        field(by_test_id(&tree, "route-home"), "id")
    );
}
