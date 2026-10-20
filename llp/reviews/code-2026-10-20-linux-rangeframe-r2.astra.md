# Code review: the Linux rangeFrame reader (LLP 1114.007 D7), round 2 of 3, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `4c268d6669`.
- **Method:** one brief (sha256 `70b48d232edfd5db25ae0cbd0a3405cfe0dda488b0e0f2edd969ceaa1e57a30d`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a host's geometry read, theme2's 17:27 ruling), then a narrow check of the last fix. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (2 BLOCKING, 2 NIT).
- **Disposition:** the bidi caret, the transform and wording NITs taken; the empty-line answers ruled by theme2 (18:42, G3) as the web's stated deviation ("Round 2").

**Verdict: BLOCKING.**

Round-one reload isolation, input/Android refusal, CRLF overlap, and restored wasm/JS numeric comparison check out in source. The supplied height, newline and wrap examples are addressed. Remaining findings:

1. **BLOCKING — collapsed bidi boundaries choose the wrong fragment.** [host/linux/src/text/field.rs:293](host/linux/src/text/field.rs:293) always selects the following logical span. For LTR `"abc אבג"`, `4..4` therefore lands at the Hebrew run’s right edge. Chromium collects both boundary fragments and returns the first empty rectangle: the preceding Latin fragment’s end. A caret-anchored popover shifts across the Hebrew word. Preserve fragment ordering and test both direction changes. This comparison follows [Chromium’s fragment collection](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/layout/layout_text.cc) and [bounding rule](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/dom/range.cc).

2. **BLOCKING — empty and trailing-empty values still differ from the stated Chrome oracle.** [host/linux/src/text/field.rs:300](host/linux/src/text/field.rs:300) returns a font-height strut rectangle for an empty value, and the synthesized final line supplies the caret for `"ab\n"` at `3..3`. Chromium’s implementation resolves an empty value to no text node, yielding a zero rectangle; trailing-newline offsets resolve to the last text node’s end, after `"ab"`. The web glue returns these answers unchanged. These ordinary composer states need explicit probes and either matching behavior or an approved, documented deviation; the new tests cover neither. Source evidence: [offset resolution](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/html/forms/text_control_element.cc), [OpaqueRange geometry](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/third_party/blink/renderer/core/dom/opaque_range.cc).

3. **NIT — empty-box filtering precedes transformation.** [host/linux/src/paint/field.rs:141](host/linux/src/paint/field.rs:141) discards locally zero-width boxes before transforming them. Under rotation, those boxes acquire nonzero bounding widths. For a rotated `"a\n\nb"` range `1..3`, later break fragments are consequently omitted. Filter the transformed bounds, as [Chromium does](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/dom/range.cc). The existing rotation test contains ordinary text on both lines and misses this case.

4. **NIT — stale “line boxes” wording remains.** [docs/contract-grammar.md:1161](docs/contract-grammar.md:1161) and [LLP 1114.007:194](llp/1114.007-highlights-in-a-text-field.rfc.md:194) still describe Linux as reading “glyphs’ boxes and line boxes,” contradicting the newly specified font-content boxes.

Chrome conclusions above are inferred from source, not browser measurements. Cargo was blocked by the read-only sandbox; Bun produced no test results before cancellation. No files changed.
