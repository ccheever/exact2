# Code review: a refused inspect said in the Inspector, round 1 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** one brief (sha256 `3bff3337df3ed31309c3205f2f399807ec4bd6a8965e1cf383e267959a60293e`), shared by both reviewers, each blind to the other's review; HEAD `9471bb7fa8`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (from its findings; its narration before them left out).
- **Verdict:** 1 BLOCKER, 2 SHOULD-FIX
- **Disposition:**
  Taken in the round 1 commit:
  - Astra 1, Grok 1 (BLOCKER: owning inspect was taken for inspecting; the server names a build's inspector on sending and keeps it after a refusal, so any listing erased a real refusal, and a fresh tool never showed one): ownership never clears a refusal. One clears on a selection from that build, on Inspect sent again, on re-registration (its status reset), and when another tool or none owns inspect there. Tested, including a status naming this tool's inspector, after which the line stays. Restoring the old clear-on-ownership fails the test.
  - Grok 3 (an Inspect that errored dropped the refusals it was retrying): the refusals before the send are restored on an error.
  - Astra 3, Grok 2 (a build re-registered under the same id kept an old refusal): a listing whose status is reset clears it. Tested.
  - Astra 2 (grid labels could outlive the layout in the poll's text): the label map is gone. The Inspector names a grid copy by joining the refusal with gridAt.tiles as it draws; otherwise the row's own name.
  - Astra 4 (the test relied on an earlier restart): it restarts the stub itself.
  The suite passes 67 of 67.

1. **BLOCKER** `apps/design-tool/app.ts:408` — `owned()` sets `inspectRefused` to null on every `builds` and `build-status` whose `inspector` is this client. The server sets that field when it sends `inspect` and leaves it set when the page answers `unsupported` (`scripts/tamber-builds.mjs` `inspect()` and the `unsupported` report). The next listing — a status report, a pin, a hold, a registration, or hello — drops a line that is still the page's refusal. On a fresh tool the same function runs after `buildOf` (`app.ts:400`): hello's record is copied in, then deleted in that `takeBuilds`, so a refusal kept from an earlier session while this client is still the inspector never appears. An `unsupported` event that arrives before the build is in `buildsNow` is ignored (`app.ts:1116`); the listing that later introduces the build is cleared by `owned()` when `inspector` matches.

2. **SHOULD-FIX** `apps/design-tool/app.ts:400` — A build that registers again under the same id is a new server record with `unsupported` dropped, then announced. `buildOf` keeps `was.inspectRefused` for any id already in the map. The line goes only when `owned()` also sees this client as `inspector`. A page that left inspect and reloaded, while another build is still being inspected, keeps the old line. The same expression ignores `unsupported: { what: 'inspect' }` on a build the tool already knows, so a missed `unsupported` event is not restored from hello.

3. **SHOULD-FIX** `apps/design-tool/app.ts:1430` — `sendInspect` clears every refusal before `call` returns. On `error` nothing puts them back, and a later listing will not either, because a known build keeps the null `was` value. A rejected or lost Inspect click removes the reasons that click was retrying.

The other conditions hold. A selection clears that build (`app.ts:1124`). A build that leaves the list is dropped with `takeBuilds`. At most three lines are rendered, then `and N more builds could not turn Inspect on` (`app.ts:390`, `app.contract:1683`). With inspect off, the empty text is the previous sentence (`app.contract:1689`). A grid copy is named `The ${label} copy (${host})`; anything else uses the Builds row, `${host} · ${id}` plus ` · the gallery` when that row does (`app.ts:388`, `app.contract:1806`).

`grid.ts` is right. `labels` is cleared at the start of every layout (`grid.ts:193`) and filled only for tiles in that pass that resolved to a build (`grid.ts:234`). One copy, Grid, and States all go through that pass, so a label does not survive a mode change or a new States selection.

The contract test is deterministic: insertion order is the builds-event order, and the clocks match the rest of the file. It checks the cap, a selection promoting the fourth line, and Inspect off then on. It posts listings with no `inspector` and no later `build-status`, and the stub's inspect handler does not announce `builds`, so `owned()` never runs. It stays green while finding 1 wipes the line on the real server.
grok exit 0
