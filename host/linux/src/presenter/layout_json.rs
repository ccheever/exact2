//! The agent's `layout` reply (LLP 1012 §1), as JSON.
use super::*;

impl<D: DataSource> Presenter<D> {
    /// The agent's `layout`: every node's box in the viewport (scroll
    /// folded in), scroll containers with their offsets, by id. With a
    /// `node`, the runner's explanation of that node (LLP 1035.002 D1) plus
    /// what a painter knows — its painted box and a 1:1 capture; no window,
    /// no screen, nothing mounted, and the reply says so rather than
    /// guessing.
    pub fn layout_json(&mut self, node: Option<u32>, include_plan: bool) -> String {
        let clock = self.host.now();
        let (vw, vh) = self.viewport;
        let mut boxes: Vec<PaintedBox> = self.boxes().to_vec();
        boxes.sort_by_key(|b| b.id);
        let mut s = String::new();
        // LLP 1012 §1: the insets a device's shell told (`window.rs`; zeros
        // on a host with none, which has no safe area or software keyboard);
        // the fold is whatever the agent preferred (LLP 1078 D7).
        let fold = self.host.runner().viewport().fold;
        let [top, right, bottom, left, keyboard] = self.env_insets().map(|v| num(r2(v)));
        let _ = write!(
            s,
            "{{\"clock\":{},\"viewport\":{{\"w\":{},\"h\":{}}},\"env\":{{\"safe-area-inset-top\":{top},\"safe-area-inset-right\":{right},\"safe-area-inset-bottom\":{bottom},\"safe-area-inset-left\":{left},\"keyboard-inset-height\":{keyboard},\"device-posture\":\"{}\",\"horizontal-viewport-segments\":{},\"vertical-viewport-segments\":{},\"viewport-segments\":[",
            num(clock),
            num(r2(vw)),
            num(r2(vh)),
            fold.posture.keyword(),
            fold.cols,
            fold.rows
        );
        for (i, r) in self.segments.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let _ = write!(
                s,
                "[{},{},{},{}]",
                num(r2(r.x)),
                num(r2(r.y)),
                num(r2(r.width)),
                num(r2(r.height))
            );
        }
        s.push_str("]},\"nodes\":[");
        let limits = self.collection_scroll_limits();
        for (i, b) in boxes.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let _ = write!(
                s,
                "{{\"id\":{},\"x\":{},\"y\":{},\"w\":{},\"h\":{}",
                b.id,
                num(r2(b.rect.0)),
                num(r2(b.rect.1)),
                num(r2(b.rect.2)),
                num(r2(b.rect.3))
            );
            if let Some((sx, sy)) = b.scroll {
                let (rx, ry) = self.scroll_range(b.id, &limits);
                let _ = write!(
                    s,
                    ",\"sx\":{},\"sy\":{},\"rx\":{},\"ry\":{}",
                    num(r2(sx)),
                    num(r2(sy)),
                    num(r2(rx)),
                    num(r2(ry))
                );
            }
            s.push('}');
        }
        s.push(']');
        if let Some(id) = node {
            let detail = self.host.agent(&format!(
                "{{\"op\":\"node\",\"id\":{id},\"plan\":{include_plan}}}"
            ));
            if detail.starts_with("{\"error\"") {
                return detail;
            }
            // Reopen the runner's object: exactly its last brace, never the
            // inner object's before it.
            let mut detail = detail;
            if detail.ends_with('}') {
                detail.pop();
            }
            // @ref LLP 1038 D6; LLP 1035.002 D1 — hidden rows remain in
            // the kernel, but have no painted box and refuse input.
            let (route_hidden, inert) = self.host.route_visibility(id);
            let hidden = route_hidden || self.placement_hidden(id);
            let _ = write!(
                detail,
                ",\"visible\":{{\"hidden\":{hidden},\"inert\":{inert}}}"
            );
            match boxes.iter().find(|b| b.id == id) {
                Some(b) => {
                    let _ = write!(
                        detail,
                        ",\"space\":{{\"viewport\":{{\"x\":{},\"y\":{},\"w\":{},\"h\":{}}},\"capture\":{{\"scale\":1}}}}",
                        num(r2(b.rect.0)),
                        num(r2(b.rect.1)),
                        num(r2(b.rect.2)),
                        num(r2(b.rect.3))
                    );
                }
                None if self.placement_hidden(id) => detail.push_str(",\"space\":{\"viewport\":{\"x\":0,\"y\":0,\"w\":0,\"h\":0},\"capture\":{\"scale\":1}}"),
                None => detail.push_str(",\"space\":{\"capture\":{\"scale\":1}}"),
            }
            // @ref LLP 1043.000 §3 D7 — ordinary paragraphs keep the existing
            // reply shape, while a flowed empty paragraph still reports its facts.
            if let Some(p) = self.paragraph(id).filter(|p| p.flow_line_height() > 0.0) {
                detail.push_str(",\"fragments\":[");
                for (i, f) in p.fragments().iter().enumerate() {
                    if i > 0 {
                        detail.push(',');
                    }
                    let _ = write!(detail, "{{\"start\":{},\"end\":{},\"x\":{},\"y\":{},\"width\":{},\"band\":{},\"height\":{}}}", f.start, f.end, num(f.x as f64), num(f.y as f64), num(f.width as f64), f.line, num(p.flow_line_height() as f64));
                }
                detail.push(']');
            }
            detail.push_str(",\"native\":{\"unavailable\":true");
            if let Some(source) = self
                .host
                .runner()
                .kernel()
                .node(id)
                .filter(|n| n.node_type == NodeType::Image)
                .and_then(|n| n.props.str(PropId::ImageSource))
                .filter(|s| s.starts_with("symbol:"))
            {
                let name = &source[7..];
                let reason = if name == "sf/" || name.is_empty() {
                    "empty"
                } else if !name.starts_with("sf/") && exact_kernel::symbol_path(name).is_none() {
                    "role"
                } else {
                    "platform"
                };
                detail.push_str(",\"symbol\":{\"found\":false,\"source\":");
                quote(source, &mut detail);
                detail.push_str(",\"name\":");
                quote(name.strip_prefix("sf/").unwrap_or(name), &mut detail);
                let _ = write!(detail, ",\"reason\":\"{reason}\"}}");
            }
            let _ = write!(detail, "}},\"observed\":{{\"clock\":{}}}}}", num(clock));
            s.push_str(",\"node\":");
            s.push_str(&detail);
        }
        s.push('}');
        s
    }
}
