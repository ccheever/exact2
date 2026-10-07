//! The order siblings paint in, on every host (LLP 1083.000).
//!
//! CSS paints a stacking context in phases (CSS 2.1 Appendix E): in-flow
//! boxes, then positioned boxes and child stacking contexts at level 0 in
//! tree order, then positive `z-index`, with negative `z-index` under the
//! in-flow boxes. A native host paints each child's whole subtree in turn.
//! The two agree when every child of a parent is ordered by one [`Rank`] and
//! the web isolates (`isolation: isolate`) exactly the boxes [`decide`]
//! names: an isolated box traps what would otherwise paint outside its turn.
//!
//! This module is the one definition. The kernel runs it over its arena
//! ([`Kernel::paint_order`]); the server document runs it over its own tree
//! through the same functions; the JS target's `paint.js` mirrors them per
//! instance, and the fixture holds every path to the same answers.
//!
//! Per node: the children's [`Potentials`] first (what each lets escape),
//! then the children's isolation left to right ([`decide`]), then the
//! node's own potentials from its children's records ([`potentials`]).
//! A node's potentials never depend on its own isolation, so one post-order
//! pass is enough and a second changes nothing.

use crate::arena::NodeArena;
use crate::generated::{Display, Isolation, MixBlendMode, NodeType, PositionType, PropId, StyleId};
use crate::id::ViewId;
use crate::kernel::Kernel;

/// Authored `z-index` is clamped to this, and the two values above it are
/// the exit ghost's and the Arrange lift's on every host (§3.7).
pub const Z_MAX: i32 = i32::MAX - 2;

/// A node's paint facts of its own (§2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Own {
    /// Its used `position` is not `static`; a root counts.
    pub positioned: bool,
    /// A stacking context by its own values or by host policy (D2, D3).
    pub stacks: bool,
    /// Its applicable `z-index`: authored, on a positioned box or a flex or
    /// grid item, clamped to ±[`Z_MAX`].
    pub z: Option<i32>,
    /// It stacks only by host policy, motion or structure (D2, D3), not by
    /// CSS it carries: the web writes `isolation: isolate` for it.
    pub policy: bool,
    /// Outside the box tree's painting (an element inside an `svg`, a
    /// head, a top-layer dialog or popover): no rank, no isolation, nothing
    /// escapes it into its parent.
    pub outside: bool,
}

impl Own {
    /// It paints at level 0 when it is not in flow: positioned with no
    /// non-zero `z`, or stacking with `z` 0 or none.
    pub fn level0(&self) -> bool {
        let nonzero = matches!(self.z, Some(z) if z != 0);
        (self.positioned || self.stacks) && !nonzero
    }
    fn traps(&self, isolated: bool) -> bool {
        self.stacks || isolated
    }
}

/// What a node lets escape into its parent's painting (§2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Potentials {
    /// A non-zero `z-index` that nothing between traps.
    pub z: bool,
    /// Something at level 0 that nothing between traps, not under a
    /// positioned box within the node.
    pub level0: bool,
}

/// One child's record, once its parent's list is decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Child {
    /// Its own facts.
    pub own: Own,
    /// Whether its parent's list isolates it.
    pub isolated: bool,
    /// What its own children let escape into it.
    pub potentials: Potentials,
}

/// What `child` contributes to its parent's potentials.
pub fn contribution(child: &Child) -> Potentials {
    let Child {
        own,
        isolated,
        potentials,
    } = *child;
    if own.outside {
        return Potentials::default();
    }
    let open = !own.traps(isolated);
    Potentials {
        z: matches!(own.z, Some(z) if z != 0) || (open && potentials.z),
        // An isolated box is a stacking context at level 0 itself.
        level0: own.level0() || isolated || (open && !own.positioned && potentials.level0),
    }
}

/// A node's potentials: the OR of its children's contributions.
pub fn potentials(children: &[Child]) -> Potentials {
    children
        .iter()
        .map(contribution)
        .fold(Potentials::default(), |a, c| Potentials {
            z: a.z || c.z,
            level0: a.level0 || c.level0,
        })
}

/// Which of a parent's children the web isolates, left to right (§2.3):
/// (z) a non-stacking child that lets a non-zero `z-index` escape; (a) a
/// static, non-stacking child that lets something at level 0 escape, after
/// a sibling that is positioned, stacks, is isolated or leaks; (b) a static,
/// non-stacking child after a sibling that leaks.
pub fn decide(children: &[(Own, Potentials)]) -> Vec<bool> {
    let mut out = Vec::with_capacity(children.len());
    // Whether an earlier sibling is positioned, stacks, is isolated or leaks;
    // and whether one leaks.
    let (mut layered, mut leaked) = (false, false);
    for &(own, pot) in children {
        if own.outside {
            out.push(false);
            continue;
        }
        let free = !own.stacks;
        let stat = !own.positioned;
        let isolated =
            free && (pot.z || (stat && pot.level0 && (layered || leaked)) || (stat && leaked));
        let leaks = stat && free && !isolated && pot.level0;
        layered |= own.positioned || own.stacks || isolated || leaks;
        leaked |= leaks;
        out.push(isolated);
    }
    out
}

/// A child's place among its siblings (§2.4), as an integer twice the rank:
/// a non-zero `z` is `2z`, rank ½ is 1, in flow is 0. Lossless, and the
/// encoding the Apple batch carries.
pub fn rank(own: &Own, isolated: bool) -> i64 {
    match own.z {
        Some(z) if z != 0 => 2 * i64::from(z),
        _ if own.outside => 0,
        _ if own.positioned || own.stacks || isolated => 1,
        _ => 0,
    }
}

/// One node's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Placed {
    /// The web isolates it.
    pub isolated: bool,
    /// Twice its rank among its siblings ([`rank`]).
    pub rank: i64,
    /// It stacks only by policy ([`Own::policy`]): the web writes
    /// `isolation: isolate` for it whether or not it is isolated.
    pub policy: bool,
}

/// The motion properties that change stacking while they animate (D3).
fn stacking_property(p: exact_motion::Property) -> bool {
    use exact_motion::Property as P;
    matches!(
        p,
        P::Translate | P::Scale | P::Rotate | P::Opacity | P::Layout
    )
}

/// Whether a node's motion can stack it (D3), so it stacks at rest.
fn motion_stacks(s: &crate::StyleProps) -> bool {
    let m = &s.mask;
    let transition = s.transition.0.iter().any(|t| {
        use exact_motion::TransitionProperty as T;
        match t.property {
            T::All => true,
            T::Property(p) => stacking_property(p),
            T::BorderColor => false,
        }
    });
    let animation = s.animation.0.iter().any(|a| {
        a.keyframes
            .0
            .iter()
            .any(|k| k.values.iter().any(|(p, _)| stacking_property(*p)))
    });
    (m.has(StyleId::Transition) && transition)
        || (m.has(StyleId::Animation) && animation)
        || m.has(StyleId::LayoutTransition)
        || m.has(StyleId::ExitAnimation)
}

/// What [`own_from`] reads of one node: its rows, props and type, and the
/// three facts that come from around it. Any tree can fill it in: the
/// kernel's arena ([`own`]), the server document's tree, a host's mirror.
#[derive(Clone, Copy)]
pub struct Facts<'a> {
    /// Its style rows.
    pub style: &'a crate::StyleProps,
    /// Its props.
    pub props: &'a crate::PropList,
    /// Its node type.
    pub kind: NodeType,
    /// It is a root.
    pub root: bool,
    /// Its parent's `display`, when it has a parent.
    pub parent_display: Option<Display>,
    /// A sibling is a text-flow exclusion (`flow::is_exclusion`).
    pub beside_exclusion: bool,
    /// A child has a `-exact-layout-transition` row.
    pub holds_layout_transition: bool,
}

/// A node's own facts from its rows, props and structure (§2.1, D2, D3).
pub fn own_from(f: Facts<'_>) -> Own {
    let s = f.style;
    let m = &s.mask;
    let props = f.props;
    let kind = f.kind;
    let outside = kind.is_svg_element()
        || kind.is_metadata()
        || props.str(PropId::SemanticTag) == Some("dialog")
        || props.str(PropId::Popover).is_some();
    let positioned = s.position_type != PositionType::Static || f.root;
    let item = matches!(f.parent_display, Some(Display::Flex | Display::Grid));
    let z =
        (m.has(StyleId::ZIndex) && (positioned || item)).then(|| s.z_index.clamp(-Z_MAX, Z_MAX));
    let authored = (m.has(StyleId::Opacity) && s.opacity < 1.0)
        || m.has(StyleId::Translate)
        || m.has(StyleId::Scale)
        || m.has(StyleId::Rotate)
        || (m.has(StyleId::Transform) && !s.transform.0.is_empty())
        || (m.has(StyleId::PressScale) && s.press_scale != 1.0)
        || (m.has(StyleId::Filter) && !s.filter.is_none())
        || (m.has(StyleId::BackdropBlur) && s.backdrop_blur > 0.0)
        || (m.has(StyleId::ClipPath) && s.rare.clip_path != crate::clip::ClipPath::default())
        || (m.has(StyleId::MaskImage)
            && s.rare.mask_image != crate::gradient::BackgroundImage::default())
        || (m.has(StyleId::Perspective) && s.perspective > 0.0)
        || s.mix_blend_mode != MixBlendMode::Normal
        || s.isolation == Isolation::Isolate
        || s.position_type == PositionType::Sticky
        || z.is_some();
    // Host policy (D2): what the hosts paint as a stacking context of their own.
    let button = kind == NodeType::Control && props.str(PropId::Type) == Some("button");
    let style = props.str(PropId::ButtonStyle).unwrap_or("bordered");
    let host = kind == NodeType::Canvas
        || (kind == NodeType::Image
            && (m.has(StyleId::TintColor)
                || props.str(PropId::ImageSource).is_some_and(|s| s.starts_with("symbol:"))))
        || (button && style.ends_with("glass"))
        || (button
            && props.bool(PropId::Disabled) == Some(true)
            && !matches!(style, "bordered" | "gray"))
        || props.str(PropId::BackgroundMaterial).is_some()
        || props.str(PropId::NavigationPresentation) == Some("modal")
        // A root traps what its children let escape, as a native host's
        // root view does: a negative z-index stays above its background.
        || f.root;
    // Structural (D2, D3): a paragraph beside an exclusion; the parent of a
    // child with a layout transition.
    let flowing = kind == NodeType::Text && f.beside_exclusion;
    let policy = host || motion_stacks(s) || flowing || f.holds_layout_transition;
    Own {
        positioned,
        stacks: authored || policy,
        policy: policy && !authored,
        z,
        outside,
    }
}

/// [`own_from`] for a node of the kernel's arena.
pub fn own(arena: &NodeArena, slot: u32) -> Own {
    let beside = arena
        .parent(slot)
        .is_some_and(|p| beside_exclusion(arena, p));
    own_beside(arena, slot, beside)
}

fn beside_exclusion(arena: &NodeArena, parent: u32) -> bool {
    arena.children(parent).iter().any(|&c| {
        #[cfg(test)]
        tests::EXCLUSION_VISITS.with(|n| n.set(n.get() + 1));
        crate::flow::is_exclusion(arena, c)
    })
}

fn own_beside(arena: &NodeArena, slot: u32, beside_exclusion: bool) -> Own {
    own_from(Facts {
        style: arena.style(slot),
        props: arena.props(slot),
        kind: arena.node_type(slot),
        root: arena.is_root(slot),
        parent_display: arena.parent(slot).map(|p| arena.style(p).display),
        beside_exclusion,
        holds_layout_transition: arena
            .children(slot)
            .iter()
            .any(|&c| arena.style(c).mask.has(StyleId::LayoutTransition)),
    })
}

impl Kernel {
    /// Every live node's [`Placed`] under every root, in one post-order pass.
    pub fn paint_order(&self) -> Vec<(ViewId, Placed)> {
        let arena = self.arena();
        let mut out = Vec::new();
        for &root in arena.roots() {
            let own_root = own(arena, root);
            walk(arena, root, &mut out);
            out.push((
                arena.local_id(root),
                Placed {
                    isolated: false,
                    rank: rank(&own_root, false),
                    policy: own_root.policy,
                },
            ));
        }
        out
    }
}

impl Kernel {
    /// [`Kernel::paint_order`] for a caller that kept earlier answers:
    /// `known(view)` is a node's potentials when nothing under it changed
    /// since they were found. Such a node is decided among its siblings
    /// from them and its own facts, and not walked: its descendants are
    /// left out, their answers being as they were. Each answer comes with
    /// the node's potentials, for the caller to keep.
    pub fn paint_order_reusing(
        &self,
        known: &mut dyn FnMut(ViewId) -> Option<Potentials>,
    ) -> Vec<(ViewId, Placed, Potentials)> {
        let arena = self.arena();
        let mut out = Vec::new();
        for &root in arena.roots() {
            let own_root = own(arena, root);
            let pot = walk_reusing(arena, root, &mut out, known);
            out.push((
                arena.local_id(root),
                Placed {
                    isolated: false,
                    rank: rank(&own_root, false),
                    policy: own_root.policy,
                },
                pot,
            ));
        }
        out
    }
}

/// [`walk`], taking a child's potentials from `known` instead of walking it.
fn walk_reusing(
    arena: &NodeArena,
    slot: u32,
    out: &mut Vec<(ViewId, Placed, Potentials)>,
    known: &mut dyn FnMut(ViewId) -> Option<Potentials>,
) -> Potentials {
    let children = arena.children(slot);
    let beside = beside_exclusion(arena, slot);
    let records: Vec<(Own, Potentials)> = children
        .iter()
        .map(|&c| {
            let pot = match known(arena.local_id(c)) {
                Some(pot) => pot,
                None => walk_reusing(arena, c, out, known),
            };
            (own_beside(arena, c, beside), pot)
        })
        .collect();
    let isolated = decide(&records);
    let mut list = Vec::with_capacity(children.len());
    for ((&c, &(own, pot)), iso) in children.iter().zip(&records).zip(isolated) {
        out.push((
            arena.local_id(c),
            Placed {
                isolated: iso,
                rank: rank(&own, iso),
                policy: own.policy,
            },
            pot,
        ));
        list.push(Child {
            own,
            isolated: iso,
            potentials: pot,
        });
    }
    potentials(&list)
}

/// Decides `slot`'s children, records them, and returns `slot`'s potentials.
fn walk(arena: &NodeArena, slot: u32, out: &mut Vec<(ViewId, Placed)>) -> Potentials {
    let children = arena.children(slot);
    let beside = beside_exclusion(arena, slot);
    let records: Vec<(Own, Potentials)> = children
        .iter()
        .map(|&c| (own_beside(arena, c, beside), walk(arena, c, out)))
        .collect();
    let isolated = decide(&records);
    let mut list = Vec::with_capacity(children.len());
    for ((&c, &(own, pot)), iso) in children.iter().zip(&records).zip(isolated) {
        out.push((
            arena.local_id(c),
            Placed {
                isolated: iso,
                rank: rank(&own, iso),
                policy: own.policy,
            },
        ));
        list.push(Child {
            own,
            isolated: iso,
            potentials: pot,
        });
    }
    potentials(&list)
}

#[cfg(test)]
mod tests {
    use super::*;

    std::thread_local! {
        pub(super) static EXCLUSION_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    #[test]
    fn reusing_a_subtree_s_potentials_answers_the_rest_as_a_full_pass() {
        use crate::{Op, StyleProps, StyleValue};
        let style = |id: u32, rows: &[(StyleId, StyleValue)]| {
            let mut patch = StyleProps::default();
            for (row, value) in rows {
                patch.set_dynamic(*row, value).unwrap();
            }
            Op::SetStyle {
                id,
                patch: Box::new(patch),
            }
        };
        // 1 > [2 > [3 > [4 abs z2], 5], 6 > [7 opacity .5, 8 relative], 9]
        let mut ops: Vec<_> = (1..=9)
            .map(|id| Op::CreateView {
                id,
                node_type: NodeType::View,
            })
            .collect();
        for (id, children) in [
            (1, vec![2, 6, 9]),
            (2, vec![3, 5]),
            (3, vec![4]),
            (6, vec![7, 8]),
        ] {
            ops.push(Op::SetChildren { id, children });
        }
        let text = |s: &str| StyleValue::Text(s.into());
        ops.push(style(
            4,
            &[
                (StyleId::PositionType, text("absolute")),
                (StyleId::ZIndex, StyleValue::Number(2.0)),
            ],
        ));
        ops.push(style(7, &[(StyleId::Opacity, StyleValue::Number(0.5))]));
        ops.push(style(8, &[(StyleId::PositionType, text("relative"))]));
        ops.push(Op::AttachRoot { id: 1 });
        let mut kernel = Kernel::with_monospace();
        kernel.apply(0, 1, &ops).unwrap();
        let full = kernel.paint_order();
        assert!(
            full.iter().any(|(_, p)| p.rank != 0 && p.rank != 1),
            "a z-index is ranked"
        );
        let fresh = kernel.paint_order_reusing(&mut |_| None);
        assert_eq!(
            fresh.iter().map(|(v, p, _)| (*v, *p)).collect::<Vec<_>>(),
            full
        );
        // Subtrees 2 and 6 known: their own places are answered again, their
        // descendants are not, and every answer equals the full pass's.
        let pot = |v: ViewId| fresh.iter().find(|(id, ..)| *id == v).map(|(.., p)| *p);
        let known = [
            kernel.arena().local_id(kernel.arena().slot_of(2).unwrap()),
            kernel.arena().local_id(kernel.arena().slot_of(6).unwrap()),
        ];
        let reused =
            kernel.paint_order_reusing(&mut |v| known.contains(&v).then(|| pot(v)).flatten());
        let listed: Vec<ViewId> = reused.iter().map(|(v, ..)| *v).collect();
        assert_eq!(listed.len(), 4, "1, 2, 6 and 9: {listed:?}");
        for (v, placed, _) in &reused {
            assert_eq!(
                Some(placed),
                full.iter().find(|(id, _)| id == v).map(|(_, p)| p)
            );
        }
    }

    #[test]
    fn ten_thousand_siblings_have_linear_exclusion_visits() {
        use crate::Op;
        let mut kernel = Kernel::with_monospace();
        let mut ops: Vec<_> = (1..=10_001)
            .map(|id| Op::CreateView {
                id,
                node_type: NodeType::Text,
            })
            .collect();
        ops.push(Op::SetChildren {
            id: 1,
            children: (2..=10_001).collect(),
        });
        ops.push(Op::AttachRoot { id: 1 });
        kernel.apply(0, 1, &ops).unwrap();
        EXCLUSION_VISITS.with(|n| n.set(0));
        assert_eq!(kernel.paint_order().len(), 10_001);
        let visits = EXCLUSION_VISITS.with(|n| n.get());
        assert!(
            visits <= 10_000,
            "10,000 siblings required {visits} exclusion visits"
        );
    }

    const PLAIN: Own = Own {
        positioned: false,
        stacks: false,
        policy: false,
        z: None,
        outside: false,
    };
    const REL: Own = Own {
        positioned: true,
        ..PLAIN
    };
    fn abs(z: i32) -> Own {
        Own {
            positioned: true,
            stacks: true,
            z: Some(z),
            ..PLAIN
        }
    }
    /// A static holder whose one child is `child` (a leaf).
    fn holder(child: Own) -> (Own, Potentials) {
        (
            PLAIN,
            potentials(&[Child {
                own: child,
                isolated: false,
                potentials: Potentials::default(),
            }]),
        )
    }
    const NONE: Potentials = Potentials {
        z: false,
        level0: false,
    };

    #[test]
    fn a_sticky_header_over_plain_rows_is_not_lifted_under_them() {
        let sticky = Own {
            positioned: true,
            stacks: true,
            ..PLAIN
        };
        let list = [(sticky, NONE), (PLAIN, NONE), (PLAIN, NONE)];
        assert_eq!(decide(&list), [false, false, false]);
        assert_eq!(rank(&sticky, false), 1);
        assert_eq!(rank(&PLAIN, false), 0);
    }

    #[test]
    fn the_nested_z_fixture_traps_its_red_box() {
        // S abs z1, T(P abs z2): T is isolated by (z); S's z1 paints over it.
        let list = [(abs(1), NONE), holder(abs(2))];
        assert_eq!(decide(&list), [false, true]);
        assert_eq!(rank(&abs(1), false), 2);
        assert_eq!(rank(&PLAIN, true), 1);
    }

    #[test]
    fn a_level0_escape_isolates_the_static_box_after_it() {
        // T(P), X: X by (b); T is the first, so not isolated.
        let list = [holder(REL), (PLAIN, NONE)];
        assert_eq!(decide(&list), [false, true]);
    }

    #[test]
    fn a_z_holder_is_isolated_even_first_and_its_follower_is_not() {
        // T(P abs z2), X: T by (z); X lets nothing escape and T traps.
        let list = [holder(abs(2)), (PLAIN, NONE)];
        assert_eq!(decide(&list), [true, false]);
        // A positioned holder of a negative z is isolated too.
        let a = (
            REL,
            potentials(&[Child {
                own: abs(-1),
                ..Default::default()
            }]),
        );
        assert_eq!(decide(&[a, (PLAIN, NONE)]), [true, false]);
    }

    #[test]
    fn a_holder_after_a_positioned_sibling_traps_and_a_plain_follower_stays_in_flow() {
        // S relative, T(P), X: T by (a); X not.
        let list = [(REL, NONE), holder(REL), (PLAIN, NONE)];
        assert_eq!(decide(&list), [false, true, false]);
    }

    #[test]
    fn a_positioned_boxs_level0_descendants_do_not_escape_it() {
        // S(relative, holds Q relative), X: X not isolated.
        let s = (
            REL,
            potentials(&[Child {
                own: REL,
                ..Default::default()
            }]),
        );
        assert!(s.1.level0);
        let c = contribution(&Child {
            own: REL,
            isolated: false,
            potentials: s.1,
        });
        assert!(c.level0, "S itself is level 0");
        assert_eq!(decide(&[s, (PLAIN, NONE)]), [false, false]);
    }

    #[test]
    fn a_trap_exports_only_its_own_z() {
        let inner = Potentials {
            z: true,
            level0: true,
        };
        let trapped = Child {
            own: PLAIN,
            isolated: true,
            potentials: inner,
        };
        assert_eq!(
            contribution(&trapped),
            Potentials {
                z: false,
                level0: true
            }
        );
        let z = Child {
            own: abs(3),
            isolated: false,
            potentials: inner,
        };
        assert_eq!(
            contribution(&z),
            Potentials {
                z: true,
                level0: false
            }
        );
    }

    #[test]
    fn ranks_order_negative_in_flow_level0_positive() {
        let mut r = [
            rank(&abs(5), false),
            rank(&REL, false),
            rank(&PLAIN, false),
            rank(&abs(-2), false),
        ];
        r.sort();
        assert_eq!(r, [-4, 0, 1, 10]);
        // A stacking box whose z does not apply is ½, not its z.
        let faded = Own {
            stacks: true,
            ..PLAIN
        };
        assert_eq!(rank(&faded, false), 1);
    }

    #[test]
    fn deciding_twice_changes_nothing() {
        let list = [
            (REL, NONE),
            holder(REL),
            holder(abs(4)),
            (PLAIN, NONE),
            holder(REL),
        ];
        assert_eq!(decide(&list), decide(&list));
    }
}
