use crate::Value;

/// Whether changing a bound field constructs a new world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgumentKind {
    /// Construct a new world when this value changes.
    Setup,
    /// A boolean edge: either transition reconstructs through the setup path.
    Restart,
    /// Pass the new value to subsequent ticks.
    Live,
}
/// Typed canvas arguments. Derive this on a named struct; field order is wire order.
///
/// ```compile_fail
/// use exact_game::Args;
/// #[derive(Default, Args)]
/// struct Options { #[live(typo)] volume: f32 }
/// ```
/// ```compile_fail
/// use exact_game::Args;
/// #[derive(Default, Args)]
/// struct Options { #[live = true] volume: f32 }
/// ```
/// ```compile_fail
/// use exact_game::Args;
/// #[derive(Default, Args)]
/// #[live]
/// struct Options { volume: f32 }
/// ```
/// ```compile_fail
/// use exact_game::Args;
/// #[derive(Default, Args)]
/// #[live(typo)]
/// struct Options { volume: f32 }
/// ```
/// ```compile_fail
/// use exact_game::Args;
/// #[derive(Default, Args)]
/// #[live = true]
/// struct Options { volume: f32 }
/// ```
pub trait Args: crate::Data {
    /// Ordered field names and their binding behavior.
    const FIELDS: &'static [(&'static str, ArgumentKind)];
    /// Decode all values before any world or clock mutation.
    fn decode(values: &[Value]) -> Result<Self, String>;
    /// Refuse nonfinite floats and integers outside the portable wire range.
    fn check_scalars(&self) -> Result<(), String>;
    /// Canonical wire values encoded from the decoded fields.
    fn values(&self) -> Vec<Value>;
    /// Whether any setup field differs.
    fn setup_changed(&self, next: &Self) -> bool;
}
impl Args for () {
    const FIELDS: &'static [(&'static str, ArgumentKind)] = &[];
    fn decode(values: &[Value]) -> Result<Self, String> {
        arity(values, &[])?;
        Ok(())
    }
    fn check_scalars(&self) -> Result<(), String> {
        Ok(())
    }
    fn values(&self) -> Vec<Value> {
        vec![]
    }
    fn setup_changed(&self, _: &Self) -> bool {
        false
    }
}
/// Support for the Args derive; not a string lookup API.
#[doc(hidden)]
pub fn arity(values: &[Value], fields: &[(&str, ArgumentKind)]) -> Result<(), String> {
    let count = fields.len();
    if values.len() > count {
        Err(format!(
            "expected {count} arguments ({}), got {}",
            fields
                .iter()
                .map(|(name, _)| *name)
                .collect::<Vec<_>>()
                .join(", "),
            values.len()
        ))
    } else {
        Ok(())
    }
}
/// Supported scalar argument types, used by the derive.
#[doc(hidden)]
pub trait Argument: Sized {
    const EXPECTED: &'static str;
    fn value(value: &Value) -> Option<Self>;
}
/// Decode one positional field with an author-facing refusal.
#[doc(hidden)]
pub fn field<T: Argument>(values: &[Value], index: usize, name: &str) -> Result<T, String> {
    values.get(index).and_then(T::value).ok_or_else(|| {
        format!(
            "{name}: expected {}, got {}",
            T::EXPECTED,
            values.get(index).map_or_else(
                || "no value".into(),
                |v| crate::values::value_json(v, false)
            )
        )
    })
}
impl Argument for bool {
    const EXPECTED: &'static str = "a boolean";
    fn value(v: &Value) -> Option<Self> {
        v.as_bool()
    }
}
impl Argument for String {
    const EXPECTED: &'static str = "text";
    fn value(v: &Value) -> Option<Self> {
        v.as_str().map(str::to_owned)
    }
}
macro_rules! integer {
    ($t:ty, $lo:expr, $hi:expr, $expected:literal) => {
        impl Argument for $t {
            const EXPECTED: &'static str = $expected;
            fn value(v: &Value) -> Option<Self> {
                v.as_number()
                    .filter(|n| n.is_finite() && n.fract() == 0.0 && *n >= $lo && *n <= $hi)
                    .map(|n| n as Self)
            }
        }
    };
}
// Bound values are f64: 64-bit integer fields accept only exactly portable safe integers.
integer!(
    u32,
    0.0,
    u32::MAX as f64,
    "a whole number ≥ 0 (at most 4294967295)"
);
integer!(
    u64,
    0.0,
    9_007_199_254_740_991.0,
    "a whole number ≥ 0 (at most 9007199254740991)"
);
integer!(
    i32,
    i32::MIN as f64,
    i32::MAX as f64,
    "a whole number in -2147483648..=2147483647"
);
integer!(
    i64,
    -9_007_199_254_740_991.0,
    9_007_199_254_740_991.0,
    "a whole number in -9007199254740991..=9007199254740991"
);
impl Argument for f64 {
    const EXPECTED: &'static str = "a finite number";
    fn value(v: &Value) -> Option<Self> {
        v.as_number().filter(|n| n.is_finite())
    }
}
impl Argument for f32 {
    const EXPECTED: &'static str = "a finite f32 number";
    fn value(v: &Value) -> Option<Self> {
        v.as_number().map(|n| n as f32).filter(|n| n.is_finite())
    }
}

#[cfg(test)]
mod restart_tests {
    use crate::{Args, Game, Input, Sim, Transform, Value, World};
    #[derive(Default, Args)]
    struct Options {
        seed: u64,
        #[live]
        paused: bool,
        #[restart]
        restart: bool,
    }
    struct Example;
    impl Game for Example {
        const ID: &'static str = "restart-test";
        type Args = Options;
        fn setup(w: &mut World, args: &Options) {
            w.reseed(args.seed);
            w.spawn_named("player", Transform::default());
        }
        fn tick(w: &mut World, _: &Input, _: &Options) {
            w.get_mut::<Transform>("player").unwrap().position.x += 1.0;
        }
        fn paused(args: &Options) -> bool {
            args.paused
        }
    }
    #[test]
    fn both_edges_reconstruct_but_identical_bind_and_live_pause_do_not() {
        let mut sim = Sim::<Example>::new(Options::default()).unwrap();
        let bind = |restart, paused| {
            [
                Value::Number(0.0),
                Value::Bool(paused),
                Value::Bool(restart),
            ]
        };
        for edge in [true, false] {
            sim.run(100.0);
            assert!(sim.global_position("player").unwrap().x > 0.0);
            sim.bind(&bind(edge, false), None).unwrap();
            assert_eq!(sim.global_position("player").unwrap().x, 0.0);
            let count = sim.restarted;
            sim.run(100.0);
            let hash = sim.world().hash();
            sim.bind(&bind(edge, false), None).unwrap();
            assert_eq!(sim.world().hash(), hash);
            sim.bind(&bind(edge, true), None).unwrap();
            sim.run(100.0);
            assert_eq!(sim.world().hash(), hash);
            assert_eq!(sim.restarted, count);
            assert!(sim
                .bind(
                    &[Value::Number(0.0), Value::Bool(false), Value::Number(1.0)],
                    None
                )
                .is_err());
            assert_eq!(sim.restarted, count);
            sim.bind(&bind(edge, false), None).unwrap();
        }
        assert_eq!(sim.restarted, 2);
        assert!(sim.agent(r#"{"op":"state"}"#).contains(r#""restarted":2"#));
    }
}

#[cfg(test)]
mod author_tests {
    use crate::{Game, Input, Sim, World};
    struct Empty;
    impl Game for Empty {
        const ID: &'static str = "author-edges";
        type Args = ();
        fn setup(_: &mut World, _: &()) {}
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    #[test]
    fn assets_constructor_and_pin_use_the_simulation_identity() {
        let mut sim =
            Sim::<Empty>::with_assets((), |_| Err::<Vec<u8>, _>("unexpected asset")).unwrap();
        sim.run(1000.);
        sim.assert_pin(&format!(
            "{{\"game\":\"author-edges\",\"ticks\":{{\"60\":\"0x{:016x}\"}}}}",
            sim.world().hash()
        ));
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            sim.assert_pin("{\"game\":\"author-edges\",\"ticks\":{}}")
        }))
        .unwrap_err();
        let message = failure.downcast_ref::<String>().unwrap();
        assert!(message.contains("author-edges") && message.contains("60"));
    }
    #[test]
    fn r14_pin_refuses_foreign_missing_empty_and_malformed_identity() {
        let sim = Sim::<Empty>::new(()).unwrap();
        for (text, identity) in [
            (
                format!(
                    r#"{{"game":"foreign","ticks":{{"0":"0x{:016x}"}}}}"#,
                    sim.world().hash()
                ),
                "foreign",
            ),
            ("{}".into(), "missing"),
            ("".into(), "invalid"),
            ("{".into(), "invalid"),
        ] {
            let failure =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sim.assert_pin(&text)))
                    .unwrap_err();
            let message = failure.downcast_ref::<String>().unwrap();
            assert!(
                message.contains("author-edges") && message.contains(identity),
                "{message}"
            );
        }
    }
}
