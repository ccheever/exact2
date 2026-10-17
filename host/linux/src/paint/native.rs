//! Elements a platform draws: `video` (LLP 1042), `iframe` (LLP 1020), a
//! native module's view (LLP 1024) and a GPU canvas's window (LLP 1009). The kernel owns only their box; the walk
//! names each with its box and what the platform needs to show it, and a
//! backend whose reader hosts platform views draws the view's own drawing
//! there, under the same clips and above what came before it, so it scrolls,
//! clips and stacks as the box does. Other backends draw nothing for them.
use super::*;

/// Which platform element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeKind {
    /// A `video`: a platform player.
    Video = 0,
    /// An `iframe`: a platform web view.
    WebView = 1,
    /// A native module's view, by module name.
    Module = 2,
    /// A GPU canvas's window (LLP 1009): the reader makes one and gives it
    /// to the host, whose module presents into it.
    Surface = 3,
    /// A text field the shell hosts (LLP 1115.001 D8): the platform's own
    /// field, drawn here in the content box of the container this host
    /// paints. What it is and what it holds are not in the stream: the
    /// shell is told of its fields beside it (`presenter/hosted.rs`).
    Field = 4,
    /// A view an app's Kotlin hatch hosts at a node's border box (LLP
    /// 1115.001.000 D4), over the node's own paint: what it is is the
    /// reader's; the props carry how many times the node has been given one.
    Hatch = 5,
}

impl Painter {
    /// @ref LLP 1115.001.000 D4 — a view a Kotlin hatch hosts at `node`:
    /// over its paint, in its border box `rect`, where the node is. A node
    /// that is a platform element already is named by its own kind.
    pub(super) fn hosted_view(&mut self, node: &NodeRef<'_>, rect: Rect4, ts: Transform) {
        let element = matches!(
            node.node_type,
            NodeType::Video
                | NodeType::WebView
                | NodeType::NativeView
                | NodeType::TextInput
                | NodeType::Canvas
        );
        if let Some(generation) = crate::hatches::reader::hosted(node.id).filter(|_| !element) {
            // Never a kept recording: the view comes and goes between paints.
            self.row_refuse();
            let props = format!("{{\"gen\":{generation}}}");
            self.backend
                .native(node.id, NativeKind::Hatch, &Shape::rect(rect), ts, &props);
        }
    }

    /// Name `node`'s platform element to the backend.
    pub(super) fn native(
        &mut self,
        node: &NodeRef<'_>,
        content: Rect4,
        outer: &Shape,
        ts: Transform,
    ) {
        let p = node.props;
        let flag = |id| p.bool(id) == Some(true);
        let (kind, props) = match node.node_type {
            NodeType::Video => (
                NativeKind::Video,
                serde_json::json!({
                    "src": p.str(PropId::Src),
                    "autoplay": flag(PropId::Autoplay),
                    "loop": flag(PropId::Loop),
                    "muted": flag(PropId::Muted),
                    "paused": flag(PropId::Paused),
                    "fit": match node.style.object_fit {
                        ObjectFit::Contain => "contain",
                        ObjectFit::Cover => "cover",
                        ObjectFit::None => "none",
                        ObjectFit::ScaleDown => "scale-down",
                        ObjectFit::Fill => "fill",
                    },
                }),
            ),
            NodeType::WebView => (
                NativeKind::WebView,
                serde_json::json!({ "src": p.str(PropId::Src) }),
            ),
            NodeType::NativeView => (
                NativeKind::Module,
                serde_json::json!({
                    "module": p.str(PropId::NativeViewModuleName),
                    "generation": node.key.generation,
                    "slot": node.key.index,
                    "disabled": flag(PropId::Disabled),
                    "props": p.str(PropId::NativeViewProps),
                }),
            ),
            NodeType::Canvas => (NativeKind::Surface, serde_json::json!({})),
            _ => return,
        };
        if content.2 <= 0.0 || content.3 <= 0.0 {
            return;
        }
        let clip = Shape {
            rect: content,
            radii: outer.radii,
            corners: None,
        };
        self.backend
            .native(node.id, kind, &clip, ts, &props.to_string());
    }
}
