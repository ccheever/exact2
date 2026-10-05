#[path = "src/markdown.rs"]
mod markdown;

fn main() {
    // Mixed-source ownership is part of the bake, not only the linked app.
    println!("cargo:rerun-if-changed=src/markdown.rs");
    exact_js_bake::build_mixed_with(
        std::path::Path::new(".."),
        "macos",
        &markdown::Markdown,
        &["renderMarkdown", "renderPullRequestMarkdown"],
        |javascript| Box::new(markdown::mixed(javascript, exact_runner::Placement::Main)),
    )
    .expect("bake T3 Code for macOS");
}
