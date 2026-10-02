//! The bake for a Contract game UI, optionally with one linked Rust data source.
use std::{env, fs, path::PathBuf};

/// Bake a host from the declaration produced by its GPU build, without linking gameplay.
pub fn bake_declaration(platform: &str, app_dir: &str) {
    require_declaration(app_dir);
    bake(platform, app_dir);
}

/// Bake a host with an ordinary linked data source wrapped by host storage.
pub fn bake_data_declaration<D: contract::DataSource + Default>(
    platform: &str,
    app_dir: &str,
    data_type: &str,
) {
    require_declaration(app_dir);
    let wrapped = format!("exact_data_host::Storage<{data_type}>");
    let constructor = format!("exact_data_host::Storage::new({data_type}::default())");
    bake_source(
        platform,
        app_dir,
        D::default(),
        &wrapped,
        &constructor,
        true,
    );
}

fn require_declaration(app_dir: &str) {
    let path = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join(app_dir)
        .join(".shells/surfaces.json");
    println!("cargo:rerun-if-changed={}", path.display());
    fs::metadata(&path).expect("build the game's GPU shell before its host");
}

/// Bake the app named by the generated shell, with no JS or Rust module.
/// `apple` selects macOS or iOS from Cargo's target OS.
pub fn bake(platform: &str, app_dir: &str) {
    bake_source(platform, app_dir, (), "()", "Default::default()", false);
}

fn bake_source<D: contract::DataSource>(
    platform: &str,
    app_dir: &str,
    data: D,
    data_type: &str,
    constructor: &str,
    has_data: bool,
) {
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
    let grants = data.grants().to_string();
    let baked = contract::bake(plan, data).unwrap_or_else(|e| panic!("bake: {e}"));
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(out.join("app.plan"), baked.encode()).unwrap();
    let target = env::var("TARGET").unwrap();
    let compat = exact_bake::compatibility_id(&app, platform, &target, &manifest, Some(&grants))
        .unwrap_or_else(|e| panic!("compatibility id: {e}"));
    assert_eq!(
        compat.inputs["rustMode"], "off",
        "generated game hosts require app.json rust: false"
    );
    fs::write(out.join("compat.json"), compat.to_json()).unwrap();
    let entry = contract::rust_entry(data_type, constructor, "off").unwrap();
    let linked = if platform == "web" {
        contract::web_linked(&baked, &compat.inputs)
    } else {
        String::new()
    };
    let host = match platform {
        "web" => "exact_web::host!(AppData, PLAN, COMPAT, app_data);",
        "macos" | "ios" => {
            "exact_apple::host!(AppData, PLAN, COMPAT, None, std::ptr::null(), app_data);"
        }
        "linux" if has_data => {
            "fn main() { std::process::exit(exact_linux::run::<AppData>(PLAN, COMPAT)); }"
        }
        "linux" => "fn main() { std::process::exit(exact_linux::app::run_empty(PLAN, COMPAT)); }",
        _ => panic!("unsupported game app platform: {platform}"),
    };
    fs::write(
        out.join("entry.rs"),
        format!(
            r#"
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
{entry}
{linked}
{host}
"#
        ),
    )
    .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_runner::Value;
    #[test]
    fn empty_v5_call_binds_every_surface_default() {
        #[derive(Default)]
        struct Defaults;
        impl exact_gpu::Surface for Defaults {
            fn arguments(&self) -> Vec<(&'static str, Value)> {
                vec![
                    ("seed", Value::Number(7.)),
                    ("paused", Value::Bool(true)),
                    ("label", Value::str("default")),
                ]
            }
            fn bind(
                &mut self,
                values: &[Value],
                _: Option<f64>,
            ) -> Result<(), exact_gpu::SurfaceError> {
                assert_eq!(
                    values,
                    self.arguments()
                        .into_iter()
                        .map(|(_, v)| v)
                        .collect::<Vec<_>>()
                );
                Ok(())
            }
            fn render(
                &mut self,
                _: &exact_gpu::Frame,
                _: &exact_gpu::wgpu::Device,
                _: &exact_gpu::wgpu::Queue,
                _: &mut exact_gpu::wgpu::CommandEncoder,
                _: &exact_gpu::wgpu::TextureView,
                _: exact_gpu::wgpu::TextureFormat,
            ) -> bool {
                false
            }
        }
        let plan =
            contract::compile("component App\n  view\n    canvas surface=arena()\n").unwrap();
        let bytes = plan.encode();
        assert_eq!(&bytes[4..8], &5u32.to_le_bytes());
        let plan = exact_plan::Plan::decode(&bytes).unwrap();
        assert_eq!(plan.surfaces[0].mode, exact_plan::SurfaceArgsMode::Named);
        let mut runner = exact_runner::Runner::boot(
            plan,
            (),
            exact_kernel::Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        let update = runner.take_surface_updates().remove(0);
        static REGISTRY: exact_gpu::Registry = exact_gpu::Registry {
            surfaces: &[("arena", 0, || Box::<Defaults>::default())],
            shaders: &[],
        };
        let mut module = exact_gpu::Module::new(&REGISTRY);
        let id = module.create_headless("arena").unwrap();
        assert!(module.bind_json(id, &update.arguments_json(), None));
    }

    #[test]
    fn game_declaration_checks_names_and_arity_in_unmounted_branches() {
        let dir = std::env::temp_dir().join(format!("game-declaration-{}", std::process::id()));
        fs::create_dir_all(dir.join(".shells")).unwrap();
        fs::write(
            dir.join(".shells/surfaces.json"),
            serde_json::json!({"arena": [
                {"name": "seed", "default": 0.0},
                {"name": "paused", "default": false},
                {"name": "restart", "default": false},
                {"name": "label", "default": ""},
            ]})
            .to_string(),
        )
        .unwrap();
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
            let result = contract::compile_path_source(&dir.join("app.contract"), &format!(
                "component App\n  state visible = false\n  view\n    column\n      when visible\n        canvas surface={call}\n      else\n        text \"Menu\"\n"
            )).map(|plan| contract::bake(plan, ()).unwrap());
            assert_eq!(result.is_ok(), valid, "{call}: {:?}", result.as_ref().err());
        }
        fs::remove_dir_all(dir).unwrap();
    }
}
