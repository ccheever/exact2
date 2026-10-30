# Code review: symbol tests catch up with symbols.js and timer-observations, quick review (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `8f7ce5c7ce`.
- **Method:** one brief (sha256 `bd5b63ffb1da51e637259f75308db1280cd1e8de2966351abaa610494d5d81e0`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

No BLOCKER/MAJOR/MINOR/NIT findings.

- The seeded sets in [symbol-size:70](host/web/tests/symbol-size.test.mjs:70), [symbol-hole:23](host/web/tests/symbol-hole.test.mjs:23), and [bar-symbols:60](host/web/tests/bar-symbols.test.mjs:60) faithfully supply these fixtures’ images to the real `refresh`. Existing assertions remain intact. Registration and scope cleanup remain outside their coverage.
- The no-op `image` in [symbol-hole:72](host/web/tests/symbol-hole.test.mjs:72) removes only registration bookkeeping. All 13 icon-form cases produced identical asserted output with the real registration hook and the stand-in.
- `host/` comes last in [app-frame:12](host/web/tests/app-frame.test.mjs:12) and [symbol-size:337](host/web/tests/symbol-size.test.mjs:337), so it cannot shadow either earlier root. Eight in-memory route checks confirmed correct resolution.

No other stale assumptions found in these harnesses. Browser tests could not run because `rolldown/utils` is unavailable.

Verdict: LGTM for both test-only fixes; existing coverage is preserved.
