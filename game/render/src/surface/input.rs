//! A host's input event (`exact_gpu`) as the engine's, from the local player.
use exact_gpu::InputEvent;

pub(super) fn engine_event(event: &InputEvent) -> exact_game::InputEvent {
    use exact_game::{InputEvent as E, PointerPhase as P};
    use exact_gpu::PointerPhase as Q;
    match event {
        InputEvent::Key {
            code, down, at_ms, ..
        } => E::Key {
            code: code.clone(),
            down: *down,
            at_ms: *at_ms,
            player: 0,
        },
        InputEvent::Pointer {
            id,
            phase,
            x,
            y,
            dx,
            dy,
            kind,
            buttons,
            at_ms,
        } => E::Pointer {
            id: u64::from(*id),
            phase: match phase {
                Q::Down => P::Down,
                Q::Move => P::Move,
                Q::Up => P::Up,
                Q::Cancel => P::Cancel,
            },
            x: *x,
            y: *y,
            dx: *dx,
            dy: *dy,
            // A finger's contact is its phase, not a mouse button.
            buttons: if *kind == exact_gpu::PointerKind::Touch {
                0
            } else {
                *buttons
            },
            at_ms: *at_ms,
            player: 0,
        },
        InputEvent::Control {
            name,
            id,
            phase,
            x,
            y,
            at_ms,
        } => E::Control {
            name: name.clone(),
            id: u64::from(*id),
            phase: match phase {
                Q::Down => P::Down,
                Q::Move => P::Move,
                Q::Up => P::Up,
                Q::Cancel => P::Cancel,
            },
            x: *x,
            y: *y,
            at_ms: *at_ms,
            player: 0,
        },
        InputEvent::Wheel { dx, dy, at_ms, .. } => E::Wheel {
            dx: *dx,
            dy: *dy,
            at_ms: *at_ms,
            player: 0,
        },
        InputEvent::Pad {
            index,
            connected,
            standard,
            buttons,
            axes,
            at_ms,
        } => E::Pad {
            index: *index,
            connected: *connected,
            standard: *standard,
            buttons: *buttons,
            axes: *axes,
            at_ms: *at_ms,
            player: 0,
        },
        InputEvent::Blur { at_ms } => E::Blur {
            at_ms: *at_ms,
            player: 0,
        },
        InputEvent::Message { text, at_ms } => E::Message {
            text: text.clone(),
            at_ms: *at_ms,
            player: 0,
        },
    }
}
