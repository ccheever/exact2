//! @ref LLP 1121 D8 (stage 8 B): a node's host-known state looks reach the
//! kernel resolved, as a static row is, and a token's live edit resolves
//! them again.

use exact_kernel::{Kernel, LOOK_ACTIVE, LOOK_HOVER};
use exact_plan::{Plan, Value};
use exact_runner::{DataError, DataSource, Runner};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}
use std::path::PathBuf;

fn app(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-looks-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir.join("app.contract")
}

fn booted(plan: Plan) -> Runner<NoData> {
    let plan = contract::bake(plan, NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

const DESIGN: &str = "tamber t\n\ntheme\n  accent.soft = #eeeeee\n  ink = #111111\n";

const VIEW: &str = "component App\n  state n = 0\n  action go\n    n = n + 1\n  view\n    column testId=\"card\" press=go background-color=\"#ffffff\"\n      :active\n        background-color = \"accent.soft\"\n        opacity = 0.9\n      :hover\n        border-color = \"ink\"\n      text \"c\"\n";

fn looks(r: &Runner<NoData>) -> String {
    let k = r.kernel();
    let node = k.node_by_key(k.find_by_test_id("card")[0]).unwrap();
    format!(
        "{:?}",
        node.looks()
            .iter()
            .map(|(s, p)| (*s, p.background_color, p.opacity, p.border_color_top))
            .collect::<Vec<_>>()
    )
}

#[test]
fn a_nodes_looks_reach_the_kernel_resolved_and_follow_a_live_token() {
    let path = app("looks", &[("app.tamber", DESIGN), ("app.contract", VIEW)]);
    let r = booted(contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}")));
    let k = r.kernel();
    let node = k.node_by_key(k.find_by_test_id("card")[0]).unwrap();
    let states: Vec<u8> = node.looks().iter().map(|(s, _)| *s).collect();
    assert_eq!(
        states,
        [LOOK_ACTIVE, LOOK_HOVER],
        "one look a state, ascending"
    );
    let before = looks(&r);
    assert!(before.contains("0.9"), "{before}");
    // The base is untouched: the look is the host's to apply.
    assert_eq!(
        format!("{:?}", node.style.background_color),
        "Some(Fixed(Color(4294967295)))",
        "the base keeps its own white"
    );
    let mut r = r;
    let (_, set) = r.set_token("accent.soft", "#ff0000", &[]).unwrap();
    assert!(set >= 1, "the look's rows were set again");
    let after = looks(&r);
    assert_ne!(before, after, "the look follows the token");
    // A token no look reads sets no look.
    let (_, set) = r.set_token("ink", "#222222", &[]).unwrap();
    assert!(set >= 1);
}
