//! Known reader sources are compiled with the shared Markdown parser.

use super::{Em, HOST_FACTS};
use crate::style;
use exact_kernel::PropId;
use exact_plan::{BindingKind, NodesId, Opcode, Plan, Value};
use exact_runner::vm::instructions;
use std::fmt::Write as _;

fn resource_path(plan: &Plan, code: &[u8], depth: u8) -> Option<Vec<usize>> {
    if depth == 32 {
        return None;
    }
    let mut ins = instructions(code);
    let first = ins.next()?.ok()?;
    let mut path = match first.op {
        Opcode::LoadResource => vec![first.args[0] as usize],
        Opcode::LoadDerive => resource_path(
            plan,
            plan.code(plan.derives.get(first.args[0] as usize)?.body),
            depth + 1,
        )?,
        _ => return None,
    };
    while let Some(i) = ins.next() {
        let i = i.ok()?;
        match i.op {
            Opcode::Field => path.push(i.args[0] as usize),
            Opcode::Return if ins.next().is_none() => return Some(path),
            _ => return None,
        }
    }
    None
}

fn initial(plan: &Plan, path: &[usize]) -> Option<String> {
    let resource = plan.resources.get(path[0])?;
    if resource.reader || HOST_FACTS.contains(&plan.str(resource.source)) {
        return None;
    }
    let value = Value::from_bytes(plan.bytes(resource.initial)).ok()?;
    let mut current = &value;
    for &field in &path[1..] {
        let Value::Record(fields) = current else {
            return None;
        };
        current = fields.get(field)?;
    }
    current.as_str().map(str::to_owned)
}

impl Em<'_> {
    pub(super) fn markdown(&mut self, node: u32, e: &str, f: &str, code: &[u8]) {
        // Atom faces and their platform views remain runtime bindings.
        if self.plan.node(NodesId(node)).bindings.iter().any(|b| {
            let b = self.plan.binding(b);
            b.kind == BindingKind::Prop && b.id == PropId::Atoms as u16
        }) {
            let md = self.uses.rt("md");
            let _ = write!(self.out, "{md}({e},{f},false);");
            return;
        }
        let path = resource_path(self.plan, code, 0);
        let source = style::literal(self.plan, code)
            .and_then(|v| v.as_str().map(str::to_owned))
            .or_else(|| path.as_ref().and_then(|p| initial(self.plan, p)));
        if let Some(path) = path {
            self.uses.markdown_paths.insert(path);
        }
        let prepared = source.map(|source| {
            let pieces = exact_web_capabilities::markdown::pieces(&source, "", "");
            self.uses
                .share('m', serde_json::to_string(&(source, pieces)).unwrap())
        });
        let md = self.uses.rt("md");
        let prepared = prepared.map(|v| format!(",{v}")).unwrap_or_default();
        let _ = write!(self.out, "{md}({e},{f}{prepared});");
    }
}
