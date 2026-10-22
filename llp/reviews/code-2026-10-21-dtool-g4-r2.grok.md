# Code review: the design tool's Gallery view, G4, round 2 of 2 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** one brief (sha256 `9811c48335708167da4769d76031e49bff6a80957b50853bb7c67d7a15365d46`), shared by both reviewers, each blind to the other's review; HEAD `41786fcae1`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (from its findings; its narration before them left out).
- **Verdict:** nothing blocks (2 NIT); round 1's closed.
- **Disposition:**
  Nothing blocks from either reviewer. All taken post-round, without another round (noted in SIGNOFFS):
  - Astra 1, Grok NIT 2 (framing): the gallery page's header must equal the loop's FRAME_ANCESTORS exactly (the tool's origin and its own), on the real loop. The browser's refusal of a frame from another origin rests on that header, as the canvas's own tests do.
  - Astra 2, Grok NIT 1 (the param case): the join test gives a gallery cell a param row. It stays the app's param rung, with no template and so no detach, beside the token row's token rung. The contract test asserts no tokenize or detach on the token row, and no tokenize on the literal one.

1. **Reload URL on toggle** — `apps/design-tool/grid.ts:159`. When the framed page changes, `reloaded`, `sentTo`, `went`, `seen`, and `place` are cleared before a tile `src` is chosen. A cell reloaded at `/post/3` then frames `/__dev/gallery/`. The grid contract covers that path at `apps/design-tool/app.test.contract:2280`.
2. **Foreign-origin framing** — `host/web-js/dev-gallery.test.mjs:95`. The gallery HTML response is asserted to carry `frame-ancestors`. That header is applied by the same `frameOnlyByTool` wrap as every other HTML response (`host/web-js/dev.mjs:887`, before `gallery.handle`). The `example.com` case remains origin filtering in `pageOf`.
3. **Inspector join** — `scripts/tamber-inspect.test.mjs:795` calls `join` on a gallery file: a token row stays on the token rung with no template, and a literal row gets no write rung and the gallery reason. The contract test at `apps/design-tool/app.test.contract:2292` shows the token edit and no row, promote, detach, or state control. Promote, tokenize, and detach are still derived only from a template rung or a param that has a template (`apps/design-tool/inspector.ts:58`).
4. **Stub restart** — `apps/design-tool/stub/tamber-stub.ts:451` sets `galleryCopies = []` with the other copy configuration.

**Re-checked on the whole change.** `galleryFrame` is the local loopback frame plus `/__dev/gallery/`. The toggle and the “kept edit, not each scrub frame” note are local-only and hidden in States; paired mode keeps the gateway frame. The wasm loop answers `/__dev/gallery/` with the plain-text 404 (`host/web/dev.mjs:1340`). A gallery build is labelled, does not lead, and is not sent `navigate` or reloaded onto an app route; pins, holds, and give-back still run. `behind` is that build’s own pin revision. The Live chip is the proposal class. Token and param edits set the open file to the app design (`apps/design-tool/app.contract:1180`).

1. **NIT** — `scripts/tamber-inspect.test.mjs:795`. The new join test never passes a param row. A whole-value param is an earlier branch (`scripts/tamber-inspect.mjs:383`): it stays a param edit, and the gallery file is attached only when the write rung is `template`, which a gallery file does not get. That branch is untested.

2. **NIT** — `host/web-js/dev-gallery.test.mjs:95`. The `frame-ancestors` check is unanchored and accepts any `127.0.0.1` port, so an extra ancestor or the wrong tool port would still pass. The header the loop sets is the tool origin only.

No files changed. No builds or test suites run.
