//! CSS Animations: `@keyframes` and the `animation` row, sampled in closed form.
//!
//! @ref LLP 1055 D5 (CSS Animations Level 1 over the Web Animations timing
//! model); LLP 1002 D1 (the style row is the binding)
//!
//! An [`Animations`] list is the value of a node's `animation` style row. Each
//! entry carries the keyframes its `animation-name` resolved to, so a host has
//! everything it needs from the node's style: the web emits it as CSS, Apple
//! lowers it to Core Animation, and the engine samples it everywhere else.
//! Sampling is a pure function of local time, so a seek and sixty frames give
//! the same bits.
//!
//! A keyframe colour may be a `light-dark()` pair (LLP 1062 D9): the light
//! value is the keyframe's, the dark one rides beside it, and which one plays
//! is the appearance the animation started under, as Chrome resolves a rule
//! once.

use crate::easing::{Easing, EasingError};
use crate::property::{Property, Value};

mod parse;
pub use parse::{easing_css, link, value_css, LONGHANDS};

/// Most animations one node may declare.
pub const MAX_ANIMATIONS: usize = 8;
/// Most keyframes one `@keyframes` rule may hold.
pub const MAX_KEYFRAMES: usize = 32;

/// `animation-direction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Direction {
    /// Every iteration plays forwards.
    #[default]
    Normal = 0,
    /// Every iteration plays backwards.
    Reverse = 1,
    /// Even iterations forwards, odd backwards.
    Alternate = 2,
    /// Even iterations backwards, odd forwards.
    AlternateReverse = 3,
}

/// `animation-fill-mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum FillMode {
    /// Nothing outside the active interval.
    #[default]
    None = 0,
    /// The last value after the end.
    Forwards = 1,
    /// The first value during the delay.
    Backwards = 2,
    /// Both.
    Both = 3,
}

impl Direction {
    /// The CSS keyword.
    pub fn name(self) -> &'static str {
        ["normal", "reverse", "alternate", "alternate-reverse"][self as usize]
    }
    /// From the wire discriminant.
    pub fn from_wire(v: u8) -> Option<Direction> {
        [
            Direction::Normal,
            Direction::Reverse,
            Direction::Alternate,
            Direction::AlternateReverse,
        ]
        .get(v as usize)
        .copied()
    }
}

impl FillMode {
    /// The CSS keyword.
    pub fn name(self) -> &'static str {
        ["none", "forwards", "backwards", "both"][self as usize]
    }
    /// From the wire discriminant.
    pub fn from_wire(v: u8) -> Option<FillMode> {
        [
            FillMode::None,
            FillMode::Forwards,
            FillMode::Backwards,
            FillMode::Both,
        ]
        .get(v as usize)
        .copied()
    }
    fn backwards(self) -> bool {
        matches!(self, FillMode::Backwards | FillMode::Both)
    }
    fn forwards(self) -> bool {
        matches!(self, FillMode::Forwards | FillMode::Both)
    }
}

/// One keyframe: an offset in `[0, 1]`, its own timing function if it set
/// one, and the values it declares.
#[derive(Debug, Clone, PartialEq)]
pub struct Keyframe {
    /// The selector as a fraction (`from` is 0, `to` is 1).
    pub offset: f64,
    /// This keyframe's `animation-timing-function`, applying to the interval
    /// that starts here; `None` takes the animation's.
    pub easing: Option<Easing>,
    /// The declared values, one per property.
    pub values: Vec<(Property, Value)>,
    /// A `light-dark()` colour's value under a dark appearance, for each of
    /// `values` that has one; `values` holds the light one (LLP 1062 D9).
    pub dark: Vec<(Property, Value)>,
}

impl Keyframe {
    /// A property's value under an appearance.
    fn get(&self, property: Property, dark: bool) -> Option<Value> {
        let own =
            |list: &[(Property, Value)]| list.iter().find(|(q, _)| *q == property).map(|(_, v)| *v);
        dark.then(|| own(&self.dark))
            .flatten()
            .or_else(|| own(&self.values))
    }
}

/// A resolved `@keyframes` rule: keyframes in offset order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Keyframes(pub Vec<Keyframe>);

/// One entry of the `animation` row.
#[derive(Debug, Clone, PartialEq)]
pub struct Animation {
    /// `animation-name`, as authored.
    pub name: String,
    /// `animation-duration`, seconds.
    pub duration: f64,
    /// `animation-delay`, seconds; negative starts partway through.
    pub delay: f64,
    /// `animation-timing-function`, per keyframe interval. Never a spring.
    pub easing: Easing,
    /// `animation-iteration-count`; `f64::INFINITY` is `infinite`.
    pub iterations: f64,
    /// `animation-direction`.
    pub direction: Direction,
    /// `animation-fill-mode`.
    pub fill: FillMode,
    /// `animation-play-state: paused`.
    pub paused: bool,
    /// The keyframes the name resolved to; empty when no rule has the name
    /// (CSS keeps such an animation: it runs and animates nothing).
    pub keyframes: Keyframes,
}

/// The `animation` row: up to [`MAX_ANIMATIONS`] entries. Later entries win
/// over earlier ones for a property both animate, as in CSS's composite order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Animations(pub Vec<Animation>);

/// Why an animation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationError {
    /// More than [`MAX_ANIMATIONS`] entries.
    TooMany,
    /// More than [`MAX_KEYFRAMES`] keyframes, or more than one value per
    /// property in one keyframe.
    TooManyKeyframes,
    /// A time, count, offset or value was not finite.
    NonFinite,
    /// A negative duration or iteration count, or an offset outside `[0, 1]`.
    OutOfRange,
    /// Keyframes out of offset order.
    Unordered,
    /// A scalar property carried a second component.
    ValueShape,
    /// The easing was invalid.
    Easing(EasingError),
    /// A property keyframes do not animate: half of a `box-shadow` without
    /// the other, or a dark value for no colour of the keyframe.
    NotAnimatable(Property),
    /// An `-exact-exit-animation` that never ends — an `infinite` count, or
    /// `paused` — would keep its leaving node forever (LLP 1063).
    Endless,
}

impl Default for Animation {
    fn default() -> Self {
        Animation {
            name: String::new(),
            duration: 0.0,
            delay: 0.0,
            easing: Easing::Ease,
            iterations: 1.0,
            direction: Direction::Normal,
            fill: FillMode::None,
            paused: false,
            keyframes: Keyframes::default(),
        }
    }
}

/// Where local time falls (Web Animations §4.5.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Before the delay ends.
    Before,
    /// Playing.
    Active,
    /// After the last iteration.
    After,
}

impl Keyframes {
    /// Every property some keyframe declares, in wire order.
    pub fn properties(&self) -> Vec<Property> {
        let mut out: Vec<Property> = Vec::new();
        for frame in &self.0 {
            for (p, _) in &frame.values {
                if !out.contains(p) {
                    out.push(*p);
                }
            }
        }
        out.sort();
        out
    }

    /// Check the rule's shape.
    pub fn validate(&self) -> Result<(), AnimationError> {
        if self.0.len() > MAX_KEYFRAMES {
            return Err(AnimationError::TooManyKeyframes);
        }
        let mut last = 0.0;
        for frame in &self.0 {
            if !frame.offset.is_finite() {
                return Err(AnimationError::NonFinite);
            }
            if !(0.0..=1.0).contains(&frame.offset) {
                return Err(AnimationError::OutOfRange);
            }
            if frame.offset < last {
                return Err(AnimationError::Unordered);
            }
            last = frame.offset;
            if let Some(e) = &frame.easing {
                e.validate().map_err(AnimationError::Easing)?;
            }
            if frame.values.len() > Property::ALL.len() {
                return Err(AnimationError::TooManyKeyframes);
            }
            for (i, (p, v)) in frame.values.iter().enumerate() {
                if frame.values[..i].iter().any(|(q, _)| q == p) {
                    return Err(AnimationError::TooManyKeyframes);
                }
                if !v.is_finite() {
                    return Err(AnimationError::NonFinite);
                }
                if !v.fits(*p) {
                    return Err(AnimationError::ValueShape);
                }
                // `box-shadow` is one declaration: its geometry and colour
                // are set together or not at all (LLP 1062 D9).
                let half = match p {
                    Property::BoxShadow => Some(Property::ShadowColor),
                    Property::ShadowColor => Some(Property::BoxShadow),
                    _ => None,
                };
                if half.is_some_and(|h| !frame.values.iter().any(|(q, _)| *q == h)) {
                    return Err(AnimationError::NotAnimatable(*p));
                }
            }
            for (i, (p, v)) in frame.dark.iter().enumerate() {
                if !p.is_color() || !frame.values.iter().any(|(q, _)| q == p) {
                    return Err(AnimationError::NotAnimatable(*p));
                }
                if frame.dark[..i].iter().any(|(q, _)| q == p) {
                    return Err(AnimationError::TooManyKeyframes);
                }
                if !v.is_finite() {
                    return Err(AnimationError::NonFinite);
                }
            }
        }
        Ok(())
    }

    /// The keyframes one property takes part in, as (offset, easing, value),
    /// with CSS's implicit `0%` and `100%` from `underlying` where no
    /// keyframe declares it (CSS Animations 1 §3). A `light-dark()` colour
    /// takes its value under `dark`.
    pub fn track(
        &self,
        property: Property,
        underlying: Value,
        dark: bool,
    ) -> Vec<(f64, Option<&Easing>, Value)> {
        let mut out: Vec<(f64, Option<&Easing>, Value)> = self
            .0
            .iter()
            .filter_map(|f| {
                f.get(property, dark)
                    .map(|v| (f.offset, f.easing.as_ref(), v))
            })
            .collect();
        if out.first().is_none_or(|f| f.0 > 0.0) {
            out.insert(0, (0.0, None, underlying));
        }
        if out.last().is_none_or(|f| f.0 < 1.0) {
            out.push((1.0, None, underlying));
        }
        out
    }
}

impl Animation {
    /// Check every rule the sampler relies on.
    pub fn validate(&self) -> Result<(), AnimationError> {
        if !self.duration.is_finite() || !self.delay.is_finite() || self.iterations.is_nan() {
            return Err(AnimationError::NonFinite);
        }
        if self.duration < 0.0 || self.iterations < 0.0 {
            return Err(AnimationError::OutOfRange);
        }
        self.easing.validate().map_err(AnimationError::Easing)?;
        self.keyframes.validate()
    }

    /// Whether every number that must be finite is (`iterations` may be
    /// infinite).
    pub fn is_finite(&self) -> bool {
        !matches!(self.validate(), Err(AnimationError::NonFinite))
    }

    /// The active duration: duration × iteration count (0 when either is 0).
    pub fn active_duration(&self) -> f64 {
        if self.duration == 0.0 || self.iterations == 0.0 {
            0.0
        } else {
            self.duration * self.iterations
        }
    }

    /// Local time (seconds since the animation started, delay included) at
    /// which it ends; infinite for an infinite iteration count.
    pub fn end_time(&self) -> f64 {
        (self.delay + self.active_duration()).max(0.0)
    }

    /// The phase at local time `t` (Web Animations §4.5.8, positive rate).
    pub fn phase(&self, t: f64) -> Phase {
        let end = self.end_time();
        let before = self.delay.min(end).max(0.0);
        let after = (self.delay + self.active_duration()).min(end).max(0.0);
        if t < before {
            Phase::Before
        } else if t >= after {
            Phase::After
        } else {
            Phase::Active
        }
    }

    /// The directed progress at local time `t`, or `None` when the animation
    /// has no effect then (outside its active interval without a fill).
    pub fn directed_progress(&self, t: f64) -> Option<f64> {
        // An unresolved local time (NaN) is the idle phase: no effect (Web
        // Animations 1, animation effect phases).
        if t.is_nan() {
            return None;
        }
        let ad = self.active_duration();
        let phase = self.phase(t);
        let active = match phase {
            Phase::Before => {
                if !self.fill.backwards() {
                    return None;
                }
                (t - self.delay).max(0.0)
            }
            Phase::Active => t - self.delay,
            Phase::After => {
                if !self.fill.forwards() {
                    return None;
                }
                (t - self.delay).min(ad).max(0.0)
            }
        };
        let overall = if self.duration == 0.0 {
            if phase == Phase::Before {
                0.0
            } else {
                self.iterations
            }
        } else {
            active / self.duration
        };
        let mut simple = if overall.is_infinite() {
            0.0
        } else {
            overall % 1.0
        };
        if simple == 0.0 && phase != Phase::Before && active == ad && self.iterations != 0.0 {
            simple = 1.0;
        }
        let iteration = if phase == Phase::After && self.iterations.is_infinite() {
            f64::INFINITY
        } else if simple == 1.0 {
            overall.floor() - 1.0
        } else {
            overall.floor()
        };
        let odd = iteration.is_finite() && (iteration as i64).rem_euclid(2) == 1;
        let forwards = match self.direction {
            Direction::Normal => true,
            Direction::Reverse => false,
            Direction::Alternate => !odd,
            Direction::AlternateReverse => odd,
        };
        Some(if forwards { simple } else { 1.0 - simple })
    }

    /// One property's animated value at local time `t` over `underlying` (the
    /// value the property would have without this animation), or `None` when
    /// the animation does not apply to it then.
    pub fn sample(&self, t: f64, property: Property, underlying: Value) -> Option<Value> {
        self.sample_in(t, property, underlying, false)
    }

    /// [`Animation::sample`] under an appearance: a keyframe's `light-dark()`
    /// colour takes its dark value when `dark` (LLP 1062 D9).
    pub fn sample_in(
        &self,
        t: f64,
        property: Property,
        underlying: Value,
        dark: bool,
    ) -> Option<Value> {
        if !self
            .keyframes
            .0
            .iter()
            .any(|f| f.values.iter().any(|(p, _)| *p == property))
        {
            return None;
        }
        let p = self.directed_progress(t)?;
        Some(self.value_at(p, property, underlying, dark))
    }

    /// The keyframe effect at directed progress `p` for one property, a
    /// `light-dark()` colour under `dark`.
    pub fn value_at(&self, p: f64, property: Property, underlying: Value, dark: bool) -> Value {
        let track = self.keyframes.track(property, underlying, dark);
        // The interval: the last keyframe at or before p (the last of equal
        // offsets), and the next one after it; at p = 1 the final interval.
        let mut start = 0;
        for (i, frame) in track.iter().enumerate() {
            if frame.0 <= p && i + 1 < track.len() {
                start = i;
            }
        }
        if p >= 1.0 {
            start = track.len().saturating_sub(2);
            while start > 0 && track[start].0 >= 1.0 {
                start -= 1;
            }
        }
        let (a, b) = (&track[start], &track[(start + 1).min(track.len() - 1)]);
        let span = b.0 - a.0;
        if span <= 0.0 {
            return if p >= b.0 { b.2 } else { a.2 };
        }
        let local = (p - a.0) / span;
        let eased = a.1.unwrap_or(&self.easing).progress(local);
        a.2.lerp(b.2, eased)
    }
}

impl Animations {
    /// No animation (`animation: none`).
    pub const NONE: Animations = Animations(Vec::new());

    /// Check every entry.
    pub fn validate(&self) -> Result<(), AnimationError> {
        if self.0.len() > MAX_ANIMATIONS {
            return Err(AnimationError::TooMany);
        }
        self.0.iter().try_for_each(Animation::validate)
    }

    /// Whether every number that must be finite is.
    pub fn is_finite(&self) -> bool {
        self.0.iter().all(Animation::is_finite)
    }

    /// Local time at which the last entry ends; zero for none, infinite when
    /// one is endless.
    pub fn end_time(&self) -> f64 {
        self.0.iter().map(Animation::end_time).fold(0.0, f64::max)
    }

    /// [`Animations::validate`], and every entry runs to an end: the rule for
    /// an `-exact-exit-animation`, whose node is removed when it ends (LLP 1063).
    pub fn validate_ending(&self) -> Result<(), AnimationError> {
        self.validate()?;
        if self.0.iter().any(|a| a.paused || !a.end_time().is_finite()) {
            return Err(AnimationError::Endless);
        }
        Ok(())
    }

    /// Every property some entry animates, in wire order.
    pub fn properties(&self) -> Vec<Property> {
        let mut out: Vec<Property> = Vec::new();
        for a in &self.0 {
            for p in a.keyframes.properties() {
                if !out.contains(&p) {
                    out.push(p);
                }
            }
        }
        out.sort();
        out
    }
}

#[cfg(test)]
mod tests;
