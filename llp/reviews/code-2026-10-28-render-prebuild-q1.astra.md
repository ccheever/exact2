# Code review: document-render.test's warm-up build (main red), quick review, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `9b98519547`.
- **Method:** a narrow brief naming the earlier findings and their fixes; a quick review (a mechanical main-red fix, test only), then a narrow re-check. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Reviewed the first version (a cargo build of the renderer alone); its SHOULD and its note led to the warm-up build the narrow re-check reviewed.

**SHOULD — [host/web-js/document-render.test.mjs:12](host/web-js/document-render.test.mjs:12):** Add `env: { ...process.env, EXACT_UPDATE_TRUST: 'development' }`. The renderer’s `cargo run` forces that value, and the bake tracks it with `rerun-if-env-changed`. With the usual unset environment, the first Rust test still reruns the bake and recompiles/relinks.

Package, binary, profile, default features, working directory, and `CARGO_TARGET_DIR` inheritance match. Build failures report clearly; `beforeAll` placement is sound.

One existing cold-build dependency remains: [build.mjs:387](host/web-js/build.mjs:387) builds Caltrain’s separate WASM data module inside these tests. The async prepare step normally warms it; this hook alone does not.

Read-only review; no builds or tests run.
