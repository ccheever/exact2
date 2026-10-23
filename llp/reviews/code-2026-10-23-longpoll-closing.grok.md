# Code review: long-poll streams (LLP 1016.000 D8), round the closing re-check (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `982e939705`.
- **Method:** one brief (sha256 `cfaf6aab6c063f2243b74464b343bb43ce09ea630e143f6fa4ebbfe81ebe3090`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking:** none.

**Should-fix:** none.

**Nits:** none.

Both fixes hold. `host/web-js/ts-stream.js:25` refuses a long poll when the `Request` has a body and `init.body` is `null` or `undefined`, before `streamed()` runs. The Fetch standard uses that input body in both cases (and throws for `GET`/`HEAD`). `host/web/http-body.js:162` now requires an `http://` or `https://` prefix, matching `runner/src/request.rs:465`. `https:example.test/changes` fails that check; `new URL` would otherwise turn it into `https://example.test/changes`. `host/web-js/rust-data.js:105` throws that refusal with no `refuse` flag, so the turn's writes are not applied. `bun test host/web/http-body.test.mjs`: 18 pass, 0 fail.
