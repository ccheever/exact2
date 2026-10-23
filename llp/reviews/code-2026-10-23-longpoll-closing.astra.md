# Code review: long-poll streams (LLP 1016.000 D8), round the closing re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `982e939705`.
- **Method:** one brief (sha256 `cfaf6aab6c063f2243b74464b343bb43ce09ea630e143f6fa4ebbfe81ebe3090`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking:** none.  
**Should-fix:** none.  
**Nits:** none.

Both fixes look correct by inspection:

- [ts-stream.js:25](host/web-js/ts-stream.js:25) rejects inherited Request bodies when `init.body` is undefined or null.
- [http-body.js:162](host/web/http-body.js:162) matches native’s required HTTP(S) prefix; [rust-data.js:105](host/web-js/rust-data.js:105) rejects it before writes apply.

`bun test host/web/http-body.test.mjs` could not execute tests: its Cargo setup was denied filesystem writes by the read-only sandbox.
