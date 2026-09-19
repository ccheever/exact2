//! The bake for a Contract game UI with no app data module.
use exact_runner::{DataError, DataSource, Value};
use std::{env, fs, path::PathBuf};

#[derive(Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// Bake the app named by the generated shell, with no JS or Rust module.
/// `apple` selects macOS or iOS from Cargo's target OS.
pub fn bake(platform: &str, app_dir: &str) {
    bake_declared(platform, app_dir, None);
}

/// The game bake reflects the same surface declaration used by every host.
pub fn bake_game<G: exact_game::Game>(platform: &str, app_dir: &str) {
    bake_declared(platform, app_dir, Some(&game_arguments::<G>));
}
fn game_arguments<G: exact_game::Game>(name: &str) -> Option<Vec<String>> {
    (name == G::NAME).then(|| {
        <G::Args as exact_game::Args>::FIELDS
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .collect()
    })
}

/// Bake against every surface exported by a module, using its own argument names.
pub fn bake_registry(platform: &str, app_dir: &str, registry: &exact_gpu::Registry) {
    bake_declared(
        platform,
        app_dir,
        Some(&|name| surface_arguments(registry, name)),
    );
}

fn surface_arguments(registry: &exact_gpu::Registry, name: &str) -> Option<Vec<String>> {
    registry
        .surfaces
        .iter()
        .find(|(surface, _, _)| *surface == name)
        .map(|(_, _, factory)| {
            factory()
                .arguments()
                .into_iter()
                .map(|(name, _)| name.to_owned())
                .collect()
        })
}

type SurfaceArguments<'a> = &'a dyn Fn(&str) -> Option<Vec<String>>;

fn bake_declared(platform: &str, app_dir: &str, arguments: Option<SurfaceArguments<'_>>) {
    let platform = if platform == "apple" {
        if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios") {
            "ios"
        } else {
            "macos"
        }
    } else {
        platform
    };
    let app = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join(app_dir);
    println!("cargo:rerun-if-env-changed=EXACT_ASSET_ROOTS");
    for path in [
        "app.contract",
        "app.json",
        ".shells/app.json",
        "assets",
        "deck",
        "gpu/shaders",
    ] {
        let path = app.join(path);
        // A nonexistent watch path makes Cargo rebuild every invocation.
        if path.exists() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    let manifest = contract::Manifest::read(&app.join(".shells"))
        .unwrap_or_else(|e| panic!("resolved app.json: {e}"));
    let mut plan = contract::compile_path(&app.join("app.contract"))
        .unwrap_or_else(|e| panic!("app.contract: {e}"));
    if plan.app_id.is_empty() {
        plan.app_id = manifest.id.clone();
    }
    let baked = match arguments {
        Some(arguments) => contract::bake_with_surface_arguments(plan, NoData, arguments),
        None => contract::bake(plan, NoData),
    }
    .unwrap_or_else(|e| panic!("bake: {e}"));
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(out.join("app.plan"), baked.encode()).unwrap();
    let target = env::var("TARGET").unwrap();
    let compat = contract::compatibility_id(&app, platform, &target, &manifest, Some(""))
        .unwrap_or_else(|e| panic!("compatibility id: {e}"));
    assert_eq!(
        compat.inputs["rustMode"], "off",
        "a game UI without data sources requires app.json rust: false"
    );
    fs::write(out.join("compat.json"), compat.to_json()).unwrap();
    let entry = contract::rust_entry("NoData", "NoData", "off").unwrap();
    let host = match platform {
        "web" => "exact_web::host!(AppData, PLAN, COMPAT, app_data);",
        "macos" | "ios" => {
            "exact_apple::host!(AppData, PLAN, COMPAT, None, std::ptr::null(), app_data);"
        }
        "linux" => "fn main() { std::process::exit(exact_linux::run::<AppData>(PLAN, COMPAT)); }",
        _ => panic!("unsupported game app platform: {platform}"),
    };
    fs::write(out.join("entry.rs"), format!(r#"
#[derive(Default)]
struct NoData;
impl exact_runner::DataSource for NoData {{
    fn query(&mut self, source: &str, _: &[exact_runner::Value]) -> Result<exact_runner::Value, exact_runner::DataError> {{
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }}
}}
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
{entry}
{host}
"#)).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Radar;
    impl exact_gpu::Surface for Radar {
        fn bind(
            &mut self,
            _: &[exact_gpu::Value],
            _: Option<f64>,
        ) -> Result<(), exact_gpu::SurfaceError> {
            Ok(())
        }
        fn arguments(&self) -> Vec<(&'static str, exact_gpu::Value)> {
            vec![("zoom", exact_gpu::Value::Number(1.))]
        }
        fn render(
            &mut self,
            _: &exact_gpu::Frame,
            _: &exact_gpu::wgpu::Device,
            _: &exact_gpu::wgpu::Queue,
            _: &exact_gpu::wgpu::TextureView,
            _: exact_gpu::wgpu::TextureFormat,
        ) -> bool {
            false
        }
    }
    #[test]
    fn every_exported_surface_uses_its_own_names_at_bake() {
        let registry = exact_gpu::Registry {
            surfaces: &[("radar", 1, || Box::<Radar>::default())],
            shaders: &[],
        };
        for (call, valid) in [
            ("radar(zoom=2)", true),
            ("radar(seed=2)", false),
            ("world(zoom=2)", false),
        ] {
            let plan = contract::compile(&format!(
                "component App\n  view\n    canvas surface={call}\n"
            ))
            .unwrap();
            let result = contract::bake_with_surface_arguments(plan, NoData, |name| {
                surface_arguments(&registry, name)
            });
            assert_eq!(result.is_ok(), valid, "{call}");
        }
    }

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
                game_arguments::<TestGame>(name)
            });
            assert_eq!(result.is_ok(), valid, "{call}: {:?}", result.as_ref().err());
        }
    }
}
