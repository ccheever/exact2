//! Shared by the art baker and generated game build scripts.
use std::path::Path;

pub(super) fn write_changed(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    if std::fs::read(path).ok().as_deref() == Some(bytes) {
        return Ok(());
    }
    let tmp = path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap().to_string_lossy(),
        std::process::id()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| format!("{}: {e}", tmp.display()))?;
    let result = file
        .write_all(bytes)
        .and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map_err(|e| format!("{}: {e}", path.display()))
}

/// Refuse oversized carriers before decoding or writing them.
pub fn check_size(name: &str, size: usize) -> Result<(), String> {
    if size > 64 * 1024 * 1024 {
        Err(format!("asset `{name}` exceeds 64 MiB"))
    } else {
        Ok(())
    }
}

/// Validate the game's single declared JSON level using its `Data` derive, then
/// deliver it separately from executable code. No source bytes enter the module.
pub fn bake_game_level<G: exact_game::Game>(app: impl AsRef<Path>) -> Result<(), String> {
    let Some(level) = G::LEVEL else { return Ok(()) };
    let source = app.as_ref().join(level.name);
    println!("cargo:rerun-if-changed={}", source.display());
    let bytes = std::fs::read(&source).map_err(|e| format!("{}: {e}", source.display()))?;
    check_size(level.name, bytes.len())?;
    let text = std::str::from_utf8(&bytes).map_err(|e| format!("{}: {e}", level.name))?;
    level.decode(text).map_err(|e| e.to_string())?;
    let output = app.as_ref().join("assets").join(level.name);
    std::fs::create_dir_all(output.parent().unwrap()).map_err(|e| e.to_string())?;
    write_changed(&output, &bytes)
}
