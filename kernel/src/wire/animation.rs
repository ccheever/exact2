//! The `animation` row's bytes (grammar: `schema.json` `_animations`).
//!
//! @ref LLP 1055 D5 — each entry travels with its resolved keyframes, so a
//! decoded row is complete without the plan.

use super::codec::{Reader, Writer};
use crate::error::DecodeError;
use exact_motion::animation::Keyframe;
use exact_motion::{
    Animation, Animations, Direction, FillMode, Keyframes, Property, TimingFunction, Value,
    MAX_ANIMATIONS, MAX_KEYFRAMES,
};

impl Reader<'_> {
    /// Read an `animation` row and validate it as the sampler will.
    pub fn animations(&mut self) -> Result<Animations, DecodeError> {
        let count = self.u8()? as usize;
        if count > MAX_ANIMATIONS {
            return Err(DecodeError::BadAnimation);
        }
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            let name = self.string()?.to_string();
            let duration = self.f32()? as f64;
            let delay = self.f32()? as f64;
            let TimingFunction::Easing(easing) = self.timing_function()? else {
                return Err(DecodeError::BadAnimation);
            };
            let iterations = self.f32()? as f64;
            let direction = Direction::from_wire(self.u8()?).ok_or(DecodeError::BadAnimation)?;
            let fill = FillMode::from_wire(self.u8()?).ok_or(DecodeError::BadAnimation)?;
            let paused = self.u8()? != 0;
            let frames = self.u8()? as usize;
            if frames > MAX_KEYFRAMES {
                return Err(DecodeError::BadAnimation);
            }
            let mut keyframes = Vec::with_capacity(frames);
            for _ in 0..frames {
                let offset = self.f32()? as f64;
                let easing = match self.u8()? {
                    0 => None,
                    1 => match self.timing_function()? {
                        TimingFunction::Easing(e) => Some(e),
                        TimingFunction::Spring(_) => return Err(DecodeError::BadAnimation),
                    },
                    _ => return Err(DecodeError::BadAnimation),
                };
                let n = self.u8()? as usize;
                if n > Property::ALL.len() {
                    return Err(DecodeError::BadAnimation);
                }
                let mut values = Vec::with_capacity(n);
                for _ in 0..n {
                    let p = Property::from_wire(self.u8()?).ok_or(DecodeError::BadAnimation)?;
                    // Each value carries its property's components: one,
                    // two for `translate`, four for a colour.
                    let mut c = [0.0f64; 4];
                    for slot in c.iter_mut().take(p.components()) {
                        *slot = self.f32()? as f64;
                    }
                    values.push((
                        p,
                        Value {
                            x: c[0],
                            y: c[1],
                            z: c[2],
                            w: c[3],
                        },
                    ));
                }
                keyframes.push(Keyframe {
                    offset,
                    easing,
                    values,
                });
            }
            out.push(Animation {
                name,
                duration,
                delay,
                easing,
                iterations,
                direction,
                fill,
                paused,
                keyframes: Keyframes(keyframes),
            });
        }
        let animations = Animations(out);
        animations
            .validate()
            .map_err(DecodeError::InvalidAnimation)?;
        Ok(animations)
    }
}

impl Writer {
    /// Append an `animation` row.
    pub fn animations(&mut self, a: &Animations) {
        debug_assert!(a.0.len() <= MAX_ANIMATIONS);
        self.u8(a.0.len() as u8);
        for entry in &a.0 {
            self.string(&entry.name);
            self.f32(entry.duration as f32);
            self.f32(entry.delay as f32);
            self.timing_function(&TimingFunction::Easing(entry.easing.clone()));
            self.f32(entry.iterations as f32);
            self.u8(entry.direction as u8);
            self.u8(entry.fill as u8);
            self.u8(entry.paused as u8);
            self.u8(entry.keyframes.0.len() as u8);
            for frame in &entry.keyframes.0 {
                self.f32(frame.offset as f32);
                match &frame.easing {
                    None => self.u8(0),
                    Some(e) => {
                        self.u8(1);
                        self.timing_function(&TimingFunction::Easing(e.clone()));
                    }
                }
                self.u8(frame.values.len() as u8);
                for (p, v) in &frame.values {
                    self.u8(*p as u8);
                    for c in [v.x, v.y, v.z, v.w].into_iter().take(p.components()) {
                        self.f32(c as f32);
                    }
                }
            }
        }
    }
}
