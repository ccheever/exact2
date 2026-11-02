//! Inline archives preserve one paragraph and an optional image, not box layout.
use super::{refusal, CompileError};
use contract_syntax::Span;
use exact_kernel::{Display, Kernel, NodeType, PropId};
use exact_plan::Plan;
use exact_runner::{DataError, DataSource, Runner, Value, Viewport};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

pub(super) fn check(plan: &Plan, families: &[String], span: Span) -> Result<(), Vec<CompileError>> {
    if !families.iter().any(|f| f == "accessoryInline") {
        return Ok(());
    }
    // iOS's nominal gallery content, the watch's text slot, and its intrinsic
    // glyph proposal. Runtime admission also checks actual props and sizes.
    for (width, height) in [(225.0, 26.0), (168.0, 11.5), (13.5, 13.5)] {
        let fail = |why: String| {
            vec![refusal("snapshot-inline", format!(
            "widget {} accessoryInline at {width} × {height}: {why}; emit exactly one `text` or `time` leaf plus at most one optional image, using `snapshot_viewport` to keep other families' layout", plan.snapshot_entry), span)]
        };
        let runner = Runner::boot(
            plan.clone(),
            NoData,
            Kernel::with_monospace(),
            Viewport::sized(width, height),
            "/",
        )
        .map_err(|e| fail(format!("preview evaluation failed: {e:?}")))?;
        let kernel = runner.kernel();
        let mut stack = kernel.roots();
        let (mut text, mut images, mut other) = (0, 0, 0);
        while let Some(id) = stack.pop() {
            let Some(node) = kernel.node(id) else {
                continue;
            };
            if node.style.display == Display::None {
                continue;
            }
            match node.node_type {
                NodeType::Text => text += 1,
                NodeType::Image => images += 1,
                NodeType::View | NodeType::Pressable
                    if node.props.str(PropId::Type) != Some("progress")
                        && matches!(
                            node.props.str(PropId::SemanticTag).unwrap_or(""),
                            "" | "main" | "section" | "header" | "footer" | "article" | "aside"
                        ) =>
                {
                    stack.extend(node.children())
                }
                _ => other += 1,
            }
        }
        if text != 1 || images > 1 || other != 0 {
            return Err(fail(format!(
                "preview has {text} text/time leaves, {images} images and {other} other leaves"
            )));
        }
    }
    Ok(())
}
