//! The display loop's input dispatch, testable without a DRM master.
use crate::input::InputEvent;
use crate::Presenter;
use exact_runner::DataSource;

pub(super) fn dispatch<D: DataSource>(
    p: &mut Presenter<D>,
    pointer: &mut (f32, f32),
    viewport: (f32, f32),
    scale: f32,
    event: InputEvent,
    now_ms: f64,
) -> Result<(), String> {
    if !matches!(event, InputEvent::Key { .. } | InputEvent::Cancel)
        && !p.display_input_mapping(viewport, scale)
    {
        // An uncertified physical release must end capture without executing
        // a drop. Refusing its coordinates must not strand a hold or item pin.
        if matches!(event, InputEvent::Button(false)) {
            p.pointer_cancel(now_ms)?;
        }
        return Err("pointer mapping does not match the acknowledged surface".into());
    }
    match event {
        InputEvent::Motion(dx, dy) => {
            pointer.0 = (pointer.0 + dx / scale).clamp(0., viewport.0 - 1.);
            pointer.1 = (pointer.1 + dy / scale).clamp(0., viewport.1 - 1.);
            p.set_pointer(Some(*pointer));
            // A canvas sees the device's motion, unbounded by the screen edge.
            p.raw_motion(dx / scale, dy / scale);
            p.pointer_move(pointer.0, pointer.1, now_ms)?;
        }
        InputEvent::Absolute(fx, fy) => {
            if let Some(fx) = fx {
                pointer.0 = (fx * viewport.0).clamp(0., viewport.0 - 1.);
            }
            if let Some(fy) = fy {
                pointer.1 = (fy * viewport.1).clamp(0., viewport.1 - 1.);
            }
            p.set_pointer(Some(*pointer));
            p.pointer_move(pointer.0, pointer.1, now_ms)?;
        }
        InputEvent::Button(true) => {
            p.pointer_down(pointer.0, pointer.1, now_ms)?;
        }
        InputEvent::Button(false) => {
            p.pointer_up(pointer.0, pointer.1, now_ms)?;
        }
        InputEvent::Aux(bit, down) => p.pointer_aux(bit, down, pointer.0, pointer.1, now_ms),
        InputEvent::Cancel => p.pointer_cancel(now_ms)?,
        InputEvent::Wheel(dx, dy) => p.wheel_at(pointer.0, pointer.1, dx, dy),
        // The same targeted path as VNC and agent keys: text, Escape blur and a
        // canvas's held keys (`hardware_key`). Escape also ends a pointer hold.
        InputEvent::Key {
            code,
            shift,
            down,
            repeat,
        } => {
            let Some((code, key)) = crate::input::key(code, shift) else {
                return Ok(());
            };
            if code == "Escape" && down {
                p.pointer_cancel(now_ms)?;
            }
            p.hardware_key(code, key, down, repeat);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presenter::PainterChoice;
    use exact_kernel::PropId;
    use exact_runner::{DataError, Value};
    use std::path::PathBuf;
    /// An evdev key press (1 Escape, 45 x).
    fn press(code: u16) -> InputEvent {
        InputEvent::Key {
            code,
            shift: false,
            down: true,
            repeat: false,
        }
    }
    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(name.into()))
        }
    }
    fn fixture() -> Presenter<NoData> {
        let plan = contract::compile(r#"component App
  state count = 0
  action reply
    count = count + 1
  view
    column
      box testId="row" width=400 height=100 swiperight=reply touch-action="pan-y" transition="translate spring(300, 30, 1)"
        text "swipe"
      text `${count}` testId="count"
"#).unwrap();
        Presenter::boot_with(
            &plan.encode(),
            NoData,
            (400., 500.),
            1.,
            PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
            PainterChoice::Cpu,
        )
        .unwrap()
        .0
    }
    fn count(p: &Presenter<NoData>) -> &str {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id("count")[0])
            .unwrap()
            .props
            .str(PropId::Text)
            .unwrap()
    }
    #[test]
    fn relative_and_absolute_display_events_release_or_escape_once() {
        for cancel in [None, Some(press(1)), Some(InputEvent::Cancel)] {
            let mut p = fixture();
            let mut at = (20., 40.);
            for (time, event) in [
                (0., InputEvent::Button(true)),
                (10., InputEvent::Motion(20., 0.)), // scale2: recognition at30
                (30., InputEvent::Absolute(Some(0.275), None)), // logical110
            ] {
                dispatch(&mut p, &mut at, (400., 500.), 2., event, time).unwrap();
            }
            assert!(p.collection_interaction().is_some());
            if let Some(event) = cancel {
                dispatch(&mut p, &mut at, (400., 500.), 2., event, 40.).unwrap();
            }
            dispatch(
                &mut p,
                &mut at,
                (400., 500.),
                2.,
                InputEvent::Button(false),
                50.,
            )
            .unwrap();
            assert_eq!(count(&p), if cancel.is_some() { "0" } else { "1" });
            assert_eq!(p.collection_interaction(), None);
        }
    }

    #[test]
    fn viewport_pointer_mapping_requires_the_acknowledged_extent_not_live_c() {
        let mut p = fixture();
        let a = p.display_frame().unwrap();
        assert!(p.display_complete(&a));
        assert!(p.resize(300., 300.).is_none());
        let b = p.display_frame().unwrap();
        let b_pixels = b.pixels.data().to_vec();
        assert!(p.resize(800., 1000.).is_none());
        let mut at = (20., 40.);
        let absolute = InputEvent::Absolute(Some(0.25), Some(0.5));
        assert!(dispatch(&mut p, &mut at, (800., 1000.), 1., absolute, 1.).is_err());
        assert_eq!(at, (20., 40.), "refusal must not reinterpret a pointer");
        dispatch(&mut p, &mut at, (400., 500.), 1., absolute, 1.).unwrap();
        assert_eq!(at, (100., 250.));
        assert_eq!(b.pixels.data(), b_pixels);
        assert!(p.display_complete(&b));
        // No C paint between ACK and input. B's mapping is now authoritative.
        assert!(dispatch(&mut p, &mut at, (400., 500.), 1., absolute, 2.).is_err());
        assert_eq!(at, (100., 250.));
        dispatch(&mut p, &mut at, (300., 300.), 1., absolute, 2.).unwrap();
        assert_eq!(at, (75., 150.));
        assert!(p.dirty());
        assert_eq!(b.pixels.data(), b_pixels);
    }

    #[test]
    fn viewport_pointer_scale_or_missing_origin_witness_refuses_but_keys_cancel_continue() {
        let mut p = fixture();
        let a = p.display_frame().unwrap();
        assert!(p.display_complete(&a));
        let mut at = (20., 40.);
        for (extent, scale) in [((400., 500.), 2.), ((800., 1000.), 1.)] {
            assert!(dispatch(
                &mut p,
                &mut at,
                extent,
                scale,
                InputEvent::Motion(8., 4.),
                1.
            )
            .is_err());
            assert_eq!(at, (20., 40.));
            dispatch(&mut p, &mut at, extent, scale, press(45), 1.).unwrap();
            dispatch(&mut p, &mut at, extent, scale, InputEvent::Cancel, 1.).unwrap();
        }
        let b = p.display_frame().unwrap();
        p.reload(
            &contract::compile("component App\n  view\n    text \"new\"\n")
                .unwrap()
                .encode(),
            NoData,
        )
        .unwrap();
        assert!(dispatch(
            &mut p,
            &mut at,
            (400., 500.),
            1.,
            InputEvent::Button(true),
            2.
        )
        .is_err());
        assert!(p.display_complete(&b)); // Old runtime's buffer released only.
        assert!(dispatch(
            &mut p,
            &mut at,
            (400., 500.),
            1.,
            InputEvent::Motion(1., 1.),
            3.
        )
        .is_err());
        assert_eq!(at, (20., 40.));
        mismatched_physical_release_cancels_existing_hold_without_drop();
    }

    fn mismatched_physical_release_cancels_existing_hold_without_drop() {
        for (extent, scale) in [((800., 1000.), 1.), ((400., 500.), 2.)] {
            let mut p = fixture();
            let a = p.display_frame().unwrap();
            assert!(p.display_complete(&a));
            assert!(p.resize(300., 300.).is_none());
            let b = p.display_frame().unwrap();
            let b_pixels = b.pixels.data().to_vec();
            assert!(p.resize(800., 1000.).is_none());
            let mut at = (20., 40.);
            for (time, event) in [
                (1., InputEvent::Button(true)),
                (10., InputEvent::Motion(20., 0.)),
                (20., InputEvent::Motion(70., 0.)),
            ] {
                dispatch(&mut p, &mut at, (400., 500.), 1., event, time).unwrap();
            }
            assert!(
                p.collection_interaction().is_some(),
                "recognized hold pins its item"
            );
            assert!(p.contact_position().is_some());
            let certified_position = at;
            assert!(dispatch(
                &mut p,
                &mut at,
                extent,
                scale,
                InputEvent::Button(false),
                30.
            )
            .is_err());
            assert_eq!(
                at, certified_position,
                "refusal never guesses new coordinates"
            );
            assert_eq!(
                count(&p),
                "0",
                "cancellation must not execute a successful swipe"
            );
            assert!(p.contact_position().is_none());
            assert!(
                p.collection_interaction().is_none(),
                "physical UP must release its pin"
            );
            assert_eq!(b.pixels.data(), b_pixels);
            assert!(p.display_frame().is_none());
            assert!(p.display_complete(&b));
        }
    }
    #[test]
    fn header_height_relative_absolute_release_cancel_and_escape() {
        let plan = contract::compile(r#"component App
  state target = 180
  state count = 0
  state seen = 0
  action release(h: number, v: number)
    count = count + 1
    seen = h
    target = 360
  view
    box width=400 height=500
      column id="panel" testId="panel" position="absolute" bottom=0 width=400 height=target max-height="100%" box-sizing="border-box" transition="height spring(300,30,1)"
        box heightDragFor="panel" heightrelease=release height=40
          text "drag header"
      text `${count}` testId="count"
      text `${seen}` testId="seen"
"#).unwrap();
        for cancel in [None, Some(InputEvent::Cancel), Some(press(1))] {
            let mut p = Presenter::boot_with(
                &plan.encode(),
                NoData,
                (400., 500.),
                1.,
                PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
                PainterChoice::Cpu,
            )
            .unwrap()
            .0;
            let mut at = (200., 340.);
            for (time, event) in [
                (0., InputEvent::Button(true)),
                (10., InputEvent::Motion(0., -20.)), // scale2 -> recognition y330
                (30., InputEvent::Absolute(None, Some(0.42))), // y210 -> height300
            ] {
                dispatch(&mut p, &mut at, (400., 500.), 2., event, time).unwrap();
            }
            let k = p.host().kernel();
            assert_eq!(
                k.node_by_key(k.find_by_test_id("panel")[0])
                    .unwrap()
                    .frame
                    .height,
                300.
            );
            if let Some(event) = cancel {
                dispatch(&mut p, &mut at, (400., 500.), 2., event, 40.).unwrap();
            }
            for time in [50., 60.] {
                dispatch(
                    &mut p,
                    &mut at,
                    (400., 500.),
                    2.,
                    InputEvent::Button(false),
                    time,
                )
                .unwrap();
            }
            assert_eq!(count(&p), if cancel.is_some() { "0" } else { "1" });
            assert!(p.collection_interaction().is_none());
            if cancel.is_none() {
                let k = p.host().kernel();
                assert_eq!(
                    k.node_by_key(k.find_by_test_id("seen")[0])
                        .unwrap()
                        .props
                        .str(PropId::Text),
                    Some("300")
                );
            }
        }
    }
}
