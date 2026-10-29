//! A module's substitute is a plan constant (`Plan::str` of its row; a
//! runner's plan is never written after boot, and a new plan is a new
//! runner): it is parsed once per runner and host target, not on each read.
//! A scroll dispatch, which polls every module's substitute, parses none
//! once they are read, and a runner booted on another plan reads that plan's.
use super::SUBSTITUTE_PARSES;
use crate::{Answer, DataError, DataSource, Event, Outcome, Runner, ScrollEvent, Store};
use exact_kernel::{Kernel, NodeType};
use exact_plan::{asm::Asm, builder::PlanBuilder, EventKind, Opcode, Plan, TypeKind, Value};

struct Source;

impl DataSource for Source {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!("the fixture answers nothing")
    }
    fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
        Ok(Answer::Now(Value::str("x")))
    }
    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        unreachable!("answers now")
    }
}

/// Two native modules, each with a substitute that polls (`counts`), and a
/// view whose `scroll` counts each report in `scrolls` (a commit each).
fn plan(timeout: &str) -> Plan {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let string = b.primitive(TypeKind::String);
    let number = b.primitive(TypeKind::Number);
    for name in ["steps", "flights"] {
        let source = b.source(name, &[], string);
        // A permission read on `target-7` alone (`this_target` drops it elsewhere).
        let substitute = format!(
            r#"{{"counts":{{"unit":"count"}},"timeout":"{timeout}","permission":{{"name":"motion","targets":["target-7"]}}}}"#
        );
        b.module(
            source,
            "test.motion",
            name,
            false,
            false,
            "never",
            "",
            &substitute,
        );
    }
    let mut zero = Asm::new();
    zero.number(0.);
    let zero = b.code(zero);
    let scrolls = b.slot("scrolls", number, zero);
    let mut body = Asm::new();
    body.load_slot(scrolls)
        .number(1.)
        .simple(Opcode::Add)
        .store_slot(scrolls);
    let body = b.code(body);
    let scrolled = b.action(
        "scrolled",
        &[("x", number), ("y", number)],
        &[scrolls],
        body,
    );
    b.node(
        NodeType::View as u8,
        None,
        None,
        0,
        &[],
        &[(EventKind::Scroll, scrolled, &[])],
        None,
    );
    b.finish().unwrap()
}

fn boot(plan: Plan) -> Runner<Source> {
    Runner::boot_stored(
        plan,
        Source,
        Kernel::with_monospace(),
        vec![],
        Default::default(),
        "/",
    )
    .unwrap()
}

/// A scroll report the view handles: the dispatch commits and its handler runs.
fn scroll(r: &mut Runner<Source>, at: f64) {
    let root = r.roots()[0];
    let before = r.slot("scrolls").cloned();
    let event = ScrollEvent {
        top: at,
        height: 2000.,
        client_height: 600.,
        ..Default::default()
    };
    let advanced = r.dispatch_at(root, Event::Scroll(event), at);
    assert!(advanced.error.is_none(), "{:?}", advanced.error);
    assert!(!advanced.receipts.is_empty(), "the dispatch committed");
    assert_ne!(r.slot("scrolls").cloned(), before, "its handler ran");
}

fn parses() -> usize {
    SUBSTITUTE_PARSES.with(|n| n.get())
}

#[test]
fn a_committed_scroll_dispatch_parses_no_substitute_once_read() {
    // The count is the thread's: from zero here, whatever ran on it before.
    SUBSTITUTE_PARSES.with(|n| n.set(0));
    let mut r = boot(plan("5000"));
    for (row, name) in r.plan.modules.clone().iter().zip(["steps", "flights"]) {
        assert!(r.substitute_polls(row), "{name} polls");
    }
    // Each substitute read once, before the dispatches measured.
    scroll(&mut r, 16.);
    let read = parses();
    assert!(read <= 2, "each substitute parsed at most once: {read}");
    for k in 2..40 {
        scroll(&mut r, 16. * k as f64);
    }
    assert_eq!(
        parses(),
        read,
        "38 committed scroll dispatches parsed no substitute again"
    );
}

#[test]
fn a_runner_on_another_plan_reads_that_plans_substitutes() {
    let first = boot(plan("5000"));
    let second = boot(plan("250"));
    let row = |r: &Runner<Source>| r.plan.modules[0].clone();
    assert_eq!(first.substitute(&row(&first))["timeout"], "5000");
    assert_eq!(second.substitute(&row(&second))["timeout"], "250");
    assert_eq!(
        first.substitute(&row(&first))["timeout"],
        "5000",
        "each runner its own plan's"
    );
}

/// The host target can be said again and again, any string: the cache holds
/// one entry a module row, and each read is for the target said last (a
/// permission on `target-7` alone shows there and nowhere else).
#[test]
fn a_changing_target_keeps_one_substitute_a_module_read_for_it() {
    let mut r = boot(plan("5000"));
    let rows = r.plan.modules.clone();
    for k in 0..50 {
        r.set_module_target(&format!("target-{k}"));
        for row in &rows {
            let permission = r.substitute(row)["permission"].is_object();
            assert_eq!(permission, k == 7, "target-{k}");
        }
        assert!(r.extensions.substitutes.borrow().len() <= rows.len());
    }
    r.set_module_target("ios");
    assert_eq!(r.substitute(&rows[0])["timeout"], "5000");
    assert!(!r.substitute(&rows[0])["permission"].is_object());
}
