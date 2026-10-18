//! What a button invokes (`exact_kernel::Invocation`), by the table the
//! Apple hosts' `InvocationTests.swift` reads too: `fixtures/invocation.tsv`.

use exact_kernel::invocation::Action;
use exact_kernel::{Invocation, Kernel, NodeType, Op, PropId, PropValue};

const TABLE: &str = include_str!("fixtures/invocation.tsv");

fn props(field: &str) -> Vec<(PropId, String)> {
    field
        .split(';')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (name, value) = pair.split_once('=').expect("name=value");
            let prop = match name {
                "id" => PropId::Id,
                "popover" => PropId::Popover,
                "semanticTag" => PropId::SemanticTag,
                "commandfor" => PropId::Commandfor,
                "command" => PropId::Command,
                "popovertarget" => PropId::Popovertarget,
                "popovertargetaction" => PropId::Popovertargetaction,
                other => panic!("no prop {other} in the table's vocabulary"),
            };
            (prop, value.to_string())
        })
        .collect()
}

fn create(id: u32, node_type: NodeType, set: &[(PropId, String)], ops: &mut Vec<Op>) {
    ops.push(Op::CreateView { id, node_type });
    for (prop, value) in set {
        ops.push(Op::SetProp {
            id,
            prop: *prop,
            value: PropValue::Str(value.as_str().into()),
        });
    }
}

#[test]
fn a_button_invokes_what_the_shared_table_says() {
    let rows: Vec<Vec<&str>> = TABLE
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| line.split('\t').collect())
        .collect();
    let mut kernel = Kernel::with_monospace();
    let (mut ops, mut children) = (Vec::new(), Vec::new());
    for row in rows
        .iter()
        .filter(|row| row[0] == "element" || row[0] == "paragraph")
    {
        let id = row[1].parse().unwrap();
        if row[0] == "paragraph" {
            create(id, NodeType::Text, &[], &mut ops);
            let runs: Vec<u32> = rows
                .iter()
                .filter(|run| run[0] == "run" && run[2] == row[1])
                .map(|run| run[1].parse().unwrap())
                .collect();
            for run in rows
                .iter()
                .filter(|run| run[0] == "run" && run[2] == row[1])
            {
                let mut set = props(run[3]);
                set.push((PropId::Text, "run".to_string()));
                create(run[1].parse().unwrap(), NodeType::Text, &set, &mut ops);
            }
            ops.push(Op::SetChildren { id, children: runs });
        } else {
            create(id, NodeType::View, &props(row[2]), &mut ops);
        }
        children.push(id);
    }
    let cases: Vec<&Vec<&str>> = rows.iter().filter(|row| row[0] == "case").collect();
    ops.push(Op::CreateView {
        id: 1,
        node_type: NodeType::View,
    });
    ops.push(Op::SetChildren {
        id: 1,
        children: children.clone(),
    });
    ops.push(Op::AttachRoot { id: 1 });
    kernel.apply(0, 1, &ops).unwrap();
    for row in rows.iter().filter(|row| row[0] == "rename") {
        let set = Op::SetProp {
            id: row[1].parse().unwrap(),
            prop: PropId::Id,
            value: PropValue::Str(row[2].into()),
        };
        kernel.apply(0, 1, &[set]).unwrap();
    }
    let mut ops = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        let id = 100 + index as u32;
        let (node_type, mut set) = match case[1] {
            "button" => (NodeType::Pressable, Vec::new()),
            "nativeButton" => (
                NodeType::Control,
                vec![(PropId::Type, "button".to_string())],
            ),
            "view" => (NodeType::View, Vec::new()),
            other => panic!("no source kind {other}"),
        };
        set.extend(props(case[2]));
        create(id, node_type, &set, &mut ops);
        children.push(id);
    }
    ops.push(Op::SetChildren { id: 1, children });
    kernel.apply(0, 1, &ops).unwrap();

    for (index, case) in cases.iter().enumerate() {
        let why = case[5];
        let source = kernel.node(100 + index as u32).unwrap();
        let got = Invocation::of(&kernel, &source);
        let expect = (case[3] != "-").then(|| {
            let (target, action) = case[3].split_once(' ').unwrap();
            let action = match action {
                "show" => Action::Show,
                "hide" => Action::Hide,
                "toggle" => Action::Toggle,
                "showModal" => Action::ShowModal,
                "close" => Action::Close,
                other => panic!("no action {other}"),
            };
            (target.parse::<u32>().unwrap(), action)
        });
        let got_pair = got.map(|i| (kernel.node_by_key(i.target).unwrap().id, i.action));
        assert_eq!(got_pair, expect, "{why}");
        assert_eq!(
            got.is_some_and(|i| i.opens_popover()),
            case[4] == "menu",
            "{why}: a menu"
        );
    }
}
