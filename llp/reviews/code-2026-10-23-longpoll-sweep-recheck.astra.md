# Code review: long-poll streams (LLP 1016.000 D8), round the sweep's re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `6b968c7f57`.
- **Method:** one brief (sha256 `0f19e8beafd90eb70f965ccc3bfce7e73e6e8429af275e3b8ebdb948094ce382`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- [ts-stream.js:25](host/web-js/ts-stream.js:25): `body: null` bypasses the inherited-body refusal. A body-bearing POST `Request` with `{method: "GET", body: null, exactStream, exactLongPoll}` opens a poll after silently dropping its body. Fetch still inherits the input body here and requires rejection. [Fetch §5.4](https://fetch.spec.whatwg.org/#dom-request)
- [rust-data.js:105](host/web-js/rust-data.js:105): The new check uses [http-body.js:162](host/web/http-body.js:162), which accepts `https:example.test/changes`; native validation requires `https://`. A tag-10 reply therefore applies writes and can start polling on JS while native refuses before writes.

**Should-fix:** none.  
**Nits:** none.

Other fixes look correct by inspection; no additional terminal regression found. The agreed `dt/refhang` defect is excluded.

All four permitted test commands were blocked before test execution by sandbox write denial.
