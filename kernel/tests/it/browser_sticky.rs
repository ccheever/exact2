//! `position: sticky` (LLP 1083) against literal Chrome: layout places the
//! box as a relative one with no offset, and [`StickyConstraint::offset`]
//! moves it as its scroller scrolls.
//!
//! Chrome 154, 2026-10-03, by the method of `browser_cases.rs`, plus
//! `scrollbar-width: none`. Each line is a case's name, the root's
//! declarations, its nodes (`id>parent>declarations`, joined by ` & `), the
//! scroller's id, and one measurement per scroll offset (`x,y:` then each
//! sticky box's border box in the scroller's border-box space at offset
//! zero, `id=x,y,w,h`), joined by `|`. The offsets are the ones Chrome
//! clamped the asked ones to.
use crate::browser_cases::{css_rows, lay_out_with, props, Rows};

#[test]
fn sticky_boxes_land_where_chrome_puts_them() {
    let mut wrong = Vec::new();
    for line in include_str!("fixtures/browser_sticky.tsv").lines() {
        let field: Vec<_> = line.split('\t').collect();
        let nodes: Vec<(u32, u32, Rows)> = field[2]
            .split(" & ")
            .map(|node| {
                let part: Vec<_> = node.splitn(3, '>').collect();
                (
                    part[0].parse().unwrap(),
                    part[1].parse().unwrap(),
                    css_rows(part[2]),
                )
            })
            .collect();
        // The outer box is Chrome's `flow-root`; every case's scroller is a
        // formatting context of its own, so the kernel's `block` is the same.
        let root = field[1].replace("flow-root", "block");
        let k = lay_out_with(props(&css_rows(&root)), nodes, &[], &[]);
        let scroller: u32 = field[3].parse().unwrap();
        for at in field[4].split('|') {
            let (scroll, boxes) = at.split_once(':').unwrap();
            let s: Vec<f32> = scroll.split(',').map(|v| v.parse().unwrap()).collect();
            for b in boxes.split(' ') {
                let (id, rect) = b.split_once('=').unwrap();
                let id: u32 = id.parse().unwrap();
                let want: Vec<f32> = rect.split(',').map(|v| v.parse().unwrap()).collect();
                let c = k.sticky_constraint(k.arena().key_of(id).unwrap()).unwrap();
                assert_eq!(c.scroller, scroller, "{}: {id}'s scroller", field[0]);
                let (dx, dy) = c.offset((s[0], s[1]));
                let n = c.natural;
                let got = [n[0] + dx, n[1] + dy, n[2] - n[0], n[3] - n[1]];
                if got.iter().zip(&want).any(|(g, w)| (g - w).abs() > 0.01) {
                    wrong.push(format!(
                        "{} at {scroll}: {id} {got:?}, Chrome {want:?}",
                        field[0]
                    ));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_box_that_is_not_sticky_has_no_constraint_and_no_scroller_means_the_root() {
    let nodes = vec![
        (2, 1, css_rows("position:relative;top:10px;height:30px")),
        (3, 1, css_rows("position:sticky;top:10px;height:30px")),
    ];
    let k = lay_out_with(props(&css_rows("display:block")), nodes, &[], &[]);
    assert_eq!(k.sticky_constraint(k.arena().key_of(2).unwrap()), None);
    let c = k.sticky_constraint(k.arena().key_of(3).unwrap()).unwrap();
    assert_eq!(c.scroller, 1);
    // Laid out in flow, its inset no offset: right after the relative box.
    assert_eq!(c.natural, [0.0, 30.0, 800.0, 60.0]);
    assert_eq!(c.insets, [Some(10.0), None, None, None]);
}
