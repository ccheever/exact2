# Code review: stage 4 commit 6: uses, resolve and scrub sessions, round 2, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** the change commit and the first-round commit.
- **Method:** one brief (sha256 `3dcb3143ecdfc6f8ec244069a9ea328137f5633dcbd49dcee574135e1bc50c22`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocking findings; P2s (a queued update announcing `opened`; slot carriage; a rebuild dropping the preview) and P3s (idle; `via` for slots and derives undocumented).
- **Disposition:** applied in the second-round commit.

---

Nothing is blocking. A scrub writes nothing while it is open, release writes once against the opening digests or writes nothing, and a preview that lands after an end is taken back. No P0 or P1.

## Findings

[P2] A queued update announces `opened` for a scrub that already ended — `scripts/tamber-scrub.mjs:43` and `scripts/tamber-server.mjs:502`

The first update is still previewing (`shown` is false, so the server has not emitted `opened`). A second update sits in `waiting`. A competing write, disconnect, server loss, or idle calls `end`, which does not emit (nothing has previewed) and resolves that waiter with `{ok: {scrub, ended}}` rather than 409. `scrubbing` treats any non-superseded `ok.scrub` it has not seen as `reason: 'opened'`. Subscribers stay on "previewing" with no later end. The builds are on the committed design: the in-flight preview returns 409 and `show` restores. The next real scrub emits another `opened`.

[P2] Slot carriage misses a copied token and invents sites from the rest of an action — `contract/cli/src/tamber_uses.rs:155`

`holds` marks a slot only when its own init, or the whole body of an action that writes it, pushes the token's name. It does not follow `LoadSlot` or `LoadDerive`, and derives are solved only after slots. `state src = "brand"; state ink = src; color=ink` and `derive tone = "brand"; state ink = tone; color=ink` paint the token and produce no site (`color=tone` does). The other way: an action that assigns `ink = "#111"` and also sends or logs `"brand"` writes `ink`, so `color=ink` is reported as a site of `brand`. Direct, alias, and synthetic literals are matched on the same word split as the theme scanner, so `oklab(...)` does not name `lab(...)`.

[P2] A non-design rebuild drops the preview and does not end the session — `host/web/src/dev.rs:241`

`build` takes `Session.scrub` on every successful compile. Only a design-only edit emits a live batch; a logic rebuild prints `plan` and `host/web/dev.mjs:486` clears `designSnapshot`. The wire's scrub stays open, so the tool still says "previewing". Web pages reload onto the file. A native host that applied the scrub batch is not on that reload, `/__dev/design` is empty, and `restore` is empty because the compiler already dropped the scrub. Cancel and idle leave that host on the gesture. Release still writes the last previewed ops. The JS loop (`host/web-js/dev.mjs:140`) and the TypeScript producer (`host/web/dev.mjs:674`) clear their preview the same way; the JS loop does send a correcting batch when the design shape holds.

## Lower

[P3] Idle is still measured from the preview's end — `scripts/tamber-scrub.mjs:73`

Arrival starts the 30 s timer, then a successful `show` calls `idle()` again. A preview that finishes in 10 s keeps the scrub until 30 s after that finish. The test waits on instant previews, so both clocks pass it.

[P3] `via` for a slot or a derive is not in the protocol — `docs/tamber-protocol.md:178`

The read documents absent, `{token}`, and `{synthetic, through}`. Round 2 returns `{slot}` and `{derive}` (`tamber_uses.rs:172`, `tamber_uses.rs:199`). A client that switches on the documented shapes misreads those sites.

## Round 2, and the tests

These fixes hold: the scrub is open from the first update; an end during that preview restores when the preview returns; a release waits out the preview and will not commit another client's scrub; `releasing` answers 409; a second client is refused by name; an update back to the committed text restores and stays open; the wasm loop applies each scrub line in stdout order; the JS loop reads the design after the build and sends the text on stdin; `with_design_text` puts the thread-local back on unwind, and a normal build never sets it. Scope is enforced (`scope !== 'write'` is 403). One preview runs at a time, so one compiler is spawned at a time. Each batch carries `base` and `design`; a host that does not have `base` goes stale and fetches `/__dev/design`, which is the scrub snapshot while a preview is up.

The tests do not cover the queued `opened` event, a slot copied from a slot or a derive, or a logic rebuild during a scrub. `uses_names_every_site` puts the compared `"brand"` on a `width` row, which the colour-codec filter already drops. The hand-edit-then-release case (`tamber-scrub.test.mjs:277`) expects a digest conflict; if the 100 ms watcher ends the scrub first, release is 409 and that assertion fails. Either way the file is not written by the release.
