//! The build script of an app's Android crate, the library the Canvas reader
//! loads (LLP 1076 §3.3, `host/android`): the Linux bake, which the Canvas
//! host shares, then what the APK's build reads beside the plan.

use crate::{build, BakeError};
use std::path::{Path, PathBuf};

/// Bake `app` for the Canvas host into `OUT_DIR`, as `build(app, "linux")`
/// does, then write the hatches' install and the plan's uses and permissions
/// beside it (`contract::native::android_entry`). Off Android the crate is
/// empty and nothing is baked.
pub fn build_android(app: &Path) -> Result<(), BakeError> {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("android") {
        return Ok(());
    }
    // `android_entry` writes paths that `include!` reads from OUT_DIR.
    let app = app
        .canonicalize()
        .map_err(|e| BakeError(format!("{}: {e}", app.display())))?;
    build(&app, "linux")?;
    println!("cargo:rerun-if-changed={}", app.join("app.json").display());
    let out = PathBuf::from(
        std::env::var_os("OUT_DIR").ok_or_else(|| BakeError("build requires OUT_DIR".into()))?,
    );
    let bytes = std::fs::read(out.join("app.plan"))
        .map_err(|e| BakeError(format!("the bake's plan: {e}")))?;
    let plan = exact_plan::Plan::decode(&bytes)
        .map_err(|e| BakeError(format!("the bake's plan: {e:?}")))?;
    let manifest =
        contract::Manifest::read(&app).map_err(|e| BakeError(format!("app.json: {e}")))?;
    contract::native::android_entry(&app, &manifest, &plan, &out)
        .map_err(|e| BakeError(format!("android: {e}")))
}
