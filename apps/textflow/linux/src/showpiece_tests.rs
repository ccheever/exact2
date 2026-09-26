//! @ref LLP 1043.000 §3 D7/D8 — real app coverage, live drag, seek and CPU cost.
use crate::tests::{boot_size, id};
use exact_linux::Presenter;
use std::time::{Duration, Instant};
use textflow_data::Textflow;
const SCENES: [&str; 6] = ["drag", "orbs", "dancer", "ball", "editorial", "polygon"];
fn select(p: &mut Presenter<Textflow>, scene: &str) {
    p.tap(id(p, &format!("scene-{scene}"))).unwrap();
    p.frame();
}
fn prose(scene: &str) -> Vec<String> {
    if scene == "editorial" {
        vec!["editorial-left".into(), "editorial-right".into()]
    } else {
        vec![format!("{scene}-prose")]
    }
}
fn check(p: &Presenter<Textflow>, scene: &str, slack: bool) {
    for name in prose(scene) {
        let view = id(p, &name);
        let node = p.host().kernel().node(view).unwrap();
        assert!(!node.flow_skipped(), "{name}: flow skipped");
        assert!(!node.flow_shapes().is_empty(), "{name}: missing exclusions");
        let para = p.paragraph(view).unwrap();
        let source = node
            .text_runs()
            .iter()
            .map(|r| &*r.text)
            .collect::<String>();
        let mut end = 0;
        let mut slots = vec![];
        assert!(!para.fragments().is_empty(), "{name}: empty output");
        for f in para.fragments() {
            assert_eq!(f.start, end, "{name}: source reordered/lost");
            end = f.end;
            exact_textflow::intervals(
                node.flow_shapes(),
                f.y,
                f.y + para.flow_line_height(),
                node.frame.width,
                0.,
                &mut slots,
            );
            if f.width > 0. {
                assert!(
                    slots
                        .iter()
                        .any(|&(a, b)| f.x >= a - 0.01 && f.x + f.width <= b + 0.01),
                    "{name}: fragment {f:?} intersects shape: {slots:?}"
                );
            }
            assert!(
                f.y + para.flow_line_height() <= node.frame.height + 0.1,
                "{name}: clips at {} of {}",
                f.y + para.flow_line_height(),
                node.frame.height
            );
        }
        assert_eq!(end, source.len(), "{name}: lost trailing bytes");
        let bottom = para.fragments().last().unwrap().y + para.flow_line_height();
        if slack {
            assert!(
                bottom <= node.frame.height * 0.8 + 0.1,
                "{name}: normal type lacks 20% slack: {bottom}/{}",
                node.frame.height
            );
        }
    }
}
fn signature(p: &Presenter<Textflow>, scene: &str) -> Vec<Vec<u32>> {
    prose(scene)
        .iter()
        .map(|name| {
            p.paragraph(id(p, name))
                .unwrap()
                .fragments()
                .iter()
                .flat_map(|f| {
                    [
                        f.start as u32,
                        f.end as u32,
                        f.x.to_bits(),
                        f.y.to_bits(),
                        f.width.to_bits(),
                        f.line,
                    ]
                })
                .collect()
        })
        .collect()
}
#[test]
fn all_scenes_cover_every_byte_once_avoid_all_shapes_and_tolerate_larger_fonts() {
    let mut failures = Vec::new();
    for size in [(960., 900.), (390., 844.)] {
        for scene in SCENES {
            let mut p = boot_size(true, size);
            select(&mut p, scene);
            for large in [false, true] {
                if large {
                    p.tap(id(&p, "type-size")).unwrap();
                }
                for ms in [0., 800., 2400., 15_000.] {
                    // Continue forwards, selecting resets only app time, not host time.
                    select(&mut p, scene);
                    let now = p.host().now();
                    assert!(p.clock(now + ms).1.is_none());
                    p.frame();
                    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        check(&p, scene, !large)
                    }))
                    .is_err()
                    {
                        failures.push(format!("{scene} {size:?} large={large} ms={ms}"));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "all failing cases: {failures:?}");
}
#[test]
fn fresh_boots_at_the_same_clock_have_bit_identical_fragments() {
    for scene in SCENES {
        let mut a = boot_size(true, (960., 900.));
        select(&mut a, scene);
        let mut b = boot_size(true, (960., 900.));
        select(&mut b, scene);
        for time in [16., 800., 2400.] {
            assert!(a.clock(time).1.is_none());
            assert!(b.clock(time).1.is_none());
            a.frame();
            b.frame();
            assert_eq!(
                signature(&a, scene),
                signature(&b, scene),
                "{scene} at {time}"
            );
        }
    }
}
#[test]
fn live_pan_reflows_before_release_and_cancels_when_its_owner_disappears() {
    let mut p = boot_size(true, (960., 900.));
    let before = signature(&p, "drag");
    let orb = id(&p, "orb-1");
    let f = p.host().kernel().node(orb).unwrap().frame;
    assert!(p
        .pointer_down(f.x + f.width / 2., f.y + f.height / 2., 0.)
        .unwrap());
    assert!(p
        .pointer_move(f.x + f.width / 2. + 120., f.y + f.height / 2. + 40., 16.)
        .unwrap());
    p.frame();
    let moved = p.host().kernel().node(orb).unwrap().frame;
    assert_eq!((moved.x - f.x, moved.y - f.y), (120., 40.));
    assert_ne!(signature(&p, "drag"), before);
    check(&p, "drag", true);
    assert!(p
        .pointer_up(moved.x + f.width / 2., moved.y + f.height / 2., 16.)
        .unwrap());
    assert!(p.clock(800.).1.is_none());
    p.frame();
    assert_eq!(p.host().kernel().node(orb).unwrap().frame, moved);
    p.pointer_down(moved.x + f.width / 2., moved.y + f.height / 2., 800.)
        .unwrap();
    select(&mut p, "dancer");
    assert!(!p.pointer_move(600., 600., 816.).unwrap());
    // Worst pointer displacement is clamped by app state, never moves off paper.
    for size in [(960., 900.), (390., 844.)] {
        let mut p = boot_size(true, size);
        let f = p.host().kernel().node(id(&p, "orb-1")).unwrap().frame;
        p.pointer_down(f.x + f.width / 2., f.y + f.height / 2., 0.)
            .unwrap();
        for (x, y, ms) in [(1e10, -1e10, 16.), (-1e10, 1e10, 32.)] {
            p.pointer_move(x, y, ms).unwrap();
            p.frame();
            check(&p, "drag", true);
            let f = p.host().kernel().node(id(&p, "orb-1")).unwrap().frame;
            let q = p.host().kernel().node(id(&p, "drag-prose")).unwrap().frame;
            assert!(f.x >= q.x - 0.01 && f.y >= q.y - 0.01);
            assert!(f.x + f.width <= q.x + q.width + 0.01);
            assert!(f.y + f.height <= q.y + q.height + 0.01);
        }
        p.pointer_up(-1e10, 1e10, 32.).unwrap();
        // A phone's deeper logical page must not leave an off-page/sticky orb on resize.
        assert!(p.resize(960., 900.).is_none());
        p.frame();
        let f = p.host().kernel().node(id(&p, "orb-1")).unwrap().frame;
        let q = p.host().kernel().node(id(&p, "drag-prose")).unwrap().frame;
        assert!(f.y + f.height <= q.y + q.height + 0.01);
        p.pointer_down(f.x + f.width / 2., f.y + f.height / 2., 32.)
            .unwrap();
        p.pointer_move(f.x + f.width / 2. + 5., f.y + f.height / 2. - 5., 48.)
            .unwrap();
        p.frame();
        let moved = p.host().kernel().node(id(&p, "orb-1")).unwrap().frame;
        assert!((moved.y - f.y + 5.).abs() < 0.01);
        check(&p, "drag", true);
    }
}
#[test]
fn removing_exclusions_is_a_real_negative_control_for_each_new_scene() {
    for scene in ["drag", "orbs", "dancer"] {
        let mut p = boot_size(false, (960., 900.));
        select(&mut p, scene);
        assert!(p
            .paragraph(id(&p, &format!("{scene}-prose")))
            .unwrap()
            .fragments()
            .is_empty());
        // check's nonempty coverage/exclusion assertions must fail, never vacuously pass.
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(&p, scene, false)))
                .is_err()
        );
    }
}
#[test]
fn per_scene_cpu_frame_cost() {
    for scene in SCENES {
        let mut p = boot_size(true, (960., 900.));
        select(&mut p, scene);
        for n in 1..=8 {
            p.clock(n as f64 * 16.);
            p.frame();
        }
        let pan_origin = if scene == "drag" {
            let f = p.host().kernel().node(id(&p, "orb-1")).unwrap().frame;
            let xy = (f.x + f.width / 2., f.y + f.height / 2.);
            p.pointer_down(xy.0, xy.1, 128.).unwrap();
            Some(xy)
        } else {
            None
        };
        let before = p.text().borrow().flowing;
        let (mut commit, mut paint) = (Duration::ZERO, Duration::ZERO);
        let frames = 120;
        let mut samples = Vec::with_capacity(frames);
        for n in 9..9 + frames {
            let frame_start = Instant::now();
            let start = Instant::now();
            assert!(p.clock(n as f64 * 16.).1.is_none());
            if let Some((x, y)) = pan_origin {
                p.pointer_move(
                    x + (n as f32 * 0.04).sin() * 100.,
                    y + (n as f32 * 0.04).cos() * 35.,
                    n as f64 * 16.,
                )
                .unwrap();
            }
            commit += start.elapsed();
            let start = Instant::now();
            p.frame();
            paint += start.elapsed();
            samples.push(frame_start.elapsed());
        }
        samples.sort();
        let flow = p.text().borrow().flowing - before;
        let us = |d: Duration| d.as_secs_f64() * 1e6 / frames as f64;
        println!("M6 {scene} 960x900 CPU {frames} frames: {:.1} us/frame; commit+kernel {:.1}; flow {:.1}; paint {:.1}; p95 {:.1}; max {:.1}",us(commit+paint),us(commit),us(flow),us(paint.saturating_sub(flow)), samples[frames*95/100].as_secs_f64()*1e6, samples.last().unwrap().as_secs_f64()*1e6);
    }
}
