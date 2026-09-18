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
        ".scene/scene.contract",
        "app.json",
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
    let plan = contract::compile_path(&app.join("app.contract"))
        .unwrap_or_else(|e| panic!("app.contract: {e}"));
    let baked = contract::bake(plan, NoData).unwrap_or_else(|e| panic!("bake: {e:?}"));
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(out.join("app.plan"), baked.encode()).unwrap();
    let manifest = contract::Manifest::read(&app).unwrap_or_else(|e| panic!("app.json: {e}"));
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
