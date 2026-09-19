//! Device input inside a canvas that asks for it (LLP 1041.002 S1).

/// A pointer's contact phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerPhase {
    /// Contact began.
    Down,
    /// Position changed.
    Move,
    /// Contact ended.
    Up,
    /// The platform cancelled the contact.
    Cancel,
}

/// The web's pointer types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    /// A mouse.
    Mouse,
    /// A finger.
    Touch,
    /// A pen.
    Pen,
}

/// One device event, stamped by the host, with coordinates in canvas points.
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    /// A physical key while the canvas has focus.
    Key {
        /// `KeyboardEvent.code`, such as `KeyW` or `Space`.
        code: String,
        /// `KeyboardEvent.key`, such as `w` or a space.
        key: String,
        /// Pressed rather than released.
        down: bool,
        /// The platform's repeat flag.
        repeat: bool,
        /// Host clock in milliseconds.
        at_ms: f64,
    },
    /// A raw pointer contact or motion.
    Pointer {
        /// Platform pointer identifier.
        id: u32,
        /// Contact phase.
        phase: PointerPhase,
        /// Horizontal point in the canvas.
        x: f32,
        /// Vertical point in the canvas.
        y: f32,
        /// Device kind.
        kind: PointerKind,
        /// The web's pressed-button bit mask.
        buttons: u32,
        /// Host clock in milliseconds.
        at_ms: f64,
    },
    /// A canvas child feeding a named surface action, in control-local points.
    Control {
        /// The child's declared action.
        name: String,
        /// Host contact identifier.
        id: u32,
        /// Contact phase.
        phase: PointerPhase,
        /// Horizontal point inside the control.
        x: f32,
        /// Vertical point inside the control.
        y: f32,
        /// Host clock in milliseconds.
        at_ms: f64,
    },
    /// A wheel at a point in the canvas.
    Wheel {
        /// Horizontal delta.
        dx: f32,
        /// Vertical delta.
        dy: f32,
        /// Horizontal point in the canvas.
        x: f32,
        /// Vertical point in the canvas.
        y: f32,
        /// Host clock in milliseconds.
        at_ms: f64,
    },
    /// Focus left the canvas: whatever was held is released.
    Blur {
        /// Host clock in milliseconds.
        at_ms: f64,
    },
}
