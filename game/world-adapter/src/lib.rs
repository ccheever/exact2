//! Contract conversions for the external world surface adapter.
#![deny(unsafe_code)]
pub mod args;
pub mod publication;
mod surface;
pub use surface::WorldSurface;
pub use {exact_gpu, exact_world};

/// Register a nonspatial game as the ordinary `world` surface.
#[macro_export]
macro_rules! module {
    ($game:ty) => {
        pub static REGISTRY: $crate::exact_gpu::Registry = $crate::exact_gpu::Registry {
            surfaces: &[(
                "world",
                <<$game as $crate::exact_world::Game>::Args as $crate::exact_world::Args>::FIELDS
                    .len(),
                || Box::new($crate::WorldSurface::<$game>::default()),
            )],
            shaders: &[],
        };
        $crate::exact_gpu::module!(REGISTRY);
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn bounded_public_report_names_busy_work_and_observation() {
        use exact_world::{json, Work, World};
        let w = World::new(60, 0);
        w.busy("controller moving").unwrap();
        w.work("assets", Work::Pending).unwrap();
        w.work("timer", Work::Deadline(90)).unwrap();
        let mut out = json::Encoder::default();
        w.report(&mut out).unwrap();
        let text = out.finish().unwrap();
        for expected in [
            "controller moving",
            "assets",
            "timer",
            "90",
            "observation",
            "truncated",
        ] {
            assert!(text.contains(expected), "{text}");
        }
        for i in 0..64 {
            if i > 1 {
                w.work(&format!("extra-{i:02}"), Work::Ready).unwrap();
            }
        }
        let mut out = json::Encoder::default();
        w.report(&mut out).unwrap();
        let text = out.finish().unwrap();
        assert!(text.contains("\"truncated\":true"), "{text}");
        assert!(!text.contains("extra-63"));
        assert!(w.work("overflow", Work::Pending).is_err());
    }
}

#[cfg(test)]
mod atomic_publication {
    #[test]
    fn late_invalid_field_preserves_publications_and_game_history() {
        use exact_world::*;
        #[derive(Default, Data)]
        struct Record {
            a: u32,
            z: String,
        }
        let w = World::new(60, 0);
        w.publish("a", 0u32).unwrap();
        let before = w.publications().clone();
        let cursor = w.journal_next();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::publication::publish_record(
                &w,
                &Record {
                    a: 1,
                    z: "x".repeat(11_000),
                },
            )
        }))
        .is_err());
        assert_eq!(*w.publications(), before);
        assert_eq!(w.journal_next(), cursor);
    }
}

/// Emit the shell's argument declaration without constructing a world or device.
pub fn emit_declaration<G: exact_world::Game>(app_dir: &str) {
    use exact_world::Args;
    let rows: Vec<_> = G::Args::FIELDS
        .iter()
        .zip(args::argument_values(&G::Args::default()).unwrap())
        .map(|((name, _), value)| {
            let value = match value {
                exact_plan::Value::Number(n) => serde_json::json!(n),
                exact_plan::Value::Bool(b) => serde_json::json!(b),
                exact_plan::Value::Str(s) => serde_json::json!(s.as_ref()),
                _ => panic!("unsupported default"),
            };
            serde_json::json!({"name":name,"default":value})
        })
        .collect();
    let path = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join(app_dir)
        .join(".shells/surfaces.json");
    let text = serde_json::to_string_pretty(&serde_json::json!({"world":rows})).unwrap() + "\n";
    if std::fs::read_to_string(&path).ok().as_deref() != Some(&text) {
        std::fs::write(path, text).unwrap();
    }
}
