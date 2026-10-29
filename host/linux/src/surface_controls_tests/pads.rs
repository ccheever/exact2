//! Gamepads through the presenter (LLP 1127.003 D2, D9).
use super::*;

#[test]
fn controller_polling_needs_an_input_canvas_but_survives_editor_focus() {
    let (mut p, path) = fixture();
    assert!(p.has_gamepad_input());
    let editor = find(&p, "editor");
    p.set_focus(Some(editor), 0.);
    assert!(
        p.has_gamepad_input(),
        "focus may return to a game at any time"
    );
    // The probe's canvas 98 paints but does not accept input.
    for canvas in p.surfaces.canvases.values_mut() {
        canvas.id = 98;
    }
    assert!(!p.has_gamepad_input());
    let canvases = std::mem::take(&mut p.surfaces.canvases);
    assert!(
        !p.has_gamepad_input(),
        "ordinary app windows need no polling"
    );
    p.surfaces.canvases = canvases;
    done(p, path);
}

/// LLP 1127.003 D9: the pads follow the focus. The canvas they leave hears
/// each pad go; the one they reach hears each pad as it is, so a button held
/// across the move is released by the canvas that now has it.
#[test]
fn pads_follow_the_focus_and_release_the_canvas_they_leave() {
    let (mut p, path) = fixture();
    let (a, b, editor) = (find(&p, "a"), find(&p, "b"), find(&p, "editor"));
    let id = |p: &Presenter<()>, view| p.surfaces.canvases[&view].id;
    let pad = |e: &Value| {
        (
            e["t"].clone(),
            e["connected"].clone(),
            e["buttons"][0].clone(),
        )
    };
    let mut south = [0.; 17];
    south[0] = 1.;
    p.set_focus(Some(a), 0.);
    p.hardware_pad(0, true, south, [0.; 4], 0.);
    let (to, _, event) = last_input(&p);
    assert_eq!(
        (to, pad(&event)),
        (id(&p, a), (json!("pad"), json!(true), json!(1.)))
    );
    p.set_focus(Some(b), 0.);
    let (to, _, event) = last_input(&p);
    assert_eq!(
        (to, pad(&event)),
        (id(&p, b), (json!("pad"), json!(true), json!(1.)))
    );
    let previous: Value = serde_json::from_str(unsafe {
        std::ffi::CStr::from_ptr(p.surfaces.abis[""]
            .symbol::<unsafe extern "C" fn() -> *const std::ffi::c_char>(
            b"test_previous_input",
        )())
        .to_str()
        .unwrap()
    })
    .unwrap();
    assert_eq!(
        pad(&previous),
        (json!("pad"), json!(false), json!(0.)),
        "a hears the pad go"
    );
    p.hardware_pad(0, true, [0.; 17], [0.; 4], 0.);
    let (to, _, event) = last_input(&p);
    assert_eq!(
        (to, pad(&event)),
        (id(&p, b), (json!("pad"), json!(true), json!(0.)))
    );
    // An editor takes the pads from every canvas; they wait for the next canvas.
    p.hardware_pad(0, true, south, [0.; 4], 0.);
    p.set_focus(Some(editor), 0.);
    let (to, count, event) = last_input(&p);
    assert_eq!(
        (to, pad(&event)),
        (id(&p, b), (json!("pad"), json!(false), json!(0.)))
    );
    p.hardware_pad(0, true, south, [0.5, 0., 0., 0.], 0.);
    assert_eq!(
        last_input(&p).1,
        count,
        "no canvas hears a pad while an editor has the focus"
    );
    p.set_focus(Some(a), 0.);
    let (to, _, event) = last_input(&p);
    assert_eq!(
        (to, pad(&event)),
        (id(&p, a), (json!("pad"), json!(true), json!(1.)))
    );
    assert_eq!(event["axes"][0], json!(0.5));
    done(p, path);
}

/// A buffered press and release keep the times the device reported them,
/// and pads a host stopped reading are not replayed to the next canvas.
#[test]
fn pad_states_keep_their_report_times_and_forgotten_pads_are_not_replayed() {
    let (mut p, path) = fixture();
    let (a, b) = (find(&p, "a"), find(&p, "b"));
    let previous = |p: &Presenter<()>| -> Value {
        serde_json::from_str(unsafe {
            std::ffi::CStr::from_ptr(p.surfaces.abis[""]
                .symbol::<unsafe extern "C" fn() -> *const std::ffi::c_char>(
                b"test_previous_input",
            )())
            .to_str()
            .unwrap()
        })
        .unwrap()
    };
    let mut south = [0.; 17];
    south[0] = 1.;
    p.set_focus(Some(a), 0.);
    p.hardware_pad(0, true, south, [0.; 4], 100.);
    p.hardware_pad(0, true, [0.; 17], [0.; 4], 309.);
    let (_, count, last) = last_input(&p);
    assert_eq!(
        (previous(&p)["at"].clone(), last["at"].clone()),
        (json!(100.), json!(309.))
    );
    p.hardware_pad(0, true, south, [0.; 4], 400.);
    p.forget_pads();
    let (to, after, event) = last_input(&p);
    assert_eq!(
        (to, after, event["connected"].clone()),
        (p.surfaces.canvases[&a].id, count + 2, json!(false))
    );
    p.set_focus(Some(b), 0.);
    assert_eq!(
        last_input(&p).1,
        after,
        "a forgotten pad is not replayed to b"
    );
    done(p, path);
}

/// LLP 1127.003 D2: a pad's report times survive the route from the reader
/// to the canvas, through the first report choosing which canvas has the
/// pads, so a 150 ms press stays 150 ms and a 210 ms one 210 ms. The rows are
/// those `game/render`'s `pad_rows_tap_and_hold_by_their_report_times` sends
/// into a world.
#[test]
fn pad_report_times_survive_the_read_and_the_first_owner() {
    use crate::gamepad::{read, Change, Fetch, Layout, Opened, State};
    for release in [250., 310.] {
        let (mut p, path) = fixture();
        assert_eq!(
            p.focus, None,
            "the first canvas that takes input has the pads"
        );
        let state = State::new([None; 8], Layout::Positional);
        let mut pads = vec![(
            (),
            Opened {
                index: 0,
                state,
                announced: false,
            },
        )];
        // BTN_EAST down at 100 ms on the device's clock, up at `release`; the
        // host's clock reads 1000 ms more.
        let report =
            |value, at| Fetch::Changes(vec![(Change::Key(0x131, value), at), (Change::Report, at)]);
        let mut rows = Vec::new();
        for (fetched, now) in [(report(1, 100.), 120.), (report(0, release), release + 20.)] {
            let mut fetched = Some(fetched);
            for (index, reading, at) in
                read(&mut pads, (now, now + 1000.), |_| fetched.take().unwrap())
            {
                let r = reading.unwrap();
                p.hardware_pad(index, true, r.buttons, r.axes, at);
                let (to, _, event) = last_input(&p);
                rows.push((to, event["buttons"][1].clone(), event["at"].clone()));
            }
        }
        let to = rows[0].0;
        assert_eq!(
            rows,
            [
                (to, json!(0.), json!(1100.)),
                (to, json!(1.), json!(1100.)),
                (to, json!(0.), json!(release + 1000.)),
            ]
        );
        done(p, path);
    }
}
