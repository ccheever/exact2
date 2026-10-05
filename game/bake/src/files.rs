//! Shared by the art baker and generated game build scripts.
use std::path::Path;

/// Refuse oversized carriers before decoding or writing them.
pub fn check_size(name: &str, size: usize) -> Result<(), String> {
    if size > 64 * 1024 * 1024 {
        Err(format!("asset `{name}` exceeds 64 MiB"))
    } else {
        Ok(())
    }
}

/// Validate each declared JSON file (`Game::LEVELS`) using its `Data` derive.
/// The authored file is `assets/<name>`, delivered as it is and separately from
/// executable code: no source bytes enter the module, and the dev loop sends
/// an edit to a running page as it sends any asset, without a build.
pub fn bake_game_levels<G: exact_game::Game>(app: impl AsRef<Path>) -> Result<(), String> {
    for level in G::LEVELS {
        let source = app.as_ref().join("assets").join(level.name);
        println!("cargo:rerun-if-changed={}", source.display());
        let bytes = std::fs::read(&source).map_err(|e| format!("{}: {e}", source.display()))?;
        check_size(level.name, bytes.len())?;
        let text = std::str::from_utf8(&bytes).map_err(|e| format!("{}: {e}", level.name))?;
        level.decode(text).map_err(|e| e.to_string())?;
    }
    Ok(())
}
