//! The language-neutral corpus and every law of the core.
//!
//! @ref LLP 1038 §3 (laws), D9 (these same records run through the TS binding).
//! Expected visits are plain JSON, including every intermediate value and
//! refusal. URL, component-encoding and query-read answers were read from
//! Chrome; no router implementation writes its own expected values here.

use exact_route::*;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Corpus {
    tables: BTreeMap<String, Table>,
    param_names: BTreeMap<String, Vec<String>>,
    canonical: Vec<Pair>,
    location_of: Vec<Pair>,
    components: Vec<Pair>,
    search: Vec<Search>,
    matches: Vec<MatchCase>,
    chains: Vec<ChainCase>,
    paths: Vec<PathCase>,
    checks: Vec<CheckCase>,
    sequences: Vec<Sequence>,
    reads: Vec<ReadCase>,
    laws: Vec<Law>,
    law_locations: BTreeMap<String, Vec<String>>,
}
#[derive(Deserialize)]
struct Pair {
    input: String,
    expected: String,
}
#[derive(Deserialize)]
struct Search {
    url: String,
    name: String,
    expected: String,
}
#[derive(Deserialize)]
struct MatchCase {
    table: String,
    location: String,
    expected: Option<Match>,
}
#[derive(Deserialize)]
struct ChainCase {
    table: String,
    location: String,
    expected: Vec<Destination>,
}
#[derive(Deserialize)]
struct PathCase {
    table: String,
    name: String,
    params: Vec<String>,
    expected: Option<String>,
    error: Option<String>,
}
#[derive(Deserialize)]
struct CheckCase {
    name: String,
    table: Table,
    error: Option<String>,
    route: Option<usize>,
}
#[derive(Deserialize)]
struct Sequence {
    name: String,
    table: String,
    steps: Vec<Step>,
}
#[derive(Deserialize)]
struct Step {
    verb: String,
    arg: Option<String>,
    expected: Router,
    refusal: Option<Refusal>,
}
#[derive(Deserialize)]
struct ReadCase {
    sequence: usize,
    step: usize,
    name: String,
    expected: Vec<String>,
}
#[derive(Deserialize)]
struct Law {
    name: String,
    statement: String,
}

fn corpus() -> &'static Corpus {
    static CORPUS: OnceLock<Corpus> = OnceLock::new();
    CORPUS.get_or_init(|| serde_json::from_str(include_str!("corpus.json")).unwrap())
}

fn success(result: (Router, Option<Refusal>)) -> Router {
    assert_eq!(result.1, None);
    result.0
}

fn apply(table: &Table, r: Router, verb: &str, arg: Option<&str>) -> (Router, Option<Refusal>) {
    match verb {
        "launch" => Router::launch(table, arg.unwrap()),
        "open" => open(table, r, arg.unwrap()),
        "push" => push(table, r, arg.unwrap()),
        "replace" => replace(table, r, arg.unwrap()),
        "back" => back(table, r),
        "select" => select(table, r, arg.unwrap()),
        "go" => go(table, r, arg.unwrap()),
        _ => panic!("unknown corpus verb {verb}"),
    }
}

fn finish(failures: Vec<String>) {
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn corpus_tables_and_parameter_orders() {
    let c = corpus();
    assert_eq!(c.tables.len(), 3);
    for (name, table) in &c.tables {
        assert_eq!(table.check(), Ok(()), "{name}");
        assert_eq!(table.param_names(), c.param_names[name], "{name}");
        let launch = success(Router::launch(table, "/"));
        assert!(launch
            .tabs
            .iter()
            .all(|tab| tab.stack.len() == 1 && tab.stack[0].name == tab.name));
    }
    let value = &c.sequences[0].steps[0].expected;
    let json = serde_json::to_string(value).unwrap();
    assert!(json.contains(r#""params":{"person":"","post":"","question":""}"#));
    assert_eq!(serde_json::from_str::<Router>(&json).unwrap(), *value);
}

#[test]
fn corpus_matches_and_chains() {
    let c = corpus();
    let mut failures = Vec::new();
    for case in &c.matches {
        let table = &c.tables[&case.table];
        let result = table.matches(&case.location);
        let expected_pattern = case
            .expected
            .clone()
            .filter(|m| table.routes.iter().any(|r| r.name == m.name && !r.notfound));
        let pattern = table.matches_pattern(&case.location);
        if pattern != expected_pattern {
            failures.push(format!(
                "pattern {} {}: {pattern:?} != {expected_pattern:?}",
                case.table, case.location
            ));
        }
        if result != case.expected {
            failures.push(format!(
                "match {} {}: {result:?} != {:?}",
                case.table, case.location, case.expected
            ));
        }
    }
    for case in &c.chains {
        let result = c.tables[&case.table].chain(&case.location);
        if result != case.expected {
            failures.push(format!(
                "chain {} {}: {result:?} != {:?}",
                case.table, case.location, case.expected
            ));
        }
    }
    finish(failures);
}

#[test]
fn corpus_paths_and_static_rejects() {
    let c = corpus();
    let mut failures = Vec::new();
    for case in &c.paths {
        let args: Vec<&str> = case.params.iter().map(String::as_str).collect();
        let result = c.tables[&case.table].path(&case.name, &args);
        let (value, error) = match result {
            Ok(s) => (Some(s), None),
            Err(e) => (None, Some(e.code)),
        };
        if value != case.expected || error != case.error {
            failures.push(format!(
                "path {} {}: {value:?}, {error:?}",
                case.table, case.name
            ));
        }
    }
    for case in &c.checks {
        let result = case.table.check().err();
        let error = result.as_ref().map(|e| &e.code);
        let route = result.as_ref().map(|e| e.route);
        if error != case.error.as_ref() || route != case.route {
            failures.push(format!("check {}: {result:?}", case.name));
        }
    }
    finish(failures);
}

#[test]
fn corpus_incoming_urls_and_component_encoding() {
    let mut failures = Vec::new();
    for (pairs, operation) in [
        (&corpus().location_of, location_of as fn(&str) -> String),
        (&corpus().components, encode_uri_component),
    ] {
        for pair in pairs {
            let actual = operation(&pair.input);
            if actual != pair.expected {
                failures.push(format!(
                    "{:?}: {actual:?} != {:?}",
                    pair.input, pair.expected
                ));
            }
        }
    }
    finish(failures);
}

#[test]
fn corpus_reads() {
    let c = corpus();
    let mut failures = Vec::new();
    for case in &c.search {
        let e = Entry {
            id: 0,
            name: String::new(),
            url: case.url.clone(),
            tab: String::new(),
            params: Params::new(),
        };
        let value = search_param(&e, &case.name);
        if value != case.expected {
            failures.push(format!(
                "search {:?} {:?}: {value:?} != {:?}",
                case.url, case.name, case.expected
            ));
        }
    }
    for case in &c.reads {
        let r = &c.sequences[case.sequence].steps[case.step].expected;
        let value = params(r, &case.name);
        if value != case.expected {
            failures.push(format!(
                "params {} {} {:?}: {value:?} != {:?}",
                case.sequence, case.step, case.name, case.expected
            ));
        }
    }
    for sequence in &c.sequences {
        for step in &sequence.steps {
            let r = &step.expected;
            let expected = &r.tabs.iter().find(|t| t.name == r.tab).unwrap().stack;
            assert_eq!(stack(r), expected);
            assert_eq!(top(r), expected.last());
            assert_eq!(depth(r), expected.len());
        }
    }
    finish(failures);
}

#[test]
fn corpus_verb_sequences() {
    let c = corpus();
    let mut failures = Vec::new();
    for sequence in &c.sequences {
        let table = &c.tables[&sequence.table];
        let mut r = Router::default();
        for (i, step) in sequence.steps.iter().enumerate() {
            let before = r.clone();
            let (next, refusal) = apply(table, r, &step.verb, step.arg.as_deref());
            if next != step.expected || refusal != step.refusal {
                failures.push(format!(
                    "{} step {i} {} {:?}\nactual {} {:?}\nexpected {} {:?}",
                    sequence.name,
                    step.verb,
                    step.arg,
                    serde_json::to_string(&next).unwrap(),
                    refusal,
                    serde_json::to_string(&step.expected).unwrap(),
                    step.refusal
                ));
            }
            if refusal.is_some() {
                assert_eq!(next, before, "a refused verb changed its value");
            }
            assert!(next.next >= before.next);
            let before_ids: BTreeSet<_> = before
                .tabs
                .iter()
                .flat_map(|t| &t.stack)
                .map(|e| e.id)
                .collect();
            for entry in next.tabs.iter().flat_map(|t| &t.stack) {
                assert!(
                    before_ids.contains(&entry.id) || entry.id >= before.next,
                    "an old id was reused"
                );
            }
            r = next;
        }
    }
    finish(failures);
}

// Each named law is a test, exercised over every recorded reachable value of
// all three tables. The JSON supplies its statement and the location domain.
fn law(name: &str, run: impl Fn(&Table, &Router, &[String])) {
    let c = corpus();
    assert_eq!(c.laws.len(), 9);
    assert!(!c
        .laws
        .iter()
        .find(|l| l.name == name)
        .unwrap()
        .statement
        .is_empty());
    for sequence in &c.sequences {
        for step in &sequence.steps {
            run(
                &c.tables[&sequence.table],
                &step.expected,
                &c.law_locations[&sequence.table],
            );
        }
    }
}

#[test]
fn law_back_undoes_a_push_on_the_selected_stack() {
    law("back_push", |t, r, urls| {
        for url in urls {
            let pushed = success(push(t, r.clone(), url));
            let popped = success(back(t, pushed));
            assert_eq!(stack(&popped), stack(r));
            assert_eq!(popped.tabs, r.tabs);
        }
    });
}
#[test]
fn law_back_at_root_is_unchanged() {
    law("back_root", |t, r, _| {
        let root = success(select(t, r.clone(), &r.tab));
        assert_eq!(depth(&root), 1);
        assert_eq!(success(back(t, root.clone())), root);
    });
}
#[test]
fn law_selecting_a_tab_twice_leaves_its_root() {
    law("select_twice", |t, r, _| {
        for tab in &r.tabs {
            let once = success(select(t, r.clone(), &tab.name));
            let twice = success(select(t, once, &tab.name));
            assert_eq!(depth(&twice), 1);
            assert_eq!(top(&twice), tab.stack.first());
        }
    });
}
#[test]
fn law_open_is_idempotent() {
    law("open_idempotent", |t, r, urls| {
        for url in urls {
            let once = success(open(t, r.clone(), url));
            assert_eq!(success(open(t, once.clone(), url)), once);
        }
    });
}
#[test]
fn law_replace_keeps_the_visit_id() {
    law("replace_id", |t, r, urls| {
        for url in urls {
            let (replaced, refusal) = replace(t, r.clone(), url);
            if let Some(refusal) = refusal {
                assert_eq!(depth(r), 1);
                assert_ne!(t.matches(url).unwrap().name, r.tab);
                assert_eq!(
                    refusal.message,
                    "replace cannot change the tab's root route"
                );
                assert_eq!(replaced, *r);
            }
            assert_eq!(top(&replaced).unwrap().id, top(r).unwrap().id);
            assert_eq!(replaced.next, r.next);
            assert_eq!(replaced.tab, r.tab);
        }
    });
}
#[test]
fn law_go_to_the_top_is_unchanged() {
    law("go_top", |t, r, _| {
        assert_eq!(success(go(t, r.clone(), &top(r).unwrap().url)), *r);
    });
}
#[test]
fn law_visit_ids_are_distinct_and_below_next() {
    law("unique_ids", |_, r, _| {
        let mut ids = BTreeSet::new();
        for tab in &r.tabs {
            assert!(!tab.stack.is_empty());
            assert_eq!(tab.stack[0].name, tab.name);
            for entry in &tab.stack {
                assert!(ids.insert(entry.id));
                assert!(entry.id < r.next);
                assert_eq!(entry.tab, tab.name);
            }
        }
    });
}
#[test]
fn law_entries_roundtrip_through_matching() {
    law("entry_roundtrip", |t, r, _| {
        for entry in r.tabs.iter().flat_map(|t| &t.stack) {
            assert_eq!(canonical(&entry.url), entry.url);
            assert_eq!(
                t.matches(&entry.url),
                Some(Match {
                    name: entry.name.clone(),
                    params: entry.params.clone()
                })
            );
        }
    });
}
#[test]
fn law_canonical_is_idempotent_and_agrees_with_chrome() {
    assert!(corpus()
        .laws
        .iter()
        .any(|l| l.name == "canonical_idempotent"));
    let mut failures = Vec::new();
    for pair in &corpus().canonical {
        let actual = canonical(&pair.input);
        if actual != pair.expected {
            failures.push(format!(
                "Chrome {:?}: {actual:?} != {:?}",
                pair.input, pair.expected
            ));
        }
        if canonical(&actual) != actual {
            failures.push(format!(
                "not idempotent: {:?} -> {actual:?} -> {:?}",
                pair.input,
                canonical(&actual)
            ));
        }
    }
    finish(failures);
}

#[test]
fn empty_values_and_id_exhaustion_refuse_without_partial_changes() {
    let table = &corpus().tables["messages"];
    let empty = Router::default();
    assert_eq!(stack(&empty), []);
    assert_eq!(top(&empty), None);
    assert_eq!(depth(&empty), 0);
    assert_eq!(success(back(table, empty.clone())), empty);
    for verb in ["open", "push", "replace", "go"] {
        let (after, reason) = apply(table, empty.clone(), verb, Some("/absent"));
        assert_eq!(after, empty);
        assert_eq!(reason.unwrap().message, "no route matches /absent");
    }
    let mut exhausted = success(Router::launch(table, "/"));
    exhausted.next = (1u64 << 53) - 1;
    for verb in ["push", "open", "go"] {
        let (after, reason) = apply(table, exhausted.clone(), verb, Some("/t/5/details"));
        assert_eq!(after, exhausted);
        assert_eq!(reason.unwrap().message, "router entry ids exhausted");
    }
    assert_eq!(
        success(replace(table, exhausted.clone(), "/?draft=1")).next,
        exhausted.next
    );
    let mut almost = exhausted;
    almost.next -= 1;
    let (after, reason) = open(table, almost.clone(), "/t/5/details");
    assert_eq!(
        after, almost,
        "allocating the first ancestor must not partly commit"
    );
    assert_eq!(reason.unwrap().message, "router entry ids exhausted");
}

// @ref LLP 1038 D9 — replay the existing expected values through the real VM.
#[path = "../../runner/tests/support/router.rs"]
mod runner_plan;

struct NoData;
impl exact_runner::DataSource for NoData {
    fn query(
        &mut self,
        source: &str,
        _: &[exact_plan::Value],
    ) -> Result<exact_plan::Value, exact_runner::DataError> {
        panic!("corpus has no source {source}")
    }
}

#[test]
fn corpus_verb_sequences_through_the_runner() {
    let c = corpus();
    let mut failures = Vec::new();
    for sequence in &c.sequences {
        let plan = runner_plan::plan(&c.tables[&sequence.table]);
        let mut runner = None;
        for (i, step) in sequence.steps.iter().enumerate() {
            if step.verb == "launch" {
                runner = Some(
                    exact_runner::Runner::boot(
                        plan.clone(),
                        NoData,
                        exact_kernel::Kernel::with_monospace(),
                        Default::default(),
                        step.arg.as_deref().unwrap(),
                    )
                    .unwrap(),
                );
            } else {
                let args = step
                    .arg
                    .as_deref()
                    .map(exact_plan::Value::str)
                    .into_iter()
                    .collect();
                runner
                    .as_mut()
                    .expect("launch first")
                    .act(&step.verb, args)
                    .unwrap();
            }
            let r = runner.as_ref().unwrap();
            let value = r.carry().router.unwrap().1;
            if value != step.expected {
                failures.push(format!(
                    "runner {} step {i}: {value:?} != {:?}",
                    sequence.name, step.expected
                ));
            }
            if let Some(refusal) = &step.refusal {
                if !r.journal().any(|line| line.contains(&refusal.message)) {
                    failures.push(format!(
                        "runner {} step {i}: missing refusal {refusal}",
                        sequence.name
                    ));
                }
            }
        }
    }
    finish(failures);
}

#[test]
fn corpus_paths_survive_push_without_losing_parameters() {
    let c = corpus();
    for case in &c.paths {
        let table = &c.tables[&case.table];
        let args = case.params.iter().map(String::as_str).collect::<Vec<_>>();
        let Ok(url) = table.path(&case.name, &args) else {
            continue;
        };
        let r = success(push(table, success(Router::launch(table, "/")), &url));
        let entry = top(&r).unwrap();
        assert_eq!(entry.name, case.name);
        let route = table
            .routes
            .iter()
            .find(|route| route.name == case.name)
            .unwrap();
        for (name, value) in route
            .pattern
            .split('/')
            .filter_map(|s| s.strip_prefix(':'))
            .zip(&case.params)
        {
            assert_eq!(&entry.params[name], value);
        }
    }
}
