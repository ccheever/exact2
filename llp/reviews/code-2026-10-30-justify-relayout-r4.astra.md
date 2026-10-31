# Code review: web hosts follow a parent's justify-items (canvas), narrow re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `/tmp/dt3-rv-c4`.
- **Method:** one brief (sha256 `c6321c43fc2a457a4380debc31da5d56110379e7294f6442803f9d261425177c`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **MAJOR — [rt.js:654](/tmp/dt3-rv-c4/host/web-js/rt.js:654): Canvas adoption retains stale inline alignment.** Rust rendering under `justify-items:normal` writes inline `justify-self:start`. Adoption removes that property only for progress. Changing the parent’s bound alignment to `center` updates the custom property, but the canvas’s retained inline `start` overrides its class. Confirmed by executing the adoption function against the repository’s DOM implementation. A bound canvas position can overwrite it; an ordinary static-position canvas remains stale.

- **MAJOR — [style.rs:463](/tmp/dt3-rv-c4/host/web-js/src/style.rs:463): Nested canvases read their own alignment variable.** For `box justify-items=center > canvas justify-items=normal > canvas`, the outer canvas declares `--exact-progress-aligned:initial` for its child. Its own `justify-self:var(--exact-progress-aligned,start)` consequently resolves to `start`, losing the box’s centering. The wasm host correctly consults the outer canvas’s parent. A progress child produces the same collision. This is a static difference between hosts, also affecting bound updates.

- **MAJOR — [sized_css.rs:26](/tmp/dt3-rv-c4/host/web/src/sized_css.rs:26): Grid `stretch` remains incorrectly suppressed.** This is a retained gap: both hosts force `start` under `justify-items:stretch`, including grid parents. For a 240px grid track containing an auto-width canvas with `height:40px`, the kernel’s explicit-stretch path supplies 240px width; the hosts retain the ratio-derived 80px width. Explicit grid stretch applies to replaced elements too; the natural-size exception belongs to `normal`. See [CSS alignment sizing](https://drafts.csswg.org/css-align-3/#justify-self-property). The shared block/grid exemption therefore remains too broad.

- **MINOR — [rows.rs:538](/tmp/dt3-rv-c4/host/web-js/src/rows.rs:538): Server-side classification is not correct for every CSS spelling.** The allow-list covers the intended canonical keywords, but the unparsed server string `\63 enter` means `center` and returns `initial`. Conversely, `safe
