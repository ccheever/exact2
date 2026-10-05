---
name: 20261005-reference-logic-test-ports
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Reference logic tests pass against the done areas

## Outcome

Every reference test file that `20261005-reference-logic-tests-done-areas` classifies as
`port` for an already-done area runs against the clone's module with its original test
names, and passes; every `swift` row's cases run in the named AppKit test binary and pass. Where a ported test fails, the clone's logic is changed to the reference
behavior (the user's rule: use T3 Code's logic; change only what exact2 requires, recorded in
the file header). This proves that the done areas use the same logic as T3 Code.

**Planned split.** This ticket is one row in the plan so that the scope is tracked now. At its
`prepare`, split it into one ticket per area from the map (for example connection, sidebar,
conversation, composer, version control, right panel, pages, settings, desktop shell). Each
area ticket is a separate PR from the updated integration branch (no stacks). Keep this
name for the first area and use `20261005-reference-logic-test-ports-<area>` for the rest.

## Scope and exclusions

Included: the `port` and `swift` rows of the map for done areas (the `swift` cases go into the AppKit binary that the row names, with the README recipe); logic fixes that a failing ported test
shows; the map's status column updated to `ported` with the test file path.

Excluded: rows the map classifies `n/a-ui`, `n/a-server`, `n/a-excluded`, `n/a-electron`, or
`later-ticket` (those belong to the feature tickets named in the map); new features.

## Context and guidance

Parent specification: [spec](../spec.md) (Goal and logic reuse). Map: `REFERENCE-TESTS.md`
from [20261005-reference-logic-tests-done-areas](20261005-reference-logic-tests-done-areas.md).
Conversion rules (from the clone's practice): `bun:test`; a `now` argument instead of fake
timers (exact2 data sources have no timers, issue X19); the clone's fake native harness
instead of `vi.mock`; original test names. Keep the whole `bun test` run under 60 s.
Library revision: `20261005-platforms-v3`. Selected topics: foundations (data-module
boundary), state-and-data (snapshot semantics), testing-and-debugging (static tests are not
runtime evidence; a pass is meaningful only if the assertions are).
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code,
so find it by symbol.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-reference-logic-tests-done-areas](20261005-reference-logic-tests-done-areas.md) | pending | Merged (the map exists) | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged | pending |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |

## Issue assessment at preparation

Checked sources and time: {{at prepare}}.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X19](../issues/20261005-x19-data-source-timers.md) | Timers in data sources | `EXACT2-GAPS.md` X19 | nonblocking (workaround: `now` argument) | record in each converted file header |

## Implementation notes

Port one reference test file per commit, with its source path and commit in the header.
A logic fix goes in its own commit after the failing test, so the reviewer sees the
divergence. A fix that changes what the user sees also gets an agent drive and an oracle
pixel pair for the affected screen.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Every `port` row of the area passes | Fresh `bun install --frozen-lockfile` | `bun test examples/macos/t3-code` | All ported tests pass; names match the reference titles (title comparison) | macOS host | log + map diff |
| Every `swift` row of the area passes | Xcode 27.0 | The named AppKit binaries (README recipe) | The ported cases pass with the reference case names | macOS 26.6.2 | binary logs |
| User-visible fixes proven | Lane fixture backend | Agent drive of each screen a fix changed; oracle pair | Matches the oracle | macOS 1280×840 | transcript, shot pair |
| Divergences fixed, not hidden | — | Review each logic-fix commit | Each fix makes the clone match the reference; no ported assertion was weakened | — | review note |
| No regression | Lane fixture backend | Clone checks, AppKit binaries, matrix | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs, matrix |
| Repository gates | `git add -A` | `bun scripts/caps.mjs`; the five checks | Pass | macOS | logs |

Task-owned source paths: the ported `*.test.ts` files, the clone modules they fix,
`REFERENCE-TESTS.md`.

## Progress

Planned.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

After the map merges: `prepare` splits this ticket per area (plan revision), then
`implement` the first area.
