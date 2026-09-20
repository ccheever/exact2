//! Game declarations over the engine-free Contract shell bake.
pub use exact_app_shell::{bake, bake_declaration, bake_registry};
use exact_runner::Value;
use std::{env, fs, path::PathBuf};

/// Emit the game interface during the GPU build. Unchanged declarations retain
/// their timestamp, so a gameplay edit cannot invalidate a host's bake.
pub fn emit_declaration<G: exact_game::Game>(app_dir: &str) {
    let app = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join(app_dir);
    write_declaration::<G>(&app.join(".shells/surfaces.json"));
}
fn declaration<G: exact_game::Game>() -> serde_json::Value {
    use exact_game::Args;
    let defaults = G::Args::default().values();
    let arguments: Vec<_> = G::Args::FIELDS
        .iter()
        .zip(defaults)
        .map(|((name, _), value)| {
            let value = match value {
                Value::Number(n) => serde_json::json!(n),
                Value::Bool(b) => serde_json::json!(b),
                Value::Str(s) => serde_json::json!(s.as_ref()),
                _ => panic!("unsupported surface argument default: {name}"),
            };
            serde_json::json!({"name": name, "default": value})
        })
        .collect();
    serde_json::json!({G::NAME: arguments})
}
fn write_declaration<G: exact_game::Game>(path: &std::path::Path) {
    let text = serde_json::to_string_pretty(&declaration::<G>()).unwrap() + "\n";
    if fs::read_to_string(path).ok().as_deref() != Some(&text) {
        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&temporary, text).expect("write game surface declaration");
        fs::rename(temporary, path).expect("publish complete game surface declaration");
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use exact_app_shell::{declaration_arguments, NoData};
    #[derive(Default, exact_game::Args)]
    struct Options {
        seed: u32,
        #[live]
        paused: bool,
        #[restart]
        restart: bool,
        label: String,
    }
    struct TestGame;
    impl exact_game::Game for TestGame {
        const ID: &'static str = "simulation";
        const NAME: &'static str = "arena";
        type Args = Options;
        fn setup(_: &mut exact_game::World, _: &Options) {
            panic!("argument validation must not construct a game");
        }
        fn tick(_: &mut exact_game::World, _: &exact_game::Input, _: &Options) {}
    }
    #[test]
    fn d6_declaration_defaults_and_timestamp_are_stable() {
        let path = std::env::temp_dir().join(format!("d6-declaration-{}.json", std::process::id()));
        write_declaration::<TestGame>(&path);
        let before = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1234);
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(before))
            .unwrap();
        write_declaration::<TestGame>(&path);
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before);
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            value["arena"][0],
            serde_json::json!({"name":"seed","default":0.0})
        );
        assert_eq!(value["arena"][1]["default"], false);
        assert_eq!(value["arena"][3]["default"], "");
        assert!(declaration_arguments(&value, "world").is_none());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn game_bake_checks_declared_names_and_arity_in_unmounted_branches() {
        for (call, valid) in [
            ("arena()", true),
            (
                "arena(seed=7, paused=true, restart=false, label=\"hello\")",
                true,
            ),
            ("arena(label=\"hello\", seed=7)", true),
            ("arena(7, false, false, \"hello\")", true),
            ("arena(7, false, false, \"hello\", 9)", false),
            ("arena(typo=7)", false),
            ("world(seed=7)", false),
            ("simulation(seed=7)", false),
        ] {
            let plan = contract::compile(&format!(
                "component App\n  state visible = false\n  view\n    column\n      when visible\n        canvas surface={call}\n      else\n        text \"Menu\"\n"
            )).unwrap();
            let result = contract::bake_with_surface_arguments(plan, NoData, |name| {
                declaration_arguments(&declaration::<TestGame>(), name)
            });
            assert_eq!(result.is_ok(), valid, "{call}: {:?}", result.as_ref().err());
        }
    }
}
