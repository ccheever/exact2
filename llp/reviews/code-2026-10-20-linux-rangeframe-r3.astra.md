# Code review: the Linux rangeFrame reader (LLP 1114.007 D7), round 3 of 3, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `1280ff8f71`.
- **Method:** one brief (sha256 `f6016248bcc75e0ddcd5a5278704e41cc75f08c92413dcc8f6d505648929aa9c`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a host's geometry read, theme2's 17:27 ruling), then a narrow check of the last fix. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (1 BLOCKING).
- **Disposition:** taken: the first fragment left to right, measured under `direction: rtl` ("Round 3").

**Verdict: BLOCKING — one remaining bidi caret defect.**

Round 1 fixes check out in source: text-content heights and baselines, LF/CRLF overlap, upstream soft wraps, per-fragment transforms, input/Android refusal, reload isolation, and restored wasm/JS numeric comparison.

Round 2’s supplied bidi example is fixed. Empty-field and empty-line behavior follows G3, including the synthesized final line. Empty-box filtering now follows transformation; publication requires a matching box. Acknowledged/pending pictures, failed frames, and reload sessions remain separated. The `{ slots }` exclusion affects only Linux’s `sizes`; grammar, LLP 1114.007, and QUEUE record the approved deviations.

1. **BLOCKING — the bidi fix assumes logical order equals fragment order.** [host/linux/src/text/field.rs:296](host/linux/src/text/field.rs:296) always prefers the span ending at the offset. For a `direction="rtl"` textarea containing `"אבג abc"`, collapsed `4..4` therefore uses the preceding space’s end, at **`abc`’s right edge**. The painted caret uses the following span’s left edge ([field.rs:217](host/linux/src/text/field.rs:217)), at **`abc`’s left edge**: a popover moves across the whole Latin word.

   Chromium’s [range traversal](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/layout/layout_text.cc#L520) visits these fragments in [visual order](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/layout/inline/inline_cursor.h#L482), so its first empty rectangle also comes from `abc`. This counterexample is inferred from source, not newly browser-measured. Preserve visual fragment ordering when resolving boundary carets; the current regression covers only the supplied LTR example.

Validation: Cargo was blocked opening `.cargo-build-lock`. Bun reported **7 passed, 1 failed**; the failure was a sandbox-denied sweep-log write during existing test setup. No files modified.
