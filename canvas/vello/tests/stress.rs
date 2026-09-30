#![cfg(target_vendor = "apple")]
//! The ABI hammered from 8 threads: canvases made, replayed and freed
//! across 32 slots with random lists from the Rust recorder, for a few
//! seconds. Every replay must succeed; the process must not crash.
use exact_canvas::Context2d;
use exact_canvas_vello::surface::{make, IOSurfaceRef};
use exact_canvas_vello::*;
use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Slot {
    canvas: *mut c_void,
    surfaces: [IOSurfaceRef; 2],
    front: Option<usize>,
    size: (u32, u32),
}

// SAFETY: the ABI's canvases and IOSurfaces may move between threads; each
// slot is used under its own mutex.
unsafe impl Send for Slot {}

fn draw(ctx: &Context2d, seed: &mut u64, w: f64, h: f64) {
    let mut rnd = || {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (*seed >> 33) as f64 / (1u64 << 31) as f64
    };
    let ops = [
        "source-over",
        "multiply",
        "destination-out",
        "source-in",
        "copy",
        "xor",
        "lighter",
    ];
    for _ in 0..(rnd() * 20.0) as usize + 1 {
        match (rnd() * 9.0) as u32 {
            0 => ctx.fill_rect(rnd() * w, rnd() * h, rnd() * w, rnd() * h),
            1 => {
                ctx.begin_path();
                let _ = ctx.arc(
                    rnd() * w,
                    rnd() * h,
                    rnd() * 20.0 + 1.0,
                    0.0,
                    std::f64::consts::TAU,
                );
                ctx.fill();
            }
            2 => {
                ctx.set_line_width(rnd() * 8.0 + 0.5);
                ctx.begin_path();
                ctx.move_to(rnd() * w, rnd() * h);
                ctx.bezier_curve_to(
                    rnd() * w,
                    rnd() * h,
                    rnd() * w,
                    rnd() * h,
                    rnd() * w,
                    rnd() * h,
                );
                ctx.stroke();
            }
            3 => ctx.set_fill_style_str(&format!(
                "rgba({},{},{},{})",
                (rnd() * 255.0) as u8,
                (rnd() * 255.0) as u8,
                90,
                rnd()
            )),
            4 => {
                let _ =
                    ctx.set_global_composite_operation(ops[(rnd() * ops.len() as f64) as usize]);
            }
            5 => {
                ctx.save();
                ctx.begin_path();
                ctx.rect(rnd() * w, rnd() * h, w / 2.0, h / 2.0);
                ctx.clip();
            }
            6 => ctx.restore(),
            7 => {
                ctx.set_shadow_color("rgba(0,0,0,0.5)");
                ctx.set_shadow_blur(rnd() * 6.0);
                ctx.set_shadow_offset_x(3.0);
            }
            _ => ctx.clear_rect(rnd() * w, rnd() * h, rnd() * w, rnd() * h),
        }
    }
}

#[test]
fn eight_threads_hammer_the_abi() {
    if ecg_canvas_new(1, 1, 1.0).is_null() {
        eprintln!("no GPU: skipped");
        return;
    }
    let slots: Arc<Vec<Mutex<Option<Slot>>>> =
        Arc::new((0..32).map(|_| Mutex::new(None)).collect());
    let end = Instant::now() + Duration::from_secs(4);
    let replays = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let threads: Vec<_> = (0..8u64)
        .map(|t| {
            let slots = slots.clone();
            let replays = replays.clone();
            std::thread::spawn(move || {
                let mut seed = 0x9e37_79b9_7f4a_7c15u64 ^ (t << 32);
                let mut next = || {
                    seed = seed
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    seed >> 33
                };
                while Instant::now() < end {
                    let i = (next() % 32) as usize;
                    let mut slot = slots[i].lock().unwrap();
                    match next() % 10 {
                        0 => {
                            if let Some(s) = slot.take() {
                                // SAFETY: from ecg_canvas_new, not used after.
                                unsafe { ecg_canvas_free(s.canvas) };
                                s.surfaces.iter().for_each(|x| make::release(*x));
                            }
                        }
                        _ => {
                            if slot.is_none() {
                                let (w, h) =
                                    (40 + (next() % 200) as u32, 30 + (next() % 150) as u32);
                                let c = ecg_canvas_new(w, h, 2.0);
                                assert!(!c.is_null());
                                *slot = Some(Slot {
                                    canvas: c,
                                    surfaces: [make::surface(w, h), make::surface(w, h)],
                                    front: None,
                                    size: (w, h),
                                });
                            }
                            let s = slot.as_mut().unwrap();
                            let ctx = Context2d::new();
                            let mut sd = next();
                            draw(
                                &ctx,
                                &mut sd,
                                f64::from(s.size.0) / 2.0,
                                f64::from(s.size.1) / 2.0,
                            );
                            let lists = ctx.take_lists();
                            let ptrs: Vec<*const u8> = lists.iter().map(|l| l.as_ptr()).collect();
                            let lens: Vec<usize> = lists.iter().map(|l| l.len()).collect();
                            let back = s.front.map_or(0, |f| 1 - f);
                            let prev = s.front.map_or(std::ptr::null_mut(), |f| s.surfaces[f].0);
                            // SAFETY: live canvas, surfaces and lists.
                            let r = unsafe {
                                ecg_canvas_replay(
                                    s.canvas,
                                    s.surfaces[back].0,
                                    prev,
                                    ptrs.as_ptr(),
                                    lens.as_ptr(),
                                    lists.len(),
                                    std::ptr::null(),
                                )
                            };
                            assert_eq!(r, 0, "replay failed");
                            s.front = Some(back);
                            let _ = ecg_memory();
                            replays.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                    if next() % 97 == 0 {
                        ecg_trim();
                    }
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    for s in slots.iter() {
        if let Some(s) = s.lock().unwrap().take() {
            // SAFETY: from ecg_canvas_new.
            unsafe { ecg_canvas_free(s.canvas) };
            s.surfaces.iter().for_each(|x| make::release(*x));
        }
    }
    let n = replays.load(std::sync::atomic::Ordering::Relaxed);
    eprintln!("{n} replays from 8 threads");
    assert!(n > 100);
}

/// Each replay's pixels are in its target when the replay returns: 8
/// threads, 4 canvases each, three surfaces a canvas used in turn, every
/// frame an opaque cover in a colour of its own, the corner read back at
/// once (the host shows the target right after the call).
#[test]
fn every_replay_lands_in_its_target_before_it_returns() {
    if ecg_canvas_new(1, 1, 1.0).is_null() {
        eprintln!("no GPU: skipped");
        return;
    }
    let bad = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let in_use = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let threads: Vec<_> = (0..8u32)
        .map(|t| {
            let bad = bad.clone();
            let in_use = in_use.clone();
            std::thread::spawn(move || {
                let (w, h) = (180u32, 120u32);
                for life in 0..6u32 {
                    let mut canvases: Vec<(*mut c_void, [IOSurfaceRef; 3], Option<usize>)> = (0..4)
                        .map(|_| (ecg_canvas_new(w, h, 3.0), [make::surface(w, h), make::surface(w, h), make::surface(w, h)], None))
                        .collect();
                    for round in 0..20u32 {
                        for (c, (canvas, s, front)) in canvases.iter_mut().enumerate() {
                            let rgb = [(round * 11 + c as u32 * 29 + t * 7) % 256, (life * 60 + c as u32 * 13) % 256, (round * 3 + t) % 256];
                            let ctx = Context2d::new();
                            ctx.set_fill_style_str(&format!("rgb({},{},{})", rgb[0], rgb[1], rgb[2]));
                            ctx.fill_rect(0.0, 0.0, 60.0, 40.0);
                            ctx.save();
                            ctx.begin_path();
                            ctx.rect(2.0, 2.0, 56.0, 28.0);
                            ctx.clip();
                            for i in 0..20 {
                                ctx.set_fill_style_str("rgba(255,40,0,0.8)");
                                ctx.begin_path();
                                let _ = ctx.arc(f64::from((i * 7 + round) % 60), f64::from((i * 5 + round) % 40), 3.0, 0.0, std::f64::consts::TAU);
                                ctx.fill();
                            }
                            ctx.restore();
                            ctx.stroke_rect(4.0, 4.0, 20.0, 10.0);
                            let lists = ctx.take_lists();
                            let ptrs: Vec<*const u8> = lists.iter().map(|l| l.as_ptr()).collect();
                            let lens: Vec<usize> = lists.iter().map(|l| l.len()).collect();
                            let back = front.map_or(0, |f| (f + 1) % 3);
                            let prev = front.map_or(std::ptr::null_mut(), |f| s[f].0);
                            // SAFETY: live canvas, surfaces and lists.
                            let r = unsafe {
                                ecg_canvas_replay(*canvas, s[back].0, prev, ptrs.as_ptr(), lens.as_ptr(), lists.len(), std::ptr::null())
                            };
                            assert_eq!(r, 0);
                            *front = Some(back);
                            if let Some(f) = front.map(|f| (f + 1) % 3) {
                                if make::in_use(s[f]) {
                                    in_use.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                }
                            }
                            let px = make::pixels(s[back], w, h);
                            let at = ((h - 1) * w + (w - 1)) as usize * 4;
                            let got = [px[at + 2], px[at + 1], px[at]];
                            if got.map(u32::from) != rgb {
                                bad.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                eprintln!("thread {t} life {life} canvas {c} round {round}: corner {got:?}, drew {rgb:?}");
                            }
                        }
                    }
                    for (canvas, s, _) in canvases.drain(..) {
                        // SAFETY: from ecg_canvas_new.
                        unsafe { ecg_canvas_free(canvas) };
                        s.iter().for_each(|x| make::release(*x));
                    }
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    assert_eq!(
        bad.load(std::sync::atomic::Ordering::Relaxed),
        0,
        "replays that returned before their pixels landed"
    );
    // The host picks a target only among surfaces not `IOSurfaceIsInUse`
    // (Core Animation's use count): the module must not hold them in use.
    assert_eq!(
        in_use.load(std::sync::atomic::Ordering::Relaxed),
        0,
        "surfaces the module left in use between replays"
    );
}
