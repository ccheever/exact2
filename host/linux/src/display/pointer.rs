//! The display loop's input dispatch, testable without a DRM master.
use crate::input::{InputEvent, Key};
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
    match event {
        InputEvent::Motion(dx, dy) => {
            pointer.0 = (pointer.0 + dx / scale).clamp(0., viewport.0 - 1.);
            pointer.1 = (pointer.1 + dy / scale).clamp(0., viewport.1 - 1.);
            p.set_pointer(Some(*pointer));
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
        InputEvent::Cancel => p.pointer_cancel(now_ms)?,
        InputEvent::Wheel(dx, dy) => p.wheel_at(pointer.0, pointer.1, dx, dy),
        InputEvent::Key(Key::Char(c)) => p.key(Some(c), false, now_ms),
        InputEvent::Key(Key::Backspace) => p.key(None, true, now_ms),
        InputEvent::Key(Key::Escape) => {
            p.pointer_cancel(now_ms)?;
            p.blur();
        }
        InputEvent::Key(Key::Enter) => p.key(Some('\n'), false, now_ms),
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
    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(name.into()))
        }
    }
    fn fixture() -> Presenter<NoData> {
        let plan = contract::compile(r#"component App
  state count = 0
  action reply writes count
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
        for cancel in [
            None,
            Some(InputEvent::Key(Key::Escape)),
            Some(InputEvent::Cancel),
        ] {
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
    fn header_height_relative_absolute_release_cancel_and_escape() {
        let plan = contract::compile(r#"component App
  state target = 180
  state count = 0
  state seen = 0
  action release(h: number, v: number) writes target, count, seen
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
        for cancel in [
            None,
            Some(InputEvent::Cancel),
            Some(InputEvent::Key(Key::Escape)),
        ] {
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
