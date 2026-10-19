# Code review: LLP 1122 T4 step 1, the Hold control, round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `335242300`.
- **Method:** one brief (sha256 `51bd3a4ba32cf55cfba001b79611f2a1592c0268e349aab9374887dd752d5ee5`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high; smaller points).
- **Disposition:** taken in `ab3c809bed` (the reviewed `52c780fd1`, `335242300` are `1f9f9766a`, `493b45ac2` after a rebase): a build's `held` report updates its hold (no listing needed) and confirms only the hold it answers, since a changed hold drops the listing's earlier report; a let-go with no reason keeps the last word and Let go from here says 'Let go.'; the notes are cleared with a new server run; the held look's rows show only while the hold on this copy is held, each at its part's place (a held row's source is a place), and the page is asked once a hold to report the selection again; a hold's end is said even when the selection is stale (only the controls go); a held hold on another node says Held; the press's node is named when the build names it (stage 9 E's `held.site`), which until E lands it cannot. The test drives the wire's own flow (a `held` event alone, the join's held rows, Let go). `335242300`: confirmed a straight move.

Two correctness bugs block this step. The helper move in `335242300` does not: `columnsOf`, `branchOf`, and `inputsOf` were copied into `branches.ts` with the same bodies, and `app.ts` re-exports them.

## Blocking

### 1. "Held" does not follow the `held` report, and a new hold is confirmed by the previous one

`app.ts:1069` handles `held` only when `state` is missing. A confirmation (`{event:'held', state:'active'}`) does not touch the build. `holds.ts:42` prints "Held" whenever the listing's `held.state` is non-null, and it does not check that it is the same state as `hold.state`.

The server does not announce `builds` when a host confirms. It stores `b.held` and emits `held`. It also leaves that old `held` in place when a new `hold` is sent (`scripts/tamber-builds.mjs` around the `held` report and `hold()`). The stub does the opposite: `tamber-stub.ts:126` clears `held` and emits `builds`, which is the only path the test drives.

- Select site 3, press Pressed. The `builds` listing has `hold.state: "active"` and `held: null`, so the panel says "Holding pressed…". The host then reports `held` with `state: "active"`. The panel stays on "Holding" until some later `build-status` happens to carry `held`.
- That later listing has arrived, so the panel says "Held pressed". Press Hovered. The new `builds` event has `hold.state: "hover"` and the previous `held: {state:'active'}`. `holds.ts:42` treats any `held.state` as confirmation, so the panel says "Held hovered" before the host has applied hover.
- Another client's hold uses the same listing, so it has the same lag and the same false confirmation.

The test at `app.test.contract:1197` injects a `builds` listing that already contains `held: {state:'active'}`, so it never sees a `held` event change the word.

### 2. Held rows are drawn as base rows, and the test feeds that shape

On the wire, `held.rows[].source` is a place, not `authored` / `inherited`. The join writes the `:active` part's `at`, or the node's source, and sets `state` (`scripts/tamber-inspect.mjs` join; the inspect test expects `source.line`). Those rows have no `origin`, `token`, or `editable`. The web host also sets `held.provenance: "incomplete"` when it has computed values but cannot name a token (`host/web/input-glue.js` `heldOf`).

`inspector.ts:66-70` runs those rows through the base-row mapper. An object `source` is indexed in `SOURCES` and then `String`'d, so the declaring part's file and span show as `[object Object]`. A null place shows as `null`. The token control reads `editable.token`, which this join never sets. `inspector.ts:81` never reads `held.provenance`.

The fixture at `app.test.contract:1197` is a base row: `source: "authored"`, `origin: "class:PostButton"`, `editable.token: "accent.fill"`. The assertions ("Open accent.fill", no `held-note`) pass on that fixture and would fail on a selection the server actually publishes.

Pressing the button also does not refresh the open selection. `sendHold` (`app.ts:1344`) only sends `hold`. The host reports `held` and does not send `selected` again, so the copy already on screen never gains `held.rows` until the next click. Once a listing does carry `held.state`, `inspector.ts:81` (`holding.held?.state && !e.held?.rows`) says "Provenance incomplete" for that stale selection. The same note appears for a different node of the build, whose selection omits `held` because the hold is elsewhere (`heldOf` only answers for the asked node or the resolved press). An empty `rows: []` does not count as missing, because an array is truthy.

### 3. Why a hold ended is a sticky note, and a reasonless echo wipes the server's clearing

`app.ts:1069` stores one sentence per build, and only `sendHold` deletes it (`app.ts:1349`). The listing's `held.reason` is never read.

- New plan, no single corresponding site. The server emits `hold-cleared` with reason `no single site corresponds in the build's new plan`, then `hold: null`. The panel can say that. `sendHold(null)` also frames the host, which answers `held` with `state: null` and no reason (`host/web/dev.js` clear path). That event matches `!e.state` and replaces the note with `The hold was let go: .`
- Let go from this tool does the same: the only sentence is `The hold was let go: .` `reasonText` has nothing for an empty reason.
- Restart: `app.ts:1032` drops builds and the selection on a new epoch and leaves `holdNotes`. Holds do not survive a restart, but build ids do (`web-1`). The next selection on that id, with `hold: null`, shows the previous run's sentence (for example the old "real input" line).
- A missed-event resync has the same hole. Hello's `held: {state:null, reason:'input'}` is stored and ignored, so the panel keeps the pre-gap sentence or says nothing.
- Another client disconnects. The server clears the hold in the listing and does not emit `client`. The old note stays, and when `hold` becomes null it is shown again as the reason this hold ended.

## Smaller

The press line does not meet D8's "name the node the press resolves to" or review2's "say the ancestor when the press target is an ancestor." `holds.ts:42` always appends the same hedge, including when this node is the press target. The listing's `hold` is the site the tool asked for. The `held` event is `{build, state, reason}` and carries no resolved site, so the tool cannot name that node.

A confirmed hold on another node is still worded "Holding …" (`holds.ts:41`). Let go stays disabled there (`hold.state` is `""` unless the hold's site and instance match this selection), so the other hold is only cleared by replacing it or by selecting that copy.

A pin or any other `buildKey` change stales the selection, and `inspector.ts:76` plus the `stale == ""` guard hide the whole hold block while the host keeps the hold. That is safe after a new plan (the old site must not be reused). It also hides Let go after a pin.

`335242300` is a straight move. `whenText` in `branches.ts` matches the one still in `app.ts`. Call sites go through the re-exports. Nothing in that commit blocks.
