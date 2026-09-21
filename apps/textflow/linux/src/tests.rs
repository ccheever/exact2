//! @ref LLP 1043.000 §3 D7/D8 — the actual app, clock, glyphs and CPU pixels.
use exact_kernel::{PropId, ViewId};
use exact_linux::{presenter::PainterChoice, Presenter};
use std::time::{Duration, Instant};
use textflow_data::Textflow;
fn boot(wrap: bool) -> Presenter<Textflow> {
    let mut p = boot_size(wrap, (960., 900.));
    p.tap(id(&p, "scene-ball")).unwrap();
    p.frame();
    p
}
pub(super) fn boot_size(wrap: bool, size: (f32, f32)) -> Presenter<Textflow> {
    let text = include_str!("../../app.contract");
    let text = if wrap {
        text.to_owned()
    } else {
        text.replace("wrap-flow=\"both\"", "wrap-flow=\"auto\"")
    };
    let plan = contract::compile(&text).unwrap();
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        Textflow::default(),
        size,
        1.,
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/..")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.frame();
    p
}
pub(super) fn id(p: &Presenter<Textflow>, name: &str) -> ViewId {
    let k = p.host().kernel();
    k.arena()
        .iter_live()
        .find_map(|slot| {
            let n = k.node_by_key(k.arena().key(slot))?;
            (n.props.str(PropId::TestId) == Some(name)).then_some(n.id)
        })
        .unwrap()
}
fn avoids(p: &Presenter<Textflow>) -> bool {
    let para = id(p, "ball-prose");
    let a = p.host().kernel().node(para).unwrap().frame;
    let b = p.host().kernel().node(id(p, "ball")).unwrap().frame;
    let (cx, cy, r) = (
        b.x + b.width / 2. - a.x,
        b.y + b.height / 2. - a.y,
        b.width / 2. + 9.9,
    );
    let paragraph = p.paragraph(para).unwrap();
    // Test actual painted glyph advance boxes, including the ordinary layout in
    // the negative control. An empty fragment list alone cannot pass the test.
    let mut glyphs = 0;
    for run in paragraph.layout_runs() {
        for g in run.glyphs {
            glyphs += 1;
            let dx = (cx - cx.clamp(g.x, g.x + g.w.max(0.))).abs();
            let dy = (cy - cy.clamp(run.line_top, run.line_top + run.line_height)).abs();
            if dx * dx + dy * dy < r * r {
                return false;
            }
        }
    }
    glyphs > 100
}
#[test]
fn app_clock_flows_around_ball_and_auto_is_a_real_negative_control() {
    for wrap in [true, false] {
        let mut p = boot(wrap);
        for time in [16., 960., 2560.] {
            assert!(p.clock(time).1.is_none());
            p.frame();
            assert_eq!(avoids(&p), wrap, "clock {time}, wrap {wrap}");
            let para = p.paragraph(id(&p, "ball-prose")).unwrap();
            assert_eq!(!para.fragments().is_empty(), wrap);
        }
    }
}
#[test]
fn damage_is_local_and_pixels_equal_a_fresh_full_repaint() {
    let mut p = boot(true);
    let old = p.frame();
    assert_eq!(
        std::sync::Arc::strong_count(&old),
        2,
        "display and repaint share pixels"
    );
    let old_pixels = std::sync::Arc::downgrade(&old);
    assert!(p.clock(176.).1.is_none());
    let partial = p.frame();
    assert_eq!(
        std::sync::Arc::strong_count(&old),
        1,
        "history retires the prior frame"
    );
    assert_eq!(std::sync::Arc::strong_count(&partial), 2);
    let damage = p.damage_rects().to_vec();
    assert!(!damage.is_empty(), "flow_changed must reach CPU damage");
    assert!(damage
        .iter()
        .all(|r| r.0 > 100. && r.1 > 200. && r.0 + r.2 < 850. && r.1 + r.3 < 810.));
    let full = p.frame();
    assert!(
        partial.data() == full.data(),
        "clipped replay differs from full repaint"
    );
    let mut changed = 0;
    for y in 0..900 {
        for x in 0..960 {
            let offset = (y * 960 + x) * 4;
            if old.data()[offset..offset + 4] != partial.data()[offset..offset + 4] {
                changed += 1;
                assert!(damage.iter().any(|&(a, b, w, h)| (x as f32) >= a
                    && (x as f32) < a + w
                    && (y as f32) >= b
                    && (y as f32) < b + h));
            }
        }
    }
    assert!(changed > 100);
    drop(old);
    assert!(old_pixels.upgrade().is_none(), "no hidden pixel history");
    let json = p.layout_json(Some(id(&p, "ball-prose")), false);
    assert!(json.contains("\"fragments\":[{\"start\":0,"));
    assert!(p
        .host()
        .agent("{\"op\":\"tree\"}")
        .contains("The river was the first thing"));
}
// @ref LLP 1043.000 §3 D4/D7 — scrolling cannot reuse stale exclusion ink.
#[test]
fn scrolling_flow_repaints_changed_exclusions_and_reuses_ordinary_text() {
    let mut p = boot_size(true, (960., 500.));
    p.tap(id(&p, "scene-ball")).unwrap();
    p.frame();
    for (delta, time) in [(80., 960.), (-40., 1760.), (120., 2560.)] {
        p.wheel_at(500., 350., 0., delta);
        let before = p.frame();
        assert!(p.page().1 > 0., "exercise the scrolled full-repaint path");
        let para = id(&p, "ball-prose");
        let old_flow = p.paragraph(para).unwrap().fragments().to_vec();
        let ordinary: Vec<_> = p
            .host()
            .kernel()
            .arena()
            .iter_live()
            .filter_map(|slot| {
                let node = p
                    .host()
                    .kernel()
                    .node_by_key(p.host().kernel().arena().key(slot))?;
                let paragraph = p.paragraph(node.id)?;
                paragraph
                    .fragments()
                    .is_empty()
                    .then(|| (node.id, std::ptr::from_ref(paragraph)))
            })
            .collect();
        assert!(
            !ordinary.is_empty(),
            "ordinary visible text is the cache control"
        );
        assert!(p.clock(time).1.is_none());
        let changed = p.frame();
        assert!(
            p.damage_rects().is_empty(),
            "scrolling falls back to full repaint"
        );
        assert_ne!(
            old_flow,
            p.paragraph(para).unwrap().fragments(),
            "clock {time}"
        );
        // Compare the accepted flow and painted pixels below. The older glyph
        // advance-box helper also counts hanging spaces, which are not ink.
        assert!(
            before
                .data()
                .iter()
                .zip(changed.data())
                .filter(|(a, b)| a != b)
                .count()
                > 100
        );
        for (view, previous) in ordinary {
            if let Some(current) = p.paragraph(view) {
                assert!(
                    std::ptr::eq(previous, current),
                    "ordinary text must keep its layout"
                );
            }
        }
        let mut fresh = boot_size(true, (960., 500.));
        fresh.tap(id(&fresh, "scene-ball")).unwrap();
        fresh.frame();
        fresh.wheel_at(500., 350., 0., p.page().1);
        assert!(fresh.clock(time).1.is_none());
        assert!(
            changed.data() == fresh.frame().data(),
            "scroll + flow must match fresh pixels"
        );
    }
}

#[test]
fn ball_cpu_frame_cost() {
    let mut p = boot(true);
    for n in 1..=8 {
        p.clock(n as f64 * 16.);
        p.frame();
    }
    let before = (p.text().borrow().flowing, p.text().borrow().flow_walk);
    let mut commit = Duration::ZERO;
    let mut paint = Duration::ZERO;
    let frames = 120;
    for n in 9..9 + frames {
        let start = Instant::now();
        assert!(p.clock(n as f64 * 16.).1.is_none());
        commit += start.elapsed();
        let start = Instant::now();
        p.frame();
        paint += start.elapsed();
    }
    let engine = p.text().borrow();
    let layout = engine.flowing - before.0;
    let walk = engine.flow_walk - before.1;
    let us = |d: Duration| d.as_secs_f64() * 1e6 / frames as f64;
    println!("M3 ball CPU 960x900 @1x, {frames} frames: {:.1} us/frame; commit+kernel {:.1}; flow+glyph placement {:.1} (walker {:.1}); painting/replay {:.1}", us(commit+paint), us(commit), us(layout), us(walk), us(paint.saturating_sub(layout)));
}

#[test]
fn clock_bounces_instead_of_sticking_and_pause_freezes_geometry() {
    let mut p = boot(true);
    let ball = id(&p, "ball");
    assert!(p.clock(1800.).1.is_none());
    let right = p.host().kernel().node(ball).unwrap().frame.x;
    assert!(p.clock(3200.).1.is_none());
    let left = p.host().kernel().node(ball).unwrap().frame.x;
    assert!(left < right - 100., "the ball must leave the right wall");
    p.tap(id(&p, "pause")).unwrap();
    let frozen = p.host().kernel().node(ball).unwrap().frame;
    assert!(p.clock(4800.).1.is_none());
    assert_eq!(p.host().kernel().node(ball).unwrap().frame, frozen);
}
