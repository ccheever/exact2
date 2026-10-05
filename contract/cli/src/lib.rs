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

/// The Lean backend: a program as a term of `semantics/` (LLP-free; see
/// `semantics/README.md`).
pub mod lean;
mod logic;
mod manifest;
mod map;
pub mod native;
pub mod picker;
mod resolve;
mod rust;
mod sources;
mod strings;
mod surface;
mod symbols;
mod typescript;

pub use contract_types::strings::Strings;
/// The data seam, re-exported for an app's build script: the bake asks the
/// crate its grants for the compatibility id (`Caltrain.grants()`).
pub use exact_runner::DataSource;
pub use logic::{rust_entry, web_linked, web_rust_mode};
pub use manifest::Manifest;
pub use map::{plan_digest, SourceMap};
pub use resolve::Origin;
pub use rust::rust;
pub use sources::{Package, Source, SourceGraph};
pub use symbols::symbols_json;
pub use typescript::typescript;

/// The app's strings tables (LLP 1060 D1), `None` for an app without a
/// `strings` directory, every refusal in one message: the bake reads grant
/// purposes from them (LLP 1069.008 D1).
pub fn strings_tables(app_root: &Path) -> Result<Option<std::sync::Arc<Strings>>, String> {
    strings::load(app_root, &app_root.join("app.contract")).map_err(|all| {
        all.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })
}

use contract_syntax::{Expr, File, Span, Step, TapForm, TestDecl};
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
    pub file: Option<Box<Path>>,
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
    let mut file = contract_syntax::parse(src)?;
    contract_syntax::resolve_clock_timelines(&mut file)?;
    if let Some(u) = file.uses.first() {
        return Err(CompileError {
            pass: "use",
            id: "contract-use-unresolved".into(),
            message: format!(
                "`use {} from \"{}\"` needs this file's own path to resolve: compile it with `contract build <file>` (`compile_path`)",
                u.names.iter().map(|n| n.name.as_str()).collect::<Vec<_>>().join(", "),
                u.path
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
        file: Some(path.into()),
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

/// Every source compiling `path` reads, the root first, with where each came
/// from, and the loader's refusals if it stopped (LLP 1091 D10): what a
/// capture copies, a watcher watches, and a deploy freezes.
pub fn source_graph(path: &Path) -> SourceGraph {
    let refused = |message: String| SourceGraph {
        sources: Vec::new(),
        packages: Vec::new(),
        consulted: Vec::new(),
        errors: vec![CompileError {
            pass: "use",
            id: "contract-use-unreadable".into(),
            message,
            span: Span::default(),
            file: Some(path.into()),
            related: Box::new([]),
        }],
    };
    let src = match std::fs::read_to_string(path) {
        Ok(src) => src,
        Err(e) => return refused(format!("{}: {e}", path.display())),
    };
    let source_root = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    match source_root.canonicalize() {
        Ok(app_root) => sources::graph(path, &src, &app_root),
        Err(e) => refused(format!("{}: {e}", source_root.display())),
    }
}

/// A file [`fix_uses`] wrote, and the `use` lines it wrote there.
pub type WrittenUses = (PathBuf, Vec<String>);

/// Write the `use` lines each file of the program rooted at `path` lacks
/// (LLP 1091 D1), as `contract-use-missing` names them: each file written,
/// with its lines, and the refusals no line answers (a name two files
/// declare, a generated name), which stay the author's.
pub fn fix_uses(path: &Path) -> Result<(Vec<WrittenUses>, Vec<CompileError>), Vec<CompileError>> {
    let source_root = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let unreadable = |p: &Path, e: std::io::Error| CompileError {
        pass: "use",
        id: "contract-use-unreadable".into(),
        message: format!("{}: {e}", p.display()),
        span: Span::default(),
        file: Some(p.into()),
        related: Box::new([]),
    };
    let app_root = source_root
        .canonicalize()
        .map_err(|e| vec![unreadable(source_root, e)])?;
    let mut written: Vec<WrittenUses> = Vec::new();
    // A written line can only bring names, never hide one, so a second
    // pass finds nothing new; the third is a bound, not a loop.
    for _ in 0..3 {
        let src = read_source(path).map_err(|e| vec![e])?;
        let fixes = sources::use_fixes(path, &src, &app_root)?;
        let mut wrote = false;
        let mut refused = Vec::new();
        for fix in fixes {
            if !fix.unresolved.is_empty() || fix.lines.is_empty() {
                refused.push(fix.error.clone());
            }
            if fix.lines.is_empty() {
                continue;
            }
            let before =
                std::fs::read_to_string(&fix.path).map_err(|e| vec![unreadable(&fix.path, e)])?;
            let after = sources::apply_uses(&before, &fix.lines);
            if after != before {
                std::fs::write(&fix.path, &after).map_err(|e| vec![unreadable(&fix.path, e)])?;
                written.push((fix.path, fix.lines.into_iter().map(|l| l.text).collect()));
                wrote = true;
            }
        }
        if !wrote {
            return Ok((written, refused));
        }
    }
    Ok((written, Vec::new()))
}

/// For a build script: `cargo:rerun-if-changed` for every source compiling
/// `path` reads, and each package's `package.json` (LLP 1091 D10), so an
/// edit to a used file or a library rebuilds the plan, not only an edit to
/// the root.
pub fn rerun_if_changed(path: &Path) {
    println!("cargo:rerun-if-changed={}", path.display());
    let graph = source_graph(path);
    for source in &graph.sources {
        if source.path.is_absolute() {
            println!("cargo:rerun-if-changed={}", source.path.display());
        }
    }
    for manifest in graph.consulted {
        println!("cargo:rerun-if-changed={}", manifest.display());
    }
}

/// Canvas surface arguments checked against a game's emitted declaration
/// (`.shells/surfaces.json`, written by its last GPU build), apart from the
/// compile: the author's commands report them as warnings while the game's
/// Rust is newer than the declaration, and `contract types`/`rust` never
/// stop on them (the platformer's diary, R4). A bake checks them as errors,
/// against the declaration its GPU build has just written.
pub struct SurfaceFindings {
    /// Every call the declaration refuses.
    pub findings: Vec<CompileError>,
    /// The game source newer than the declaration, when one is.
    pub newer: Option<PathBuf>,
}

/// [`SurfaceFindings`] for the file at `path`; Err when its sources or the
/// declaration cannot be read.
pub fn surface_findings(path: &Path) -> Result<SurfaceFindings, Vec<CompileError>> {
    let src = read_source(path).map_err(|e| vec![e])?;
    let app_root = app_root(path)?;
    let (file, sources) = sources::load(path, &src, &app_root)?;
    let findings = match surface::arguments(&app_root).map_err(|e| vec![e])? {
        Some(declared) => contract_analyze::check_surface_arguments(&file, &declared)
            .err()
            .unwrap_or_default()
            .into_iter()
            .map(|e| sources.resolve(e.into()))
            .collect(),
        None => Vec::new(),
    };
    Ok(SurfaceFindings {
        findings,
        newer: surface::newer_rust(&app_root),
    })
}

/// [`compile_path_all`] without the surface-argument check, which the
/// author's commands make apart ([`surface_findings`]).
pub fn compile_path_all_unchecked(
    path: &Path,
    mapped: bool,
) -> Result<(Plan, Option<SourceMap>), Vec<CompileError>> {
    let src = read_source(path).map_err(|e| vec![e])?;
    compile_path_checked(path, &src, mapped, false)
}

fn app_root(path: &Path) -> Result<PathBuf, Vec<CompileError>> {
    let source_root = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    source_root.canonicalize().map_err(|e| {
        vec![CompileError {
            pass: "use",
            id: "contract-use-unreadable".into(),
            message: format!("{}: {e}", source_root.display()),
            span: Span::default(),
            file: Some(path.into()),
            related: Box::new([]),
        }]
    })
}

fn compile_path_output(
    path: &Path,
    src: &str,
    mapped: bool,
) -> Result<(Plan, Option<SourceMap>), Vec<CompileError>> {
    compile_path_checked(path, src, mapped, true)
}

fn compile_path_checked(
    path: &Path,
    src: &str,
    mapped: bool,
    surfaces: bool,
) -> Result<(Plan, Option<SourceMap>), Vec<CompileError>> {
    let app_root = app_root(path)?;
    let (file, sources) = sources::load(path, src, &app_root)?;
    native::check(&file, &app_root).map_err(|all| {
        all.into_iter()
            .map(|e| sources.resolve(e))
            .collect::<Vec<_>>()
    })?;
    picker::check(&file, Some(&app_root)).map_err(|all| {
        all.into_iter()
            .map(|e| sources.resolve(e))
            .collect::<Vec<_>>()
    })?;
    // Surface findings join the compile's own refusals; neither hides the other.
    let surface: Vec<CompileError> = match surfaces.then(|| surface::arguments(&app_root)) {
        Some(declared) => match declared.map_err(|e| vec![e])? {
            Some(declared) => contract_analyze::check_surface_arguments(&file, &declared)
                .err()
                .unwrap_or_default()
                .into_iter()
                .map(|e| sources.resolve(e.into()))
                .collect(),
            None => Vec::new(),
        },
        None => Vec::new(),
    };
    let joined = |mut all: Vec<CompileError>| {
        all.extend(surface.iter().cloned());
        all.truncate(MAX_DIAGNOSTICS);
        all
    };
    let strings = strings::load(&app_root, path).map_err(joined)?;
    let (mut plan, sites) =
        compile_file_output(&file, Some(&app_root), strings, mapped).map_err(|all| {
            joined(
                all.into_iter()
                    .map(|e| sources.resolve(e))
                    .collect::<Vec<_>>(),
            )
        })?;
    if !surface.is_empty() {
        return Err(surface);
    }
    if app_root.join("app.json").is_file() {
        let manifest = Manifest::read(&app_root).map_err(|message| CompileError {
            pass: "app",
            id: "app-manifest".into(),
            message,
            span: Span::default(),
            file: Some(path.into()),
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
                file: Some(path.into()),
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
/// The file's top-level launch lines lead each test's steps, unless the test
/// names the same fact itself (habits F7, calendar F13).
pub fn tests(src: &str) -> Result<Vec<TestDecl>, CompileError> {
    let file = contract_syntax::parse(src)?;
    let word = |s: &Step| std::mem::discriminant(s);
    Ok(file
        .tests
        .into_iter()
        .map(|mut test| {
            let own: Vec<_> = test.steps.iter().map(word).collect();
            let inherited = file.launch.iter().filter(|l| !own.contains(&word(l)));
            test.steps = inherited.cloned().chain(test.steps).collect();
            test
        })
        .collect())
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
            let line = step.span().line;
            match step {
                Step::Tap {
                    target,
                    form,
                    modifiers,
                    ..
                } => {
                    s.push_str("{\"op\":\"tap\",\"target\":");
                    q(target, &mut s);
                    s.push_str(",\"form\":");
                    match form {
                        TapForm::Press => s.push_str("\"press\""),
                        TapForm::Hover => s.push_str("\"hover\""),
                        TapForm::Dblclick => s.push_str("\"dblclick\""),
                        TapForm::Contextmenu => s.push_str("\"contextmenu\""),
                        TapForm::Into(key) => {
                            s.push_str("\"into\",\"key\":");
                            q(key, &mut s);
                        }
                        TapForm::Pinch { scale, at } => {
                            s.push_str(&format!("\"pinch\",\"scale\":{scale}"));
                            if let Some((x, y)) = at {
                                s.push_str(&format!(",\"at\":[{x},{y}]"));
                            }
                        }
                    }
                    if !modifiers.is_empty() {
                        s.push_str(",\"modifiers\":");
                        q(modifiers, &mut s);
                    }
                }
                Step::Drag {
                    target,
                    dx,
                    dy,
                    from,
                    mouse,
                    press,
                    over,
                    hold,
                    during,
                    ..
                } => {
                    s.push_str("{\"op\":\"drag\",\"target\":");
                    q(target, &mut s);
                    s.push_str(&format!(",\"dx\":{dx},\"dy\":{dy}"));
                    if let Some((x, y)) = from {
                        s.push_str(&format!(",\"from\":[{x},{y}]"));
                    }
                    if *mouse {
                        s.push_str(",\"mouse\":true");
                    }
                    for (name, ms) in [("press", press), ("over", over), ("hold", hold)] {
                        if let Some(ms) = ms {
                            s.push_str(&format!(",\"{name}\":{ms}"));
                        }
                    }
                    if !during.is_empty() {
                        s.push_str(",\"during\":[");
                        for (i, op) in during.iter().enumerate() {
                            if i > 0 {
                                s.push(',');
                            }
                            q(op, &mut s);
                        }
                        s.push(']');
                    }
                }
                Step::Size { width, height, .. } => {
                    s.push_str(&format!(
                        "{{\"op\":\"size\",\"width\":{width},\"height\":{height}"
                    ));
                }
                Step::Epoch { value, .. } => {
                    s.push_str("{\"op\":\"epoch\",\"value\":");
                    q(value, &mut s);
                }
                Step::TimeZone { zone, .. } => {
                    s.push_str("{\"op\":\"time-zone\",\"value\":");
                    q(zone, &mut s);
                }
                Step::Locale { tag, .. } => {
                    s.push_str("{\"op\":\"locale\",\"value\":");
                    q(tag, &mut s);
                }
                Step::Seed { seed, .. } => {
                    s.push_str(&format!("{{\"op\":\"seed\",\"value\":{seed}"));
                }
                Step::Type {
                    target,
                    text,
                    append,
                    ..
                } => {
                    s.push_str("{\"op\":\"type\",\"target\":");
                    q(target, &mut s);
                    s.push_str(",\"text\":");
                    q(text, &mut s);
                    s.push_str(&format!(",\"append\":{append}"));
                }
                Step::Reload { .. } => s.push_str("{\"op\":\"reload\""),
                Step::Close { .. } => s.push_str("{\"op\":\"close\""),
                Step::BeforeData { .. } => s.push_str("{\"op\":\"before-data\""),
                Step::Resize { width, height, .. } => {
                    s.push_str(&format!(
                        "{{\"op\":\"resize\",\"width\":{width},\"height\":{height}"
                    ));
                }
                Step::Key {
                    target,
                    key,
                    phase,
                    duration,
                    ..
                } => {
                    s.push_str("{\"op\":\"key\",\"target\":");
                    q(target, &mut s);
                    s.push_str(",\"key\":");
                    q(key, &mut s);
                    if let Some(phase) = phase {
                        s.push_str(",\"phase\":");
                        q(phase, &mut s);
                    }
                    if let Some(ms) = duration {
                        s.push_str(&format!(",\"for\":{ms}"));
                    }
                }
                Step::Pick { target, paths, .. } => {
                    s.push_str("{\"op\":\"pick\",\"target\":");
                    q(target, &mut s);
                    s.push_str(",\"paths\":[");
                    for (i, path) in paths.iter().enumerate() {
                        if i > 0 {
                            s.push(',');
                        }
                        q(path, &mut s);
                    }
                    s.push(']');
                }
                Step::Clipboard {
                    target, edit, text, ..
                } => {
                    s.push_str("{\"op\":\"clipboard\",\"target\":");
                    q(target, &mut s);
                    s.push_str(",\"edit\":");
                    q(edit, &mut s);
                    s.push_str(",\"text\":");
                    q(text, &mut s);
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
                        Expr::List(items, _) if items.is_empty() => s.push_str("[]"),
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
    // Media alone: a text has no app, so no `file_handlers` (LLP 1069.002 D1).
    picker::check(&file, None).map_err(first)?;
    compile_file_output(&file, asset_root, None, false)
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
    strings: Option<std::sync::Arc<contract_types::strings::Strings>>,
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
    let checked = contract_types::check_all(file, mapped, contract_lower::tags::style, strings)
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
    let runner = first_frame(&plan, data, true)?;
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
        // @ref LLP 1069.005 D2 — nor is an answer that drew randomness: its
        // value would be one draw every install shares.
        if pending.contains(&name)
            || runner.resource_is_placeholder(&name)
            || runner.resource_draws_entropy(&name)
        {
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

/// The bake's refusals for a build that does not bake — the web's JS target
/// (LLP 1071), whose page asks its data module after it boots: the same
/// shape checks and the same layout lint, at the same point, so the web
/// loop fails where a native bake would (files diary F13). The frame linted
/// is the one that page shows first — every app source not yet answering,
/// each resource at its placeholder. What only an answer shows is the
/// native bake's alone, and a boot that needs an answer to finish (an
/// `else source()` row) is not refused here: the page's own boot reports it.
pub fn check(plan: &Plan) -> Result<(), BakeError> {
    struct Unanswered;
    impl DataSource for Unanswered {
        fn ready(&self) -> bool {
            false
        }
        fn query(
            &mut self,
            source: &str,
            _: &[exact_plan::Value],
        ) -> Result<exact_plan::Value, exact_runner::DataError> {
            Err(exact_runner::DataError::Unavailable(format!(
                "{source} answers in the page, not at build"
            )))
        }
    }
    // Only the unanswered source is excused: a trap, a shape, a derive's
    // type or anything else the boot refuses fails here as a bake fails
    // (review C2).
    match first_frame(plan, Unanswered, false) {
        Err(BakeError::Runner(RunnerError::Data {
            error: exact_runner::DataError::Unavailable(_),
            ..
        }))
        | Ok(_) => Ok(()),
        Err(refused) => Err(refused),
    }
}

/// The checks every build runs before it uses a plan, ending at the first
/// frame laid out and linted: the runner, booted on `data`, for the bake.
/// `answered` is false for [`check`]'s frame, whose placeholders say
/// nothing about the content a layout verdict may rest on.
fn first_frame<D: DataSource>(
    plan: &Plan,
    data: D,
    answered: bool,
) -> Result<Runner<D>, BakeError> {
    use exact_runner::{delivery, page, viewport};
    fact_shape(
        plan,
        delivery::SOURCE,
        &delivery::FIELDS,
        "bake-delivery-field",
    )?;
    fact_shape(
        plan,
        viewport::SOURCE,
        viewport::FIELDS,
        "bake-viewport-field",
    )?;
    fact_shape(plan, page::SOURCE, page::FIELDS, "bake-page-field")?;
    surface::shape(plan)?;
    let mut runner = Runner::boot(
        plan.clone(),
        data,
        Kernel::with_monospace(),
        exact_runner::Viewport::sized(LINT_VIEWPORT.0 as f64, LINT_VIEWPORT.1 as f64),
        "/",
    )?;
    lint(&mut runner, answered)?;
    Ok(runner)
}

/// The runner's own sources (LLP 1030 D7 delivery; LLP 1039 D1 viewport;
/// LLP 1069.000 D2 page) are answered by the runner, not by the data crate,
/// so the fields each can fill are a closed set — a declared field it does
/// not know would refuse every boot on every device, which is a build-time
/// refusal here instead, naming the field.
fn fact_shape(
    plan: &Plan,
    source: &str,
    fields: &[&str],
    id: &'static str,
) -> Result<(), BakeError> {
    for row in plan.resources.iter() {
        if plan.str(row.source) != source {
            continue;
        }
        let name = plan.str(row.name);
        let ty = plan.type_(row.ty);
        if ty.kind != exact_plan::TypeKind::Record {
            return Err(BakeError::Lint {
                site: None,
                id,
                message: format!(
                    "`resource {name} = {source}()` must be `as shape` a record of {}",
                    fields.join(", ")
                ),
            });
        }
        for f in ty.fields.iter() {
            let field = plan.str(plan.field(f).name);
            if !fields.contains(&field) {
                return Err(BakeError::Lint {
                    site: None,
                    id,
                    message: format!(
                        "`{name}` declares `{field}`, which {source} does not answer; it answers {}",
                        fields.join(", ")
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
///
/// Unanswered (`answered` false, [`check`]), a verdict that could rest on a
/// placeholder is not given: an empty list's `scroll`, or a button whose
/// label is a resource's empty zero. What stays is a pressable whose zero
/// area is its own style's — `display: none` on it or an ancestor, a zero
/// `width` or `height` — which no answer changes (files diary F13: a hidden
/// shortcut button).
fn lint<D: DataSource>(runner: &mut Runner<D>, answered: bool) -> Result<(), BakeError> {
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
            NodeType::ScrollView | NodeType::List if answered => {
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
                if !answered && !styled_out(kernel, node) {
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

/// Whether a node's zero area is its style's: `display: none` on it or an
/// ancestor, or a zero `width` or `height` of its own.
fn styled_out(kernel: &Kernel, node: exact_kernel::NodeRef<'_>) -> bool {
    let zero = |d: Dimension| matches!(d, Dimension::Points(p) if p == 0.0);
    if zero(node.style.width) || zero(node.style.height) {
        return true;
    }
    let mut at = Some(node);
    while let Some(n) = at {
        if matches!(n.style.display, exact_kernel::Display::None) {
            return true;
        }
        at = n.parent.and_then(|p| kernel.node(p));
    }
    false
}
