//! Bake T3 Code for macOS. The generated sources the bake reads are made by `t3-code-data`'s
//! build script, which Cargo runs before this one compiles.

fn main() {
    exact_js_bake::build_mixed_with(
        std::path::Path::new(".."),
        "macos",
        &t3_code_data::Markdown,
        &[
            "renderMarkdown",
            "renderPullRequestMarkdown",
            "renderFileMarkdown",
        ],
        |javascript| {
            Box::new(t3_code_data::mixed(
                javascript,
                exact_runner::Placement::Main,
            ))
        },
    )
    .expect("bake T3 Code for macOS");
}
