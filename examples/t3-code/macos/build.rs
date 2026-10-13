#[path = "src/markdown.rs"]
mod markdown;

/// Pierre's file-icon tables for the Markdown module the bake runs (pierre_icons.rs), as `generate`
/// has just written them (the library embeds the same file).
fn pierre_tables() -> &'static str {
    static TABLES: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    TABLES.get_or_init(|| {
        std::fs::read_to_string("pierre-icons.tsv").expect("tools/generate writes pierre-icons.tsv")
    })
}

fn main() {
    generate();
    // Mixed-source ownership is part of the bake, not only the linked app.
    println!("cargo:rerun-if-changed=src/markdown.rs");
    exact_js_bake::build_mixed_with(
        std::path::Path::new(".."),
        "macos",
        &markdown::Markdown,
        &[
            "renderMarkdown",
            "renderPullRequestMarkdown",
            "renderFileMarkdown",
        ],
        |javascript| Box::new(markdown::mixed(javascript, exact_runner::Placement::Main)),
    )
    .expect("bake T3 Code for macOS");
}

/// The generated sources (grammar tables, icon data, morph plans, test vectors) are built, not
/// committed: `tools/generate/generate.mjs` writes them before the bake reads them, and names its
/// inputs and outputs to Cargo (`rerun-if-changed`).
fn generate() {
    let status = exact_bake::bun()
        .arg("../tools/generate/generate.mjs")
        .status()
        .map_err(exact_bake::bun_error)
        .expect("run tools/generate/generate.mjs");
    assert!(status.success(), "tools/generate/generate.mjs failed");
}
