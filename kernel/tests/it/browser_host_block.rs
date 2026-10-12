//! A containing block a host gives a subtree (LLP 1115.002 D8.2 and its
//! proof, D8.3), against literal Chrome.
//!
//! Chrome 155, 2026-10-12, by the method of `browser_cases.rs` and
//! `browser_position.rs`: the page is plain HTML/CSS inside an 800 by 600
//! box (the kernel's offer), its outer box the kernel root, id 1. The same
//! method was first run over `browser_containing_block.tsv` and gave that
//! fixture's 155 cases unchanged.
//!
//! A node with a block is taken out of the page and put, alone, in an
//! element of the block's size (`position: absolute; width; height`):
//! Chrome lays the same subtree out inside it. As a root it is `display:
//! flow-root` unless it names a display and `position: relative` unless it
//! names one. On an axis where the block is a maximum, it gets the CSS the
//! document names, a modal `dialog`'s: that axis's insets `auto` and
//! automatic margins zero, an automatic size `fit-content`, and a
//! `max-width` or `max-height` that keeps its border box at the maximum
//! (less its padding and border where it is `content-box`, and no more than
//! its own). Frames of the subtree are measured from that element, the
//! page's from the 800px box. A block's used size is the given length where
//! it is the size, and where it is a maximum the root's far edge plus its
//! end margin, at most the maximum.
//!
//! A fixture line is `name`, the root's declarations, the nodes
//! (`id>parent>declarations>`, joined by ` & `), the blocks (`id:WxH`, a
//! maximum written `<=N`, joined by spaces; `-` for none), Chrome's frames
//! (`id=x,y,w,h`) and the blocks' used sizes (`id=w,h`), separated by tabs.
use crate::browser_cases::{css_rows, lay_out_with, mismatches, props, Rows};
use exact_kernel::{
    BlockAxis, HostBlock, Kernel, MonospaceMeasurer, NodeFlags, Offer, StyleId, StyleValue,
};

const OFFER: Offer = Offer::definite(800.0, 600.0);

/// Rows for the declarations `css_rows` has no row for.
fn rows(css: &str) -> Rows {
    let mut out = Rows::new();
    for decl in css.split(';').filter(|d| !d.is_empty()) {
        match decl.split_once(':').unwrap() {
            ("flex-basis", value) => {
                let percent = value.strip_suffix('%').unwrap().parse().unwrap();
                out.push((StyleId::FlexBasis, StyleValue::Percent(percent)));
            }
            _ => out.extend(css_rows(decl)),
        }
    }
    out
}

struct Case<'a> {
    name: &'a str,
    root: &'a str,
    nodes: Vec<(u32, u32, &'a str)>,
    blocks: Vec<(u32, HostBlock)>,
    frames: Vec<(u32, [f32; 4])>,
    used: Vec<(u32, [f32; 2])>,
}

fn numbers<const N: usize>(field: &str) -> Vec<(u32, [f32; N])> {
    field
        .split(' ')
        .filter(|part| *part != "-")
        .map(|part| {
            let (id, list) = part.split_once('=').unwrap();
            let v: Vec<f32> = list.split(',').map(|x| x.parse().unwrap()).collect();
            (id.parse().unwrap(), std::array::from_fn(|i| v[i]))
        })
        .collect()
}

fn cases(fixture: &str) -> Vec<Case<'_>> {
    let axis = |text: &str| match text.strip_prefix("<=") {
        Some(most) => BlockAxis::AtMost(most.parse().unwrap()),
        None => BlockAxis::Is(text.parse().unwrap()),
    };
    fixture
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let field: Vec<_> = line.split('\t').collect();
            let nodes = field[2]
                .split(" & ")
                .map(|node| {
                    let part: Vec<_> = node.splitn(4, '>').collect();
                    (part[0].parse().unwrap(), part[1].parse().unwrap(), part[2])
                })
                .collect();
            let blocks = field[3]
                .split(' ')
                .filter(|part| *part != "-")
                .map(|part| {
                    let (id, size) = part.split_once(':').unwrap();
                    let (width, height) = size.split_once('x').unwrap();
                    let block = HostBlock {
                        width: axis(width),
                        height: axis(height),
                    };
                    (id.parse().unwrap(), block)
                })
                .collect();
            Case {
                name: field[0],
                root: field[1],
                nodes,
                blocks,
                frames: numbers(field[4]),
                used: numbers(field[5]),
            }
        })
        .collect()
}

/// Every live node's frame, scroll extent and whether it is hidden, as bits.
fn published(k: &Kernel) -> Vec<(u32, [u32; 6], bool)> {
    let arena = k.arena();
    let mut out: Vec<_> = arena
        .iter_live()
        .map(|slot| {
            let (f, c) = (arena.frame(slot), arena.content(slot));
            let bits = [f.x, f.y, f.width, f.height, c.0, c.1].map(f32::to_bits);
            let hidden = arena.flags(slot).has(NodeFlags::HIDDEN);
            (arena.local_id(slot), bits, hidden)
        })
        .collect();
    out.sort_by_key(|row| row.0);
    out
}

fn fresh(k: &Kernel) -> Vec<(u32, [u32; 6], bool)> {
    let mut fresh = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    fresh.compute_layout(1, OFFER).unwrap();
    published(&fresh)
}

/// The fixtures of D8.3 and the semantics of D8.2: a percentage child, an
/// absolutely positioned child, a form whose height depends on its width,
/// content that overflows a maximum, a block inside a block, and a block
/// that goes away. Each case is laid out without its blocks, then given
/// them (a block that appears), then has them taken back.
#[test]
fn subtrees_in_host_blocks_match_chrome() {
    let mut wrong = Vec::new();
    let all = cases(include_str!("fixtures/browser_host_block.tsv"));
    assert!(all.len() >= 20 && all.iter().filter(|c| c.blocks.is_empty()).count() >= 3);
    for case in &all {
        let nodes = case
            .nodes
            .iter()
            .map(|(id, parent, css)| (*id, *parent, rows(css)));
        let mut k = lay_out_with(props(&rows(case.root)), nodes.collect(), &[], &[]);
        let page = published(&k);
        for (id, block) in &case.blocks {
            k.set_host_block(*id, Some(*block)).unwrap();
        }
        k.compute_layout(1, OFFER).unwrap();
        wrong.extend(mismatches(case.name, &k, &case.frames));
        for (id, want) in &case.used {
            let got = k.host_block_size(*id).unwrap();
            if (got.0 - want[0]).abs() > 0.01 || (got.1 - want[1]).abs() > 0.01 {
                wrong.push(format!(
                    "{} #{id}: used size {got:?}, Chrome {want:?}",
                    case.name
                ));
            }
        }
        if case.used.len() != case.blocks.len() {
            wrong.push(format!(
                "{}: a block has no used size in the fixture",
                case.name
            ));
        }
        // What the engine kept from the page's layout changes nothing: a
        // kernel that never laid out without the blocks publishes the same.
        if published(&k) != fresh(&k) {
            wrong.push(format!(
                "{}: a block that appears differs from one there from the start",
                case.name
            ));
        }
        for (id, _) in &case.blocks {
            k.set_host_block(*id, None).unwrap();
            assert_eq!(k.host_block_size(*id), None);
        }
        k.compute_layout(1, OFFER).unwrap();
        if published(&k) != page {
            wrong.push(format!(
                "{}: with its blocks taken back the page is not as it was",
                case.name
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "{}\n{} wrong",
        wrong.join("\n"),
        wrong.len()
    );
}
