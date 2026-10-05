---
name: 20261005-hot-file-split
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

Parent specification: [spec](../spec.md). Why: the round-11 handoff recorded `client.ts`
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
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |

## Issue assessment at preparation

Checked sources and time: {{at prepare}}.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X9](../issues/20261005-x09-root-component-across-files.md) | Resources in child components / root across files | `EXACT2-GAPS.md` X9 | nonblocking (this ticket is the workaround) | `issue-open` when convenient |

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

Task-owned source paths: `examples/macos/t3-code/{client.ts,client-ops-*.ts,app.contract,*.contract,keyboard-dispatch.ts,modules/apple/T3Module.swift,modules/apple/T3Transport.swift,AGENT-HANDOFF.md}`.

## Progress

Planned.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after the clone-on-main PR merges. Every feature ticket waits for this PR.
