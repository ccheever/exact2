//! Native paint/source metadata captured before queuing one exact paragraph.
//! This owns no second UTF-8 document, Kernel, view tree or mutable catalog.
use crate::{paint::rgba, text::RunPaint};
use exact_kernel::{
    Kernel, NodeKey, NodeRef, NodeType, PropId, RegionTextRequest, RegionTextSource,
};
use std::{ops::Range, sync::Arc};

const MAX_RUNS: usize = exact_kernel::region::REGION_NODES;
const MAX_LINK_BYTES: usize = 64 * 1024;

pub(super) struct PaintSource {
    pub source: Arc<RegionTextSource>,
    pub dark: bool,
    pub runs: Box<[TextSourceRun]>,
    pub palette: Box<[RunPaint]>,
    colors: Box<[exact_kernel::ColorValue]>,
}
/// A run from the exact accepted source, including its original node generation.
/// Reading this does not route an action to a current or recycled node.
pub struct TextSourceRun {
    /// Original text-leaf generation.
    pub leaf: NodeKey,
    /// UTF-8 byte range in the canonical paragraph source.
    pub bytes: Range<usize>,
    /// UTF-16 selection range in that same source.
    pub utf16: Range<usize>,
    // URL bytes and the original link generation are retained together. Actual
    // routing additionally requires a current publication/consumer context.
    /// Captured link generation and URL, not a later live href lookup.
    pub link: Option<(NodeKey, Box<str>)>,
}
impl PaintSource {
    pub(super) fn palette(&self, dark: bool) -> std::borrow::Cow<'_, [RunPaint]> {
        if dark == self.dark {
            return std::borrow::Cow::Borrowed(&self.palette);
        }
        // Resolve the immutable authored light/dark pair, not current nodes.
        // This visits runs only; no source UTF-16 scan or paragraph work.
        self.palette
            .iter()
            .zip(&self.colors)
            .map(|(run, color)| RunPaint {
                color: rgba(color.resolve(dark)),
                source: run.source,
            })
            .collect::<Vec<_>>()
            .into()
    }
    pub(super) fn matches(&self, request: &RegionTextRequest, dark: bool) -> bool {
        Arc::ptr_eq(&self.source, request.source()) && self.dark == dark
    }
    pub(super) fn capture(
        kernel: &Kernel,
        request: &RegionTextRequest,
        dark: bool,
    ) -> Result<Self, &'static str> {
        let owner = kernel
            .node_by_key(request.stamp().owner())
            .ok_or("removed paragraph")?;
        if owner.paragraph_stamp().as_ref() != Some(request.stamp()) {
            return Err("stale paragraph paint/source stamp");
        }
        if owner.node_type != NodeType::Text {
            return Err("region input control is not a paragraph snapshot");
        }
        let mut capture = Capture {
            kernel,
            dark,
            runs: Vec::new(),
            palette: Vec::new(),
            colors: Vec::new(),
            bytes: 0,
            utf16: 0,
            link_bytes: 0,
            visited: 0,
        };
        capture.walk(&owner)?;
        if capture.bytes != request.source().bytes() {
            return Err("paragraph source extent mismatch");
        }
        Ok(Self {
            source: request.source().clone(),
            dark,
            runs: capture.runs.into_boxed_slice(),
            palette: capture.palette.into_boxed_slice(),
            colors: capture.colors.into_boxed_slice(),
        })
    }
}
struct Capture<'a> {
    kernel: &'a Kernel,
    dark: bool,
    runs: Vec<TextSourceRun>,
    palette: Vec<RunPaint>,
    colors: Vec<exact_kernel::ColorValue>,
    bytes: usize,
    utf16: usize,
    link_bytes: usize,
    visited: usize,
}
impl Capture<'_> {
    fn walk(&mut self, node: &NodeRef<'_>) -> Result<(), &'static str> {
        self.visited += 1;
        if self.visited > MAX_RUNS {
            return Err("paragraph source node limit");
        }
        if let Some(text) = node.props.str(PropId::Text) {
            let bytes = self.bytes + text.len();
            // One source-revision scan, never one UTF-8 copy or per-offer scan.
            let utf16 = self.utf16 + text.encode_utf16().count();
            let mut link = None;
            let mut cursor = Some(node.id);
            let mut depth = 0;
            while let Some(id) = cursor {
                depth += 1;
                if depth > MAX_RUNS {
                    return Err("paragraph link ancestry limit");
                }
                let n = self.kernel.node(id).ok_or("detached paragraph link")?;
                if let Some(href) = n.props.str(PropId::Href) {
                    self.link_bytes = self
                        .link_bytes
                        .checked_add(href.len())
                        .ok_or("link metadata overflow")?;
                    if self.link_bytes > MAX_LINK_BYTES {
                        return Err("paragraph link bytes limit");
                    }
                    link = Some((n.key, href.into()));
                    break;
                }
                cursor = n.parent;
            }
            self.runs.push(TextSourceRun {
                leaf: node.key,
                bytes: self.bytes..bytes,
                utf16: self.utf16..utf16,
                link,
            });
            self.palette.push(RunPaint {
                color: if crate::paint::paints(self.kernel, node.id, None) {
                    rgba(node.text_color().resolve(self.dark))
                } else {
                    [0, 0, 0, 0]
                },
                source: node.id,
            });
            self.colors.push(node.text_color());
            self.bytes = bytes;
            self.utf16 = utf16;
        } else {
            for id in node.children() {
                if let Some(child) = self
                    .kernel
                    .node(id)
                    .filter(|n| n.node_type == NodeType::Text)
                {
                    self.walk(&child)?;
                }
            }
        }
        Ok(())
    }
}
