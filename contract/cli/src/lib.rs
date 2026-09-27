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

mod logic;
mod manifest;
mod map;
pub mod native;
mod rust;
mod sources;
mod surface;
mod symbols;
mod typescript;

/// The data seam, re-exported for an app's build script: the bake asks the
/// crate its grants for the compatibility id (`Caltrain.grants()`).
pub use exact_runner::DataSource;
pub use logic::{rust_entry, web_linked, web_rust_mode};
pub use manifest::Manifest;
pub use map::{plan_digest, SourceMap};
pub use rust::rust;
pub use symbols::symbols_json;
pub use typescript::typescript;

use contract_syntax::{Expr, File, Span, Step, TestDecl};
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
        /// The offending plan node, when a measured node caused the refusal.
        /// A development source map resolves it to the authored declaration.
        site: Option<exact_plan::NodesId>,
    },
}

impl std::fmt::Display for BakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BakeError::Runner(e) => write!(f, "{e:?}"),
            BakeError::Lint { id, message, .. } => write!(f, "[{id}] {message}"),
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

/// A related authored location, with its own independently resolved file.
#[derive(Debug, Clone, PartialEq)]
pub struct RelatedLocation {
    /// Original token range and compilation-local file identity.
    pub span: Span,
    /// Resolved source path, absent for standalone source text.
    pub file: Option<PathBuf>,
    /// Why this declaration or binding is relevant.
    pub note: String,
}

/// Any rejection from any pass, with its stable id and span.
#[derive(Debug, Clone, PartialEq)]
pub struct CompileError {
    /// Which pass.
    pub pass: &'static str,
    /// Stable id.
    pub id: String,
    /// What went wrong.
    pub message: String,
    /// Original token range, including its compilation-local source identity.
    pub span: Span,
    /// Resolved file path, absent only when compiling standalone source text.
    pub file: Option<PathBuf>,
    /// Other authored declarations or bindings involved in this rejection.
    pub related: Box<[RelatedLocation]>,
}

impl CompileError {
    /// A machine-readable diagnostic (LLP 1035.005 D2). Columns are one-based
    /// UTF-8 byte offsets with an exclusive end; zero means no source range.
    /// Standalone source has a null file. Every related location owns its file;
    /// no location is guessed from message text.
    pub fn to_json(&self) -> String {
        serde_json::json!({
            "id": self.id,
            "message": self.message,
            "file": self.file.as_ref().map(|file| file.to_string_lossy()),
            "line": self.span.line,
            "col": self.span.col,
            "end_col": self.span.end_col,
            "related": self.related.iter().map(|related| serde_json::json!({
                "file": related.file.as_ref().map(|file| file.to_string_lossy()),
                "line": related.span.line,
                "col": related.span.col,
                "end_col": related.span.end_col,
                "note": related.note,
            })).collect::<Vec<_>>(),
        })
        .to_string()
    }
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(file) = &self.file {
            write!(f, "{}:", file.display())?;
        }
        write!(f, "{} [{}] {}", self.span, self.id, self.message)?;
        for related in self.related.iter() {
            write!(f, "\n  ")?;
            if let Some(file) = &related.file {
                write!(f, "{}:", file.display())?;
            }
            write!(f, "{}: {}", related.span, related.note)?;
        }
        Ok(())
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
                    span: e.span,
                    file: None,
                    related: Box::new([]),
                }
            }
        }
    };
}

from_pass!(contract_syntax::SyntaxError, "syntax");
from_pass!(contract_types::TypeError, "types");
impl From<contract_analyze::AnalyzeError> for CompileError {
    fn from(error: contract_analyze::AnalyzeError) -> Self {
        Self {
            pass: "analyze",
            id: error.id.into(),
            message: error.message,
            span: error.span,
            file: None,
            related: error
                .related
                .into_iter()
                .map(|related| RelatedLocation {
                    span: related.span,
                    file: None,
                    note: related.note,
                })
                .collect(),
        }
    }
}
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
            span: u.span,
            file: None,
            related: Box::new([]),
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
    compile_path_source(path, &read_source(path)?)
}

/// Compile a file with the development map of the nodes it produces.
pub fn compile_path_mapped(path: &Path) -> Result<(Plan, SourceMap), CompileError> {
    compile_path_source_mapped(path, &read_source(path)?)
}

fn read_source(path: &Path) -> Result<String, CompileError> {
    let src = std::fs::read_to_string(path).map_err(|e| CompileError {
        pass: "use",
        id: "contract-use-unreadable".into(),
        message: e.to_string(),
        span: Span::default(),
        file: Some(path.to_path_buf()),
        related: Box::new([]),
    })?;
    Ok(src)
}

/// Compile source bytes with their file path for relative `use` and font
/// resolution. Unlike [`compile_path`], this never re-reads the root file;
/// callers that watch a file can compile the exact snapshot they observed.
pub fn compile_path_source(path: &Path, src: &str) -> Result<Plan, CompileError> {
    compile_path_output(path, src, false)
        .map(|(plan, _)| plan)
        .map_err(first)
}

/// At most this many diagnostics from one compile.
pub const MAX_DIAGNOSTICS: usize = 20;

/// Compile a file by path, as [`compile_path`] does, and report every
/// independent refusal rather than the first: within a pass each
/// declaration, statement, element and attribute is checked whatever its
/// neighbours' fate, a misspelled or mistyped call site comes before what
/// it broke, and a pass runs only when the ones before it succeeded. At
/// most [`MAX_DIAGNOSTICS`]. `mapped` also returns the development map.
pub fn compile_path_all(
    path: &Path,
    mapped: bool,
) -> Result<(Plan, Option<SourceMap>), Vec<CompileError>> {
    let src = read_source(path).map_err(|e| vec![e])?;
    compile_path_output(path, &src, mapped)
}

/// [`compile_path_all`] over the exact observed root source snapshot, as
/// [`compile_path_source`] is over the first refusal: a watcher reports every
/// refusal of the bytes it saw without reading the file again.
pub fn compile_path_source_all(
    path: &Path,
    src: &str,
    mapped: bool,
) -> Result<(Plan, Option<SourceMap>), Vec<CompileError>> {
    compile_path_output(path, src, mapped)
}

fn first(mut all: Vec<CompileError>) -> CompileError {
    all.swap_remove(0)
}

/// Compile the exact observed root source snapshot and retain its source map.
/// Relative imports and fonts resolve as in [`compile_path_source`].
pub fn compile_path_source_mapped(
    path: &Path,
    src: &str,
) -> Result<(Plan, SourceMap), CompileError> {
    compile_path_output(path, src, true)
        .map(|(plan, map)| (plan, map.expect("map requested")))
        .map_err(first)
}

fn compile_path_output(
    path: &Path,
    src: &str,
    mapped: bool,
) -> Result<(Plan, Option<SourceMap>), Vec<CompileError>> {
    let source_root = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let app_root = source_root.canonicalize().map_err(|e| CompileError {
        pass: "use",
        id: "contract-use-unreadable".into(),
        message: format!("{}: {e}", source_root.display()),
        span: Span::default(),
        file: Some(path.to_path_buf()),
        related: Box::new([]),
    })?;
    let (file, sources) = sources::load(path, src, &app_root)?;
    native::check(&file, &app_root).map_err(|all| {
        all.into_iter()
            .map(|e| sources.resolve(e))
            .collect::<Vec<_>>()
    })?;
    if let Some(declared) = surface::arguments(&app_root)? {
        contract_analyze::check_surface_arguments(&file, &declared)
            .map_err(|e| sources.resolve(e.into()))?;
    }
    let (mut plan, sites) = compile_file_output(&file, Some(&app_root), mapped).map_err(|all| {
        all.into_iter()
            .map(|e| sources.resolve(e))
            .collect::<Vec<_>>()
    })?;
    if app_root.join("app.json").is_file() {
        let manifest = Manifest::read(&app_root).map_err(|message| CompileError {
            pass: "app",
            id: "app-manifest".into(),
            message,
            span: Span::default(),
            file: Some(path.to_path_buf()),
            related: Box::new([]),
        })?;
        if !plan.app_id.is_empty() && plan.app_id != manifest.id {
            return Err(vec![CompileError {
                pass: "app",
                id: "app-identity".into(),
                message: format!(
                    "the plan names app {}, but app.json names {}",
                    plan.app_id, manifest.id
                ),
                span: Span::default(),
                file: Some(path.to_path_buf()),
                related: Box::new([]),
            }]);
        }
        plan.app_id = manifest.id;
    }
    Ok((plan, sites.map(|sites| SourceMap::new(sites, sources))))
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
    compile_file_output(&file, asset_root, false)
        .map(|(plan, _)| plan)
        .map_err(first)
}

impl From<CompileError> for Vec<CompileError> {
    fn from(e: CompileError) -> Self {
        vec![e]
    }
}

fn compile_file_output(
    file: &File,
    asset_root: Option<&Path>,
    mapped: bool,
) -> Result<(Plan, Option<contract_lower::Sites>), Vec<CompileError>> {
    // Each pass runs on what the one before it accepted, and reports all of
    // its own refusals.
    fn each<E: Into<CompileError>>(
        all: Vec<E>,
        hint: impl Fn(CompileError) -> CompileError,
    ) -> Vec<CompileError> {
        all.into_iter()
            .take(MAX_DIAGNOSTICS)
            .map(|e| hint(e.into()))
            .collect()
    }
    let hint = |error| symbols::authored_action_hint(file, error);
    // Lowering needs what types and analysis establish; when either refuses,
    // what it would find without them (tags, attribute names, literal values)
    // is reported in the same run.
    let with_lint = |mut all: Vec<CompileError>| {
        all.extend(
            contract_lower::lint(file)
                .into_iter()
                .map(CompileError::from),
        );
        all.truncate(MAX_DIAGNOSTICS);
        all
    };
    contract_analyze::check_routes_root(file, true).map_err(CompileError::from)?;
    let checked = contract_types::check_all(file, mapped, contract_lower::tags::style)
        .map_err(|all| with_lint(each(all, hint)))?;
    let analysis =
        contract_analyze::check_all(&checked).map_err(|all| with_lint(each(all, hint)))?;
    contract_lower::lower_all(&checked, &analysis, asset_root, mapped)
        .map_err(|all| each(all, |e| e))
}

/// Boot the plan once against `data` and write every resource's boot value
/// into the plan as compiled data. The result still validates and its bytes
/// are a pure function of (source, data).
pub fn bake<D: DataSource>(mut plan: Plan, data: D) -> Result<Plan, BakeError> {
    // The manifest names the app. An unnamed stand-in source preserves it;
    // Runner::boot refuses a nonempty source identity that disagrees.
    if plan.app_id.is_empty() {
        plan.app_id = data.app_id().to_string();
    }
    delivery_shape(&plan)?;
    viewport_shape(&plan)?;
    surface::shape(&plan)?;
    let mut runner = Runner::boot(
        plan.clone(),
        data,
        Kernel::with_monospace(),
        exact_runner::Viewport {
            width: LINT_VIEWPORT.0 as f64,
            height: LINT_VIEWPORT.1 as f64,
        },
        "/",
    )?;
    lint(&mut runner)?;
    let mut b = PlanBuilder::from_plan(plan);
    let pending: Vec<String> = runner.pending().into_iter().map(|(n, _)| n).collect();
    for i in 0..runner.plan().resources.len() {
        let name = runner
            .plan()
            .str(runner.plan().resources[i].name)
            .to_string();
        // A resource that observed secrets or external storage is the device's to answer,
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
        // @ref LLP 1048.003 D6 — a source that answers later at build shows
        // its placeholder there; that is not its answer, so a launch asks it.
        // @ref LLP 1054.000.002 D4 — nor is any placeholder, even one shown
        // without a ticket (a source not ready).
        if pending.contains(&name) || runner.resource_is_placeholder(&name) {
            continue;
        }
        if let Some(v) = runner.resource(&name) {
            b.set_resource_initial(ResourcesId(i as u32), v);
            // @ref LLP 1038 D5 — the compiled value is keyed by evaluated arguments.
            b.set_resource_initial_args(
                ResourcesId(i as u32),
                runner.resource_args(&name).expect("settled resource"),
            );
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
                site: None,
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
                    site: None,
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

// @ref LLP 1039 D1 — refuse unknown host fact fields at bake.
fn viewport_shape(plan: &Plan) -> Result<(), BakeError> {
    use exact_runner::viewport::{FIELDS, SOURCE};
    for row in plan.resources.iter() {
        if plan.str(row.source) != SOURCE {
            continue;
        }
        let name = plan.str(row.name);
        let ty = plan.type_(row.ty);
        if ty.kind != exact_plan::TypeKind::Record {
            return Err(BakeError::Lint {
                site: None,
                id: "bake-viewport-field",
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
                    site: None,
                    id: "bake-viewport-field",
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
    for root in &roots {
        let site = runner.site_of(*root).map(|(site, _)| site);
        runner
            .kernel_mut()
            .compute_layout(*root, Offer::definite(w, h))
            .map_err(|e| BakeError::Lint {
                site,
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
            NodeType::ScrollView | NodeType::List => {
                if node.node_type == NodeType::List
                    && !node.props.iter().any(|(id, value)| {
                        id == exact_kernel::PropId::Virtualized
                            && matches!(value, PropValue::Bool(true))
                    })
                {
                    continue;
                }
                let s = node.style;
                if matches!(s.overflow_y, exact_kernel::Overflow::Hidden) {
                    continue;
                }
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
                        site: runner.site_of(node.id).map(|(site, _)| site),
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
                        site: runner.site_of(node.id).map(|(site, _)| site),
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
