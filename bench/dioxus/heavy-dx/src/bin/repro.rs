//! Minimal Blitz repros: inline whitespace between spans, and color emoji.
use dioxus::prelude::*;
fn main() { dioxus::launch(App); }
#[component]
fn App() -> Element {
    rsx! {
        div { style: "font: 20px system-ui; padding: 16px; display: flex; flex-direction: column; gap: 12px",
            div { "A: " span { "one" } span { " " } span { "two" } }
            div { "B: " span { "one " } span { "two" } }
            div { "C: " span { "one" } " " span { "two" } }
            div { "D: one two (plain text)" }
            div { "E: 😂 🍕 👨\u{200d}👩\u{200d}👧 🤔 ✅" }
            div { style: "font-family: 'Apple Color Emoji'", "F: 😂 🍕 (font-family Apple Color Emoji)" }
        }
    }
}
