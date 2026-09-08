//! The document the reader opens with — written in the format it reads, so
//! it is also the proof that every block kind renders.

/// The welcome document.
pub const WELCOME: &str = r#"# Markdown

A reader for Markdown files. Open one with **⌘O**, drop a path in the field
above, or hand it to the app from a terminal:

```sh
mdview README.md
mdview ~/projects/exact2/llp
```

Everything below is what this reader draws.

## Text

Prose wraps at a comfortable reading width, with **bold**, *slanted*, and
`inline code`. A [link](https://example.com) is underlined and opens where it
points; a link to a file beside this one opens it here instead.

## Lists

- An unordered item
- Another, long enough to wrap so that the hanging indent under the marker is
  visible rather than merely asserted
  - A nested item
1. An ordered item
2. And the next

## Quotes and rules

> A block quote sits inside a rule on its leading edge.

---

## Code

```rust
fn main() {
    println!("nothing here is executed — a code block is text");
}
```

## Tables

| Column | What it holds |
| --- | --- |
| `path` | where the file came from |
| `blocks` | what the parser made of it |
"#;
