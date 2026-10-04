//! `difftest lowering`: translation validation of the real compiler.
//!
//! For each case the Rust compiler's plan supplies the bytecode of every
//! derive, root slot initializer, resource argument and action body. A
//! generated Lean module (`Contract.LowerCheck`) decodes each body, runs it
//! on the VM model (`Contract.Vm`) in the configurations the semantics
//! reaches along the case's script, and compares the result with `eval`
//! (an action: `exec`'s effects); it also compares each body with what
//! `Contract.Lower` compiles from the semantics' syntax, instruction for
//! instruction. A disagreement between the VM run and the semantics is a
//! divergence; a structural difference is reported, not a failure.

use crate::leanrun::{self, LeanCase};
use crate::script::Case;
use crate::{clone_case, compile, prepare, work_dir, Verdict};
use contract::lean::string;
use exact_plan::{Opcode, Operand, Plan, Stdlib};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

/// One case ready for the Lean module.
struct Ready {
    lean: LeanCase,
    plan: String,
}

/// A case, ready for Lean or why not.
type Entry = (Case, Option<Ready>, Option<String>);

/// What one case's check reported.
#[derive(Default)]
struct Report {
    lines: Vec<String>,
    sum: [u64; 7],
    boot_refused: bool,
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// The plan as a `LowerCheck.PlanInfo` term.
fn plan_info(plan: &Plan) -> String {
    let strings: Vec<String> = plan.strings.iter().map(|s| string(s)).collect();
    let types: Vec<String> = plan
        .types
        .iter()
        .map(|t| format!("({}, {})", string(plan.str(t.name)), t.fields.len))
        .collect();
    let slots: Vec<String> = plan
        .slots
        .iter()
        .map(|s| format!("({}, {})", string(plan.str(s.name)), s.owner.is_some()))
        .collect();
    let derives: Vec<String> = plan
        .derives
        .iter()
        .map(|d| string(plan.str(d.name)))
        .collect();
    let resources: Vec<String> = plan
        .resources
        .iter()
        .map(|r| string(plan.str(r.name)))
        .collect();
    let mutations: Vec<String> = plan
        .mutations
        .iter()
        .map(|m| format!("({}, {})", string(plan.str(m.name)), m.slot.0))
        .collect();
    let mut bodies = Vec::new();
    let body = |kind: &str, name: &str, code: &[u8], writes: &[u32]| {
        let w: Vec<String> = writes.iter().map(u32::to_string).collect();
        format!(
            "{{ kind := {kind}, name := {}, bytes := \"{}\", writes := [{}] }}",
            string(name),
            hex(code),
            w.join(", ")
        )
    };
    for d in &plan.derives {
        bodies.push(body(".derive", plan.str(d.name), plan.code(d.body), &[]));
    }
    for (i, s) in plan.slots.iter().enumerate() {
        let id = exact_plan::SlotsId(i as u32);
        // Owned slots initialize in their arm's frames; a late slot after
        // boot settlement; the router and locale are outside the semantics.
        if s.owner.is_some()
            || s.late
            || plan.router == Some(id)
            || plan.locale == Some(id)
            || plan.mutations.iter().any(|m| m.slot == id)
        {
            continue;
        }
        bodies.push(body(".slot", plan.str(s.name), plan.code(s.init), &[]));
    }
    for r in &plan.resources {
        let name = plan.str(r.name);
        if name.contains('#') {
            continue;
        }
        for (k, a) in r.args.iter().enumerate() {
            let code = plan.code(plan.arg(a).expr);
            bodies.push(body(&format!("(.resource {k})"), name, code, &[]));
        }
    }
    for a in &plan.actions {
        let writes: Vec<u32> = a.writes.iter().map(|w| plan.write(w).slot.0).collect();
        bodies.push(body(
            ".action",
            plan.str(a.name),
            plan.code(a.body),
            &writes,
        ));
    }
    format!(
        "{{ strings := #[{}],\n    types := #[{}],\n    slots := [{}],\n    derives := [{}],\n    resources := [{}],\n    mutations := [{}],\n    bodies := [{}] }}",
        strings.join(", "),
        types.join(", "),
        slots.join(", "),
        derives.join(", "),
        resources.join(", "),
        mutations.join(", "),
        bodies.join(",\n      ")
    )
}

/// The roster and the opcode table, as the plan crate has them.
fn tables() -> String {
    let roster: Vec<String> = Stdlib::ALL
        .iter()
        .map(|f| format!("({}, {})", string(f.name()), f.arity()))
        .collect();
    let mut ops = Vec::new();
    for w in 0..=255u8 {
        let Some(op) = Opcode::from_wire(w) else {
            continue;
        };
        let layout: Vec<&str> = op
            .operands()
            .iter()
            .map(|o| match o {
                Operand::U8 => ".u8",
                Operand::U16 => ".u16",
                Operand::U32 => ".u32",
                Operand::F64 => ".f64",
                Operand::Str => ".str",
                Operand::Enum(_) => ".enum",
                Operand::Idx(_) => ".idx",
            })
            .collect();
        ops.push(format!(
            "({}, [{}])",
            string(&format!("{op:?}")),
            layout.join(", ")
        ));
    }
    format!(
        "def roster : Array (String × Nat) := #[{}]\n\ndef rustOpcodes : List (String × List Contract.Vm.Operand) := [{}]\n\n",
        roster.join(", "),
        ops.join(", ")
    )
}

fn module(cases: &[Ready]) -> String {
    let mut s = String::from(
        "import Contract.LowerCheck\nopen Contract\n\nset_option maxRecDepth 100000\nset_option maxHeartbeats 0\n\n",
    );
    s.push_str(&tables());
    for c in cases {
        let n = &c.lean.name;
        s.push_str(&c.lean.program);
        let _ = write!(
            s,
            "\ndef {n}_oracle : Oracle := OracleText.parse! {}\n\ndef {n}_events : List Observe.Event := [{}]\n\ndef {n}_plan : LowerCheck.PlanInfo :=\n  {}\n\n",
            string(&c.lean.oracle),
            c.lean.events.join(", "),
            c.plan
        );
    }
    s.push_str("def main : IO Unit := do\n  for l in LowerCheck.checkOpcodes rustOpcodes do IO.println l\n");
    for (i, c) in cases.iter().enumerate() {
        let _ = writeln!(
            s,
            "  IO.println \"#case {i}\"\n  for l in LowerCheck.check {n} {n}_oracle {n}_events {n}_plan roster do IO.println l",
            n = c.lean.name
        );
    }
    s
}

/// Run one batch in one Lean process: the opcode lines, then each case's.
fn run_batch(cases: &[Ready], dir: &Path, tag: &str) -> Result<(Vec<String>, Vec<Report>), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let file = dir.join(format!("{tag}-{}.lean", std::process::id()));
    std::fs::write(&file, module(cases)).map_err(|e| format!("{}: {e}", file.display()))?;
    let out = Command::new(leanrun::lake())
        .args(["env", "lean", "--run"])
        .arg(&file)
        .current_dir(leanrun::project())
        .output()
        .map_err(|e| format!("lake env lean: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{}: lean failed:\n{}{}",
            file.display(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut head = Vec::new();
    let mut reports: Vec<Report> = Vec::new();
    for line in text.lines() {
        if line.starts_with("#case ") {
            reports.push(Report::default());
            continue;
        }
        let Some(r) = reports.last_mut() else {
            head.push(line.to_string());
            continue;
        };
        if let Some(rest) = line.strip_prefix("sum ") {
            for (k, n) in rest.split(' ').enumerate().take(7) {
                r.sum[k] = n.parse().unwrap_or(0);
            }
        } else if line == "# boot refused" {
            r.boot_refused = true;
        } else {
            r.lines.push(line.to_string());
        }
    }
    if reports.len() != cases.len() {
        return Err(format!(
            "{}: expected {} cases, got {}",
            file.display(),
            cases.len(),
            reports.len()
        ));
    }
    Ok((head, reports))
}

/// A STRUCT, REFUSED or BOTH line's category: the body kind and the message,
/// names and indices left out.
fn category(line: &str) -> String {
    let (tag, rest) = line.split_once(' ').unwrap_or((line, ""));
    let mut words = rest.split(' ');
    let kind = words.next().unwrap_or("");
    let kind = kind.split('-').next().unwrap_or(kind);
    let _name = words.next();
    let msg: Vec<&str> = words.collect();
    let msg = msg.join(" ");
    let msg = match tag {
        // "<what>: vm traps T / semantics fails: E": keep T and E.
        "BOTH" => match line.split_once(": ") {
            Some((_, d)) => d
                .split('`')
                .enumerate()
                .map(|(i, p)| if i % 2 == 1 { "`…`" } else { p })
                .collect(),
            None => msg,
        },
        // "at <i>: rust X, lean Y": keep the two opcodes.
        "STRUCT" => match msg.split_once(": ") {
            Some((_, d)) => d
                .split(", ")
                .map(|part| {
                    let mut w = part.split(' ');
                    let side = w.next().unwrap_or("");
                    let op = w.next().unwrap_or("");
                    format!("{side} {op}")
                })
                .collect::<Vec<_>>()
                .join(", "),
            None => msg,
        },
        _ => msg
            .split('`')
            .enumerate()
            .map(|(i, p)| if i % 2 == 1 { "`…`" } else { p })
            .collect(),
    };
    format!("{tag} {kind}: {msg}")
}

/// Run the cases; print divergences and categories; whether all agreed.
pub fn run(cases: Vec<Case>, batch: usize, tag: &str) -> Result<bool, String> {
    leanrun::build()?;
    let mut prepared: Vec<Entry> = Vec::new();
    for (i, case) in cases.into_iter().enumerate() {
        let index = i % batch.max(1);
        match prepare(&case, index) {
            Ok(p) => {
                let plan = compile(&case).map_err(|e| format!("{}: {e}", case.name))?;
                let ready = Ready {
                    lean: clone_case(&p.lean),
                    plan: plan_info(&plan),
                };
                prepared.push((case, Some(ready), None));
            }
            Err(Verdict::Refused(e)) => prepared.push((case, None, Some(format!("refused: {e}")))),
            Err(Verdict::Emit(e)) => prepared.push((case, None, Some(format!("emit: {e}")))),
            Err(_) => prepared.push((case, None, Some("not prepared".into()))),
        }
    }
    let chunks: Vec<&[Entry]> = prepared.chunks(batch.max(1)).collect();
    let jobs = std::env::var("DIFFTEST_JOBS")
        .ok()
        .and_then(|j| j.parse::<usize>().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).max(1))
        });
    type Batch = Result<(Vec<String>, Vec<Report>), String>;
    let results: Vec<std::sync::Mutex<Option<Batch>>> =
        chunks.iter().map(|_| std::sync::Mutex::new(None)).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..jobs.min(chunks.len()) {
            scope.spawn(|| loop {
                let b = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let Some(chunk) = chunks.get(b) else { break };
                // Each batch's own copies: `Ready` holds no shared state.
                let ready: Vec<Ready> = chunk
                    .iter()
                    .filter_map(|(_, r, _)| r.as_ref())
                    .map(|r| Ready {
                        lean: clone_case(&r.lean),
                        plan: r.plan.clone(),
                    })
                    .collect();
                let r = run_batch(&ready, &work_dir(), &format!("{tag}-{b}"));
                *results[b].lock().expect("result slot") = Some(r);
            });
        }
    });
    let dir = work_dir().join("failures");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut failed = false;
    let mut kept = 0;
    let mut opcodes_reported = false;
    let mut total = [0u64; 7];
    let (mut checked, mut skipped, mut boot_refused) = (0, 0, 0);
    let mut categories: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();
    for (chunk, result) in chunks.iter().zip(results) {
        let (head, reports) = result
            .into_inner()
            .expect("result slot")
            .expect("every batch ran")?;
        if !opcodes_reported {
            for l in &head {
                println!("{l}");
                failed = true;
            }
            opcodes_reported = true;
        }
        let mut reports = reports.into_iter();
        for (case, ready, why) in chunk.iter() {
            if ready.is_none() {
                skipped += 1;
                if let Some(why) = why {
                    if !why.starts_with("refused") {
                        println!("SKIPPED {}: {why}", case.name);
                    }
                }
                continue;
            }
            let r = reports.next().expect("one report per prepared case");
            checked += 1;
            if r.boot_refused {
                boot_refused += 1;
            }
            for (t, n) in total.iter_mut().zip(r.sum) {
                *t += n;
            }
            let mut bad = false;
            for l in &r.lines {
                if l.starts_with("DIVERGE") || l.starts_with("UNDECODED") {
                    println!("{l}  [{}]", case.name);
                    bad = true;
                } else if l.starts_with("STRUCT")
                    || l.starts_with("REFUSED")
                    || l.starts_with("BOTH")
                {
                    let e = categories.entry(category(l)).or_default();
                    e.0 += 1;
                    if e.1.len() < 3 {
                        e.1.push(format!("{} [{}]", l, case.name));
                    }
                }
            }
            if bad {
                failed = true;
                kept += 1;
                let stem = dir.join(format!("{}-lowering-{kept}", std::process::id()));
                let events: Vec<String> = case.events.iter().map(|e| e.label()).collect();
                let _ = std::fs::write(stem.with_extension("contract"), &case.source);
                let _ = std::fs::write(stem.with_extension("events"), events.join("\n"));
                let _ = std::fs::write(stem.with_extension("lean.txt"), r.lines.join("\n"));
                println!("  kept: {}.{{contract,events,lean.txt}}", stem.display());
            }
        }
    }
    for (cat, (n, examples)) in &categories {
        println!("{n:6} {cat}");
        for e in examples {
            println!("         e.g. {e}");
        }
    }
    let [agree, both, diverge, same, differ, refused, undecoded] = total;
    println!(
        "difftest lowering: {checked} cases checked ({skipped} not compiled, {boot_refused} boot refused): runs {agree} agree, {both} both fail, {diverge} diverge; bodies {same} identical to Contract.Lower, {differ} differ, {refused} refused by it, {undecoded} undecoded"
    );
    Ok(!failed)
}
