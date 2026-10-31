# Code review: web hosts follow a block parent's justify-items (progress), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `/tmp/dt3-rv-c1`.
- **Method:** one brief (sha256 `d8c210e5dc64b059eb8f093f62c6797ef58a18a8b4d548b7368707b7dcd735ee`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **MAJOR — [style.rs:450](/tmp/dt3-rv-c1/host/web-js/src/style.rs:450): Alignment leaks through ancestors.** Normal/stretch parents never reset `--exact-progress-aligned`. An aligned outer box containing a progress and an inner 240px block containing another progress leaves the inner spinner inheriting `auto`. That overrides its parent’s `--exact-progress-justify:start`, stretching it to 240px instead of 20px. Emit `initial` for normal/stretch parents; custom properties inherit. [CSS specification](https://www.w3.org/TR/css-variables-1/#defining-variables)

- **MAJOR — [rows.rs:532](/tmp/dt3-rv-c1/host/web-js/src/rows.rs:532): The marker disagrees with the actual alignment writer.** `normal()` accepts `unset`, CSS-wide keywords and legacy forms, while `gridValue("justify", …)` clears or rejects them. The new mapper nevertheless writes `auto`. Thus changing alignment from `center` to `unset` makes a block spinner stretch. An in-memory execution confirmed this disagreement for `unset`, `initial`, legacy forms and `last baseline`. Classify the effective, validated value.

- **MAJOR — [element.rs:420](/tmp/dt3-rv-c1/host/web/src/element.rs:420): The blocking line cap fails.** Running caps with compilation disabled (`EXACT_CAPS_WASM=0`) reports **1,517 lines**, exceeding 1,500. The added helper takes this previously compliant file over the limit.

- **MAJOR — [document-render.test.mjs:728](/tmp/dt3-rv-c1/host/web-js/document-render.test.mjs:728): Existing assertions retain the buggy behavior.** This adoption test expects the sized spinner at `x:0` under a 240px block with `justify-items:center`, both initially and after returning from grid at line 732. The fix correctly implies `x:110`; update both expectations while retaining the display/adoption coverage.

- **MINOR — [emit_tests.rs:1003](/tmp/dt3-rv-c1/host/web-js/src/emit_tests.rs:1003): The bound assertion does not test the writer.** Every progress’s new `var(...)` expression already contains `--exact-progress-aligned`. Removing the entire new `rows.rs` branch would still satisfy this assertion. Assert the parent’s generated write and exercise alignment changes while its display remains block.

The wasm refresh path looks correct: touched parents reach `refold`, which recomputes progress children’s CSS. The admitted positional values, RTL handling and baseline fallback appropriately defer to CSS; `initial` correctly activates the custom-property fallback. Absolute progress remains bypassed, relative/sticky remain in flow, and canvas’s existing behavior is unchanged. [Alignment rules](https://www.w3.org/TR/css-align-3/#justify-block), [custom-property reset](https://www.w3.org/TR/css-variables-1/#guaranteed-invalid)

The new static assertions catch the original defect by inspection. Rust tests and live browser checks were unavailable: this read-only checkout has neither build artifacts nor installed JS dependencies.

**Verdict: request changes.**
