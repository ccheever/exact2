//! Mouse look on the live clock, by tick and display rate: the rivals diary's
//! `mouse_latency_by_tick_and_display_rate` (limit 3) over a minimal yaw camera.
//! The host calls `advance` every frame (`Clock::Live` with the frame period),
//! mouse events arrive between frames, and the drawn yaw is the last two ticks
//! blended by `Sim::alpha` (before) plus, with `MouseLook`, the unshown motion
//! at the game's rate (after). Delays run from the event to the frame that
//! shows it; judder is the coefficient of variation of the per-frame turn under
//! a steady 2,000 pt/s, 1 kHz mouse.
use exact_game::*;

const SENS: f32 = 0.0025;

#[derive(Default, Resource)]
struct Yaw(f32);
struct Look<const HZ: u32>;
impl<const HZ: u32> Game for Look<HZ> {
    const ID: &'static str = "look";
    const HZ: u32 = HZ;
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Yaw(0.));
        let look = MouseLook {
            yaw_per_point: -SENS,
            pitch_per_point: -SENS,
            pitch_limit: 1.4,
        };
        w.spawn_named("camera", (Transform::default(), Camera::default(), look));
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        let delta = input.pointer().map_or(Vec2::ZERO, |p| p.delta);
        let yaw = {
            let mut yaw = w.resource_mut::<Yaw>();
            yaw.0 -= delta.x * SENS;
            yaw.0
        };
        let camera = w.named("camera").unwrap();
        w.get_mut::<Transform>(camera).unwrap().rotation = Quat::from_rotation_y(yaw);
    }
}

fn quantile(mut v: Vec<f64>, q: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * q).round() as usize]
}

struct Row {
    first: (f64, f64),
    whole: (f64, f64),
    judder: f64,
}

fn live<const HZ: u32>(display_hz: f64, present: bool) -> Row {
    let mut sim = Sim::<Look<HZ>>::new(()).unwrap();
    sim.viewport(1280., 720.);
    let period = 1000. / display_hz;
    sim.frame_period(period);
    let mut yaws: Vec<f32> = vec![0.];
    let frame = |sim: &mut Sim<Look<HZ>>, t: f64, yaws: &mut Vec<f32>| {
        sim.advance_with(t, Clock::Live, |w, _| yaws.push(w.resource::<Yaw>().0));
        let n = yaws.len();
        let (a, b) = (yaws[n.saturating_sub(2)], yaws[n - 1]);
        let drawn = a + (b - a) * sim.alpha();
        if present {
            drawn - sim.unshown_motion().x * SENS
        } else {
            drawn
        }
    };
    let (mut x, mut t, mut seed) = (640f32, 0., 0x2545_F491u32);
    let mut mouse = |sim: &mut Sim<Look<HZ>>, dx: f32, at_ms: f64| {
        x += dx;
        sim.input(InputEvent::Pointer {
            id: 1,
            phase: PointerPhase::Move,
            x,
            y: 360.,
            dx,
            dy: 0.,
            buttons: 0,
            at_ms,
        });
    };
    mouse(&mut sim, 0., 0.);
    for _ in 0..120 {
        frame(&mut sim, t, &mut yaws);
        t += period;
    }
    let (mut first, mut whole) = (Vec::new(), Vec::new());
    for _ in 0..200 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let at = t - period + period * f64::from(seed >> 8) / f64::from(1u32 << 24);
        let shown = frame(&mut sim, t - period + 1e-6, &mut yaws);
        mouse(&mut sim, 40., at);
        let goal = shown - 40. * SENS;
        let (mut f, mut w, mut ft) = (None, None, t);
        for _ in 0..40 {
            let now = frame(&mut sim, ft, &mut yaws);
            if f.is_none() && (now - shown).abs() > 1e-6 {
                f = Some(ft - at);
            }
            if (now - goal).abs() < 1e-5 {
                w = Some(ft - at);
                break;
            }
            ft += period;
        }
        first.push(f.expect("the turn shows"));
        whole.push(w.expect("the turn completes"));
        t = ft + 10. * period;
        frame(&mut sim, t - period, &mut yaws);
    }
    let (mut turns, mut last, start) = (Vec::new(), frame(&mut sim, t, &mut yaws), t);
    let mut event = start;
    while t < start + 2000. {
        t += period;
        while event < t {
            event += 1.;
            mouse(&mut sim, 2., event);
        }
        let now = frame(&mut sim, t, &mut yaws);
        if t > start + 200. {
            turns.push(f64::from(now - last));
        }
        last = now;
    }
    let mean = turns.iter().sum::<f64>() / turns.len() as f64;
    let var = turns.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / turns.len() as f64;
    Row {
        first: (quantile(first.clone(), 0.5), quantile(first, 0.95)),
        whole: (quantile(whole.clone(), 0.5), quantile(whole, 0.95)),
        judder: var.sqrt() / mean.abs(),
    }
}

/// Before (the blended ticks) and after (with `MouseLook`'s unshown motion):
/// with it a turn completes by the frame after the event on every rate pair,
/// and the per-frame turn varies only by the 1 kHz mouse's own quantization.
#[test]
fn mouse_look_shows_at_the_next_frame_without_aliasing_judder() {
    for display in [60., 120., 144.] {
        for (hz, before, after) in [
            (30, live::<30>(display, false), live::<30>(display, true)),
            (60, live::<60>(display, false), live::<60>(display, true)),
            (120, live::<120>(display, false), live::<120>(display, true)),
            (240, live::<240>(display, false), live::<240>(display, true)),
        ] {
            println!(
                "display {display:>3} Hz, tick {hz:>3} Hz: whole turn p50/p95 {:5.1}/{:5.1} -> {:5.1}/{:5.1} ms; first change p50 {:5.1} -> {:5.1}; judder {:.3} -> {:.3}",
                before.whole.0, before.whole.1, after.whole.0, after.whole.1,
                before.first.0, after.first.0, before.judder, after.judder
            );
            let frame = 1000. / display;
            assert!(
                after.whole.1 <= frame + 0.01,
                "{display}/{hz}: {}",
                after.whole.1
            );
            assert!(after.whole.1 <= before.whole.1 + 1e-9);
            // 1 kHz quantization alone: 0.03 at 60 Hz, 0.06 at 120, 0.07 at 144.
            assert!(after.judder <= 0.08, "{display}/{hz}: {}", after.judder);
        }
    }
}
