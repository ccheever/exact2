# A figure URL is inserted raw, so ) and \ break the source

**Status:** Closed
**Resolution:** Fixed: percent-encode figure URL syntax and whitespace so both inline and borrowed block readers retain one image; regression tests cover parentheses, backslash, space and newline.
**Systems:** Markdown editor
**Severity:** P2
**Author:** Grok 4.7 for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1045

Link targets go through `destination`, which escapes `\`, `(`, `)`, `<`, `>`, and percent-encodes whitespace (`markdown/src/edit.rs`). Figure targets do not. `figure` writes `format!("{lead}![]({src}){trail}")`.

The inline scanner ends a destination at the first unescaped `)` at paren depth 0, and `\` skips the next character. `link` is tested for `https://example.com/a)b`. `Command::Figure` with that URL on an empty editor produces `![](https://example.com/a)b)`. The image target is `https://example.com/a` and `b)` is leftover text. A `\)` in the URL consumes the closing `)` as an escape. A newline in the URL splits the paragraph, so it is no longer one figure.

Use `destination` for the figure source. Done when a figure URL with `)`, `\`, or a space round-trips as one image whose target is the URL that was given.
