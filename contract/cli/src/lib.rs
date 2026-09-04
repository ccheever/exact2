//! The compiler as one call.
//!
//! @ref LLP 1004 D2 (the driver) / D4 (constant resources are compiled data)
//! / D6 (the corpus)
//!
//! `compile` runs the four passes and returns a validated plan whose bytes
//! are a pure function of the source. `bake` then boots the runner once
//! against the app's data source and writes every resource's boot value into
//! the plan, so the first frame on a device needs no host and no seam. The
//! compiler is, for one frame, a host.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod compat;

pub use compat::{compatibility_id, Compat, Manifest};
/// The data seam, re-exported for an app's build script: the bake asks the
/// crate its grants for the compatibility id (`Caltrain.grants()`).
pub use exact_runner::DataSource;

use contract_syntax::{Expr, File, Step, TestDecl, UseDecl};
use exact_kernel::{Dimension, Kernel, NodeType, Offer, PropValue};
use exact_plan::builder::PlanBuilder;
use exact_plan::{Plan, ResourcesId};
use exact_runner::{Runner, RunnerError};
use std::path::{Path, PathBuf};

/// A refusal from `bake`: the runner's, or the layout lint's (LLP 1017 P1d).
#[derive(Debug)]
pub enum BakeError {
    /// The runner refused to boot or to settle.
    Runner(RunnerError),
    /// The first frame, laid out at [`LINT_VIEWPORT`], shows a layout that
    /// cannot be what the author meant.
    Lint {
        /// Stable id: `bake-scroll-unbounded`, `bake-zero-size`, `bake-layout`.
        id: &'static str,
        /// What and where — the node by its `testId` when it has one.
        message: String,
    },
}

impl std::fmt::Display for BakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BakeError::Runner(e) => write!(f, "{e:?}"),
            BakeError::Lint { id, message } => write!(f, "[{id}] {message}"),
        }
    }
}

impl std::error::Error for BakeError {}

impl From<RunnerError> for BakeError {
    fn from(e: RunnerError) -> Self {
        BakeError::Runner(e)
    }
}

/// The viewport the lint lays the first frame out at: a phone, in points.
pub const LINT_VIEWPORT: (f32, f32) = (390.0, 844.0);

/// Any rejection from any pass, with its stable id and span.
#[derive(Debug, Clone, PartialEq)]
pub struct CompileError {
    /// Which pass.
    pub pass: &'static str,
    /// Stable id.
    pub id: String,
    /// What went wrong.
    pub message: String,
    /// Line, column.
    pub span: (u32, u32),
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{} [{}] {}",
            self.span.0, self.span.1, self.id, self.message
        )
    }
}

impl std::error::Error for CompileError {}

macro_rules! from_pass {
    ($ty:path, $pass:expr) => {
        impl From<$ty> for CompileError {
            fn from(e: $ty) -> Self {
                CompileError {
                    pass: $pass,
                    id: e.id.to_string(),
                    message: e.message,
                    span: (e.span.line, e.span.col),
                }
            }
        }
    };
}

from_pass!(contract_syntax::SyntaxError, "syntax");
from_pass!(contract_types::TypeError, "types");
from_pass!(contract_analyze::AnalyzeError, "analyze");
from_pass!(contract_lower::LowerError, "lower");

/// Compile one source text to a validated plan. A text has no path, so a
/// `use … from "./file.contract"` in it cannot be resolved: compile a file
/// that uses others with [`compile_path`].
pub fn compile(src: &str) -> Result<Plan, CompileError> {
    let file = contract_syntax::parse(src)?;
    if let Some(u) = file.uses.first() {
        return Err(CompileError {
            pass: "use",
            id: "contract-use-unresolved".into(),
            message: format!(
                "`use {} from \"{}\"` needs this file's own path to resolve: compile it with `contract build <file>` (`compile_path`)",
                u.name, u.path
            ),
            span: (u.span.line, u.span.col),
        });
    }
    compile_file(file, None)
}

/// Compile a file by path, resolving every `use … from "./other.contract"`
/// (LLP 1017 P8): the used file is loaded the same way, transitively, and
/// all of its declarations — shapes, styles, components — are merged into
/// the using file after its own, so the using file's first component stays
/// the root and a used component is a child. The named declaration must
/// exist in the used file; a name declared differently in both is refused;
/// a cycle is refused.
pub fn compile_path(path: &Path) -> Result<Plan, CompileError> {
    let src = std::fs::read_to_string(path).map_err(|e| CompileError {
        pass: "use",
        id: "contract-use-unreadable".into(),
        message: format!("{}: {e}", path.display()),
        span: (0, 0),
    })?;
    compile_path_source(path, &src)
}

/// Compile source bytes with their file path for relative `use` and font
/// resolution. Unlike [`compile_path`], this never re-reads the root file;
/// callers that watch a file can compile the exact snapshot they observed.
pub fn compile_path_source(path: &Path, src: &str) -> Result<Plan, CompileError> {
    let source_root = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let app_root = source_root.canonicalize().map_err(|e| CompileError {
        pass: "use",
        id: "contract-use-unreadable".into(),
        message: format!("{}: {e}", source_root.display()),
        span: (0, 0),
    })?;
    let root_key = path.canonicalize().unwrap_or_else(|_| {
        path.file_name()
            .map(|name| app_root.join(name))
            .unwrap_or_else(|| app_root.clone())
    });
    let mut seen = vec![root_key];
    let file = load_source(path, src, &app_root, &mut seen)?;
    compile_file(file, Some(&app_root))
}

/// The `test` blocks of a file (LLP 1017 P7) — normally `app.test.contract`
/// beside the app, holding nothing else. Parsed, never compiled: a test is a
/// script for the agent driver (`scripts/agent.mjs --test`), and its steps
/// are the eight operations plus `expect` lines that read their replies.
pub fn tests(src: &str) -> Result<Vec<TestDecl>, CompileError> {
    let file = contract_syntax::parse(src)?;
    Ok(file.tests)
}

/// The tests as JSON for the driver: `[{"name":…,"steps":[{"op":…}]}]`,
/// written by hand — no serde anywhere in the runtime (LLP 1012).
pub fn tests_json(tests: &[TestDecl]) -> String {
    fn q(s: &str, out: &mut String) {
        out.push('"');
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
    }
    let mut s = String::from("[");
    for (ti, t) in tests.iter().enumerate() {
        if ti > 0 {
            s.push(',');
        }
        s.push_str("{\"name\":");
        q(&t.name, &mut s);
        s.push_str(",\"steps\":[");
        for (si, step) in t.steps.iter().enumerate() {
            if si > 0 {
                s.push(',');
            }
            let line = match step {
                Step::Tap { span, .. }
                | Step::Type { span, .. }
                | Step::Key { span, .. }
                | Step::Clock { span, .. }
                | Step::Screenshot { span, .. }
                | Step::ExpectTree { span, .. }
                | Step::ExpectText { span, .. }
                | Step::ExpectState { span, .. } => span.line,
            };
            match step {
                Step::Tap { target, hover, .. } => {
                    s.push_str("{\"op\":\"tap\",\"target\":");
                    q(target, &mut s);
                    s.push_str(&format!(",\"hover\":{hover}"));
                }
                Step::Type { target, text, .. } => {
                    s.push_str("{\"op\":\"type\",\"target\":");
                    q(target, &mut s);
                    s.push_str(",\"text\":");
                    q(text, &mut s);
                }
                Step::Key { target, key, .. } => {
                    s.push_str("{\"op\":\"key\",\"target\":");
                    q(target, &mut s);
                    s.push_str(",\"key\":");
                    q(key, &mut s);
                }
                Step::Clock { arg, .. } => {
                    s.push_str("{\"op\":\"clock\",\"arg\":");
                    q(arg, &mut s);
                }
                Step::Screenshot { path, .. } => {
                    s.push_str("{\"op\":\"screenshot\",\"path\":");
                    q(path, &mut s);
                }
                Step::ExpectTree {
                    target, present, ..
                } => {
                    s.push_str("{\"op\":\"expect-tree\",\"target\":");
                    q(target, &mut s);
                    s.push_str(&format!(",\"present\":{present}"));
                }
                Step::ExpectText { target, value, .. } => {
                    s.push_str("{\"op\":\"expect-text\",\"target\":");
                    q(target, &mut s);
                    s.push_str(",\"value\":");
                    q(value, &mut s);
                }
                Step::ExpectState { name, value, .. } => {
                    s.push_str("{\"op\":\"expect-state\",\"name\":");
                    q(name, &mut s);
                    s.push_str(",\"value\":");
                    match value {
                        Expr::Number(n, _) => s.push_str(&format!("{n}")),
                        Expr::Str(t, _) => q(t, &mut s),
                        Expr::Bool(b, _) => s.push_str(&format!("{b}")),
                        _ => s.push_str("null"),
                    }
                }
            }
            s.push_str(&format!(",\"line\":{line}}}"));
        }
        s.push_str("]}");
    }
    s.push(']');
    s
}

fn compile_file(file: File, asset_root: Option<&Path>) -> Result<Plan, CompileError> {
    let types = contract_types::check(&file)?;
    let analysis = contract_analyze::check(&file, &types)?;
    Ok(contract_lower::lower(&file, &types, &analysis, asset_root)?)
}

fn use_error(id: &str, message: String, u: &UseDecl) -> CompileError {
    CompileError {
        pass: "use",
        id: id.into(),
        message,
        span: (u.span.line, u.span.col),
    }
}

fn load_source(
    path: &Path,
    src: &str,
    app_root: &Path,
    seen: &mut Vec<PathBuf>,
) -> Result<File, CompileError> {
    let mut file = contract_syntax::parse(src)?;
    let uses = std::mem::take(&mut file.uses);
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    for u in &uses {
        validate_use_path(u)?;
        let target = dir.join(&u.path);
        let key = target.canonicalize().map_err(|e| {
            use_error(
                "contract-use-unreadable",
                format!(
                    "`use {} from \"{}\"`: {}: {e}",
                    u.name,
                    u.path,
                    target.display()
                ),
                u,
            )
        })?;
        if !key.starts_with(app_root) {
            return Err(use_error(
                "contract-use-path",
                format!(
                    "`use {} from \"{}\"` leaves the app directory",
                    u.name, u.path
                ),
                u,
            ));
        }
        if key.extension().and_then(|extension| extension.to_str()) != Some("contract") {
            return Err(use_error(
                "contract-use-path",
                format!(
                    "`use {} from \"{}\"` resolves to a file that is not `.contract`",
                    u.name, u.path
                ),
                u,
            ));
        }
        if seen.contains(&key) {
            return Err(use_error(
                "contract-use-cycle",
                format!(
                    "`use {} from \"{}\"` returns to a file already being loaded",
                    u.name, u.path
                ),
                u,
            ));
        }
        let used_src = std::fs::read_to_string(&key).map_err(|e| {
            use_error(
                "contract-use-unreadable",
                format!(
                    "`use {} from \"{}\"`: {}: {e}",
                    u.name,
                    u.path,
                    key.display()
                ),
                u,
            )
        })?;
        seen.push(key.clone());
        let used = load_source(&key, &used_src, app_root, seen)?;
        seen.pop();
        let known = used.components.iter().any(|c| c.name == u.name)
            || used.shapes.iter().any(|s| s.name == u.name)
            || used.styles.iter().any(|s| s.name == u.name)
            || used.fns.iter().any(|f| f.name == u.name);
        if !known {
            return Err(use_error(
                "contract-use-unknown",
                format!(
                    "`{}` declares no component, shape, or style `{}`",
                    u.path, u.name
                ),
                u,
            ));
        }
        merge(&mut file, used, u)?;
    }
    Ok(file)
}

fn validate_use_path(u: &UseDecl) -> Result<(), CompileError> {
    let Some(relative) = u.path.strip_prefix("./") else {
        return Err(use_error(
            "contract-use-path",
            format!(
                "`use {} from \"{}\"` needs a portable path beginning `./`",
                u.name, u.path
            ),
            u,
        ));
    };
    if relative.is_empty()
        || u.path.contains('\\')
        || relative
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(use_error(
            "contract-use-path",
            format!(
                "`use {} from \"{}\"` must stay below its file with no `..` segments",
                u.name, u.path
            ),
            u,
        ));
    }
    Ok(())
}

fn merge(into: &mut File, from: File, u: &UseDecl) -> Result<(), CompileError> {
    let dup = |what: &str, name: &str| {
        use_error(
            "contract-use-duplicate",
            format!("`use {}` brings a {what} `{name}` that this file already has, declared differently", u.name),
            u,
        )
    };
    for f in from.fonts {
        match into.fonts.iter().find(|x| x.name == f.name) {
            Some(x) if *x == f => {}
            Some(_) => return Err(dup("font", &f.name)),
            None => into.fonts.push(f),
        }
    }
    for s in from.shapes {
        match into.shapes.iter().find(|x| x.name == s.name) {
            Some(x) if *x == s => {}
            Some(_) => return Err(dup("shape", &s.name)),
            None => into.shapes.push(s),
        }
    }
    for s in from.styles {
        match into.styles.iter().find(|x| x.name == s.name) {
            Some(x) if *x == s => {}
            Some(_) => return Err(dup("style", &s.name)),
            None => into.styles.push(s),
        }
    }
    for f in from.fns {
        match into.fns.iter().find(|x| x.name == f.name) {
            Some(x) if *x == f => {}
            Some(_) => return Err(dup("fn", &f.name)),
            None => into.fns.push(f),
        }
    }
    for c in from.components {
        match into.components.iter().find(|x| x.name == c.name) {
            Some(x) if *x == c => {}
            Some(_) => return Err(dup("component", &c.name)),
            None => into.components.push(c),
        }
    }
    Ok(())
}

/// Boot the plan once against `data` and write every resource's boot value
/// into the plan as compiled data. The result still validates and its bytes
/// are a pure function of (source, data).
pub fn bake<D: DataSource>(mut plan: Plan, data: D) -> Result<Plan, BakeError> {
    // The identity (LLP 1023 D5): the data crate's one declaration, written
    // into the header here so a served plan says whose it is; boot's gate
    // matches it against the booting binary's own crate.
    plan.app_id = data.app_id().to_string();
    delivery_shape(&plan)?;
    let mut runner = Runner::boot(plan.clone(), data, Kernel::with_monospace())?;
    lint(&mut runner)?;
    let mut b = PlanBuilder::from_plan(plan);
    for i in 0..runner.plan().resources.len() {
        let name = runner
            .plan()
            .str(runner.plan().resources[i].name)
            .to_string();
        // A resource that consulted the store is the device's to answer,
        // not the build's (LLP 1018 D4): the bake's store is empty by
        // construction, so what is compiled for it is the empty-store answer
        // — the fresh install's first frame, never a developer's session —
        // and the row says so (`reader`, LLP 1027 D4 as ruled 2026-09-03),
        // so the runner treats that value as a placeholder: a kept answer
        // from the device beats it, and a data source not ready at boot is
        // asked again at `data_ready`.
        if runner.resource_reads_store(&name) {
            b.set_resource_reader(ResourcesId(i as u32), true);
        }
        if let Some(v) = runner.resource(&name) {
            b.set_resource_initial(ResourcesId(i as u32), v);
        }
    }
    b.finish()
        .map_err(|e| BakeError::Runner(RunnerError::Plan(e)))
}

/// The delivery shape (LLP 1030 D7): `exactDelivery` is answered by the
/// runner, not by the data crate, so the fields it can fill are a closed
/// set — a declared field it does not know would refuse every boot on every
/// device, which is a build-time refusal here instead, naming the field.
fn delivery_shape(plan: &Plan) -> Result<(), BakeError> {
    use exact_runner::delivery::{FIELDS, SOURCE};
    for row in plan.resources.iter() {
        if plan.str(row.source) != SOURCE {
            continue;
        }
        let name = plan.str(row.name);
        let ty = plan.type_(row.ty);
        if ty.kind != exact_plan::TypeKind::Record {
            return Err(BakeError::Lint {
                id: "bake-delivery-field",
                message: format!(
                    "`resource {name} = {SOURCE}()` must be `as shape` a record of {}",
                    FIELDS.join(", ")
                ),
            });
        }
        for f in ty.fields.iter() {
            let field = plan.str(plan.field(f).name);
            if !FIELDS.contains(&field) {
                return Err(BakeError::Lint {
                    id: "bake-delivery-field",
                    message: format!(
                        "`{name}` declares `{field}`, which {SOURCE} does not answer; it answers {}",
                        FIELDS.join(", ")
                    ),
                });
            }
        }
    }
    Ok(())
}

/// The layout lint (LLP 1017 P1d): the compiler cannot see layout, bake can.
/// The first frame is laid out at [`LINT_VIEWPORT`] on the monospace
/// measurer, and two things the diaries lost hours to are refused with the
/// node's name: a `scroll` that is exactly as tall as its children with
/// nothing bounding it (it grows, and never scrolls — 0102, 0103), and a
/// pressable with zero area (nothing can press it — valet 0003). A pressable
/// holding an image or a canvas is exempt: their size is the host's.
fn lint<D: DataSource>(runner: &mut Runner<D>) -> Result<(), BakeError> {
    let (w, h) = LINT_VIEWPORT;
    let roots = runner.roots();
    let kernel = runner.kernel_mut();
    for root in &roots {
        kernel
            .compute_layout(*root, Offer::definite(w, h))
            .map_err(|e| BakeError::Lint {
                id: "bake-layout",
                message: format!("the first frame does not lay out: {e:?}"),
            })?;
    }
    let kernel = runner.kernel();
    for row in kernel.rows(None).unwrap_or_default() {
        let Some(node) = kernel.node(row.id) else {
            continue;
        };
        let test_id = node.props.iter().find_map(|(id, v)| match v {
            PropValue::Str(s) if id.name() == "testId" => Some(s.clone()),
            _ => None,
        });
        let at = match &test_id {
            Some(t) => format!("`{}` testId=\"{t}\"", node.node_type.name()),
            None => format!("`{}` #{}", node.node_type.name(), node.id),
        };
        match node.node_type {
            NodeType::ScrollView => {
                let s = node.style;
                let unbounded = matches!(s.height, Dimension::Auto)
                    && matches!(s.max_height, Dimension::Auto)
                    && s.flex_grow == 0.0;
                if !unbounded {
                    continue;
                }
                // The children's extent below the node's top, in its own space.
                let extent = node
                    .children()
                    .iter()
                    .filter_map(|c| kernel.node(*c))
                    .map(|c| c.frame.y + c.frame.height - node.frame.y)
                    .fold(0.0f32, f32::max);
                let bottom_padding = match s.padding_bottom {
                    Dimension::Points(p) => p,
                    _ => 0.0,
                };
                if extent > 0.0 && (node.frame.height - (extent + bottom_padding)).abs() < 0.5 {
                    return Err(BakeError::Lint {
                        id: "bake-scroll-unbounded",
                        message: format!(
                            "{at} is exactly as tall as its children ({:.0} pt) at {w:.0}×{h:.0} and nothing bounds it, so it grows with its content and never scrolls — give it a `height`, `max-height`, or `flex`",
                            node.frame.height
                        ),
                    });
                }
            }
            NodeType::Pressable => {
                if node.frame.width > 0.0 && node.frame.height > 0.0 {
                    continue;
                }
                let mut stack = node.children();
                let mut replaced = false;
                while let Some(id) = stack.pop() {
                    if let Some(c) = kernel.node(id) {
                        if matches!(c.node_type, NodeType::Image | NodeType::Canvas) {
                            replaced = true;
                            break;
                        }
                        stack.extend(c.children());
                    }
                }
                if !replaced {
                    return Err(BakeError::Lint {
                        id: "bake-zero-size",
                        message: format!(
                            "{at} has zero area ({:.0}×{:.0}) at {w:.0}×{h:.0}, so nothing can press it — give it children or a size",
                            node.frame.width, node.frame.height
                        ),
                    });
                }
            }
            _ => {}
        }
    }
    Ok(())
}
