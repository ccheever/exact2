# Code review: LLP 1146 D2, an aborted kept write in a module turn, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `7feec937aa`.
- **Method:** one brief (sha256 `eef25cd6f2ee6c3d2dceb9ae6ef96b75750cefed428c873f504dfd7e8a074866`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [bake/src/compat.rs:50](bake/src/compat.rs:50) — **Bump `TYPESCRIPT_RUNTIME`.** Newly baked bundles still require runtime 5, whose embedded prelude can execute the rejection inside a refused commit and lose its store writes. That is observable data loss, not merely earlier rejection. The compatibility rule covers behavior a bundle depends on, regardless of whether an API was added. Bump to 6 and update the runtime paragraph in the reference.

**Non-blocking**

1. [js/tests/it/outlive.rs:343](js/tests/it/outlive.rs:343) — **The server outlives its test.** The accept loop runs forever, both thread handles are discarded, and opening the shared gate does not ensure the handler finishes before another test resets it. Setup also runs outside the cleanup’s `catch_unwind`. Use a single-request server with owned cleanup and join handles covering setup and failure. Separately, the existing timeout diagnostic at line 172 panics while holding `ASKED`, poisoning subsequent tests; clone the diagnostic first.

2. [host/web-js/store-readers.test.mjs:109](host/web-js/store-readers.test.mjs:109) — **The web test permits false positives.** Every fetch rejection writes `"aborted"`, and the final assertion does not establish that the aborting commit was refused. It can pass after an unrelated fetch failure or if the first increment incorrectly commits. Assert the abort reason and that `round` remains zero after the first tap; synchronize with request arrival if claiming the server held it.

3. [docs/reference.md:1038](docs/reference.md:1038), [LLP 1146:85](llp/1146-oauth-sign-in.rfc.md:85) — **Narrow the web store guarantee.** The rejection continuation writes outside the synchronous commit. Arbitrary web store writes inside a refused commit are rolled back by `Store.restore` in `rt.js:194`. Say specifically that the abort continuation’s writes survive. The statement that wasm realms do not adopt is accurate.

I found no additional runtime or liveness blocker: adoption finishes before abort collection can race its registration; kept fetches bypass both expiry caps; competing replies settle once; abort answers bypass the ordered-refusal fence; and outstanding-work tracking plus progress releases an awaiting answer. Reload destroys the engine, workers/wasm do not adopt, and an unsupported door 23 takes the immediate-rejection fallback. The claimed-ticket guard matches the parking invariant.

The native test exercises the reported rollback failure. I did not independently reproduce its red/green result; no tests or builds were run.
