# Code review: Tamber 1.5, the inspect event (LLP 1122 T2b), round 1, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `e2fc330f6`.
- **Method:** one brief (sha256 `5c635b545c50487e7f34aa393683e5ff377f277b9ceeb1aa498ccf96d4d0daac`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 2 medium, 1 low).
- **Disposition:** taken.
  - Server:
    - a page's exit from a command since replaced publishes nothing (no `selected: null`);
    - a `selected` report from a build no one inspects any more is refused (409) and not published, so a click on its way when Stop or a lapse ends inspect mode cannot bring a selection back;
    - each build record names its owner (`inspector`).
  - Tool:
    - whether it inspects a build comes from the records (hello's, `builds`, `build-status`) and the `inspect` events, never from a command's reply, which a newer event may follow;
    - `selected: null` no longer drops ownership (the event does);
    - a selection goes stale when its build is turned off with no one taking it in the same answer, whoever had it, and at once when this tool loses it.
  - Tests:
    - tamber-builds.test (records name their owner, off everywhere including an away build, a refresh says nothing);
    - tamber-inspect.test (a selection while not inspecting refused; a replaced command's exit publishes nothing);
    - the contract tests follow the events (the stub now publishes them).

---

The publish path is complete and the version bump is consistent. The tool's fold is not: a rejected page exit and a late selection both undo the ownership the new event just established, and the tests never leave the happy path.

## High

**A rejected page exit still clears the current owner's toggle.** `scripts/tamber-builds.mjs:272` drops the owner and emits `inspect` only when `r.inspect` matches the current seq ("a newer owner's stays"). `scripts/tamber-builds.mjs:277` then emits `selected: null` anyway. The tool treats every such event as local ownership ending:

```1156:1158:apps/design-tool/app.ts
      else if (e.event === 'selected' && e.selected === null) {
        inspectOn.delete(String(e.build));
        if (selectionNow && String(selectionNow.e.build) === String(e.build)) selectionNow.stale = 'its page left inspect mode';
```

Ada is inspecting `web-1` at seq 3 and hits Escape; that `selected: null` is in flight. An agent takes `web-1` (seq 4). The server keeps the agent, emits Ada off then agent on, and the agent's tool shows "Stop inspecting" with Outline enabled. The late report does not match seq 4, so no further `inspect` event is sent, but `selected: null` is. The agent's tool deletes `web-1`, the toggle falls back to "Inspect", Outline disables, and a selection made after the takeover is marked "its page left inspect mode" while the page is still in the agent's inspect mode (the takeover frame was `on: true`, and `host/web/dev.js:598` reports `selected: null` only for a local exit).

On a real exit the `inspect` event is already sent at `scripts/tamber-builds.mjs:272` before `selected`. Stop deleting `inspectOn` in the `selected: null` branch, and apply the "page left" stale text only when this client no longer has that build in `inspectOn`. Suppressing `selected` when the seq does not match would do the same on the server.

## Medium

**A selection already on the wire becomes current again after inspect mode ends.** `apps/design-tool/app.ts:1146` replaces `selectionNow` with `stale: ''`. `apps/design-tool/app.ts:1152` stales it only as the `inspect` event is applied. `scripts/tamber-builds.mjs:275` joins and publishes any non-null `selected` with no check that `inspectors` still holds the build. The page drops a click at `host/web/dev.js:486` only if the off frame arrived before `report()`.

Stop, or a presence lapse, runs `clear` (`scripts/tamber-builds.mjs:359`): the frame goes out, then `inspect {on:false}`, and the tool hides Edit. The click POST, sent while the page was still on, is handled next and publishes `selection`. The same tool shows Edit again while the toggle says "Inspect" and Outline is off. Drop a non-null `selected` when that build has no inspector, so it cannot republish a `selection`.

**Other tools keep Edit when inspect mode actually ends.** Staleness is gated on `e.client === client` at `apps/design-tool/app.ts:1152`. A lapse or an explicit off never emits `selected: null` (`host/web/dev.js:589` reports it only when `local` is set). Ada inspects, clicks a node, and both tools show that selection. Ada's presence lapses. Ada's toggle and Edit update. Bob's tool never sees `selected: null`, the off event is for Ada, and Bob's Edit buttons stay. A takeover must not do this to Bob: the page stays in inspect mode, and that batch is Ada off then the new client on (`scripts/tamber-builds.mjs:135`). Stale a selection for every client when a batch turns a build off and no later `on` in that batch names it. Keep the immediate stale for the client who lost it, including on a takeover.

## Low

**The tests stay on the path that already worked.** `scripts/tamber-builds.test.mjs:425` checks on, a repeat, a takeover, a matching page exit, and `clear`. It does not check that refresh and an explicit off publish nothing or an off, that off-everywhere includes an away build, or that replacing or evicting a build on register (`scripts/tamber-builds.mjs:223` and `:228`) publishes off while re-registering the same id publishes nothing. `apps/design-tool/app.test.contract:933` checks the toggle for a takeover and a lapse with no `selected: null` behind it, and never Outline, a missed poll, or a new epoch. Those two paths do clear `inspectOn` (`apps/design-tool/app.ts:1106` and `:1110`) and hello does not carry owners, so a repeat `inspect on` is silent (`scripts/tamber-builds.mjs:132`); the reply at `apps/design-tool/app.ts:1414` is what turns the toggle back on, and it will also put the builds back if it lands after a newer off event.

`tamber/1.4` remains only as history in `docs/tamber-protocol.md` and the RFC. `PROTOCOL`, the protocol page, and the Rust and JS hello assertions all say `tamber/1.5`. No client compares the hello string for equality with `1.4`.

The event is published from every ownership change and from nowhere else, in the right order on the happy path. It is not safe to gate on until a rejected exit and a late selection can no longer put the toggle, Outline, and Edit back.
