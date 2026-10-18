# Code review: LLP 1122 T3 step 1, the param rung in the selection join, round 3, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `5c7138e94` (before dt/t2's rebase onto main; it is `2d59d8578` after, the shas named in the briefs map as /tmp/dt-t3/sha-map-1.txt says).
- **Method:** one brief (sha256 `a006ec781c9a3f315c2fa6f7d99e3718497953995b6ba8ecc97871b66d3263f7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 1 medium).
- **Disposition:** taken in `5684f39d3`: a describe whose files are not those its read was for is not kept (and is tried again); a write's files read the params again (the server's publish); two params whose page property would be one are named by neither in a page's text, so the suffixed one never takes the other's name; a held state's rows (stage 8 E, which the rebase brought) are scrubbed as the selection's. The stale comment is corrected. The param tests moved to scripts/tamber-param-rung.test.mjs (`a8cfb59f7`, the line cap), with a test of the files mismatch.

I would block this round. Two stale-alias bugs remain, and the CSS-name fix introduces a regression.

1. **[P1] A `describe` response is accepted under the wrong file-digest key.**  
   [scripts/tamber-server.mjs:378](/tmp/rv-dt-t3r3/scripts/tamber-server.mjs:378) installs the map without checking `r.ok.files`.

   Concrete sequence: `hello` sees D1, where `Card.fill` aliases `paper`; the subsequent `describe` reads D2, where it aliases `accent`; the file returns to D1 before the next watcher read. The cache now holds D2’s alias under D1’s key. The next D1 hello returns early at line 370, so selections continue offering `token: "accent"`.

   I reproduced this through the server with an in-memory filesystem: `readOnly: null`, expected `paper`, offered `accent`, and no retry. Round 1’s requested snapshot validation remains unfixed.

2. **[P1] Successful writes announce changed files without invalidating the param map.**  
   Invalidation happens through [scripts/tamber-server.mjs:358](/tmp/rv-dt-t3r3/scripts/tamber-server.mjs:358), but the write response only publishes its files at [line 706](/tmp/rv-dt-t3r3/scripts/tamber-server.mjs:706).

   Change `Card.fill` from `ink` to `paper` through a transaction. Once the build takes that live design, a same-plan selection passes the source checks but still offers `ink` until the watcher’s hello reaches `learnParams`. I reproduced the acknowledged write followed by `readOnly: null` and `token: "ink"`; no new describe had started. Invalidate when the write’s changed digests become known.

3. **[P2] Guessed CSS property names misidentify valid params.**  
   [scripts/tamber-inspect.mjs:132](/tmp/rv-dt-t3r3/scripts/tamber-inspect.mjs:132) derives properties by replacing punctuation, but the [JS producer adds collision suffixes](/tmp/rv-dt-t3r3/host/web-js/src/modes.rs:124).

   Two params in one file, `Card.fill-wide` and `Card-fill.wide`, receive the same guessed CSS key. In the reproduction, a dynamic expression reading the first was renamed to the second’s full name; the producer’s suffixed property became “a component param” despite both params being known. This affects both published events. The scrubber needs the producer’s actual property mapping.

The other earlier fixes checked out in the exercised cases: both events are scrubbed; dotted synthetic expressions are normalized; ordinary dotted tokens survive; role casing works without weakening ordinary token case sensitivity; inherited/dynamic direct-param links survive; internal unread flags stay private. New-plan reads withhold aliases, and populated-cache failures—including 504s and late replies—retry correctly. Conditional params withhold aliases; `modes` carries the documented full tuple for `explain`. The current UI explicitly handles the param rung.

Smaller point: [scripts/tamber-server.test.mjs:78](/tmp/rv-dt-t3r3/scripts/tamber-server.test.mjs:78) still always returns D1 and `ink`, fails only the initial describe, and retains fixed sleeps. It does not cover the file transitions above or failure after a populated map.

Six existing tests and 162 in-memory scrubbing cases passed, alongside the lifecycle reproductions. I did not run suites whose setup writes files. No files or Git state changed.
