//! The date as a host fact: the Unix time at which the runner's clock read
//! zero, and the viewer's offset from UTC. `performanceNow()` stays elapsed time; the
//! date now is `epochAtZero + performanceNow()`. Beside it, where the viewer is: their
//! locale and IANA time zone, so a source formats a date the way the device
//! would (`new Intl.DateTimeFormat(time.locale, { timeZone: time.timeZone })`)
//! without reading either from ambient state.
//! @ref LLP 1027.000.000 (Draft; the Bluesky client is its consumer)

use exact_plan::Value;

/// Reserved resource source, answered before the app data seam.
pub const SOURCE: &str = "exactTime";
/// Fields an app may declare, filled by name.
pub const FIELDS: &[&str] = &[
    "epochAtZero",
    "utcOffset",
    "locale",
    "resolvedLocale",
    "timeZone",
    "seed",
];

/// The launch seed is a whole number below this, 2^32, on every host
/// (LLP 1116.005 D5), so `%` and `>>> 0` take all of it. A source mixes a
/// round into it in 32-bit steps (`Math.imul`, `>>> 0`), as
/// docs/recipes/random-and-shuffle.md shows: a multiply by a large constant
/// still passes 2^53.
pub const SEED_LIMIT: f64 = 4_294_967_296.0;

/// What the host said about the date. Zero until it says: the bake, and a
/// host that has not supplied it, answer an unknown date as `0`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WallTime {
    /// Unix milliseconds at which `performanceNow()` read zero.
    pub epoch_at_zero: f64,
    /// Minutes east of UTC in the viewer's zone (UTC+2 is `120`).
    pub utc_offset: f64,
}

std::thread_local! {
    static LAUNCH: std::cell::RefCell<Launch> = const { std::cell::RefCell::new(Launch::NONE) };
}

/// What a host knows at launch, before the boot: the date and where the
/// viewer is (locale, zone, seed). A runner booted under it ([`launching_with`])
/// reads both from its first settle, as `set_time` and `set_place` would
/// set them after.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Launch {
    /// The date (`set_time`'s fact).
    pub time: Option<WallTime>,
    /// Where the viewer is (`set_place`'s fact, its seed included).
    pub place: Option<Place>,
}

impl Launch {
    /// Nothing known: the boot reads the defaults.
    pub const NONE: Launch = Launch {
        time: None,
        place: None,
    };
}

/// Boot under `time`, the date the host knows at launch: a runner booted
/// inside reads it from its first settle, so a resource whose arguments
/// read the time (the zone's offset in a kept answer's identity) is asked
/// with the real one, not with zero until the host's first `set_time`.
/// `None`, or a fact that does not validate, boots on zero as before.
pub fn launching<T>(time: Option<WallTime>, boot: impl FnOnce() -> T) -> T {
    launching_with(Launch { time, place: None }, boot)
}

/// Boot under what the host knows at launch ([`Launch`]): a fact that does
/// not validate is left out, and the boot reads the default for it.
pub fn launching_with<T>(launch: Launch, boot: impl FnOnce() -> T) -> T {
    /// Puts the outer launch back however `boot` leaves, an unwind too.
    struct Restore(Option<Launch>);
    impl Drop for Restore {
        fn drop(&mut self) {
            if let Some(outer) = self.0.take() {
                LAUNCH.with(|l| *l.borrow_mut() = outer);
            }
        }
    }
    let launch = Launch {
        time: launch.time.filter(|t| t.validate().is_ok()),
        place: launch.place.filter(|p| p.validate().is_ok()),
    };
    let _restore = Restore(Some(LAUNCH.with(|l| l.replace(launch))));
    boot()
}

/// `boot` under the launch being booted under, without its date: a boot
/// whose clock is not the host's (a rendered page's checkpoint keeps its
/// renderer's), told its date after.
pub fn without_launch_time<T>(boot: impl FnOnce() -> T) -> T {
    launching_with(
        Launch {
            time: None,
            place: launch_place(),
        },
        boot,
    )
}

/// The date a boot under [`launching`] reads first.
pub(crate) fn launch_time() -> WallTime {
    LAUNCH.with(|l| l.borrow().time).unwrap_or_default()
}

/// Where the viewer is, when a boot under [`launching_with`] was told.
pub(crate) fn launch_place() -> Option<Place> {
    LAUNCH.with(|l| l.borrow().place.clone())
}

impl WallTime {
    /// Refuse a non-finite or implausible fact before changing any state.
    pub fn validate(self) -> Result<(), crate::RunnerError> {
        if self.epoch_at_zero.is_finite()
            && self.epoch_at_zero >= 0.0
            && self.utc_offset.is_finite()
            && self.utc_offset.abs() <= 18.0 * 60.0
        {
            Ok(())
        } else {
            Err(crate::RunnerError::InvalidTime)
        }
    }

    /// Fill a declared field; unknown names are refused.
    pub fn field(self, name: &str) -> Option<Value> {
        match name {
            "epochAtZero" => Some(Value::Number(self.epoch_at_zero)),
            "utcOffset" => Some(Value::Number(self.utc_offset)),
            _ => None,
        }
    }
}

/// Where the viewer is: a BCP 47 locale (`en-GB`) and an IANA zone
/// (`Europe/London`). Usable by `Intl` before the host reports: `en-US`/`UTC`.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    /// BCP 47, the device's first preferred language.
    pub locale: String,
    /// IANA, the device's current zone.
    pub time_zone: String,
    /// A whole number below 2^32 ([`SEED_LIMIT`]) from the platform's
    /// secure random source, drawn once per launch: the explicit entropy a
    /// source mixes with the date and a counter for ids (LLP 1027.000's
    /// seed, supplied). Zero until the host says.
    pub seed: f64,
}

impl Default for Place {
    fn default() -> Self {
        Self {
            locale: "en-US".into(),
            time_zone: "UTC".into(),
            seed: 0.0,
        }
    }
}

impl Place {
    /// The place a host tells before a boot, as `set_place`'s input:
    /// `locale NUL timeZone NUL seed`, the seed required; `None` unless it
    /// validates.
    pub fn launched(text: &str) -> Option<Place> {
        let mut fields = text.split('\0');
        let (locale, time_zone) = (fields.next()?, fields.next()?);
        let seed = exact_num::parse_f64(fields.next()?).ok()?;
        let place = Place {
            locale: locale.into(),
            time_zone: time_zone.into(),
            seed,
        };
        place.validate().is_ok().then_some(place)
    }

    /// Refuse what no host would report: a fact that is empty, long, or has
    /// characters neither form uses. The engine's `Intl` judges the rest.
    pub fn validate(&self) -> Result<(), crate::RunnerError> {
        let fine = |s: &str, extra: &[char]| {
            !s.is_empty()
                && s.len() <= 64
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || extra.contains(&c))
        };
        if fine(&self.locale, &[])
            && fine(&self.time_zone, &['/', '_', '+'])
            && self.seed.is_finite()
            && self.seed >= 0.0
            && self.seed.fract() == 0.0
            && self.seed < SEED_LIMIT
        {
            Ok(())
        } else {
            Err(crate::RunnerError::InvalidPlace)
        }
    }

    /// Fill a declared field; the date's fields are [`WallTime`]'s.
    pub fn field(&self, name: &str) -> Option<Value> {
        match name {
            "locale" => Some(Value::str(&self.locale)),
            "timeZone" => Some(Value::str(&self.time_zone)),
            "seed" => Some(Value::Number(self.seed)),
            _ => None,
        }
    }
}
