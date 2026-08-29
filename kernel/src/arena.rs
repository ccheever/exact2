//! The columnar node arena.
//!
//! Every node is a slot; every attribute is a column indexed by slot. Topology
//! is index-based (`parents`, `children`), identity is generation-checked
//! ([`NodeKey`]), and destroyed slots go on a free list. Nothing here hashes on
//! the read path except the wire-id lookup a producer's op needs once.
//!
//! The arena is the authored truth. Frames and Taffy handles are derived
//! columns: rehydration is columns-plus-rebuild, never serialized engine state.

use std::collections::HashMap;

use taffy::NodeId;

use crate::error::ApplyError;
use crate::generated::{NodeType, PropId, StyleProps};
use crate::id::{Frame, NodeFlags, NodeKey, ViewId};
use crate::props::PropList;
use crate::text::{TextRun, TextStyle};

/// Columnar node storage.
#[derive(Debug, Default, Clone)]
pub struct NodeArena {
    generations: Vec<u32>,
    live: Vec<bool>,
    node_types: Vec<NodeType>,
    local_ids: Vec<ViewId>,
    parents: Vec<Option<u32>>,
    children: Vec<Vec<u32>>,
    styles: Vec<StyleProps>,
    props: Vec<PropList>,
    flags: Vec<NodeFlags>,
    frames: Vec<Frame>,
    /// Scrollable overflow from the last layout: the content's extent in the
    /// node's own space (width, height), Taffy's `content_size`.
    contents: Vec<(f32, f32)>,
    /// A replaced element's intrinsic size (an image's natural pixels in
    /// points), reported by the host once it has loaded; `None` until then.
    intrinsic: Vec<Option<(f32, f32)>>,
    taffy: Vec<Option<NodeId>>,
    is_root: Vec<bool>,
    free: Vec<u32>,
    roots: Vec<u32>,
    by_local: HashMap<ViewId, u32>,
    live_count: usize,
}

impl NodeArena {
    /// An empty arena.
    pub fn new() -> Self {
        Self::default()
    }

    /// Live nodes.
    pub fn live_count(&self) -> usize {
        self.live_count
    }

    /// Slots ever allocated (live plus free).
    pub fn slot_count(&self) -> usize {
        self.generations.len()
    }

    /// Whether `slot` holds a live node.
    pub fn is_live(&self, slot: u32) -> bool {
        (slot as usize) < self.live.len() && self.live[slot as usize]
    }

    /// The generation-checked key for a live slot.
    pub fn key(&self, slot: u32) -> NodeKey {
        NodeKey {
            index: slot,
            generation: self.generations[slot as usize],
        }
    }

    /// The slot a key names, if that allocation is still live.
    pub fn resolve(&self, key: NodeKey) -> Option<u32> {
        if self.is_live(key.index) && self.generations[key.index as usize] == key.generation {
            Some(key.index)
        } else {
            None
        }
    }

    /// The slot holding wire id `id`.
    pub fn slot_of(&self, id: ViewId) -> Option<u32> {
        self.by_local.get(&id).copied()
    }

    /// The key for wire id `id`.
    pub fn key_of(&self, id: ViewId) -> Option<NodeKey> {
        self.slot_of(id).map(|s| self.key(s))
    }

    /// Wire id of a slot.
    pub fn local_id(&self, slot: u32) -> ViewId {
        self.local_ids[slot as usize]
    }

    /// Node type of a slot.
    pub fn node_type(&self, slot: u32) -> NodeType {
        self.node_types[slot as usize]
    }

    /// Parent slot.
    pub fn parent(&self, slot: u32) -> Option<u32> {
        self.parents[slot as usize]
    }

    /// Ordered child slots.
    pub fn children(&self, slot: u32) -> &[u32] {
        &self.children[slot as usize]
    }

    /// Style rows.
    pub fn style(&self, slot: u32) -> &StyleProps {
        &self.styles[slot as usize]
    }

    /// Props.
    pub fn props(&self, slot: u32) -> &PropList {
        &self.props[slot as usize]
    }

    /// Dirty flags.
    pub fn flags(&self, slot: u32) -> NodeFlags {
        self.flags[slot as usize]
    }

    /// Absolute frame from the last layout.
    pub fn frame(&self, slot: u32) -> Frame {
        self.frames[slot as usize]
    }

    /// Layout-engine handle.
    pub fn taffy(&self, slot: u32) -> Option<NodeId> {
        self.taffy[slot as usize]
    }

    /// Whether the slot is a registered root.
    pub fn is_root(&self, slot: u32) -> bool {
        self.is_root[slot as usize]
    }

    /// Root slots in attach order.
    pub fn roots(&self) -> &[u32] {
        &self.roots
    }

    /// Every live slot, ascending.
    pub fn iter_live(&self) -> impl Iterator<Item = u32> + '_ {
        (0..self.live.len() as u32).filter(move |s| self.live[*s as usize])
    }

    /// Whether `ancestor` is a proper ancestor of `node`.
    pub fn is_ancestor(&self, ancestor: u32, node: u32) -> bool {
        let mut cur = self.parents[node as usize];
        while let Some(p) = cur {
            if p == ancestor {
                return true;
            }
            cur = self.parents[p as usize];
        }
        false
    }

    /// Depth below the root (a root is 0).
    pub fn depth(&self, slot: u32) -> u32 {
        let mut depth = 0;
        let mut cur = self.parents[slot as usize];
        while let Some(p) = cur {
            depth += 1;
            cur = self.parents[p as usize];
        }
        depth
    }

    /// The subtree rooted at `slot`, preorder, `slot` first.
    pub fn subtree(&self, slot: u32) -> Vec<u32> {
        let mut out = Vec::new();
        let mut stack = vec![slot];
        while let Some(s) = stack.pop() {
            out.push(s);
            stack.extend(self.children[s as usize].iter().rev());
        }
        out
    }

    /// The nearest ancestor-or-self that is measured as one text leaf: a `Text`
    /// under a `Text` is an inline run of its parent, so measurement is owned
    /// by the topmost consecutive `Text` ancestor.
    pub fn measure_owner(&self, slot: u32) -> u32 {
        let mut owner = slot;
        while let Some(p) = self.parents[owner as usize] {
            if self.node_types[p as usize] == NodeType::Text {
                owner = p;
            } else {
                break;
            }
        }
        owner
    }

    /// Whether this slot is an inline run (a `Text` whose parent is a `Text`).
    pub fn is_inline_run(&self, slot: u32) -> bool {
        self.node_types[slot as usize] == NodeType::Text
            && self.parents[slot as usize]
                .is_some_and(|p| self.node_types[p as usize] == NodeType::Text)
    }

    /// Append the text runs of the leaf rooted at `slot`, in order.
    pub fn text_runs<'a>(&'a self, slot: u32, out: &mut Vec<TextRun<'a>>) {
        let s = slot as usize;
        let style = TextStyle::from_style(&self.styles[s]);
        match self.node_types[s] {
            NodeType::TextInput => {
                // An input has a line box even when empty (the web's
                // `<input>`): its value, else its placeholder, else one space.
                let props = &self.props[s];
                let text = props
                    .str(PropId::Value)
                    .filter(|v| !v.is_empty())
                    .or_else(|| props.str(PropId::Placeholder).filter(|p| !p.is_empty()))
                    .unwrap_or(" ");
                out.push(TextRun { text, style });
            }
            _ => {
                if let Some(text) = self.props[s].str(PropId::Text) {
                    out.push(TextRun { text, style });
                } else {
                    for child in &self.children[s] {
                        if self.node_types[*child as usize] == NodeType::Text {
                            self.text_runs(*child, out);
                        }
                    }
                }
            }
        }
    }

    // ---- mutation (crate-private: only the transaction engine writes) ------

    pub(crate) fn alloc(&mut self, id: ViewId, node_type: NodeType) -> Result<u32, ApplyError> {
        debug_assert!(!self.by_local.contains_key(&id), "alloc of a live wire id");
        let slot = match self.free.pop() {
            Some(slot) => slot,
            None => {
                if self.generations.len() >= u32::MAX as usize {
                    return Err(ApplyError::SlotSpaceExhausted);
                }
                self.generations.push(0);
                self.live.push(false);
                self.node_types.push(node_type);
                self.local_ids.push(0);
                self.parents.push(None);
                self.children.push(Vec::new());
                self.styles.push(StyleProps::default());
                self.props.push(PropList::new());
                self.flags.push(NodeFlags::default());
                self.frames.push(Frame::default());
                self.contents.push((0.0, 0.0));
                self.intrinsic.push(None);
                self.taffy.push(None);
                self.is_root.push(false);
                (self.generations.len() - 1) as u32
            }
        };
        let s = slot as usize;
        let next = self.generations[s].wrapping_add(1);
        self.generations[s] = if next == 0 { 1 } else { next };
        self.live[s] = true;
        self.node_types[s] = node_type;
        self.local_ids[s] = id;
        self.parents[s] = None;
        self.children[s].clear();
        self.styles[s] = StyleProps::default();
        self.props[s].clear();
        self.flags[s] = NodeFlags::CREATED;
        self.frames[s] = Frame::default();
        self.contents[s] = (0.0, 0.0);
        self.intrinsic[s] = None;
        self.taffy[s] = None;
        self.is_root[s] = false;
        self.by_local.insert(id, slot);
        self.live_count += 1;
        Ok(slot)
    }

    pub(crate) fn free_slot(&mut self, slot: u32) {
        let s = slot as usize;
        debug_assert!(self.live[s], "free of a dead slot");
        self.by_local.remove(&self.local_ids[s]);
        self.live[s] = false;
        self.parents[s] = None;
        self.children[s] = Vec::new();
        self.styles[s] = StyleProps::default();
        self.props[s] = PropList::new();
        self.flags[s] = NodeFlags::default();
        self.frames[s] = Frame::default();
        self.contents[s] = (0.0, 0.0);
        self.intrinsic[s] = None;
        self.taffy[s] = None;
        if self.is_root[s] {
            self.roots.retain(|r| *r != slot);
            self.is_root[s] = false;
        }
        self.free.push(slot);
        self.live_count -= 1;
    }

    pub(crate) fn set_parent(&mut self, slot: u32, parent: Option<u32>) {
        self.parents[slot as usize] = parent;
    }

    pub(crate) fn set_children(&mut self, slot: u32, children: Vec<u32>) {
        self.children[slot as usize] = children;
    }

    pub(crate) fn children_mut(&mut self, slot: u32) -> &mut Vec<u32> {
        &mut self.children[slot as usize]
    }

    pub(crate) fn style_mut(&mut self, slot: u32) -> &mut StyleProps {
        &mut self.styles[slot as usize]
    }

    pub(crate) fn props_mut(&mut self, slot: u32) -> &mut PropList {
        &mut self.props[slot as usize]
    }

    pub(crate) fn flags_mut(&mut self, slot: u32) -> &mut NodeFlags {
        &mut self.flags[slot as usize]
    }

    /// The content's extent from the last layout (width, height).
    pub fn content(&self, slot: u32) -> (f32, f32) {
        self.contents[slot as usize]
    }

    /// A replaced element's intrinsic size, when the host has reported it.
    pub fn intrinsic(&self, slot: u32) -> Option<(f32, f32)> {
        self.intrinsic[slot as usize]
    }

    pub(crate) fn set_intrinsic(&mut self, slot: u32, size: Option<(f32, f32)>) {
        self.intrinsic[slot as usize] = size;
    }

    pub(crate) fn set_content(&mut self, slot: u32, content: (f32, f32)) {
        self.contents[slot as usize] = content;
    }

    pub(crate) fn set_frame(&mut self, slot: u32, frame: Frame) {
        self.frames[slot as usize] = frame;
    }

    pub(crate) fn set_taffy(&mut self, slot: u32, node: Option<NodeId>) {
        self.taffy[slot as usize] = node;
    }

    pub(crate) fn set_root(&mut self, slot: u32, root: bool) {
        let s = slot as usize;
        if self.is_root[s] == root {
            return;
        }
        self.is_root[s] = root;
        if root {
            self.roots.push(slot);
        } else {
            self.roots.retain(|r| *r != slot);
        }
    }

    /// Drop every derived layout handle (before a rebuild).
    pub(crate) fn clear_taffy(&mut self) {
        for t in self.taffy.iter_mut() {
            *t = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_fail_closed_after_reuse() {
        let mut arena = NodeArena::new();
        let a = arena.alloc(1, NodeType::View).unwrap();
        let key_a = arena.key(a);
        assert_eq!(arena.resolve(key_a), Some(a));
        arena.free_slot(a);
        assert_eq!(arena.resolve(key_a), None);
        let b = arena.alloc(1, NodeType::Text).unwrap();
        assert_eq!(b, a, "the freed slot is reused");
        assert_ne!(arena.key(b), key_a, "with a new generation");
        assert_eq!(
            arena.resolve(key_a),
            None,
            "the stale key never aliases the new node"
        );
        assert_eq!(arena.slot_of(1), Some(b));
        assert_eq!(arena.node_type(b), NodeType::Text);
        assert_eq!(arena.live_count(), 1);
    }

    #[test]
    fn generation_zero_is_never_minted() {
        let mut arena = NodeArena::new();
        let a = arena.alloc(1, NodeType::View).unwrap();
        assert_eq!(arena.key(a).generation, 1);
    }

    #[test]
    fn subtree_and_ancestry() {
        let mut arena = NodeArena::new();
        let root = arena.alloc(1, NodeType::View).unwrap();
        let a = arena.alloc(2, NodeType::View).unwrap();
        let b = arena.alloc(3, NodeType::View).unwrap();
        let c = arena.alloc(4, NodeType::Text).unwrap();
        arena.set_children(root, vec![a, b]);
        arena.set_parent(a, Some(root));
        arena.set_parent(b, Some(root));
        arena.set_children(a, vec![c]);
        arena.set_parent(c, Some(a));
        assert_eq!(arena.subtree(root), vec![root, a, c, b]);
        assert!(arena.is_ancestor(root, c));
        assert!(!arena.is_ancestor(b, c));
        assert_eq!(arena.depth(c), 2);
        assert_eq!(arena.measure_owner(c), c);
    }

    #[test]
    fn text_runs_flatten_inline_children() {
        let mut arena = NodeArena::new();
        let p = arena.alloc(1, NodeType::Text).unwrap();
        let r1 = arena.alloc(2, NodeType::Text).unwrap();
        let r2 = arena.alloc(3, NodeType::Text).unwrap();
        arena.set_children(p, vec![r1, r2]);
        arena.set_parent(r1, Some(p));
        arena.set_parent(r2, Some(p));
        arena.props_mut(r1).set(PropId::Text, "Hello ".into());
        arena.props_mut(r2).set(PropId::Text, "world".into());
        arena.style_mut(r2).font_size = 20.0;
        let mut runs = Vec::new();
        arena.text_runs(p, &mut runs);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].text, "Hello ");
        assert_eq!(runs[1].text, "world");
        assert_eq!(runs[1].style.font_size, 20.0);
        assert!(arena.is_inline_run(r1));
        assert_eq!(arena.measure_owner(r2), p);
        // An own `text` prop suppresses the children.
        arena.props_mut(p).set(PropId::Text, "override".into());
        let mut runs = Vec::new();
        arena.text_runs(p, &mut runs);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text, "override");
    }
}
