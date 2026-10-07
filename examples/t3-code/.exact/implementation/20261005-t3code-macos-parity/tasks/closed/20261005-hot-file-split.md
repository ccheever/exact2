---
name: 20261005-hot-file-split
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: 'feat(example)/t3-code-hot-file-split'
pr_url: https://github.com/ccheever/exact2/pull/147
verified_commit: null
---

# Room for parallel features in the shared app files

## Outcome

Feature PRs can add resources, actions, RPC operations and native ops without editing the
same lines of the shared files and without reaching the 1,500-line cap. The app behaves
exactly as before: every test, AppKit binary and matrix cell is unchanged.

## Scope and exclusions

Included:

1. `client.ts` (1,455 lines): move the `client.command()` operation groups into
   `client-ops-<area>.ts` files (connection, threads, composer, vcs, pull requests,
   providers, settings, devices). Target: `client.ts` ≤ 1,250 lines. Each new feature
   adds its operations to its area file or a new one.
2. `app.contract` (1,327 lines): move view sections that still sit in the root file into
   child Contract files (`use` lines per LLP 1091 naming), keeping all resources, root
   state and root actions in the root (exact2 requires root-owned resources; issue X9).
   Target: ≤ 1,150 lines, so about 20 tickets can each add a few resources.
3. `modules/apple/T3Module.swift` op routing and `T3Transport.swift` stream registration:
   one registration point per area so features add ops in their own Swift files.
4. `keyboard-dispatch.ts`: a table-driven command map so tickets add commands without
   editing shared branches.
5. Record the per-area seams in `AGENT-HANDOFF.md` ("Where a feature adds its code").

Excluded: any behavior change; new features; framework edits.

## Context and guidance

Parent specification: [spec](../../spec.md). Why: the round-11 handoff recorded `client.ts`
1455 and `app.contract` 1327 lines against the 1,500-line cap (`CLAUDE.md`), and the draft
plan planned the same split as its first integration step. With one PR per ticket into
`daehyeon/t3-code`, parallel PRs otherwise conflict on these files.
Library revision: `20261005-platforms-v3`. Selected topics: components (props have no
defaults; resources, mutations and scheduled tasks belong at the root; child state lives
with its instance), foundations (local imports start `./`; `contract build --json` succeeds
with `[]`; `contract fmt --check`).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](../20261005-clone-on-exact2-main.md) | pending | Merged | pending |

## Issue assessment at preparation

Checked sources and time: {{at prepare}}.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X9](../../issues/20261005-x09-root-component-across-files.md) | Resources in child components / root across files | `EXACT2-GAPS.md` X9 | nonblocking (this ticket is the workaround) | `issue-open` when convenient |

## Implementation notes

Move code without editing it. One commit per moved area, so `git diff --color-moved` shows
pure moves.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| No behavior change | Same fixture as the clone-on-main matrix | Clone checks; all AppKit binaries; matrix re-shot | Same test counts; every matrix cell within 0.02 of the previous run | macOS | logs, matrix |
| Line budget | — | `wc -l` and `bun scripts/caps.mjs` | `client.ts` ≤ 1,250; `app.contract` ≤ 1,150; caps pass | — | log |
| Moves only | — | `git diff --color-moved=dimmed-zebra` review | No changed logic lines except imports/`use` lines | — | review note |
| Repository gates | `git add -A` | The five checks | Pass | macOS | logs |

Task-owned source paths: `examples/t3-code/{client.ts,client-ops-*.ts,app.contract,*.contract,keyboard-dispatch.ts,modules/apple/T3Module.swift,modules/apple/T3Transport.swift,AGENT-HANDOFF.md}`.

## Progress

Implemented 2026-10-06 on `feat(example)/t3-code-hot-file-split` (from `feat(example)/t3-code`
at d78ac86ff), one commit per moved area; verification is unverified until review.

- `client.ts` 1462 → 867 lines. `command()` keeps its load, the two group runs and the
  reporting; its branches moved, as written, into `client-ops-{connection,snapshot,settings,
  composer,threads,sidebar,diff,lanes}.ts` (read and write groups) with the 18 private
  methods only those branches called. `client-ops.ts` lists the groups (`READ_OPS`,
  `WRITE_OPS`); a group hands its message and any rewritten id/value back through one record
  (setting-snapshot's setup rewrites them; the catch reads them back, as before).
  `client-shared.ts` holds the three helpers both sides use. Members the area files reach
  lost TypeScript's `private` (type-only). Not moved: the `local` and `formCommand` op lists
  in `command()`.
- `keyboard-dispatch.ts`: the main-window commands are row functions in `MAIN_ROWS`
  (button order kept).
- `app.contract` 1366 → 1357 lines, **target ≤ 1,150 not met**. Its view and shapes are the
  only parts the rules let move: the view already lives in `app-main`, `app-settings` and
  `app-overlays`; the four shapes moved to `app-shapes.contract`. The rest (151 state, 156
  actions, 42 resources, 20 derives, 7 mutations, 6 tasks) is root-owned (X9). Reaching
  1,150 needs X9 (resources in children or a root across files) or moving UI-only state
  and actions into children, which changes the plan and state lifetimes.
- `T3Module.swift` 172 → 122: `later()` keeps the read gate and the fleet, then tries
  `T3Module.areas` (`T3Module+{Connection,Files,Timeline,Devices,Sidebar,Snapshot,Composer,
  Window}.swift`), then the transport. The README test recipe leaves `T3Module*.swift` out of
  the composer, menus and r5-panels tests.
- `T3Transport.swift` 898 → 812: `perform()`'s default case tries `T3Transport.areas`; the
  saved-environment ops and `pairEnvironment` moved to `T3Transport+Environments.swift`.
- `AGENT-HANDOFF.md`: "Where a feature adds its code".

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-06) | a411dba3c..6bcc668e8 on d78ac86ff | Before → after: `bun test examples/t3-code` 1200 pass / 0 fail (117 files) → same; strict tsc clean → clean; `contract build`: 2158 slots, 20 derives, 42 resources, 1984 actions, 48014 nodes, 9590345 bytes → same, and `cmp before.plan after.plan` identical (with the `app-shapes.contract` use line last instead of first, 51,430 bytes differ at equal counts: the plan orders shapes by first use); `cargo test -p t3-code-macos --lib` 10 → 10; AppKit/XCTest binaries (README recipe) 27 run, all exit 0, per-binary counts equal before and after (e.g. composer 45, transport 36, menus 10, r5-panels 8, r8-keys 4, r10-connect 5, snapshot 9 check groups); mermaid skipped (needs a T3 server), timeline-keyboard not run (separate recipe). macOS bundle builds (`build.mjs t3-code-macos`, cargo 91.5 s). Five checks: build 0, test 0 (2927 passed), clippy 0, fmt 0, caps 0, boot 0. `contract fmt --check app.contract` fails as on the base (same 609 lines, view formatting); `app-shapes.contract` passes. `git diff --color-moved=zebra --color-moved-ws=allow-indentation-change` over the client split: 1082 lines moved; the rest are group wrappers, first-branch `if`, branch joins, method headers, `.call(this, …)` call sites and imports | Live drive (one agent call, isolated server on 16090): welcome window screenshot, then the pairing link typed and Pair pressed: `resources.data.connected = true`, `connectionsPage.state = "connected"`, `welcome.ready = true`, `modalError = ""` | `app.contract` ≤ 1,150 needs X9 |

## Next action

Review the PR (moves only); feature tickets rebase onto it and add to the seams in `AGENT-HANDOFF.md`.
