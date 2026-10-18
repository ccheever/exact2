//! Run the actual app plan with fixture replies. These checks cover the rendered
//! composer popovers; native chip text and elements have AppKit tests.

use super::PLAN;
use exact_apple::measure::{CMetrics, CRequest, CallbackMeasurer};
use exact_plan::{Plan, TypesId};
use exact_runner::{Answer, DataError, DataSource, Outcome, Request, Store, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Default)]
pub(super) struct Replies(
    pub(super) HashMap<String, Value>,
    pub(super) Vec<Vec<Value>>,
);

impl DataSource for Replies {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        self.0
            .get(source)
            .cloned()
            .ok_or_else(|| DataError::UnknownSource(source.to_owned()))
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        if source == "command" {
            self.1.push(args.to_vec());
        }
        Ok(self.0.get(source).cloned().map_or_else(
            || Answer::Later(Request::get("https://fixture.invalid/")),
            Answer::Now,
        ))
    }

    fn parse(
        &mut self,
        _: &mut Store,
        source: &str,
        args: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        self.query(source, args).map(Answer::Now)
    }
}

pub(super) fn set(plan: &Plan, ty: TypesId, value: &mut Value, path: &[&str], next: Value) {
    let index = plan.field_index(ty, path[0]).expect("fixture field exists");
    let Value::Record(fields) = value else {
        panic!("fixture field is a record")
    };
    let mut updated = fields.to_vec();
    if path.len() == 1 {
        updated[index] = next;
    } else {
        let field = plan.type_(ty).fields.iter().nth(index).unwrap();
        set(
            plan,
            plan.field(field).ty,
            &mut updated[index],
            &path[1..],
            next,
        );
    }
    *value = Value::record(updated);
}

/// Resource `name`'s first-frame answer from the app's own data module (app.ts
/// and the Rust Markdown sources), booted with no native module at date zero,
/// so its client is disconnected. The plan holds no value for it: `data` is
/// asked with `wallTime`, and the bake leaves out every answer asked with the
/// date, directly or through another answer (LLP 1027.000.000 D2). The app boots
/// once per test process; its answers are kept as bytes, since a `Value` is not
/// `Send`.
pub(super) fn first_frame(name: &str) -> Value {
    static ANSWERS: OnceLock<HashMap<String, Vec<u8>>> = OnceLock::new();
    let answers = ANSWERS.get_or_init(|| {
        let mut module = exact_js::Module::new(super::BYTECODE.to_vec(), super::APP, super::GRANTS);
        // A fixture carries no per-call budget; a loaded test machine is no gate.
        module.set_budget_ms(f64::INFINITY);
        module.load().unwrap();
        let data = t3_code_data::mixed(module, exact_runner::Placement::Main);
        let measurer = CallbackMeasurer::new(measure, std::ptr::null_mut(), None);
        let (host, _) =
            exact_apple::Host::boot(PLAN, data, Box::new(measurer), 1280.0, 840.0).unwrap();
        let runner = host.runner();
        let plan = runner.plan();
        plan.resources
            .iter()
            .map(|row| plan.str(row.name))
            .filter(|name| !runner.resource_is_placeholder(name))
            .filter_map(|name| {
                runner
                    .resource(name)
                    .map(|value| (name.to_owned(), value.to_bytes()))
            })
            .collect()
    });
    let bytes = answers
        .get(name)
        .unwrap_or_else(|| panic!("the data module answers `{name}` on the first frame"));
    Value::from_bytes(bytes).unwrap()
}

impl Replies {
    pub(super) fn fixture(&mut self, plan: &Plan, name: &str, fields: &[(&[&str], Value)]) {
        let resource = plan
            .resources
            .iter()
            .find(|row| plan.str(row.name) == name)
            .unwrap();
        let mut value = first_frame(name);
        for (path, next) in fields {
            set(plan, resource.ty, &mut value, path, next.clone());
        }
        self.0.insert(plan.str(resource.source).to_owned(), value);
    }
}

pub(super) extern "C" fn measure(_: *mut std::ffi::c_void, _: *const CRequest) -> CMetrics {
    CMetrics::default()
}

fn host(terminal: bool) -> exact_apple::Host<Replies> {
    let plan = Plan::decode(PLAN).unwrap();
    let mut replies = Replies::default();
    replies.fixture(
        &plan,
        "data",
        &[
            (&["revision"], Value::Number(1.0)),
            (&["composer", "contextShow"], Value::Bool(true)),
            (&["composer", "contextMax"], Value::str("200K")),
            (&["composer", "contextUsed"], Value::str("52K")),
            (&["composer", "contextPercent"], Value::str("26%")),
            (&["composer", "contextValueNow"], Value::Number(26.0)),
            (&["composer", "contextCompact"], Value::Bool(true)),
            (
                &["composer", "contextNote"],
                Value::str("Fixture context note"),
            ),
        ],
    );
    if terminal {
        replies.fixture(
            &plan,
            "chip",
            &[
                (&["seq"], Value::Number(7.0)),
                (&["open"], Value::Bool(true)),
                (&["kind"], Value::str("terminal")),
                (&["label"], Value::str("Terminal 2")),
                (
                    &["title"],
                    Value::str("Terminal excerpt, Terminal 2 lines 1-3"),
                ),
                (&["lines"], Value::str("Lines 1–3")),
                (&["description"], Value::str("$ ls\na.txt\nb.txt")),
                (&["rows"], Value::Number(3.0)),
                (&["x"], Value::Number(300.0)),
                (&["y"], Value::Number(600.0)),
                (&["width"], Value::Number(160.0)),
                (&["height"], Value::Number(19.5)),
            ],
        );
    }
    let measurer = CallbackMeasurer::new(measure, std::ptr::null_mut(), None);
    let (mut host, _) =
        exact_apple::Host::boot(PLAN, replies, Box::new(measurer), 1280.0, 840.0).unwrap();
    host.runner_mut()
        .set_time(1_791_446_400_000.0, 0.0)
        .unwrap();
    host.runner_mut().advance(1250.0).unwrap();
    host
}

fn node(runner: &exact_runner::Runner<Replies>, test_id: &str) -> u32 {
    let nodes = runner.kernel().find_by_test_id(test_id);
    assert_eq!(nodes.len(), 1, "one rendered {test_id}");
    runner.kernel().node_by_key(nodes[0]).unwrap().id
}

fn prop(runner: &exact_runner::Runner<Replies>, test_id: &str, name: &str) -> Option<String> {
    runner
        .kernel()
        .node(node(runner, test_id))
        .unwrap()
        .props
        .iter()
        .find(|(id, _)| id.name() == name)
        .and_then(|(_, value)| value.as_str().map(str::to_owned))
}

fn inside(runner: &exact_runner::Runner<Replies>, child: &str, parent: &str) -> bool {
    let ancestor = node(runner, parent);
    let mut current = runner.kernel().node(node(runner, child)).unwrap().parent;
    while let Some(id) = current {
        if id == ancestor {
            return true;
        }
        current = runner.kernel().node(id).unwrap().parent;
    }
    false
}

#[test]
fn context_window_details_render_as_one_unnamed_dialog() {
    let mut host = host(false);
    let runner = host.runner_mut();
    assert_eq!(
        prop(runner, "context-window-meter", "accessibilityHasPopup").as_deref(),
        Some("dialog")
    );
    runner
        .act("openOptions", vec![Value::str("context")])
        .unwrap();
    assert_eq!(
        prop(runner, "composer-options", "accessibilityRole").as_deref(),
        Some("dialog")
    );
    assert_eq!(
        prop(runner, "composer-options", "accessibilityLabel").as_deref(),
        Some("")
    );
    assert_eq!(
        prop(runner, "context-window-bar", "accessibilityRole").as_deref(),
        Some("progressbar")
    );
    assert!(inside(runner, "context-window-bar", "composer-options"));
    assert!(inside(runner, "compact-context", "composer-options"));
    runner
        .act("openOptions", vec![Value::str("runtime")])
        .unwrap();
    assert_eq!(
        prop(runner, "composer-options", "accessibilityRole").as_deref(),
        Some("listbox")
    );
    assert_eq!(
        prop(runner, "composer-options", "accessibilityLabel").as_deref(),
        Some("Runtime mode")
    );
}

#[test]
fn terminal_chip_details_render_the_record_inside_its_named_dialog() {
    let host = host(true);
    let runner = host.runner();
    assert_eq!(
        prop(runner, "chip-popover", "accessibilityRole").as_deref(),
        Some("dialog")
    );
    assert_eq!(
        prop(runner, "chip-popover", "accessibilityLabel").as_deref(),
        Some("Terminal excerpt, Terminal 2 lines 1-3")
    );
    assert_eq!(
        prop(runner, "chip-popover-label", "text").as_deref(),
        Some("Terminal 2")
    );
    assert_eq!(
        prop(runner, "chip-popover-lines", "text").as_deref(),
        Some("Lines 1–3")
    );
    assert_eq!(
        prop(runner, "chip-popover-output", "text").as_deref(),
        Some("$ ls\na.txt\nb.txt")
    );
    assert_eq!(
        prop(runner, "chip-popover-output", "accessibilityLabel").as_deref(),
        Some("Captured terminal output")
    );
    assert!(inside(runner, "chip-popover-output", "chip-popover"));
    assert!(runner
        .kernel()
        .find_by_test_id("chip-popover-instructions")
        .is_empty());
}
