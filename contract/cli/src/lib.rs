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

use exact_kernel::{Dimension, Kernel, NodeType, Offer, PropValue};
use exact_plan::builder::PlanBuilder;
use exact_plan::{Plan, ResourcesId};
use exact_runner::{DataSource, Runner, RunnerError};

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

/// Compile one source text to a validated plan.
pub fn compile(src: &str) -> Result<Plan, CompileError> {
    let file = contract_syntax::parse(src)?;
    let types = contract_types::check(&file)?;
    let analysis = contract_analyze::check(&file, &types)?;
    Ok(contract_lower::lower(&file, &types, &analysis)?)
}

/// Boot the plan once against `data` and write every resource's boot value
/// into the plan as compiled data. The result still validates and its bytes
/// are a pure function of (source, data).
pub fn bake<D: DataSource>(plan: Plan, data: D) -> Result<Plan, BakeError> {
    let mut runner = Runner::boot(plan.clone(), data, Kernel::with_monospace())?;
    lint(&mut runner)?;
    let mut b = PlanBuilder::from_plan(plan);
    for i in 0..runner.plan().resources.len() {
        let name = runner
            .plan()
            .str(runner.plan().resources[i].name)
            .to_string();
        // A resource that consulted the store is the device's to answer at
        // boot, not the build's: no compiled value (LLP 1018 D4). The bake's
        // store is empty by construction — a developer's session never
        // reaches a plan.
        if runner.resource_reads_store(&name) {
            continue;
        }
        if let Some(v) = runner.resource(&name) {
            b.set_resource_initial(ResourcesId(i as u32), v);
        }
    }
    b.finish()
        .map_err(|e| BakeError::Runner(RunnerError::Plan(e)))
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
