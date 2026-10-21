# Code review: stage 4 commit 6: uses, resolve and scrub sessions, round 3, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** the whole range, the round-1 and round-2 fixes first.
- **Method:** one brief (sha256 `c051c8b8687183970cdc1e1cac52b06b5a00b31caac516dd2af51eff74d0bc91`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (a release's build consuming the next scrub's preview; a timed-out preview leaving the compiler scrubbing; locals).
- **Disposition:** applied without re-review (the last round) in the third-round commit.

---

No P0. One P1 blocks back-to-back scrubs: a release's build still consumes whatever preview is current, so the next scrub's preview is thrown away.

## Findings

**[P1] Bind the release to the scrub it writes — host/web-js/dev.mjs:155**

`released` is a boolean, and `releasing` in `scripts/tamber-scrub.mjs:130` drops as soon as the transaction returns, not when that write's build has been sent. The next scrub is then allowed to preview while the release compile is still in flight. The value-only build that finally lands does `if (scrubbed && released) scrubbed = null` and batches from whatever preview is installed now back to the released design.

That is the usual window on the JS loop: the release compile is a full rebuild, and `exact-web-js design` often finishes first. The pages jump off the new gesture onto the value just written; the server still has the new scrub open, so a pointer-up writes ops the pages are not showing. The same flag is consumed in `host/web/src/dev.rs:249` (stdin is drained before `poll`, so a `scrub` that arrives in the 10 ms sleep is installed and then taken by the release build) and in `host/web/dev.mjs:682` (a bake already `active` is dropped, the new scrub reply is applied, and the re-queued bake then clears it). Round 2 stops a non-release build from doing this. It does not stop the release build itself.

**[P2] A timed-out preview leaves the resident compiler scrubbing — host/web/dev.mjs:289**

`scrubAsk` deletes its callback at 30 s and resolves `{error}` without telling the compiler. `Session::scrub` has already stored the preview (`host/web/src/dev.rs:332`) before the reply is written. `show` then takes the error path (`scripts/tamber-scrub.mjs:73`) and, on a first preview, `failed()` drops the session and does not `restore`. Later value-only batches are computed from the preview to itself and not sent, so hand edits never reach the page until some later build is structural. A later update that times out the same way desynchronizes the page (last pushed batch) from the compiler (unapplied preview); cancel only lines up again because the restore batch's base is stale and the page fetches the snapshot.

**[P3] A slot filled through a `let` is not a use — contract/cli/src/tamber_uses.rs:151**

`BindLocal` / `DropLocal` start a new store run, and `LoadLocal` carries nothing. `let c = "brand"; ink = c` does not mark `ink`, so `color=ink` is not a site. A `let` inside the row expression is still found, because a row's whole body is scanned. Copies through slots and derives are followed; copies through locals are not.

## Checked, not findings

- Latest-wins, idle-from-arrival, and "ended during the first preview is not announced" match the code and the tests.
- A preview that resolves after `end` is restored after it is applied. Release writes once, against `open.files` from the open, and only after `busy` and `waiting` drain. A conflict restores and does not write.
- `with_design_text` is thread-local and restored on drop, including unwind. A normal build does not wrap the read, and the JS preview is `exact-web-js design … -` on stdin.
- `uses` matches the plan's strings: comparisons in a straight run (including a concatenation), `url(#brand)`, whole names (`brand` vs `brand-2`, `lab` vs `oklab`), colour codecs only, and the node span the source map actually has. `file` other than the app design returns no sites.
- Scope is the server's `scopeOf`, not a field the client can set. Dev loops leave it as write; the propose-scope 403 is real only when a caller passes `scopeOf`. Client id is the same self-asserted string as every other write.

The session tests do what they say. They never open a second scrub while `released` is still set, and nothing times out a compiler reply that has already been applied.
