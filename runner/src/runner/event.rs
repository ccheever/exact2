//! Typed host events and their action dispatch; no host clock or gesture ownership.

use super::{DataSource, Runner, RunnerError};
use exact_kernel::{CommitReceipt, ViewId};
use exact_plan::{EventKind, Value};
use std::fmt::Write as _;

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
    /// A press on the view.
    Press,
    /// A text input changed to `value`.
    Change(String),
    /// The pointer came over the view (`true`) or left it (`false`) —
    /// `pointerenter`/`pointerleave`, not a bubbling `mouseover`.
    Hover(bool),
    /// The view took the focus.
    Focus,
    /// The view lost the focus.
    Blur,
    /// A key went down while the view had the focus: the key's name as the
    /// web spells it (`"Enter"`, `"ArrowDown"`, `"a"`).
    Key(String),
    /// Enter in an input with a `submit` handler — the web's implicit
    /// submission (HTML forms §4.10.21.2), without a form.
    Submit,
    /// An iframe finished loading (including an error document on the web).
    Load,
    /// An iframe guest posted a string to its parent (@ref LLP 1020 D2).
    Message(String),
    /// The platform requested a context menu (secondary click or long press).
    Contextmenu,
    /// A double click, or the platform’s double tap.
    Dblclick,
    /// A platform-recognized right swipe.
    Swiperight,
    /// A changed scroll position, in CSS pixels (left, top).
    Scroll(f64, f64),
    /// A standard media event; numeric time payloads are seconds.
    Media(EventKind, String),
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
        /// Finite signed scale units per second; primary pan supplies zero.
        vscale: f64,
    },
}

impl Event {
    /// Decode a media event carried as `name\npayload` through host kind 18.
    pub fn media_payload(payload: &str) -> Option<Self> {
        let (name, value) = payload.split_once('\n')?;
        let kind = EventKind::from_name(name)?;
        if (kind as u8) < EventKind::Loadedmetadata as u8 {
            return None;
        }
        if matches!(kind, EventKind::Timeupdate | EventKind::Durationchange)
            && !value.parse::<f64>().ok()?.is_finite()
        {
            return None;
        }
        Some(Self::Media(kind, value.into()))
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

    fn invalid_payload(&self) -> Option<&'static str> {
        match *self {
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

    /// Decode exactly `height,velocity`. Hosts must parse before advancing time.
    /// This synthesizes an event; physical delivery separately validates both
    /// binding generations and a live Height token before clock or action.
    pub fn height_release_payload(payload: &str) -> Option<Self> {
        let (height, velocity) = payload.split_once(',')?;
        let (height, velocity) = (height.parse().ok()?, velocity.parse().ok()?);
        valid_height_release(height, velocity).then_some(Self::HeightRelease { height, velocity })
    }

    /// Decode the scroll event's two finite CSS-pixel coordinates.
    pub fn scroll_payload(payload: &str) -> Option<Self> {
        let (left, top) = payload.split_once(',')?;
        let (left, top) = (left.parse::<f64>().ok()?, top.parse::<f64>().ok()?);
        (left.is_finite() && top.is_finite()).then_some(Self::Scroll(left, top))
    }
}

fn tuple<const N: usize>(payload: &str) -> Option<[f64; N]> {
    let mut parts = payload.split(',');
    let mut values = [0.0; N];
    for value in &mut values {
        *value = parts.next()?.parse().ok()?;
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
    /// Deliver a host event to `view`: find its handler, evaluate the curried
    /// arguments in the instance's scope now, run the action, update.
    pub fn dispatch(&mut self, view: ViewId, event: Event) -> Result<CommitReceipt, RunnerError> {
        let mut what = format!(
            "{} view {view}",
            match &event {
                Event::ReorderDrop { .. } => "reorderdrop",
                Event::Press => "press",
                Event::Change(_) => "change",
                Event::Hover(true) => "hover in",
                Event::Hover(false) => "hover out",
                Event::Focus => "focus",
                Event::Blur => "blur",
                Event::Key(_) => "key",
                Event::Submit => "submit",
                Event::Load => "load",
                Event::Message(_) => "message",
                Event::Contextmenu => "contextmenu",
                Event::Dblclick => "dblclick",
                Event::Swiperight => "swiperight",
                Event::Scroll(_, _) => "scroll",
                Event::Media(kind, _) => kind.name(),
                Event::Navigate(_) => "navigate",
                Event::HeightRelease { .. } => "heightrelease",
                Event::TransformGeometry { .. } => "transformgeometry",
                Event::TransformRelease { .. } => "transformrelease",
            }
        );
        let was_poisoned = self.poisoned;
        let result = self.dispatch_inner(view, event, &mut what);
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
            .tree
            .as_ref()
            .and_then(|t| t.find(view))
            .ok_or(RunnerError::UnknownView(view))?;
        let (kind, payload, name) = match &event {
            Event::ReorderDrop { .. } => (EventKind::Reorderdrop, None, "reorderdrop"),
            Event::Press => (EventKind::Press, None, "press"),
            Event::Change(text) => (EventKind::Change, Some(Value::str(text)), "change"),
            Event::Hover(over) => (EventKind::Hover, Some(Value::Bool(*over)), "hover"),
            Event::Focus => (EventKind::Focus, None, "focus"),
            Event::Blur => (EventKind::Blur, None, "blur"),
            Event::Key(key) => (EventKind::Key, Some(Value::str(key)), "key"),
            Event::Submit => (EventKind::Submit, None, "submit"),
            Event::Load => (EventKind::Load, None, "load"),
            Event::Message(message) => (EventKind::Message, Some(Value::str(message)), "message"),
            Event::Contextmenu => (EventKind::Contextmenu, None, "contextmenu"),
            Event::Dblclick => (EventKind::Dblclick, None, "dblclick"),
            Event::Swiperight => (EventKind::Swiperight, None, "swiperight"),
            Event::Scroll(_, _) => (EventKind::Scroll, None, "scroll"),
            Event::Media(kind, value) => (
                *kind,
                match kind {
                    EventKind::Timeupdate | EventKind::Durationchange => {
                        value.parse().ok().map(Value::Number)
                    }
                    EventKind::Error => Some(Value::str(value)),
                    _ => None,
                },
                kind.name(),
            ),
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
        match event {
            Event::ReorderDrop { item, before } => {
                args.push(Value::str(&item));
                args.push(before.map_or(Value::NONE, |s| Value::some(Value::str(&s))));
            }
            Event::Scroll(left, top) => args.extend([Value::Number(left), Value::Number(top)]),
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
        let _ = write!(
            what,
            " ({})",
            self.plan.str(self.plan.action(handler.action).name)
        );
        self.run_action(handler.action, args, &frames)
    }
}
