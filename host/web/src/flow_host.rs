//! @ref LLP 1043.000 §3 D1–D4, D7 — DOM geometry resolves in wasm, not Taffy.
use super::*;
use exact_kernel::{Dimension, Display, PositionType, WrapFlow};

impl<D: DataSource> Host<D> {
    pub(super) fn track_exclusion(&mut self, id: ViewId) {
        let node = self.runner.kernel().node(id).expect("live");
        if node.style.position_type == PositionType::Absolute
            && node.style.wrap_flow == WrapFlow::Both
        {
            self.exclusions.insert(id);
        } else {
            self.exclusions.remove(&id);
        }
    }

    // Only exclusion contexts are scanned, O(sum(context subtrees) + output).
    // Ordinary apps never allocate or walk a flow subtree on their commit path.
    pub(super) fn emit_textflow(&mut self, batch: &mut Batch) {
        if self.exclusions.is_empty() && self.textflow.is_empty() {
            return;
        }
        let kernel = self.runner.kernel();
        let mut contexts: exact_kernel::SortedMap<u32, Vec<u32>> = Default::default();
        for id in &self.exclusions {
            let node = kernel.node(*id).expect("tracked live exclusion");
            if let Some(parent) = node.parent {
                contexts.get_or_insert_with(parent, Vec::new).push(*id);
            }
        }
        let mut json = String::from("[");
        for (i, (context, exclusions)) in contexts.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            let _ = write!(
                json,
                "{{\"id\":{context},\"exclusions\":{exclusions:?},\"paragraphs\":["
            );
            let mut stack = kernel.node(*context).unwrap().children();
            stack.reverse();
            let mut first = true;
            while let Some(id) = stack.pop() {
                let node = kernel.node(id).expect("live child");
                if self.exclusions.contains(&id) || node.style.display == Display::None {
                    continue;
                }
                if node.node_type == NodeType::Text && !node.is_inline_run() {
                    if !first {
                        json.push(',');
                    }
                    first = false;
                    let definite = node.style.height != Dimension::Auto;
                    let _ = write!(json, "{{\"id\":{id},\"definite\":{definite}}}");
                } else {
                    stack.extend(node.children().into_iter().rev());
                }
            }
            json.push_str("]}");
        }
        json.push(']');
        if json != self.textflow {
            batch.textflow(&json);
            self.textflow = if self.exclusions.is_empty() {
                String::new()
            } else {
                json
            };
        }
    }
}
