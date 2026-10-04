//! @ref LLP 1093 D3 — a container's flow thread as the walk reads it: its
//! boxes in document order, as unbreakable *atoms* with a break point before
//! each, and the boxes that own them.
//!
//! An atom is a monolithic box, a paragraph (whose line boxes the walk asks
//! for when it would cross a column's end), or an empty fragmentable box. A
//! fragmentable box's own top and bottom (padding and border) are glued to
//! its first and last atom, so the break before a box is the break before its
//! first child: Class A and C points coincide, and forced breaks propagate
//! outward as Fragmentation 3 §3.1.1 says (Chrome moves a padded parent whole
//! when its first child forces a break).

use super::{FragmentRefusal, Kind};
use crate::arena::NodeArena;
use crate::generated::{BreakBetween, BreakInside, Display, NodeType, PositionType};
use crate::layout::LayoutTree;

/// An unbreakable run of the flow thread, in the container's border-box space.
#[derive(Debug, Clone)]
pub(super) struct Atom {
    pub top: f32,
    pub bottom: f32,
    /// The paragraph this atom is, if any (`Flow::paras`).
    pub para: Option<usize>,
    /// The own `margin-top` of the outermost box that starts here: a forced
    /// break keeps it, an unforced one truncates it (Fragmentation 3 §5.2).
    pub margin: f32,
    /// The break before this atom: `break-*: column`, propagated.
    pub forced: bool,
    /// The break before this atom violates `break-*: avoid` (or lies inside
    /// a box with `break-inside: avoid`).
    pub avoid: bool,
    /// A kernel-only monolithic box (D10), journalled if it does not fit.
    pub refusal: Option<(u32, FragmentRefusal)>,
}

/// A paragraph: its line boxes are the units the walk breaks between.
#[derive(Debug, Clone)]
pub(super) struct Para {
    pub slot: u32,
    /// Where line boxes start: the paragraph's content top.
    pub content_top: f32,
    /// Its content width, the offer its lines were measured at.
    pub width: f32,
    pub orphans: u32,
    pub widows: u32,
    /// Every break between its lines lies inside a `break-inside: avoid` box.
    pub avoid: bool,
    /// Each line box's bottom in content coordinates, once asked for;
    /// `Some(empty)` when the host gave none (the paragraph stays whole).
    pub lines: Option<Vec<f32>>,
}

/// A box in the flow and the atoms it spans (inclusive).
#[derive(Debug, Clone, Copy)]
pub(super) struct FlowBox {
    pub slot: u32,
    pub x: f32,
    pub top: f32,
    pub width: f32,
    pub bottom: f32,
    pub first: usize,
    pub last: usize,
}

/// An absolutely positioned box in the flow and its flow-thread top.
#[derive(Debug, Clone, Copy)]
pub(super) struct Absolute {
    pub slot: u32,
    pub x: f32,
    pub top: f32,
    /// Its containing block lies inside the flow (D10.4): it takes the
    /// column holding its own top. Otherwise it is placed against the
    /// container (or beyond), as CSS places it.
    pub in_flow: bool,
}

#[derive(Debug, Default)]
pub(super) struct Flow {
    pub atoms: Vec<Atom>,
    pub paras: Vec<Para>,
    pub boxes: Vec<FlowBox>,
    pub absolutes: Vec<Absolute>,
}

struct Builder<'a> {
    arena: &'a NodeArena,
    tree: &'a LayoutTree,
    container: u32,
    flow: Flow,
    // A `break-after` waiting for the next atom (propagated past the end of
    // every box it ends).
    after_forced: bool,
    after_avoid: bool,
}

/// The flow thread of `container`, a block multi-column container, from its
/// laid-out children.
pub(super) fn flow(arena: &NodeArena, tree: &LayoutTree, container: u32) -> Flow {
    let mut b = Builder {
        arena,
        tree,
        container,
        flow: Flow::default(),
        after_forced: false,
        after_avoid: false,
    };
    for &child in arena.children(container) {
        b.visit(child, 0.0, 0.0, false, false);
    }
    // A break before the first atom is no break: the column starts there.
    if let Some(first) = b.flow.atoms.first_mut() {
        first.forced = false;
        first.avoid = false;
    }
    b.flow
}

/// A `text` that is itself a multi-column container: one paragraph whose
/// lines flow through its columns (D1).
pub(super) fn text(
    arena: &NodeArena,
    slot: u32,
    content_top: f32,
    width: f32,
    height: f32,
) -> Flow {
    let s = arena.style(slot);
    Flow {
        atoms: vec![Atom {
            top: content_top,
            bottom: content_top + height,
            para: Some(0),
            margin: 0.0,
            forced: false,
            avoid: false,
            refusal: None,
        }],
        paras: vec![Para {
            slot,
            content_top,
            width,
            orphans: u32::from(s.orphans.max(1)),
            widows: u32::from(s.widows.max(1)),
            avoid: s.break_inside != BreakInside::Auto,
            lines: None,
        }],
        boxes: Vec::new(),
        absolutes: Vec::new(),
    }
}

impl Builder<'_> {
    // `point_avoid`: the break before this box lies inside an avoid box;
    // `inside_avoid`: the breaks inside its parent do.
    fn visit(&mut self, slot: u32, px: f32, py: f32, point_avoid: bool, inside_avoid: bool) {
        let arena = self.arena;
        let s = arena.style(slot);
        if s.display == Display::None || arena.is_inline_run(slot) {
            return;
        }
        let Some(node) = arena.taffy(slot) else {
            return;
        };
        let l = self.tree.layout(node);
        let (x, top) = (px + l.location.x, py + l.location.y);
        let bottom = top + l.size.height;
        if s.position_type == PositionType::Absolute {
            let in_flow = self.containing_block_in_flow(slot);
            self.flow.absolutes.push(Absolute {
                slot,
                x,
                top,
                in_flow,
            });
            return;
        }
        let first = self.flow.atoms.len();
        let own_avoid = s.break_inside != BreakInside::Auto;
        match super::kind(arena, slot) {
            Kind::Fragmentable => {
                let inner = inside_avoid || own_avoid;
                let mut point = point_avoid;
                for &child in arena.children(slot) {
                    let before = self.flow.atoms.len();
                    self.visit(child, x, top, point, inner);
                    if self.flow.atoms.len() > before {
                        point = inner;
                    }
                }
                if self.flow.atoms.len() == first {
                    self.push(top, bottom, None, l.margin.top, point_avoid, None);
                } else {
                    let atom = &mut self.flow.atoms[first];
                    atom.top = atom.top.min(top);
                    atom.margin = l.margin.top;
                    let last = self.flow.atoms.len() - 1;
                    let atom = &mut self.flow.atoms[last];
                    atom.bottom = atom.bottom.max(bottom);
                }
            }
            Kind::Paragraph if arena.node_type(slot) == NodeType::Text => {
                let inset_top = l.padding.top + l.border.top;
                let width = l.size.width
                    - l.padding.left
                    - l.padding.right
                    - l.border.left
                    - l.border.right;
                let para = self.flow.paras.len();
                self.flow.paras.push(Para {
                    slot,
                    content_top: top + inset_top,
                    width,
                    orphans: u32::from(s.orphans.max(1)),
                    widows: u32::from(s.widows.max(1)),
                    avoid: inside_avoid || own_avoid,
                    lines: None,
                });
                self.push(top, bottom, Some(para), l.margin.top, point_avoid, None);
            }
            Kind::Paragraph => {
                self.push(top, bottom, None, l.margin.top, point_avoid, None);
            }
            Kind::Monolithic(refusal) => {
                self.push(
                    top,
                    bottom,
                    None,
                    l.margin.top,
                    point_avoid,
                    refusal.map(|r| (slot, r)),
                );
            }
        }
        let atoms = self.flow.atoms.len();
        if atoms > first {
            // `break-before` on this box (the outermost starting here wins
            // nothing over the inner ones: any `column` forces, any avoid
            // avoids), and `break-after` for whatever comes next.
            let atom = &mut self.flow.atoms[first];
            match s.break_before {
                BreakBetween::Column => atom.forced = true,
                BreakBetween::Avoid | BreakBetween::AvoidColumn => atom.avoid = true,
                BreakBetween::Auto => {}
            }
            match s.break_after {
                BreakBetween::Column => self.after_forced = true,
                BreakBetween::Avoid | BreakBetween::AvoidColumn => self.after_avoid = true,
                BreakBetween::Auto => {}
            }
            self.flow.boxes.push(FlowBox {
                slot,
                x,
                top,
                width: l.size.width,
                bottom,
                first,
                last: atoms - 1,
            });
        }
    }

    fn push(
        &mut self,
        top: f32,
        bottom: f32,
        para: Option<usize>,
        margin: f32,
        avoid: bool,
        refusal: Option<(u32, FragmentRefusal)>,
    ) {
        self.flow.atoms.push(Atom {
            top,
            bottom,
            para,
            margin,
            forced: std::mem::take(&mut self.after_forced),
            avoid: avoid || std::mem::take(&mut self.after_avoid),
            refusal,
        });
    }

    // Whether an absolutely positioned box's containing block lies inside
    // the flow: a positioned ancestor below the container (LLP 1074 T1).
    fn containing_block_in_flow(&self, slot: u32) -> bool {
        let mut at = self.arena.parent(slot);
        while let Some(s) = at {
            if s == self.container {
                return false;
            }
            if self.arena.style(s).position_type != PositionType::Static {
                return true;
            }
            at = self.arena.parent(s);
        }
        false
    }
}
