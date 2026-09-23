//! Builder fixture for the VM seam, shared with the leaf crate's corpus runner.
//! @ref LLP 1038 D3/D9 — mirrors exactly the shapes chunk (c) will declare.
use exact_kernel::NodeType;
use exact_plan::{asm::Asm, builder::PlanBuilder, Opcode, Plan, Stdlib, TypeKind};

pub fn builder(table: &exact_route::Table) -> PlanBuilder {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let string = b.primitive(TypeKind::String);
    let number = b.primitive(TypeKind::Number);
    // Unrelated types before and between the shapes rule out hardcoded type ids.
    b.record("Unrelated", &[("tab", number)]);
    let params = b.record(
        "Params",
        &table
            .param_names()
            .into_iter()
            .map(|n| (n, string))
            .collect::<Vec<_>>(),
    );
    let entry = b.record(
        "Entry",
        &[
            ("id", number),
            ("name", string),
            ("url", string),
            ("tab", string),
            ("params", params),
        ],
    );
    let entries = b.list(entry);
    let tab = b.record("Tab", &[("name", string), ("stack", entries)]);
    let tabs = b.list(tab);
    let router = b.record(
        "Router",
        &[("tab", string), ("tabs", tabs), ("next", number)],
    );
    let strings = b.list(string);
    let mut dummy = Asm::new();
    dummy.op(Opcode::Unit, &[]);
    let dummy = b.code(dummy);
    let initial_url = b.slot("initial_url", string, dummy);
    let nav = b.slot("nav", router, dummy);
    b.set_router(nav);
    let mut url = Asm::new();
    url.load_slot(nav).call(Stdlib::Top).field(2);
    let url = b.code(url);
    b.set_slot_init(initial_url, url);
    b.derive("url", string, url);
    for (f, ty) in [
        (Stdlib::Stack, entries),
        (Stdlib::Top, entry),
        (Stdlib::Depth, number),
    ] {
        let mut a = Asm::new();
        a.load_slot(nav).call(f);
        let code = b.code(a);
        b.derive(f.name(), ty, code);
    }
    for f in [
        Stdlib::Open,
        Stdlib::Push,
        Stdlib::Replace,
        Stdlib::Back,
        Stdlib::Select,
        Stdlib::Go,
    ] {
        let mut a = Asm::new();
        a.load_slot(nav);
        let params = if f == Stdlib::Back {
            vec![]
        } else {
            a.load_param(0);
            vec![("location", string)]
        };
        a.call(f).store_slot(nav).op(Opcode::Unit, &[]);
        let code = b.code(a);
        b.action(f.name(), &params, &[nav], code);
    }
    let mut empty = Asm::new();
    empty.list(0);
    let empty = b.code(empty);
    let read_params = b.slot("read_params", strings, empty);
    let mut empty = Asm::new();
    empty.str(b.str(""));
    let empty = b.code(empty);
    let read_search = b.slot("read_search", string, empty);
    let encoded = b.slot("encoded", string, empty);
    for (name, f, slot) in [
        ("readParams", Stdlib::Params, read_params),
        ("readSearch", Stdlib::SearchParam, read_search),
        ("encode", Stdlib::EncodeURIComponent, encoded),
    ] {
        let mut a = Asm::new();
        if f != Stdlib::EncodeURIComponent {
            a.load_slot(nav);
        }
        if f == Stdlib::SearchParam {
            a.call(Stdlib::Top);
        }
        a.load_param(0)
            .call(f)
            .store_slot(slot)
            .op(Opcode::Unit, &[]);
        let code = b.code(a);
        b.action(name, &[("name", string)], &[slot], code);
    }
    let mut a = Asm::new();
    a.load_param(0).store_slot(nav).op(Opcode::Unit, &[]);
    let code = b.code(a);
    b.action("set", &[("value", router)], &[nav], code);
    // Repeated refused calls in an expression/commit journal only once.
    let mut a = Asm::new();
    a.load_slot(nav)
        .load_param(0)
        .call(Stdlib::Push)
        .load_param(0)
        .call(Stdlib::Push)
        .store_slot(nav)
        .op(Opcode::Unit, &[]);
    let code = b.code(a);
    b.action("twice", &[("location", string)], &[nav], code);
    for r in &table.routes {
        b.route(
            &r.name,
            &r.pattern,
            r.parent.map(|i| exact_plan::RoutesId(i as u32)),
            r.tab,
            r.notfound,
        );
    }
    b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    b
}

#[allow(dead_code)] // this fixture is also included by the corpus integration test
pub fn plan(table: &exact_route::Table) -> Plan {
    builder(table).finish().unwrap()
}
