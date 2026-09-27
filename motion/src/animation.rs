//! `animation`: CSS keyframe animations over the compositor properties.
//!
//! @ref LLP 1057 (keyframe animation); CSS Animations Level 1; Web Animations
//! Level 1 §4 (the timing model: phases, active time, iteration progress)
//!
//! An [`Animations`] list is the value of a node's `animation` style row. The
//! compiler resolves each `@keyframes` name before the row exists, so the row
//! carries its keyframes, never a name to look up. The web host emits the row
//! as a real `@keyframes` rule and `animation` declaration; every other host
//! samples it here, under the engine's seekable clock. Each value is a
//! closed-form function of local time, so a seek to `t` is the same bits
//! however it was reached.
//!
//! What a browser does, this does: the timing function applies per keyframe
//! interval (not across the iteration); a keyframe may name its own
//! `animation-timing-function`; a property missing from the `0%`/`100%`
//! keyframes interpolates from and to its underlying value; later animations
//! in the list replace earlier ones' values for a property they animate.

use crate::easing::{css_number, Easing, EasingError};
use crate::property::{Property, Value};

/// Most animations one node may declare.
pub const MAX_ANIMATIONS: usize = 8;

/// Most keyframe blocks one `@keyframes` rule may carry on the wire.
pub const MAX_KEYFRAMES: usize = 32;

/// One keyframe block: an offset, an optional per-interval easing, and the
/// values it sets.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyframeBlock {
    /// Position in the iteration, `[0, 1]` (`from` is 0, `to` is 1).
    pub offset: f64,
    /// `animation-timing-function` inside the block: the easing of the
    /// interval that starts here. `None` takes the animation's.
    pub easing: Option<Easing>,
    /// The values this keyframe sets, one per property.
    pub values: Vec<(Property, Value)>,
}

/// A resolved `@keyframes` rule: its authored name and its blocks, sorted by
/// offset with equal offsets merged.
#[derive(Debug, Clone, PartialEq)]
pub struct Keyframes {
    /// The authored name. Identity, and the web's rule name, derive from it.
    pub name: String,
    /// The blocks, in increasing offset.
    pub blocks: Vec<KeyframeBlock>,
}

/// `animation-direction`, in wire order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// `normal`.
    #[default]
    Normal,
    /// `reverse`.
    Reverse,
    /// `alternate`.
    Alternate,
    /// `alternate-reverse`.
    AlternateReverse,
}

/// `animation-fill-mode`, in wire order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FillMode {
    /// `none`.
    #[default]
    None,
    /// `forwards`.
    Forwards,
    /// `backwards`.
    Backwards,
    /// `both`.
    Both,
}

/// `animation-play-state`, in wire order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayState {
    /// `running`.
    #[default]
    Running,
    /// `paused`: local time holds where it was.
    Paused,
}

impl Direction {
    /// Every value, in wire order.
    pub const ALL: [Direction; 4] = [
        Direction::Normal,
        Direction::Reverse,
        Direction::Alternate,
        Direction::AlternateReverse,
    ];
    /// The CSS keyword.
    pub fn name(self) -> &'static str {
        ["normal", "reverse", "alternate", "alternate-reverse"][self as usize]
    }
}

impl FillMode {
    /// Every value, in wire order.
    pub const ALL: [FillMode; 4] = [
        FillMode::None,
        FillMode::Forwards,
        FillMode::Backwards,
        FillMode::Both,
    ];
    /// The CSS keyword.
    pub fn name(self) -> &'static str {
        ["none", "forwards", "backwards", "both"][self as usize]
    }
}

impl PlayState {
    /// Every value, in wire order.
    pub const ALL: [PlayState; 2] = [PlayState::Running, PlayState::Paused];
    /// The CSS keyword.
    pub fn name(self) -> &'static str {
        ["running", "paused"][self as usize]
    }
}

/// One entry of the `animation` shorthand, its name resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct Animation {
    /// The keyframes it plays.
    pub keyframes: Keyframes,
    /// `animation-duration`, seconds (one iteration).
    pub duration: f64,
    /// `animation-timing-function`: each interval's easing unless its
    /// keyframe names one.
    pub easing: Easing,
    /// `animation-delay`, seconds; negative starts partway through.
    pub delay: f64,
    /// `animation-iteration-count`; `f64::INFINITY` is `infinite`.
    pub iterations: f64,
    /// `animation-direction`.
    pub direction: Direction,
    /// `animation-fill-mode`.
    pub fill: FillMode,
    /// `animation-play-state`.
    pub play_state: PlayState,
}

/// A node's `animation` row: zero to [`MAX_ANIMATIONS`] animations.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Animations(pub Vec<Animation>);

/// Why an animation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationError {
    /// More animations than [`MAX_ANIMATIONS`].
    TooMany,
    /// More keyframe blocks than [`MAX_KEYFRAMES`].
    TooManyKeyframes,
    /// A keyframes rule with no name.
    EmptyName,
    /// A time, count, offset or value was NaN or infinite (a count may be
    /// `infinite`, which is `+∞`).
    NonFinite,
    /// `animation-duration` was negative.
    NegativeDuration,
    /// `animation-iteration-count` was negative.
    NegativeIterations,
    /// A keyframe offset lay outside `[0, 1]`.
    OffsetOutOfRange,
    /// Keyframe offsets were not strictly increasing.
    KeyframesNotSorted,
    /// A keyframe named a property twice.
    DuplicateProperty(Property),
    /// A keyframe named a property keyframes do not animate (numeric
    /// `height` is a transition-only trial, LLP 1002 D7; `box-shadow` is a
    /// transition's, LLP 1062).
    NotAnimatable(Property),
    /// A scalar property carried a second component.
    InvalidValueShape,
    /// An easing was invalid.
    Easing(EasingError),
    /// An `exit-animation` that never ends — an `infinite` count, or
    /// `paused` — would keep its leaving node forever (LLP 1063).
    Endless,
}

impl Keyframes {
    /// Keyframes from blocks in any order: sorted by offset, blocks at one
    /// offset merged with the later value winning (CSS Animations §3:
    /// keyframes at the same offset cascade), then validated.
    pub fn new(name: &str, mut blocks: Vec<KeyframeBlock>) -> Result<Keyframes, AnimationError> {
        blocks.sort_by(|a, b| a.offset.total_cmp(&b.offset));
        let mut merged: Vec<KeyframeBlock> = Vec::with_capacity(blocks.len());
        for block in blocks {
            match merged.last_mut() {
                Some(last) if last.offset == block.offset => {
                    for (property, value) in block.values {
                        last.values.retain(|(p, _)| *p != property);
                        last.values.push((property, value));
                    }
                    if block.easing.is_some() {
                        last.easing = block.easing;
                    }
                }
                _ => merged.push(block),
            }
        }
        let keyframes = Keyframes {
            name: name.to_string(),
            blocks: merged,
        };
        keyframes.validate()?;
        Ok(keyframes)
    }

    /// Check every rule the sampler relies on.
    pub fn validate(&self) -> Result<(), AnimationError> {
        if self.name.is_empty() {
            return Err(AnimationError::EmptyName);
        }
        if self.blocks.len() > MAX_KEYFRAMES {
            return Err(AnimationError::TooManyKeyframes);
        }
        let mut last = None;
        for block in &self.blocks {
            if !block.offset.is_finite() {
                return Err(AnimationError::NonFinite);
            }
            if !(0.0..=1.0).contains(&block.offset) {
                return Err(AnimationError::OffsetOutOfRange);
            }
            if last.is_some_and(|last| block.offset <= last) {
                return Err(AnimationError::KeyframesNotSorted);
            }
            last = Some(block.offset);
            if let Some(easing) = &block.easing {
                easing.validate().map_err(AnimationError::Easing)?;
            }
            for (i, (property, value)) in block.values.iter().enumerate() {
                if matches!(
                    property,
                    Property::Height | Property::BoxShadow | Property::ShadowColor
                ) {
                    return Err(AnimationError::NotAnimatable(*property));
                }
                if block.values[..i].iter().any(|(p, _)| p == property) {
                    return Err(AnimationError::DuplicateProperty(*property));
                }
                if !value.is_finite() {
                    return Err(AnimationError::NonFinite);
                }
                if !value.fits(*property) {
                    return Err(AnimationError::InvalidValueShape);
                }
            }
        }
        Ok(())
    }

    /// Whether any block sets `property`.
    pub fn affects(&self, property: Property) -> bool {
        self.blocks
            .iter()
            .any(|b| b.values.iter().any(|(p, _)| *p == property))
    }

    /// The rule as CSS, under `name`: `@keyframes name{0%{opacity:0.4}…}`.
    /// The web host names it uniquely per list; the compiler writes it under
    /// the authored name into the row's text form.
    pub fn rule(&self, name: &str) -> String {
        let mut out = String::from("@keyframes ");
        out.push_str(name);
        out.push('{');
        for block in &self.blocks {
            css_number(&mut out, block.offset * 100.0);
            out.push_str("%{");
            for (property, value) in &block.values {
                out.push_str(property.css_name());
                out.push(':');
                match property {
                    p if p.is_color() => {
                        let [r, g, b, a] = value.straight();
                        out.push_str("rgba(");
                        for c in [r, g, b] {
                            css_number(&mut out, (c * 255.0).round());
                            out.push(',');
                        }
                        css_number(&mut out, a);
                        out.push(')');
                    }
                    Property::Translate => {
                        css_number(&mut out, value.x);
                        out.push_str("px ");
                        css_number(&mut out, value.y);
                        out.push_str("px");
                    }
                    Property::Rotate => {
                        css_number(&mut out, value.x);
                        out.push_str("deg");
                    }
                    _ => css_number(&mut out, value.x),
                }
                out.push(';');
            }
            if let Some(easing) = &block.easing {
                out.push_str("animation-timing-function:");
                out.push_str(&easing.css());
                out.push(';');
            }
            out.push('}');
        }
        out.push('}');
        out
    }
}

/// Where local time falls (Web Animations §4.6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Before,
    Active,
    After,
}

impl Animation {
    /// Check every rule the sampler relies on.
    pub fn validate(&self) -> Result<(), AnimationError> {
        if !self.duration.is_finite() || !self.delay.is_finite() || self.iterations.is_nan() {
            return Err(AnimationError::NonFinite);
        }
        if self.duration < 0.0 {
            return Err(AnimationError::NegativeDuration);
        }
        if self.iterations < 0.0 {
            return Err(AnimationError::NegativeIterations);
        }
        self.easing.validate().map_err(AnimationError::Easing)?;
        self.keyframes.validate()
    }

    /// Whether every number is finite, save an `infinite` iteration count.
    pub fn is_finite(&self) -> bool {
        self.duration.is_finite()
            && self.delay.is_finite()
            && !self.iterations.is_nan()
            && self.easing.is_finite()
            && self.keyframes.blocks.iter().all(|b| {
                b.offset.is_finite()
                    && b.easing.as_ref().is_none_or(Easing::is_finite)
                    && b.values.iter().all(|(_, v)| v.is_finite())
            })
    }

    /// Duration times iteration count; zero when either is (Web Animations
    /// §4.8: a zero-length iteration has no active interval, even repeated
    /// forever). Infinite for an endless animation.
    pub fn active_duration(&self) -> f64 {
        if self.duration == 0.0 || self.iterations == 0.0 {
            0.0
        } else {
            self.duration * self.iterations
        }
    }

    /// Local time at which the animation ends: delay plus active duration,
    /// never negative. Infinite for an endless animation.
    pub fn end_time(&self) -> f64 {
        (self.delay + self.active_duration()).max(0.0)
    }

    /// The directed progress through the current iteration at `local`
    /// seconds since the animation started, or `None` when it applies no
    /// value there (outside its active interval and not filling).
    pub fn progress(&self, local: f64) -> Option<f64> {
        let active = self.active_duration();
        let end = self.end_time();
        let before_active = self.delay.min(end).max(0.0);
        let active_after = (self.delay + active).min(end).max(0.0);
        let backwards = matches!(self.fill, FillMode::Backwards | FillMode::Both);
        let forwards = matches!(self.fill, FillMode::Forwards | FillMode::Both);
        let (phase, time) = if local < before_active {
            (
                Phase::Before,
                backwards.then(|| (local - self.delay).max(0.0)),
            )
        } else if local >= active_after {
            (
                Phase::After,
                forwards.then(|| (local - self.delay).min(active).max(0.0)),
            )
        } else {
            (Phase::Active, Some(local - self.delay))
        };
        let time = time?;
        let overall = if self.duration == 0.0 {
            if phase == Phase::Before {
                0.0
            } else {
                self.iterations
            }
        } else {
            time / self.duration
        };
        let mut simple = if overall.is_infinite() {
            0.0
        } else {
            overall % 1.0
        };
        // The end of a whole iteration is progress 1 of that iteration, not
        // progress 0 of the next (§4.8.3.3).
        if simple == 0.0 && phase != Phase::Before && time == active && self.iterations != 0.0 {
            simple = 1.0;
        }
        let iteration = if phase == Phase::After && self.iterations.is_infinite() {
            f64::INFINITY
        } else if simple == 1.0 {
            overall.floor() - 1.0
        } else {
            overall.floor()
        };
        let odd = iteration.is_finite() && iteration % 2.0 != 0.0;
        let reverse = match self.direction {
            Direction::Normal => false,
            Direction::Reverse => true,
            Direction::Alternate => odd,
            Direction::AlternateReverse => iteration.is_finite() && !odd,
        };
        Some(if reverse { 1.0 - simple } else { simple })
    }

    /// The value this animation gives `property` at directed progress `p`
    /// over `underlying`, or `None` when no keyframe sets the property. A
    /// missing `0%`/`100%` keyframe is the underlying value (CSS Animations
    /// §3.3); the interval's easing is its start keyframe's, else the
    /// animation's.
    pub fn value(&self, property: Property, p: f64, underlying: Value) -> Option<Value> {
        let mut start = (0.0, underlying, &self.easing);
        let mut end = None;
        let mut any = false;
        for block in &self.keyframes.blocks {
            let Some(&(_, value)) = block.values.iter().find(|(q, _)| *q == property) else {
                continue;
            };
            any = true;
            if block.offset <= p {
                start = (
                    block.offset,
                    value,
                    block.easing.as_ref().unwrap_or(&self.easing),
                );
            } else {
                end = Some((block.offset, value));
                break;
            }
        }
        if !any {
            return None;
        }
        let (end_offset, end_value) = end.unwrap_or((1.0, underlying));
        if end_offset <= start.0 {
            return Some(start.1);
        }
        let local = (p - start.0) / (end_offset - start.0);
        Some(start.1.lerp(end_value, start.2.progress(local)))
    }

    /// The shorthand as CSS, the keyframes under `name`.
    pub fn css(&self, name: &str) -> String {
        let mut out = String::from(name);
        out.push(' ');
        css_number(&mut out, self.duration);
        out.push_str("s ");
        out.push_str(&self.easing.css());
        out.push(' ');
        css_number(&mut out, self.delay);
        out.push_str("s ");
        if self.iterations.is_infinite() {
            out.push_str("infinite");
        } else {
            css_number(&mut out, self.iterations);
        }
        for word in [
            self.direction.name(),
            self.fill.name(),
            self.play_state.name(),
        ] {
            out.push(' ');
            out.push_str(word);
        }
        out
    }
}

impl Animations {
    /// No animations: CSS's initial `animation: none`.
    pub const NONE: Animations = Animations(Vec::new());

    /// Validate every animation and the count.
    pub fn validate(&self) -> Result<(), AnimationError> {
        if self.0.len() > MAX_ANIMATIONS {
            return Err(AnimationError::TooMany);
        }
        self.0.iter().try_for_each(Animation::validate)
    }

    /// Whether every number is finite, save `infinite` iteration counts.
    pub fn is_finite(&self) -> bool {
        self.0.iter().all(Animation::is_finite)
    }

    /// Local time at which the last animation ends; zero for none, infinite
    /// when one is endless.
    pub fn end_time(&self) -> f64 {
        self.0.iter().map(Animation::end_time).fold(0.0, f64::max)
    }

    /// [`Animations::validate`], and every animation runs to an end: the
    /// rule for an `exit-animation`, whose node is removed when it ends.
    pub fn validate_ending(&self) -> Result<(), AnimationError> {
        self.validate()?;
        if self
            .0
            .iter()
            .any(|a| a.play_state == PlayState::Paused || !a.end_time().is_finite())
        {
            return Err(AnimationError::Endless);
        }
        Ok(())
    }

    /// The row as its self-contained text: the shorthand list, then each
    /// distinct `@keyframes` rule it names. [`Animations::parse`] reads it
    /// back; it is the form a compiled plan carries.
    pub fn text(&self) -> String {
        if self.0.is_empty() {
            return "none".into();
        }
        let mut out = String::new();
        for (i, a) in self.0.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(&a.css(&a.keyframes.name));
        }
        for (i, a) in self.0.iter().enumerate() {
            if self.0[..i].iter().all(|b| b.keyframes != a.keyframes) {
                out.push(' ');
                out.push_str(&a.keyframes.rule(&a.keyframes.name));
            }
        }
        out
    }
}
