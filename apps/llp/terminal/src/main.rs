//! `llp [corpus-or-document] [--size COLSxROWS] [op …]`: the LLP reader in
//! this terminal, or headless under the agent's verbs (see
//! [`exact_terminal::cli`]). With no path it opens the nearest `llp/`
//! directory above the working directory.

use std::path::{Path, PathBuf};

/// The nearest `llp/` at or above `from`, or `from` itself.
fn nearest(from: &Path) -> PathBuf {
    from.ancestors()
        .map(|d| d.join("llp"))
        .find(|d| d.is_dir())
        .unwrap_or_else(|| from.to_path_buf())
}

fn main() -> std::process::ExitCode {
    let entry = concat!(env!("CARGO_MANIFEST_DIR"), "/../terminal.contract");
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let named = args
        .first()
        .filter(|a| !a.starts_with("--") && Path::new(a).exists())
        .cloned();
    if named.is_some() {
        args.remove(0);
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    let path = match named {
        Some(p) => std::fs::canonicalize(&p).unwrap_or_else(|_| PathBuf::from(p)),
        None => nearest(&cwd),
    };
    exact_terminal::cli::run(
        args,
        llp_data::Llp::starting_at(&path.to_string_lossy()),
        Some(entry),
        exact_terminal::host::Mode::Fullscreen,
    )
}
