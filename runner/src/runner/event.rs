//! Typed host events and their action dispatch; no host clock or gesture ownership.

use super::{DataSource, Runner, RunnerError};
use exact_kernel::{CommitReceipt, NodeKey, ViewId};
use exact_plan::{ActionsId, Code, EventKind, HandlersId, NodesId, Opcode, TypeKind, Value};
use std::fmt::Write as _;
use std::rc::Rc;

mod focus;
mod keyboard;
pub use keyboard::KeyboardEvent;

const BINDING_ARGS: usize = 8;
const BINDING_STRING: usize = 1024;
const BINDING_STRING_TOTAL: usize = 4096;

/// A bounded retained Press/Swiperight binding. Contains no frame, tree, plan,
/// resource or History owner. A fresh Runner (including reload) invalidates it.
#[derive(Debug, Clone)]
pub struct ActionBinding {
    origin: Rc<()>,
    key: NodeKey,
    node: NodesId,
    handler: HandlersId,
    event: EventKind,
    action: ActionsId,
    args: Vec<BindingScalar>,
}

impl ActionBinding {
    /// Retained scalar UTF-8 payload bytes, for aggregate host picture budgets.
    /// Walks at most eight scalars without allocation or exposing their values;
    /// excludes scalar metadata, allocator overhead and the transient capture.
    pub fn retained_utf8_bytes(&self) -> usize {
        self.args
            .iter()
            .map(|arg| match arg {
                BindingScalar::String(value) => value.len(),
                _ => 0,
            })
            .sum()
    }
}

/// Refusal before an action, clock or host effect. Generic dispatch is unaffected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionBindingRefusal {
    /// No matching live node/handler, a different Runner, or changed arguments.
    Stale,
    /// The event, expression or action needs context outside this narrow grammar.
    Unsupported,
    /// A finite capture or code bound was exceeded.
    Limit,
    /// An earlier failed update invalidated this Runner.
    Poisoned,
}

/// Retained qualification can refuse without dispatch; a qualified ordinary
/// action can still fail with the same Runner error as generic dispatch.
#[derive(Debug)]
pub enum ActionBindingError {
    /// No action was dispatched.
    Refused(ActionBindingRefusal),
    /// The qualified action used ordinary Runner dispatch and failed there.
    Dispatch(RunnerError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BindingScalar {
    Unit,
    Bool(bool),
    Number(u64),
    String(String),
}

impl BindingScalar {
    fn capture(value: &Value, string_bytes: &mut usize) -> Result<Self, ActionBindingRefusal> {
        Ok(match value {
            Value::Unit => Self::Unit,
            Value::Bool(b) => Self::Bool(*b),
            Value::Number(n) if n.is_finite() => Self::Number(n.to_bits()),
            v @ exact_plan::str_value!() => return Self::string(v.text(), string_bytes),
            _ => return Err(ActionBindingRefusal::Unsupported),
        })
    }

    fn string(s: &str, string_bytes: &mut usize) -> Result<Self, ActionBindingRefusal> {
        // Both limits precede the copy; never retain the parent Record.
        let total = string_bytes
            .checked_add(s.len())
            .ok_or(ActionBindingRefusal::Limit)?;
        if s.len() > BINDING_STRING || total > BINDING_STRING_TOTAL {
            return Err(ActionBindingRefusal::Limit);
        }
        *string_bytes = total;
        Ok(Self::String(s.to_string()))
    }

    fn kind(&self) -> TypeKind {
        match self {
            Self::Unit => TypeKind::Unit,
            Self::Bool(_) => TypeKind::Bool,
            Self::Number(_) => TypeKind::Number,
            Self::String(_) => TypeKind::String,
        }
    }
}

/// The next instruction of a retained binding's code, through the one
/// decoder; running out or malformed code is outside the grammar.
fn binding_step(
    code: &mut impl Iterator<Item = Result<crate::vm::Instruction, crate::vm::Trap>>,
) -> Result<crate::vm::Instruction, ActionBindingRefusal> {
    code.next()
        .and_then(Result::ok)
        .ok_or(ActionBindingRefusal::Unsupported)
}

fn scalar_kind(kind: TypeKind) -> bool {
    matches!(
        kind,
        TypeKind::Unit | TypeKind::Bool | TypeKind::Number | TypeKind::String
    )
}

/// What an `input` or `change` event carries, typed by its control (LLP
/// 1069.001 D4): a text field's text, or whether a checkbox is checked.
#[derive(Debug, Clone, PartialEq)]
pub enum ControlValue {
    /// A text field's text.
    Text(String),
    /// A checkbox's (or switch's) checked state.
    Checked(bool),
    /// A file input's picked files (LLP 1069.002 D3), in selection order.
    Files(Vec<super::picker::Picked>),
    /// A text field's text with its selection as the edit left it, which
    /// its `InputEvent` reports (x2apps codeedit #2).
    Field(String, super::FieldSelection),
    /// A scripted range replacement: one noncancelable insertReplacementText input.
    Replacement(String, super::FieldSelection),
}

impl ControlValue {
    /// The value as the action receives it.
    pub(super) fn value(&self) -> Value {
        match self {
            Self::Text(text) | Self::Field(text, _) | Self::Replacement(text, _) => {
                Value::str(text)
            }
            Self::Checked(on) => Value::Bool(*on),
            Self::Files(files) => Value::list(files.iter().map(|f| f.value()).collect()),
        }
    }
}

impl From<&str> for ControlValue {
    fn from(text: &str) -> Self {
        Self::Text(text.to_owned())
    }
}

impl From<String> for ControlValue {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

impl From<bool> for ControlValue {
    fn from(on: bool) -> Self {
        Self::Checked(on)
    }
}

/// A host event aimed at a view.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Certified physical drop or explicitly synthesized List reorder request.
    ReorderDrop {
        /// Exact source string key (including the empty string).
        item: String,
        /// Exact insertion-before key; None denotes the actual logical end.
        before: Option<String>,
    },
    /// A press on the view, no modifier key held.
    Press,
    /// A press with modifier keys held (gallery F20: shift-click, ⌘-click),
    /// the `MouseEvent` flags a `press` action may take; dispatched as
    /// `Press`. [`Event::press`] makes whichever the modifiers call for.
    PressWith(KeyModifiers),
    /// A control's value moved (HTML's `input`): a text field's every
    /// keystroke, a checkbox's toggle (LLP 1069.001 D4).
    Input(ControlValue),
    /// A control's value was committed (HTML's `change`): a text field on
    /// blur or Enter, a checkbox as it toggles (LLP 1069.001 D4).
    Change(ControlValue),
    /// A file input's picker was dismissed with nothing chosen (HTML's
    /// `cancel`, LLP 1069.002 D2). A node without a `cancel` handler takes
    /// it as nothing.
    Cancel,
    /// A dialog closed, after its focus was restored.
    Close,
    /// A popover changed visibility; HTML coalesces these on the host's event loop.
    Toggle {
        /// Whether it was open before the first change in this turn.
        old_open: bool,
        /// Whether it is open after the last change in this turn.
        new_open: bool,
    },
    /// A scroll finished (DOM's `scrollend`): the person's, its momentum
    /// included, or an animated one.
    Scrollend,
    /// A text field's selection changed (HTML's `select`, x2apps codeedit
    /// #2): its text, and the selection its `InputEvent` reports.
    FieldSelect(String, super::FieldSelection),
    /// Markdown toolbar facts. Selection offsets remain local to the editor.
    Select {
        /// Space-separated active format names.
        formats: String,
        /// The selection spans differing formats or link targets.
        mixed: bool,
        /// The common link target, or empty.
        link: String,
        /// Space-separated unavailable command names.
        unavailable: String,
    },
    /// The pointer came over the view (`true`) or left it (`false`) —
    /// `pointerenter`/`pointerleave`, not a bubbling `mouseover`.
    Hover(bool),
    /// The view took the focus.
    Focus,
    /// The view lost the focus.
    Blur,
    /// A focus transition heard at the target or an ancestor.
    Focusin {
        /// The newly focused element's authored id, or empty.
        target: String,
        /// The previously focused element's id, if any.
        related_target: Option<String>,
    },
    /// A focus transition heard at the target or an ancestor.
    Focusout {
        /// The previously focused element's authored id, or empty.
        target: String,
        /// The newly focused element's id, if any.
        related_target: Option<String>,
    },
    /// A key went down while the view had the focus (DOM's `keydown`): the
    /// key's name as the web spells it (`"Enter"`, `"ArrowDown"`, `"a"`),
    /// the modifiers held, the physical key and whether it is an auto-repeat.
    /// A host writes the chord, then the code and repeat ([`KeyboardEvent`]).
    Key(KeyboardEvent),
    /// A keydown during DOM capture, before the focused target's `key`.
    Keycapture(KeyboardEvent),
    /// A key came up while the view had the focus (DOM's `keyup`, #140): a
    /// modifier's release too, the way an app learns ⌘ is no longer held.
    Keyup(KeyboardEvent),
    /// Enter in an input with a `submit` handler — the web's implicit
    /// submission (HTML forms §4.10.21.2), without a form.
    Submit,
    /// An iframe finished loading (including an error document on the web).
    Load,
    /// An iframe guest posted a string to its parent (@ref LLP 1020 D2).
    Message(String),
    /// The platform requested a context menu (secondary click or long press).
    Contextmenu,
    /// A context menu asked for at a point: the secondary click's
    /// `PointerEvent` (studio diary R22); dispatched as `Contextmenu`.
    ContextmenuAt(super::PointerEvent),
    /// The window is about to close or the app to quit (DOM's
    /// `beforeunload`, studio diary R17): an action that calls
    /// `preventDefault()` keeps it open.
    Beforeunload,
    /// A wheel's turn or a trackpad's scroll over the view (DOM's `wheel`);
    /// a pinch arrives as one with Control held, as on the web.
    Wheel(super::WheelEvent),
    /// Files dropped on the view from outside the app (DOM's `drop`), each
    /// a `doc:` handle the host minted (LLP 1069.010 D1).
    Drop(super::DropEvent),
    /// A double click, or the platform’s double tap.
    Dblclick,
    /// A touch or primary button went down on the view (DOM's
    /// `pointerdown`), before any gesture is recognized (LLP 1005 §3).
    Pointerdown(super::PointerEvent),
    /// That touch or button came up, or the platform cancelled it (DOM's
    /// `pointerup`; a `pointercancel` is delivered as one).
    Pointerup(super::PointerEvent),
    /// The pointer moved over the view, or while held after going down on
    /// it: at most one a frame (LLP 1056 §3 stage 3, as built).
    Pointermove(super::PointerEvent),
    /// A platform-recognized right swipe.
    Swiperight,
    /// A pull past the top of a scroll container asked for fresh content
    /// (UIKit's `UIRefreshControl`); the app answers through `refreshing`.
    Refresh,
    /// A changed scroll position, in CSS pixels, and the scroller's extents
    /// as the host had them: what a web handler reads off `event.target`.
    Scroll(ScrollEvent),
    /// The element's content box after layout, ResizeObserver's
    /// `contentRect` (`super::resize`).
    Resize(super::ResizeRect),
    /// IntersectionObserver's threshold-zero entry (LLP 1005).
    Intersect {
        /// Whether the target intersects the root, including edge contact.
        is_intersecting: bool,
        /// Intersection area divided by target area, in the range 0 to 1.
        intersection_ratio: f64,
    },
    /// Incremental recognized pan displacement in viewport CSS pixels.
    /// @ref LLP 1043.000 §3 D8 — the action commits layout state, never a hold.
    Pan(f64, f64),
    /// A pan that began has ended: its release velocity in viewport CSS
    /// pixels per second (vx, vy), pan's units over time. The platform's
    /// where it measures one (UIKit), `exact_motion::VelocityTracker`'s
    /// elsewhere; a cancelled contact releases at (0, 0) (LLP 1057 §10.6).
    PanRelease(f64, f64),
    /// A standard media event. Numeric payloads are seconds.
    Media(EventKind, String),
    /// DOM's `copy`, `cut` or `paste` at the focused view (spreadsheet F4,
    /// F14): the clipboard's plain text as the event carries it — what is
    /// pasted; empty on copy and cut, whose action writes the clipboard
    /// (`copyText`), as a DOM listener's `setData` does.
    Clipboard(EventKind, String),
    /// The part of the reader's text selection inside a `text` changed (the
    /// web's `selectionchange`, the reader diary): the selected text and its
    /// UTF-16 start and end in the node's own text; nothing selected there
    /// is empty with equal offsets.
    SelectionChange {
        /// What is selected of the node's text (`Range.toString()`).
        text: String,
        /// Where it starts, in UTF-16 units of the node's text.
        start: f64,
        /// Where it ends.
        end: f64,
        /// Selected fragments in viewport coordinates, sampled with the event.
        rects: Vec<crate::geometry::GeometryAnswer>,
    },
    /// An incoming location at the navigation root. @ref LLP 1038 D8/D11
    Navigate(String),
    /// An authored sheet handle released: logical height and signed pixels/second.
    HeightRelease {
        /// Finite logical pixels in [0, f32::MAX].
        height: f64,
        /// Finite signed logical pixels per second.
        velocity: f64,
    },
    /// Untransformed target/clip dimensions for one authored transform binding.
    /// Zero dimensions are valid feedback but cannot admit a physical hold.
    TransformGeometry {
        /// Target border-box width, finite logical pixels in [0, f32::MAX].
        box_width: f64,
        /// Target border-box height, in the same domain.
        box_height: f64,
        /// Direct clip's inner width, in the same domain.
        port_width: f64,
        /// Direct clip's inner height, in the same domain.
        port_height: f64,
    },
    /// Final Translate/Scale presentation and signed release velocity.
    /// This synthesized event does not prove live physical ownership.
    TransformRelease {
        /// Parent-space logical pixels, finite and f32-representable in range.
        x: f64,
        /// Parent-space logical pixels, in the same domain.
        y: f64,
        /// Positive finite scale whose f32 conversion remains positive/finite.
        scale: f64,
        /// Finite signed x velocity, logical pixels per second.
        vx: f64,
        /// Finite signed y velocity, logical pixels per second.
        vy: f64,
        /// Finite signed scale units per second, measured by the engine (LLP 1057.001 §3).
        vscale: f64,
    },
}

/// A scroll event's position and the scroller's extents, in CSS pixels: the
/// web's `scrollLeft`, `scrollTop`, `scrollWidth`, `scrollHeight`,
/// `clientWidth` and `clientHeight` of the element that scrolled, read when
/// the event fires (chat F4: a jump-to-latest pill asks whether the list is
/// at its end, `scrollHeight - scrollTop - clientHeight` as on the web).
/// A native host's `scrollWidth`/`scrollHeight` is its client size plus its
/// scroll range, so the difference is the range it clamps to.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ScrollEvent {
    /// The offset from the start, along x (`scrollLeft`).
    pub left: f64,
    /// The offset from the start, along y (`scrollTop`).
    pub top: f64,
    /// The scrolled content's width (`scrollWidth`).
    pub width: f64,
    /// The scrolled content's height (`scrollHeight`).
    pub height: f64,
    /// The scrollport's width (`clientWidth`).
    pub client_width: f64,
    /// The scrollport's height (`clientHeight`).
    pub client_height: f64,
}

impl ScrollEvent {
    fn value(&self, anchor: Option<(Value, f64)>) -> Value {
        let (key, offset) = anchor.map_or((Value::NONE, 0.0), |(key, offset)| {
            (Value::some(key), offset)
        });
        Value::record(vec![
            Value::Number(self.left),
            Value::Number(self.top),
            Value::Number(self.width),
            Value::Number(self.height),
            Value::Number(self.client_width),
            Value::Number(self.client_height),
            key,
            Value::Number(offset),
        ])
    }
}

/// The modifier keys held as a key went down: `KeyboardEvent`'s
/// `shiftKey`, `ctrlKey`, `altKey` and `metaKey` (chat F2, kanban F27 in the
/// x2apps diaries: Shift+Enter and ⌘S were not expressible).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyModifiers {
    /// Shift.
    pub shift: bool,
    /// Control.
    pub ctrl: bool,
    /// Alt, Option on a Mac.
    pub alt: bool,
    /// Meta: Command on a Mac, the Windows key elsewhere.
    pub meta: bool,
}

impl KeyModifiers {
    /// The modifiers a host writes as a chord prefix with no key —
    /// `Shift+Meta`, `Control+`, or nothing — as [`KeyModifiers::split`]
    /// names them; `None` for any other word.
    pub fn held(prefix: &str) -> Option<Self> {
        let mut held = Self::default();
        for name in prefix.split('+').filter(|n| !n.is_empty()) {
            *match name {
                "Shift" => &mut held.shift,
                "Control" => &mut held.ctrl,
                "Alt" => &mut held.alt,
                "Meta" => &mut held.meta,
                _ => return None,
            } = true;
        }
        Some(held)
    }

    /// The `MouseEvent` record `press` offers, its fields in the compiler's
    /// order (`contract/types/src/selection.rs`).
    pub fn mouse(&self) -> Value {
        Value::record(vec![
            Value::Bool(self.shift),
            Value::Bool(self.ctrl),
            Value::Bool(self.alt),
            Value::Bool(self.meta),
        ])
    }

    /// A key as a host writes it, split: the modifiers named before it with
    /// any of `Shift+`, `Control+`, `Alt+` and `Meta+`, and the key's name —
    /// the chord syntax of `aria-keyshortcuts` and Playwright
    /// (`"Shift+Enter"`, `"Meta+s"`, `"+"`, `"Shift++"`). A bare name holds
    /// no modifier.
    pub fn split(chord: &str) -> (Self, &str) {
        let mut held = Self::default();
        let mut rest = chord;
        while let Some((name, key)) = rest.split_once('+').filter(|(_, key)| !key.is_empty()) {
            *match name {
                "Shift" => &mut held.shift,
                "Control" => &mut held.ctrl,
                "Alt" => &mut held.alt,
                "Meta" => &mut held.meta,
                _ => break,
            } = true;
            rest = key;
        }
        (held, rest)
    }
}

impl Event {
    /// Decode a DOM focus event's target id and optional related id. The
    /// byte length keeps arbitrary UTF-8 ids, including newlines, unambiguous.
    pub fn focus_payload(entering: bool, payload: &str) -> Option<Self> {
        let (len, rest) = payload.split_once(':')?;
        let len = len.parse::<usize>().ok()?;
        let target = rest.get(..len)?.to_owned();
        let related_target = match rest.get(len..)? {
            "0" => None,
            rest => Some(rest.strip_prefix('1')?.to_owned()),
        };
        Some(if entering {
            Self::Focusin {
                target,
                related_target,
            }
        } else {
            Self::Focusout {
                target,
                related_target,
            }
        })
    }

    /// A press with the modifiers a host held ([`KeyModifiers::held`]):
    /// `Press` when none is; `None` for a word DOM does not name.
    pub fn press(held: &str) -> Option<Self> {
        let held = KeyModifiers::held(held)?;
        Some(if held == KeyModifiers::default() {
            Self::Press
        } else {
            Self::PressWith(held)
        })
    }

    /// A key from its chord ([`KeyModifiers::split`]), with no code.
    pub fn key(chord: &str) -> Self {
        Self::Key(KeyboardEvent::chord(chord))
    }

    /// The DOM record this event offers its action as an optional last
    /// parameter, its fields in the compiler's order
    /// (`contract_types::event_record`, `contract/types/src/selection.rs`):
    /// `key`'s and `keyup`'s `KeyboardEvent`, `press`'s `MouseEvent`, the pointer's and
    /// `contextmenu`'s `PointerEvent`, `scroll`'s `ScrollEvent`, the
    /// clipboard's `ClipboardEvent`, `wheel`'s `WheelEvent` and `drop`'s
    /// `DragEvent`.
    pub fn record(&self) -> Option<Value> {
        match self {
            Event::Toggle { old_open, new_open } => Some(Value::record(vec![
                Value::str(if *old_open { "open" } else { "closed" }),
                Value::str(if *new_open { "open" } else { "closed" }),
            ])),
            Event::Key(k) | Event::Keycapture(k) | Event::Keyup(k) => Some(k.value()),
            Event::Pointerdown(p)
            | Event::Pointerup(p)
            | Event::Pointermove(p)
            | Event::ContextmenuAt(p) => Some(p.value()),
            // A menu asked for with no point (a keyboard's menu key): the
            // record DOM gives one, at the node's origin, no button held.
            Event::Contextmenu => Some(
                super::PointerEvent {
                    offset_x: 0.0,
                    offset_y: 0.0,
                    buttons: 0.0,
                    pressure: 0.0,
                    pointer_type: "mouse".into(),
                    pointer_id: 1.0,
                    client_x: 0.0,
                    client_y: 0.0,
                    held: KeyModifiers::default(),
                }
                .value(),
            ),
            Event::Wheel(w) => Some(w.value()),
            Event::Drop(d) => Some(d.value()),
            Event::Resize(r) => Some(r.value()),
            Event::Intersect {
                is_intersecting,
                intersection_ratio,
            } => Some(Value::record(vec![
                Value::Bool(*is_intersecting),
                Value::Number(*intersection_ratio),
            ])),
            Event::Scroll(s) => Some(s.value(None)),
            Event::Clipboard(_, text) => Some(Value::record(vec![Value::str(text)])),
            Event::SelectionChange {
                text,
                start,
                end,
                rects,
            } => Some(Value::record(vec![
                Value::str(text),
                Value::Number(*start),
                Value::Number(*end),
                Value::list(rects.iter().map(|r| r.value()).collect()),
            ])),
            Event::Press => Some(KeyModifiers::default().mouse()),
            Event::PressWith(held) => Some(held.mouse()),
            Event::Media(kind, payload) => super::media_session::record(*kind, payload),
            Event::Focusin {
                target,
                related_target,
            }
            | Event::Focusout {
                target,
                related_target,
            } => Some(Value::record(vec![
                Value::str(target),
                related_target
                    .as_ref()
                    .map_or(Value::NONE, |id| Value::some(Value::str(id))),
            ])),
            _ => None,
        }
    }

    /// Decode exactly four comma-separated geometry dimensions. Hosts validate
    /// binding identity/mapping before delivery; zero suspends physical admission.
    pub fn transform_geometry_payload(payload: &str) -> Option<Self> {
        let [box_width, box_height, port_width, port_height] = tuple(payload)?;
        let event = Self::TransformGeometry {
            box_width,
            box_height,
            port_width,
            port_height,
        };
        event.invalid_payload().is_none().then_some(event)
    }

    /// Decode exactly `x,y,scale,vx,vy,vscale`. Physical hosts separately check
    /// both tokens, all three binding keys, incarnation and geometry before time.
    pub fn transform_release_payload(payload: &str) -> Option<Self> {
        let [x, y, scale, vx, vy, vscale] = tuple(payload)?;
        let event = Self::TransformRelease {
            x,
            y,
            scale,
            vx,
            vy,
            vscale,
        };
        event.invalid_payload().is_none().then_some(event)
    }

    /// Decode two finite, layout-representable pan deltas; no clock on refusal.
    pub fn pan_payload(payload: &str) -> Option<Self> {
        let [x, y] = tuple(payload)?;
        (pixel(x) && pixel(y)).then_some(Self::Pan(x, y))
    }

    /// Decode exactly `vx,vy`: two finite velocities; no clock on refusal.
    pub fn pan_release_payload(payload: &str) -> Option<Self> {
        let [vx, vy] = tuple(payload)?;
        (vx.is_finite() && vy.is_finite()).then_some(Self::PanRelease(vx, vy))
    }

    fn invalid_payload(&self) -> Option<&'static str> {
        match *self {
            Self::Pan(x, y) if !pixel(x) || !pixel(y) => Some("pan"),
            Self::PanRelease(vx, vy) if !vx.is_finite() || !vy.is_finite() => Some("panrelease"),
            Self::HeightRelease { height, velocity } if !valid_height_release(height, velocity) => {
                Some("heightrelease")
            }
            Self::TransformGeometry {
                box_width,
                box_height,
                port_width,
                port_height,
            } if ![box_width, box_height, port_width, port_height]
                .into_iter()
                .all(|v| pixel(v) && v >= 0.0) =>
            {
                Some("transformgeometry")
            }
            Self::TransformRelease {
                x,
                y,
                scale,
                vx,
                vy,
                vscale,
            } if !pixel(x)
                || !pixel(y)
                || !pixel(scale)
                || scale <= 0.0
                || (scale as f32) <= 0.0
                || ![vx, vy, vscale].into_iter().all(f64::is_finite) =>
            {
                Some("transformrelease")
            }
            _ => None,
        }
    }

    /// A checkbox's reported state, exactly `true` or `false` (LLP 1069.001
    /// D4), as the payload of an `input` or `change`.
    pub fn checked_payload(payload: &str) -> Option<ControlValue> {
        match payload {
            "true" => Some(ControlValue::Checked(true)),
            "false" => Some(ControlValue::Checked(false)),
            _ => None,
        }
    }

    /// Decode exactly `height,velocity`. Hosts must parse before advancing time.
    /// This synthesizes an event; physical delivery separately validates both
    /// binding generations and a live Height token before clock or action.
    pub fn height_release_payload(payload: &str) -> Option<Self> {
        let (height, velocity) = payload.split_once(',')?;
        let (height, velocity) = (
            exact_num::parse_f64(height).ok()?,
            exact_num::parse_f64(velocity).ok()?,
        );
        valid_height_release(height, velocity).then_some(Self::HeightRelease { height, velocity })
    }

    /// Decode the scroll event: `left,top,scrollWidth,scrollHeight,
    /// clientWidth,clientHeight`, finite CSS pixels, the extents not negative.
    pub fn scroll_payload(payload: &str) -> Option<Self> {
        let [left, top, width, height, client_width, client_height] = tuple::<6>(payload)?;
        ([left, top].iter().all(|v| v.is_finite())
            && [width, height, client_width, client_height]
                .iter()
                .all(|v| v.is_finite() && *v >= 0.0))
        .then_some(Self::Scroll(ScrollEvent {
            left,
            top,
            width,
            height,
            client_width,
            client_height,
        }))
    }
}

fn tuple<const N: usize>(payload: &str) -> Option<[f64; N]> {
    let mut parts = payload.split(',');
    let mut values = [0.0; N];
    for value in &mut values {
        *value = exact_num::parse_f64(parts.next()?).ok()?;
    }
    parts.next().is_none().then_some(values)
}

fn pixel(v: f64) -> bool {
    v.is_finite() && v.abs() <= f32::MAX as f64
}

fn valid_height_release(height: f64, velocity: f64) -> bool {
    height.is_finite() && (0.0..=f32::MAX as f64).contains(&height) && velocity.is_finite()
}

impl<D: DataSource> Runner<D> {
    /// Capture a retained Press/Swiperight target at its painted publication.
    /// Admits only scalar literal/direct field projections and closed root-store
    /// actions. Tree lookup retains its existing mounted-tree traversal cost;
    /// accepted captures own at most eight scalars and 4096 UTF-8 string bytes.
    pub fn capture_action_binding(
        &self,
        key: NodeKey,
        event: EventKind,
    ) -> Result<ActionBinding, ActionBindingRefusal> {
        use ActionBindingRefusal::{Limit, Poisoned, Stale, Unsupported};
        if self.poisoned {
            return Err(Poisoned);
        }
        if !matches!(event, EventKind::Press | EventKind::Swiperight) {
            return Err(Unsupported);
        }
        let view = self.kernel.node_by_key(key).ok_or(Stale)?.id;
        let (node, frames) = self.find(view).ok_or(Stale)?;
        if frames.len() > 32 {
            return Err(Limit);
        }
        let handler = self
            .plan
            .node(node)
            .handlers
            .iter()
            .find(|h| self.plan.handler(*h).event == event)
            .ok_or(Stale)?;
        let row = self.plan.handler(handler);
        if row.args.len as usize > BINDING_ARGS {
            return Err(Limit);
        }
        let mut string_bytes = 0;
        let mut args = Vec::with_capacity(row.args.len as usize);
        for arg in row.args.iter() {
            args.push(self.binding_projection(
                self.plan.arg(arg).expr,
                &frames,
                &mut string_bytes,
            )?);
        }
        self.binding_action(row.action, &args)?;
        Ok(ActionBinding {
            origin: self.action_binding_origin.clone(),
            key,
            node,
            handler,
            event,
            action: row.action,
            args,
        })
    }

    /// Revalidate BEFORE the host advances time or performs an action. This is
    /// not a host clock/geometry/eligibility certificate. Dispatch checks again,
    /// so intervening host work cannot turn a stale binding into a current one.
    pub fn validate_action_binding(
        &self,
        binding: &ActionBinding,
        event: EventKind,
    ) -> Result<(), ActionBindingRefusal> {
        if !Rc::ptr_eq(&binding.origin, &self.action_binding_origin) || binding.event != event {
            return Err(ActionBindingRefusal::Stale);
        }
        let current = self.capture_action_binding(binding.key, event)?;
        if current.node != binding.node
            || current.handler != binding.handler
            || current.action != binding.action
            || current.args != binding.args
        {
            return Err(ActionBindingRefusal::Stale);
        }
        Ok(())
    }

    /// Deliver a qualified retained event through ordinary dispatch. Refusals
    /// do not journal an action or touch state/effects/time. Hosts still own
    /// presented coordinates, live eligibility, and pre-clock qualification.
    pub fn dispatch_bound(
        &mut self,
        binding: &ActionBinding,
        event: Event,
    ) -> Result<CommitReceipt, ActionBindingError> {
        let kind = match event {
            Event::Press => EventKind::Press,
            Event::Swiperight => EventKind::Swiperight,
            _ => {
                return Err(ActionBindingError::Refused(
                    ActionBindingRefusal::Unsupported,
                ))
            }
        };
        self.validate_action_binding(binding, kind)
            .map_err(ActionBindingError::Refused)?;
        let view = self
            .kernel
            .node_by_key(binding.key)
            .ok_or(ActionBindingError::Refused(ActionBindingRefusal::Stale))?
            .id;
        self.dispatch(view, event)
            .map_err(ActionBindingError::Dispatch)
    }

    fn binding_projection(
        &self,
        code: Code,
        frames: &[super::Frame],
        string_bytes: &mut usize,
    ) -> Result<BindingScalar, ActionBindingRefusal> {
        use ActionBindingRefusal::{Limit, Unsupported};
        if code.len > 128 {
            return Err(Limit);
        }
        let mut r = crate::vm::instructions(self.plan.code(code)).peekable();
        let first = binding_step(&mut r)?;
        let op = first.op;
        let mut value = match op {
            Opcode::Number => Value::Number(first.number),
            Opcode::Bool => Value::Bool(first.args[0] != 0),
            Opcode::Unit => Value::Unit,
            Opcode::Str => {
                let id = exact_plan::StrId(first.args[0] as u32);
                if binding_step(&mut r)?.op != Opcode::Return || r.peek().is_some() {
                    return Err(Unsupported);
                }
                return BindingScalar::string(self.plan.str(id), string_bytes);
            }
            Opcode::LoadSlot => {
                let id = first.args[0] as u32;
                let row = self.plan.slots.get(id as usize).ok_or(Unsupported)?;
                match self.plan.owner_region(row) {
                    Some(owner) => super::Frame::row_of(frames, owner.0)
                        .and_then(|slots| slots.borrow().get(&id).cloned())
                        .ok_or(Unsupported)?,
                    None => self.slots.get(id as usize).cloned().ok_or(Unsupported)?,
                }
            }
            Opcode::LoadItem | Opcode::LoadBound => {
                let depth = first.args[0] as usize;
                let frame = frames
                    .len()
                    .checked_sub(depth + 1)
                    .and_then(|i| frames.get(i))
                    .ok_or(Unsupported)?;
                (if op == Opcode::LoadItem {
                    &frame.item
                } else {
                    &frame.bound
                })
                .clone()
                .ok_or(Unsupported)?
            }
            _ => return Err(Unsupported),
        };
        let mut fields = 0;
        loop {
            let step = binding_step(&mut r)?;
            match step.op {
                Opcode::Return if r.peek().is_none() => {
                    return BindingScalar::capture(&value, string_bytes)
                }
                Opcode::Field => {
                    fields += 1;
                    if fields > 8 {
                        return Err(Limit);
                    }
                    let index = step.args[0] as usize;
                    let Value::Record(record) = &value else {
                        return Err(Unsupported);
                    };
                    value = record.get(index).cloned().ok_or(Unsupported)?;
                }
                _ => return Err(Unsupported),
            }
        }
    }

    fn binding_action(
        &self,
        id: ActionsId,
        args: &[BindingScalar],
    ) -> Result<(), ActionBindingRefusal> {
        use ActionBindingRefusal::{Limit, Unsupported};
        let action = self.plan.action(id);
        if action.body.len > 1024
            || action.params.len as usize > BINDING_ARGS
            || action.writes.len > 8
        {
            return Err(Limit);
        }
        if action.params.len as usize != args.len() {
            return Err(Unsupported);
        }
        for (param, arg) in action.params.iter().zip(args) {
            if self.plan.type_(self.plan.param(param).ty).kind != arg.kind() {
                return Err(Unsupported);
            }
        }
        let writes: Vec<_> = action
            .writes
            .iter()
            .map(|w| self.plan.write(w).slot)
            .collect();
        for id in &writes {
            let slot = self.plan.slot(*id);
            if slot.owner.is_some() || !scalar_kind(self.plan.type_(slot.ty).kind) {
                return Err(Unsupported);
            }
        }
        let mut r = crate::vm::instructions(self.plan.code(action.body)).peekable();
        let mut stack = Vec::with_capacity(BINDING_ARGS);
        let (mut instructions, mut stores, mut literal_bytes) = (0, 0, 0usize);
        while r.peek().is_some() {
            instructions += 1;
            if instructions > 128 {
                return Err(Limit);
            }
            let step = binding_step(&mut r)?;
            let kind = match step.op {
                Opcode::Number => {
                    if !step.number.is_finite() {
                        return Err(Unsupported);
                    }
                    Some(TypeKind::Number)
                }
                Opcode::Bool => Some(TypeKind::Bool),
                Opcode::Unit => Some(TypeKind::Unit),
                Opcode::Str => {
                    let s = self.plan.str(exact_plan::StrId(step.args[0] as u32));
                    literal_bytes += s.len();
                    if s.len() > BINDING_STRING || literal_bytes > BINDING_STRING_TOTAL {
                        return Err(Limit);
                    }
                    Some(TypeKind::String)
                }
                Opcode::LoadParam => {
                    Some(args.get(step.args[0] as usize).ok_or(Unsupported)?.kind())
                }
                Opcode::StoreSlot => {
                    let id = exact_plan::SlotsId(step.args[0] as u32);
                    stores += 1;
                    if stores > 8 {
                        return Err(Limit);
                    }
                    if !writes.contains(&id) {
                        return Err(Unsupported);
                    }
                    let kind = self.plan.type_(self.plan.slot(id).ty).kind;
                    if stack.pop() != Some(kind) {
                        return Err(Unsupported);
                    }
                    None
                }
                Opcode::Return if r.peek().is_none() && stack.len() <= 1 => return Ok(()),
                _ => return Err(Unsupported),
            };
            if let Some(kind) = kind {
                if stack.len() == BINDING_ARGS {
                    return Err(Limit);
                }
                stack.push(kind);
            }
        }
        Err(Unsupported)
    }

    /// Runner-only collection events use the same action transaction and journal
    /// as host events. They have no payload; their bound arguments are evaluated
    /// in the list's scope when the edge dispatches (LLP 1054.000.006).
    pub(super) fn dispatch_edge(
        &mut self,
        view: ViewId,
        kind: EventKind,
    ) -> Result<(CommitReceipt, bool), RunnerError> {
        let name = match kind {
            EventKind::Reachstart => "reachstart",
            EventKind::Reachend => "reachend",
            _ => unreachable!("collection edge"),
        };
        let mut what = format!("{name} view {view}");
        let was_poisoned = self.poisoned;
        let mut changed = false;
        let result = (|| {
            let (node, frames) = self.find(view).ok_or(RunnerError::UnknownView(view))?;
            let handler = self
                .plan
                .node(node)
                .handlers
                .iter()
                .map(|h| self.plan.handler(h))
                .find(|h| h.event == kind)
                .cloned()
                .ok_or(RunnerError::NoHandler { view, event: name })?;
            let action = handler.action;
            let _ = write!(what, " ({})", self.plan.str(self.plan.action(action).name));
            let mut args = Vec::new();
            for a in handler.args.iter() {
                let code = self.plan.arg(a).expr;
                args.push(self.eval(code, &[], &frames)?);
            }
            let before = super::collection::EdgeState::capture(self, &frames);
            let receipt = self.run_action(action, args, &frames)?;
            changed = before.changed(self);
            Ok(receipt)
        })();
        self.log_outcome(&what, &result, was_poisoned);
        result.map(|receipt| (receipt, changed))
    }
    /// Deliver a host event to `view`: find its handler, evaluate the curried
    /// arguments in the instance's scope now, run the action, update.
    pub fn dispatch(&mut self, view: ViewId, event: Event) -> Result<CommitReceipt, RunnerError> {
        let mut what = super::lines::event(
            match &event {
                Event::ReorderDrop { .. } => "reorderdrop",
                Event::Press | Event::PressWith(_) => "press",
                Event::Input(_) => "input",
                Event::Change(_) => "change",
                Event::Cancel => "cancel",
                Event::Close => "close",
                Event::Toggle { .. } => "toggle",
                Event::Scrollend => "scrollend",
                Event::Select { .. } | Event::FieldSelect(..) => "select",
                Event::Hover(true) => "hover in",
                Event::Hover(false) => "hover out",
                Event::Focus => "focus",
                Event::Blur => "blur",
                Event::Focusin { .. } => "focusin",
                Event::Focusout { .. } => "focusout",
                Event::Key(..) => "key",
                Event::Keycapture(..) => "keycapture",
                Event::Keyup(..) => "keyup",
                Event::Submit => "submit",
                Event::Load => "load",
                Event::Message(_) => "message",
                Event::Contextmenu | Event::ContextmenuAt(_) => "contextmenu",
                Event::Beforeunload => "beforeunload",
                Event::Wheel(_) => "wheel",
                Event::Drop(_) => "drop",
                Event::Dblclick => "dblclick",
                Event::Pointerdown(_) => "pointerdown",
                Event::Pointerup(_) => "pointerup",
                Event::Pointermove(_) => "pointermove",
                Event::Swiperight => "swiperight",
                Event::Refresh => "refresh",
                Event::Scroll(_) => "scroll",
                Event::Resize(_) => "resize",
                Event::Intersect { .. } => "intersect",
                Event::Pan(_, _) => "pan",
                Event::PanRelease(_, _) => "panrelease",
                Event::Media(kind, _) | Event::Clipboard(kind, _) => kind.name(),
                Event::SelectionChange { .. } => "selectionchange",
                Event::Navigate(_) => "navigate",
                Event::HeightRelease { .. } => "heightrelease",
                Event::TransformGeometry { .. } => "transformgeometry",
                Event::TransformRelease { .. } => "transformrelease",
            },
            view,
        );
        let was_poisoned = self.poisoned;
        let outer = self.input_source.replace(view);
        let blur = matches!(event, Event::Blur);
        let path = blur.then(|| self.capture_blur_focus(view));
        let outgoing = matches!(event, Event::Focusout { .. })
            .then(|| self.detached_focus_target(view))
            .flatten();
        let commands = self.commands.len();
        let result = self.dispatch_inner(view, event, &mut what);
        if let Some(path) = path {
            self.finish_blur_focus(view, path);
        }
        if let Some(target) = outgoing {
            if result.is_err()
                || self.commands[commands..]
                    .iter()
                    .any(|c| c.name == "stopPropagation")
            {
                self.detached_focus.retain(|(id, _)| *id != target);
            }
        }
        self.input_source = outer;
        self.log_outcome(&what, &result, was_poisoned);
        result
    }

    fn dispatch_inner(
        &mut self,
        view: ViewId,
        event: Event,
        what: &mut String,
    ) -> Result<CommitReceipt, RunnerError> {
        if let Some(event) = event.invalid_payload() {
            return Err(RunnerError::InvalidEvent { event });
        }
        let (node, frames) = self
            .find(view)
            .or_else(|| {
                matches!(event, Event::Focusout { .. })
                    .then(|| self.take_detached_focus(view))
                    .flatten()
            })
            .ok_or(RunnerError::UnknownView(view))?;
        // Events without a listener do nothing: a picker's `cancel` (LLP
        // 1069.010 D3), a dialog's `close` (LLP 1021), a scroll's end.
        if matches!(event, Event::Cancel | Event::Close | Event::Scrollend) {
            let kind = match event {
                Event::Close => EventKind::Close,
                Event::Scrollend => EventKind::Scrollend,
                _ => EventKind::Cancel,
            };
            let has_handler = self
                .plan
                .node(node)
                .handlers
                .iter()
                .any(|h| self.plan.handler(h).event == kind);
            if !has_handler {
                what.push_str(" (no handler)");
                return Ok(CommitReceipt::default());
            }
        }
        // @ref LLP 1069.001 D4, LLP 1069.002 D3 — each control's payload is
        // its own kind's, held to HTML's rules (`control.rs`).
        let control = match &event {
            Event::Input(value) => Some(self.control_payload(view, "input", value)?),
            Event::Change(value) => Some(self.control_payload(view, "change", value)?),
            _ => None,
        };
        // The `InputEvent` (x2apps codeedit #2): the target as the event
        // leaves it, a text field's selection the host's report.
        let (control, record) = match (&event, control) {
            (Event::Input(v) | Event::Change(v), Some(p)) => {
                let reported = match v {
                    ControlValue::Field(_, s) | ControlValue::Replacement(_, s) => Some(*s),
                    _ => None,
                };
                let record = self.input_record(
                    view,
                    &p,
                    reported,
                    matches!(v, ControlValue::Replacement(..)),
                );
                (Some(p), Some(record))
            }
            (Event::FieldSelect(text, s), _) => (
                None,
                Some(self.input_record(view, &Value::str(text), Some(*s), false)),
            ),
            _ => (None, None),
        };
        let (kind, payload, name) = match &event {
            Event::ReorderDrop { .. } => (EventKind::Reorderdrop, None, "reorderdrop"),
            Event::FieldSelect(..) => (EventKind::Select, record.clone(), "select"),
            Event::Press | Event::PressWith(_) => (EventKind::Press, None, "press"),
            Event::Input(_) => (EventKind::Input, control, "input"),
            Event::Change(_) => (EventKind::Change, control, "change"),
            Event::Cancel => (EventKind::Cancel, None, "cancel"),
            Event::Close => (EventKind::Close, None, "close"),
            Event::Toggle { .. } => (EventKind::Toggle, None, "toggle"),
            Event::Scrollend => (EventKind::Scrollend, None, "scrollend"),
            Event::Select {
                formats,
                mixed,
                link,
                unavailable,
            } => (
                EventKind::Select,
                Some(Value::record(vec![
                    Value::str(formats),
                    Value::Bool(*mixed),
                    Value::str(link),
                    Value::str(unavailable),
                ])),
                "select",
            ),
            Event::Hover(over) => (EventKind::Hover, Some(Value::Bool(*over)), "hover"),
            Event::Focus => (EventKind::Focus, None, "focus"),
            Event::Blur => (EventKind::Blur, None, "blur"),
            Event::Focusin { .. } => (EventKind::Focusin, None, "focusin"),
            Event::Focusout { .. } => (EventKind::Focusout, None, "focusout"),
            Event::Key(k) => (EventKind::Key, Some(Value::str(&k.key)), "key"),
            Event::Keycapture(k) => (
                EventKind::Keycapture,
                Some(Value::str(&k.key)),
                "keycapture",
            ),
            Event::Keyup(k) => (EventKind::Keyup, Some(Value::str(&k.key)), "keyup"),
            Event::Submit => (EventKind::Submit, None, "submit"),
            Event::Load => (EventKind::Load, None, "load"),
            Event::Message(message) => (EventKind::Message, Some(Value::str(message)), "message"),
            Event::Contextmenu | Event::ContextmenuAt(_) => {
                (EventKind::Contextmenu, None, "contextmenu")
            }
            Event::Beforeunload => (EventKind::Beforeunload, None, "beforeunload"),
            Event::Wheel(_) => (EventKind::Wheel, None, "wheel"),
            Event::Drop(_) => (EventKind::Drop, None, "drop"),
            Event::Dblclick => (EventKind::Dblclick, None, "dblclick"),
            Event::Pointerdown(_) => (EventKind::Pointerdown, None, "pointerdown"),
            Event::Pointerup(_) => (EventKind::Pointerup, None, "pointerup"),
            Event::Pointermove(_) => (EventKind::Pointermove, None, "pointermove"),
            Event::Swiperight => (EventKind::Swiperight, None, "swiperight"),
            Event::Refresh => (EventKind::Refresh, None, "refresh"),
            Event::Scroll(_) => (EventKind::Scroll, None, "scroll"),
            Event::Resize(_) => (EventKind::Resize, None, "resize"),
            Event::Intersect { .. } => (EventKind::Intersect, None, "intersect"),
            Event::Media(kind, value) => (
                *kind,
                match kind {
                    EventKind::Timeupdate | EventKind::Durationchange => {
                        exact_num::parse_f64(value).ok().map(Value::Number)
                    }
                    EventKind::Error => Some(Value::str(value)),
                    EventKind::Fullscreenchange => Some(Value::Bool(value == "true")),
                    _ => None,
                },
                kind.name(),
            ),
            Event::Clipboard(kind, _) => (*kind, None, kind.name()),
            Event::SelectionChange { .. } => (EventKind::Selectionchange, None, "selectionchange"),
            Event::Pan(_, _) => (EventKind::Pan, None, "pan"),
            Event::PanRelease(_, _) => (EventKind::Panrelease, None, "panrelease"),
            Event::HeightRelease { .. } => (EventKind::Heightrelease, None, "heightrelease"),
            Event::TransformGeometry { .. } => {
                (EventKind::Transformgeometry, None, "transformgeometry")
            }
            Event::TransformRelease { .. } => {
                (EventKind::Transformrelease, None, "transformrelease")
            }
            Event::Navigate(location) => {
                (EventKind::Navigate, Some(Value::str(location)), "navigate")
            }
        };
        let row = self.plan.node(node);
        let handler = row
            .handlers
            .iter()
            .map(|h| self.plan.handler(h))
            .find(|h| h.event == kind)
            .cloned()
            .ok_or(RunnerError::NoHandler { view, event: name })?;
        let mut args = Vec::new();
        for a in handler.args.iter() {
            let code = self.plan.arg(a).expr;
            args.push(self.eval(code, &[], &frames)?);
        }
        if let Some(p) = payload {
            // A navigate action may deliberately ignore its location (D8).
            if kind != EventKind::Navigate || !self.plan.action(handler.action).params.is_empty() {
                args.push(p);
            }
        }
        // The event's record, after what it always carries, to an action
        // that declares one more parameter (`contract_types::event_record`).
        // A drop's `ReorderEvent` names its lists (LLP 1094 D2).
        let record = match &event {
            Event::ReorderDrop { .. } => Some(self.reorder_record(view)),
            Event::Input(_) | Event::Change(_) => record,
            Event::Scroll(s)
                if self.plan.action(handler.action).params.len as usize > args.len() + 2 =>
            {
                let anchor = self.links.lists.and_then(|lists| {
                    self.tree.as_ref().and_then(|tree| {
                        (lists.scroll_anchor)(
                            tree,
                            &self.kernel,
                            view,
                            s.left,
                            s.top,
                            s.client_width,
                            s.client_height,
                        )
                    })
                });
                Some(s.value(anchor))
            }
            _ => event.record(),
        };
        match event {
            Event::ReorderDrop { item, before } => {
                args.push(Value::str(&item));
                args.push(before.map_or(Value::NONE, |s| Value::some(Value::str(&s))));
            }
            Event::Scroll(ScrollEvent { left, top, .. })
            | Event::Resize(super::ResizeRect {
                width: left,
                height: top,
                ..
            })
            | Event::Pan(left, top)
            | Event::PanRelease(left, top) => {
                args.extend([Value::Number(left), Value::Number(top)])
            }
            Event::HeightRelease { height, velocity } => {
                args.extend([Value::Number(height), Value::Number(velocity)]);
            }
            Event::TransformGeometry {
                box_width,
                box_height,
                port_width,
                port_height,
            } => {
                args.extend([box_width, box_height, port_width, port_height].map(Value::Number));
            }
            Event::TransformRelease {
                x,
                y,
                scale,
                vx,
                vy,
                vscale,
            } => {
                args.extend([x, y, scale, vx, vy, vscale].map(Value::Number));
            }
            _ => {}
        }
        if let Some(record) = record {
            if self.plan.action(handler.action).params.len as usize > args.len() {
                args.push(record);
            }
        }
        super::lines::ran(what, self.plan.str(self.plan.action(handler.action).name));
        self.run_action(handler.action, args, &frames)
    }
}

#[cfg(test)]
mod retained_binding_tests;
