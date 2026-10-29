//! T3 Code's Rust data source: the chat Markdown (`renderMarkdown`, `renderPullRequestMarkdown`,
//! `renderFileMarkdown`) and the code colours' lexers (`highlightCode`), composed with the app's TypeScript
//! module by `mixed`. Each host links it, and each host's bake runs it.

mod highlights;
mod markdown;

pub use markdown::{mixed, Data, Markdown};

/// Pierre's file-icon tables for the Markdown module (pierre_icons.rs): the build script generates
/// `pierre-icons.tsv` before this crate compiles (tools/generate/gen-icons.mjs).
fn pierre_tables() -> &'static str {
    include_str!("../pierre-icons.tsv")
}
