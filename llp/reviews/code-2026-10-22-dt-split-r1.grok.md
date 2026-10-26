# Code review: the design tool's browser tests split and their waits trimmed, round 1 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree.
- **Method:** full review of HEAD `023a6999c0`, 9addb2a92e..HEAD (four commits), against the lead's rules (same 18 tests and assertions, cost only, glue lane runs the new files). The author (Claude) is not a reviewer.
- **Verdict:** no BLOCKER or SHOULD; one NIT (the unused import), taken in 66d0e2fc68.

The four commits keep the same 18 tests and the same 321 `expect()` lines, add the three new files to `glueTests()`, and only change wait cost. The settle swap and the coalesced `+N real` steps still satisfy the assertions they were written for.

### NIT

**`scripts/design-tool-scrub.test.mjs:8`** — unused `drive` import. Both tests in this file bind a local `const drive = spawnOwned(...)` so they can interleave `tree` with `look()`. Harness `drive()` is never called. Drop `drive` from the import.

---

### Split and glue (64ae21f069, c036625dbb)

Compared to `git show 9addb2a92e:scripts/design-tool.test.mjs`:

| | Parent | HEAD |
|---|---|---|
| Tests | 18 | 18 (12 + 2 + 2 + 2), same titles |
| `expect(` | 321 | 321; every expect line matches |

`edit()` and `repo()` moved into `scripts/design-tool-harness.mjs` with the same bodies. `polledBuild` lives only in the canvas file; `kindsOnTheJsLoop` only in the scrub file. Each of the four files calls `toolTurns()`. Storage names and temp prefixes are per file (`design-tool-ops-${pid}`, `exact-design-tool-canvas-`, …). `repo()` writes git config in the temp repo only. Glue workers are separate processes (`bun test --parallel=glueWorkers()`), so each file that loads the harness gets its own `claimPort` / `TOOL_DIST`.

`glueTests()` gained `design-tool-ops`, `design-tool-canvas`, and `design-tool-scrub` in the first commit. `scripts/async-inputs.test.mjs` and `scripts/coverage.test.mjs` do not name the old single file. `stage11-gate.mjs` still finds stage 11 in `design-tool-scale.test.mjs`. The stub’s dials.json check stayed in `design-tool.test.mjs`. Commit `023a6999c0` only updates the two doc comments in `apps/design-tool/README.md` and `apps/design-tool/app.test.contract`, and they name the four files.

### 3a. `clock settle` → `clock +1`, `clock data`

e4f2f149e0 settled before opening the remove dialog because settle had started running `after(1)` tasks, and `follow` (`apps/design-tool/app.contract:1043`, `after(1, advance)`) then ran while the box was open. `advance` sets `filesAt = changes.files`, which retriggers `loadDesign` (`app.contract:883, 1065`) and reloads the branch panel.

Those settles never went quiet: `follow` re-arms on every `changes` answer, so `agent.mjs`’s 16-round `nextCommitTasks` / `settleBound` loop kept firing it (`scripts/agent-inspect.mjs:740`). Each new `waitChanges` holds `POLL_MS` 25s (`scripts/tamber-server.mjs:114`) against a 20s inflight deadline per settle call (`host/web-js/agent.js:375`) — about 400s, `settled: false`, reason `"tasks"`. The read that matters is the first follow run.

`clock +1` fires that same `after(1)` when follow is due; `clock data` lands `loadDesign` and waits up to 20s on the new poll. If the rename’s poll is still in flight, follow is not due, and `+1` leaves it unfired (the jump waits on inflight only when a timer is due, `host/web-js/agent.js:434-439`). That is a thinner guarantee that the rename read has already landed.

The confirm still sees the box. Follow runs only when the app clock moves. The drive is `tap remove`, `tap confirm-remove-yes` with no clock op between them (`scripts/design-tool-ops.test.mjs:47`). A `changes` answer that arrives during those taps arms `after(1, advance)` and leaves it unfired (`clock data` also fires no timer). The second `+1` is after confirm, once the box is gone. That matches 7/7 passing here.

The comment at `scripts/design-tool-ops.test.mjs:44-46` describes the happy path (follow already ran). The cost note about settle’s 16×~25s rounds is right.

### 3b. Coalesced `+N real`

Parent T2 arithmetic holds: one `clock +2000 real` plus 8×(`+2000 real`, `clock data`) → `+18000 real`; `wait` 2×2s → `+4000 real`; 18× `+2000 real` → `+36000 real`. Scrub’s two `wait`s match.

`+N real` sets `to = from + elapsed` and loops until elapsed ≥ N (`scripts/agent.mjs:1214-1221`), so `+36000 real` is at least 36s of wall time; the jump’s up-to-20s inflight wait counts as elapsed. The presence checks use `Date.now()`:

- `closed - pinnedAt > 36_000` — pin is applied, then the 4s `wait`, then the 36s step; `pinnedAt` is when the polled build (200ms wall poll) gets the pin, before that 36s step.
- Hand-back 10–45s after close, and no hand-back while open — the page is gone when `drive()` returns; the 35s lease plus a 5s sweep still fit the 45s cap.

`states[]` is `replies.filter(r => r.resources)`. `clock data` replies have no `resources`. Canvas still has three `state` ops at indexes 0, 1, 2. Scrub’s `state` ops and indexes are the same. The removed intermediate `clock data`s were not those indexes.

A single long real step is a coarser pump than many 2s steps: each old `+2000 real` had its own 20s inflight budget; a `+36000 real` (`REAL_STEP_MS` 50) typically waits once, then jumps ~20s of app clock in the next iteration. `watchCanvas` (`after(1000)`) and `follow` still run on that jump, and the host still waits for a send’s reply until the 20s budget. The 18× lease wait already had no `clock data` between steps. The polled build’s frames keep running on wall time through the one long step. The canvas comment that a real-clock step is at least its named time is true; “held up to 20 s” is the agent’s jump/data deadline (server hold is 25s), which is the cost this change removes.
