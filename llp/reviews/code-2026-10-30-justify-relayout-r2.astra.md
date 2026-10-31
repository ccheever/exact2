# Code review: web hosts follow a block parent's justify-items (progress), round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `/tmp/dt3-rv-c2`.
- **Method:** one brief (sha256 `232574aede38b3e586834196cc8a7731bacfe2f30072d587b4d339b1d2cd39a3`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **MAJOR — [host/web-js/src/rows.rs:535](/tmp/dt3-rv-c2/host/web-js/src/rows.rs:535): normalization fails during JS server rendering.** `--render js` uses [dom.js:58](/tmp/dt3-rv-c2/host/web-js/dom.js:58), whose style methods preserve strings without CSS parsing. Running the actual mapper against that DOM returns `auto` for `" normal "` and `"stretch "`, instead of `initial`. Repro: bind either value on a 240px block containing an unsized progress. The emitted CSS lets the spinner stretch to 240px before adoption, following [CSS block sizing](https://drafts.csswg.org/css-align-3/#justify-block). Comments, escapes and invalid values expose the same assumption. Normalize consistently during server rendering and cover the result before adoption.

The other round-1 fixes check out by source tracing: inheritance resets, the guaranteed-invalid fallback, wasm parent-change refresh through `refold`, absolute positioning, and line caps. The emitter assertions now detect removal of the bound writer, but miss the server-rendering case above.

Validation: the existing grid-filter test passed (44 assertions), plus 46 canonical mapper checks. Rust and browser suites were not rerun; this read-only checkout lacks build artifacts and dependencies.

**Verdict: changes requested — fix JS server-render normalization before landing.**
