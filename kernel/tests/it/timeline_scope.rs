//! Timeline names resolve as CSS resolves them (LLP 1057.003 D4;
//! Scroll-driven Animations 1 §4.2): from the consumer up, the first node
//! that declares the name or scopes it decides. The cases are the phase 3
//! Chrome probe's trees, a drag timeline where Chrome's had a scroller's
//! `scroll-timeline`, and each answer is the one Chrome 154 gave, but for
//! `all`, which Chrome does not parse: those follow the specification.
//! Then re-resolution: a node inserted, removed or moved, or a row set on
//! another node, changes what a consumer follows, and the receipt names it.

use exact_kernel::{
    motion_node, CommitReceipt, Kernel, NodeType, Op, StyleId, StyleProps, StyleValue,
};
use exact_motion::{Engine, NamedTimeline, Property, Value};

/// A tree, one node a line, two spaces a level, under an implicit root:
/// a label, then `drag=`, `scope=` and `follow=` rows (a consumer follows
/// over 0–100 px). The consumer is `c`.
struct Tree {
    k: Kernel,
    labels: Vec<(String, u32)>,
}

fn rows(spec: &[&str]) -> Box<StyleProps> {
    exact_kernel::timeline::link();
    let mut s = StyleProps::default();
    for row in spec {
        let (key, value) = row.split_once('=').expect("key=value");
        let (id, value) = match key {
            "drag" => (StyleId::DragTimeline, value.to_string()),
            "scope" => (StyleId::TimelineScope, value.replace(',', ", ")),
            "follow" => {
                s.set_dynamic(
                    StyleId::AnimationRange,
                    &StyleValue::Text("0px 100px".into()),
                )
                .unwrap();
                (StyleId::AnimationTimeline, value.to_string())
            }
            _ => panic!("{key}"),
        };
        s.set_dynamic(id, &StyleValue::Text(value)).unwrap();
    }
    Box::new(s)
}

fn build(spec: &str) -> Tree {
    let mut ops = vec![Op::CreateView {
        id: 1,
        node_type: NodeType::View,
    }];
    let mut labels = Vec::new();
    let mut children: Vec<(u32, Vec<u32>)> = vec![(1, Vec::new())];
    let mut stack: Vec<u32> = vec![1];
    for line in spec.lines().filter(|l| !l.trim().is_empty()) {
        let depth = (line.len() - line.trim_start().len()) / 2;
        let mut words = line.split_whitespace();
        let label = words.next().unwrap();
        let id = labels.len() as u32 + 2;
        stack.truncate(depth + 1);
        let parent = *stack.last().unwrap();
        ops.push(Op::CreateView {
            id,
            node_type: NodeType::View,
        });
        let spec: Vec<&str> = words.collect();
        if !spec.is_empty() {
            ops.push(Op::SetStyle {
                id,
                patch: rows(&spec),
            });
        }
        children
            .iter_mut()
            .find(|c| c.0 == parent)
            .unwrap()
            .1
            .push(id);
        children.push((id, Vec::new()));
        stack.push(id);
        labels.push((label.to_string(), id));
    }
    for (id, children) in children.into_iter().filter(|c| !c.1.is_empty()) {
        ops.push(Op::SetChildren { id, children });
    }
    ops.push(Op::AttachRoot { id: 1 });
    let mut k = Kernel::with_monospace();
    k.apply(0, 1, &ops).unwrap();
    Tree { k, labels }
}

impl Tree {
    fn id(&self, label: &str) -> u32 {
        self.labels.iter().find(|l| l.0 == label).expect(label).1
    }

    fn node(&self, label: &str) -> u64 {
        motion_node(self.k.node(self.id(label)).unwrap().key)
    }

    /// What `c` follows: a label, `inactive` or `missing`.
    fn answer(&self) -> String {
        let key = self.k.node(self.id("c")).unwrap().key;
        match self.k.timeline_of(key).expect("c follows a name") {
            NamedTimeline::Source(n) => self
                .labels
                .iter()
                .find(|l| self.k.node(l.1).map(|v| motion_node(v.key)) == Some(n))
                .unwrap()
                .0
                .clone(),
            NamedTimeline::Inactive => "inactive".into(),
            NamedTimeline::Missing => "missing".into(),
        }
    }

    fn apply(&mut self, ops: &[Op]) -> CommitReceipt {
        self.k.apply(0, 2, ops).unwrap()
    }
}

#[test]
fn a_name_resolves_as_chrome_resolves_it() {
    // (case, tree, what `c` follows). The probe's scrollers are `sN` (at
    // N px), its `timeline-scope` boxes `x`, and plain boxes `b`.
    let cases = [
        ("notfound", "b\n  c follow=--t\n", "missing"),
        (
            "notfound-sibling",
            "b\n  s30 drag=--t\n  c follow=--t\n",
            "missing",
        ),
        ("scope0", "x scope=--t\n  c follow=--t\n", "inactive"),
        (
            "scope2",
            "x scope=--t\n  s30 drag=--t\n  s70 drag=--t\n  c follow=--t\n",
            "inactive",
        ),
        ("self", "c drag=--t follow=--t\n", "c"),
        ("ancestor", "s40 drag=--t\n  c follow=--t\n", "s40"),
        (
            "nearest-ancestor",
            "s20 drag=--t\n  s40 drag=--t\n    c follow=--t\n",
            "s40",
        ),
        (
            "scope1-sibling",
            "x scope=--t\n  s30 drag=--t\n  c follow=--t\n",
            "s30",
        ),
        (
            "scope1-cousin",
            "x scope=--t\n  b\n    s30 drag=--t\n  b2\n    c follow=--t\n",
            "s30",
        ),
        (
            "scope1-on-consumer",
            "c scope=--t follow=--t\n  s30 drag=--t\n",
            "s30",
        ),
        (
            "scope-other-name",
            "x scope=--u\n  s30 drag=--t\n  c follow=--t\n",
            "missing",
        ),
        (
            "scope-list",
            "x scope=--u,--t\n  s30 drag=--t\n  c follow=--t\n",
            "s30",
        ),
        (
            "nested-inner-captures",
            "x scope=--t\n  y scope=--t\n    s30 drag=--t\n  c follow=--t\n",
            "inactive",
        ),
        (
            "nested-inner-captures-other",
            "x scope=--t\n  y scope=--t\n    s30 drag=--t\n  s60 drag=--t\n  c follow=--t\n",
            "s60",
        ),
        (
            "nested-consumer-in-inner",
            "x scope=--t\n  y scope=--t\n    s30 drag=--t\n    c follow=--t\n  s60 drag=--t\n",
            "s30",
        ),
        (
            "nested-other-name-inner",
            "x scope=--t\n  y scope=--u\n    s30 drag=--t\n  c follow=--t\n",
            "s30",
        ),
        (
            "scope-nearer-than-source",
            "s20 drag=--t\n  x scope=--t\n    c follow=--t\n",
            "inactive",
        ),
        (
            "scope-nearer-than-source-with-own",
            "s20 drag=--t\n  x scope=--t\n    s50 drag=--t\n    c follow=--t\n",
            "s50",
        ),
        (
            "source-nearer-than-scope",
            "x scope=--t\n  s50 drag=--t\n  s20 drag=--t\n    c follow=--t\n",
            "s20",
        ),
        (
            "source-with-own-scope",
            "x scope=--t\n  s30 drag=--t scope=--t\n  c follow=--t\n",
            "inactive",
        ),
        (
            "source-with-own-scope-inside",
            "s30 drag=--t scope=--t\n  s60 drag=--t\n  c follow=--t\n",
            "s30",
        ),
        (
            "scope-outer-used-when-inner-lacks",
            "x scope=--t\n  s40 drag=--t\n  y scope=--u\n    c follow=--t\n",
            "s40",
        ),
        (
            "scope1-deep-source",
            "x scope=--t\n  b\n    b2\n      s30 drag=--t\n  c follow=--t\n",
            "s30",
        ),
        // `all`, which Chrome 154 does not parse, so it treats it as `none`
        // and answers the first two `missing`, the sixth `s30` and the
        // seventh `inactive`: the specification's answers.
        (
            "all-1",
            "x scope=all\n  s30 drag=--t\n  c follow=--t\n",
            "s30",
        ),
        (
            "all-2",
            "x scope=all\n  s30 drag=--t\n  s70 drag=--t\n  c follow=--t\n",
            "inactive",
        ),
        ("all-0", "x scope=all\n  c follow=--t\n", "missing"),
        (
            "all-0-outer-source",
            "s20 drag=--t\n  x scope=all\n    c follow=--t\n",
            "s20",
        ),
        (
            "all-other-name",
            "x scope=all\n  s30 drag=--u\n  c follow=--t\n",
            "missing",
        ),
        (
            "all-inner-captures-for-named-outer",
            "x scope=--t\n  y scope=all\n    s30 drag=--t\n  c follow=--t\n",
            "inactive",
        ),
        (
            "named-and-all-inner",
            "x scope=--t\n  y scope=all\n    s30 drag=--t\n  s60 drag=--t\n  c follow=--t\n",
            "s60",
        ),
        (
            "all-nested-named",
            "x scope=all\n  y scope=--t\n    s30 drag=--t\n  c follow=--t\n",
            "missing",
        ),
    ];
    let mut wrong = Vec::new();
    for (case, tree, want) in cases {
        let got = build(tree).answer();
        if got != want {
            wrong.push(format!("{case}: {got}, not {want}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

fn drag(value: &str) -> Op {
    Op::SetStyle {
        id: 0,
        patch: rows(&[&format!("drag={value}")]),
    }
}

fn on(id: u32, op: Op) -> Op {
    match op {
        Op::SetStyle { patch, .. } => Op::SetStyle { id, patch },
        other => other,
    }
}

#[test]
fn a_commit_that_moves_a_scope_re_resolves_and_names_the_consumer() {
    let mut t = build("x scope=--t\n  a drag=--t\n  box\n  c follow=--t\n");
    let c = t.k.node(t.id("c")).unwrap().key;
    assert_eq!(t.answer(), "a");
    // Inserted: a second declaration under the scope makes it inactive.
    let (x, a, bx) = (t.id("x"), t.id("a"), t.id("box"));
    let r = t.apply(&[
        Op::CreateView {
            id: 50,
            node_type: NodeType::View,
        },
        on(50, drag("--t")),
        Op::SetChildren {
            id: bx,
            children: vec![50],
        },
    ]);
    t.labels.push(("n".into(), 50));
    assert_eq!(t.answer(), "inactive");
    assert_eq!(r.timelines, vec![c], "the untouched consumer is named");
    // Moved out of the scope: `a` is the one again.
    let r = t.apply(&[
        Op::SetChildren {
            id: bx,
            children: vec![],
        },
        Op::SetChildren {
            id: 1,
            children: vec![x, 50],
        },
    ]);
    assert_eq!((t.answer().as_str(), r.timelines), ("a", vec![c]));
    // Removed: none under the scope, an inactive timeline.
    let r = t.apply(&[
        Op::SetChildren {
            id: x,
            children: vec![bx, t.id("c")],
        },
        Op::DestroyView { id: a },
    ]);
    assert_eq!((t.answer().as_str(), r.timelines), ("inactive", vec![c]));
    // A row set elsewhere: the moved node takes the name back into scope
    // as a drag timeline of another name, then of this one.
    let r = t.apply(&[Op::SetChildren {
        id: bx,
        children: vec![50],
    }]);
    assert_eq!((t.answer().as_str(), r.timelines), ("n", vec![c]));
    let r = t.apply(&[on(50, drag("--u"))]);
    assert_eq!((t.answer().as_str(), r.timelines), ("inactive", vec![c]));
    // The scope's own row cleared: no timeline in scope.
    let r = t.apply(&[Op::SetStyle {
        id: x,
        patch: rows(&["scope=none"]),
    }]);
    assert_eq!((t.answer().as_str(), r.timelines), ("missing", vec![c]));
    // A commit that changes nothing a name finds names no one; one that
    // touches the consumer itself sends its rows anyway, as created or
    // touched nodes' rows always are.
    let r = t.apply(&[on(50, drag("--v"))]);
    assert!(r.timelines.is_empty());
    let r = t.apply(&[Op::SetStyle {
        id: t.id("c"),
        patch: rows(&["follow=--v"]),
    }]);
    assert!(r.timelines.is_empty() && r.touched.contains(&c));
    assert_eq!(t.answer(), "missing", "--v is declared in no ancestor");
}

/// The engine hears the new source in the sync of the commit that moved
/// it, and the consumer follows that node's hold, not the other's.
#[test]
fn the_engine_follows_the_node_a_commit_resolves() {
    let fade = || {
        let mut s = StyleProps::default();
        s.animation =
            crate::keyframed("fade 1s linear both @keyframes fade{from{opacity:1}to{opacity:0}}");
        s.mask.set(StyleId::Animation);
        Box::new(s)
    };
    let mut t = build("row1 scope=--swipe\n  a drag=--swipe\n  c follow=--swipe\nrow2 scope=--swipe\n  b drag=--swipe\n");
    let c = t.id("c");
    let r = t.apply(&[Op::SetStyle {
        id: c,
        patch: fade(),
    }]);
    let mut e = Engine::new();
    for id in [1, t.id("row1"), t.id("a"), c, t.id("row2"), t.id("b")] {
        let mut sync = exact_kernel::MotionSync::default();
        t.k.motion_sync_node(t.k.node(id).unwrap().key, &mut sync);
        sync.apply(&mut e).unwrap();
    }
    t.k.motion_sync(&r).apply(&mut e).unwrap();
    let hold = |e: &mut Engine, node: u64, y: f64| {
        let h = e
            .begin_hold(node, Property::Translate, e.now(), None)
            .unwrap()
            .unwrap();
        e.update_hold(h.token, e.now(), Value::new(0.0, y)).unwrap();
    };
    let shown = |e: &mut Engine, t: &Tree| {
        e.frame();
        e.sampled_value(t.node("c"), Property::Opacity).unwrap().x
    };
    hold(&mut e, t.node("a"), 25.0);
    hold(&mut e, t.node("b"), 75.0);
    assert!((shown(&mut e, &t) - 0.75).abs() < 1e-9, "row 1's own card");
    // Moved into row 2: that row's card drives it, in this commit's sync.
    let r = t.apply(&[
        Op::SetChildren {
            id: t.id("row1"),
            children: vec![t.id("a")],
        },
        Op::SetChildren {
            id: t.id("row2"),
            children: vec![t.id("b"), c],
        },
    ]);
    t.k.motion_sync(&r).apply(&mut e).unwrap();
    assert!((shown(&mut e, &t) - 0.25).abs() < 1e-9, "row 2's card");
}
