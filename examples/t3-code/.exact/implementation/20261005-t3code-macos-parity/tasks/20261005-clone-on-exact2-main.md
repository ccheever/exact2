---
name: 20261005-clone-on-exact2-main
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: main
branch: feat(example)/t3-code
pr_url: https://github.com/ccheever/exact2/pull/99
verified_commit: null
---

# Clone builds, runs and is verifiable on current exact2 main

## Outcome

The newest clone (the round-12 tree that `20261005-round12-wrapup` finishes) is in git on the
integration branch `daehyeon/t3-code`, based on a pinned exact2 `main` revision. It passes the
clone's checks and the repository's five checks, launches as a bundled macOS app, pairs to an
isolated fixture backend, and runs the core conversation workflow under the agent driver. The
lane verification tools run from this worktree. The latest user direction supersedes the
repeated pixel-matrix requirement: use compilation and core behavior checks to assess the
import, and record unverified UI cases without continuing pixel-fidelity fix loops.

## Scope and exclusions

Included:

1. **Preserve the old state.** Keep the current local commit `a9f9e58ec` (Oct 3 snapshot,
   `STATUS.md`, `EXACT2-GAPS.md`, and the framework edit `js/src/parking.rs`) on a local branch
   `daehyeon/t3-code-oct3`. Keep the mc-orch worktree untouched as evidence.
2. **Initialize the integration branch** at a pinned `origin/main` SHA (record it) plus the
   root workspace registration only: the Apple crate as a workspace member outside
   `default-members`, its `Cargo.lock` lines, and the `.gitignore` lines for
   `examples/t3-code/app.contract.d.ts` and generated assets. Push `daehyeon/t3-code`
   (user decision 2026-10-05). Moving the local branch head away from `a9f9e58ec` is a branch
   reset: confirm with the user at `prepare` even though the push was approved.
3. **Import the tree** from the mc-orch worktree with
   `rsync -a --no-links --exclude UI-PARITY-TODO.tmp.md --exclude app.contract.d.ts` after
   checking for symlinks. Do not carry `js/src/parking.rs` or any other framework file.
   Then restore `EXACT2-GAPS.md` and `STATUS.md` from commit `a9f9e58ec`
   (`git show a9f9e58ec:examples/t3-code/EXACT2-GAPS.md`, same for `STATUS.md`): the
   mc-orch tree does not have them, and the plan's issue drafts cite `EXACT2-GAPS.md`. Also
   commit this plan directory (`.exact/implementation/20261005-t3code-macos-parity/`) so the
   records reach remote contributors.
4. **Compile fixes required by main** (from `EXACT2-GAPS.md` "Rebase notes"): LLP 1091 `use`
   lines for names reached through another file (`markdown.contract` uses `Icon`;
   `class=Control` without `use`); layout follow-ups of the block `<button>` change
   (`e8bc9c846`); textarea `submit` moved to `key=` + `preventDefault()` if main refuses it;
   `role="alertdialog"` columns only if main refuses them (the note says it does not).
5. **Input-semantics re-check.** Main changed modifier-only keydown, textarea Enter, focus
   order, `elementEnded`, hit testing and popover placement. Run the AppKit binaries
   `r8-keys`, `r9-input`, `r10-connect`, `composer`, `snapshot` and fix the clone where a
   result changed. Watch the SnapShot `shift+shift` chord against modifier-only keydown.
6. **Parked native call (X14).** Reproduce, without the framework edit, the reply that a
   let-go answer loses (`EXACT2-GAPS.md` X14), and write the result into
   [issue X14](../issues/closed/20261005-x14-parked-native-reply.md): the steps and output, or "not
   reproduced on the pin". Keep the app-side `T3ReadGate.swift` workaround either way; it goes
   only when X14 is resolved upstream and adopted (`issue-close`) or closed by a user decision.
   The plan files no framework PR. The framework change (`js/src/parking.rs` and
   `js/src/lib.rs` in commit `a9f9e58ec`) stays out of the app branch; it is context in X14,
   which may offer it as a proposed patch. If the reply is not lost on the pin, say so in X14
   and ask the user whether to close the issue.
7. **Tools in this worktree.** Copy the lane tools from mc-orch
   `target/t3-ui-parity/` (`lane-build.mjs`, `lane-backend.sh`, `drive.mjs`, compose/cmp,
   `swift-all.sh`, real-input tools under `lanes/r8-integrate/tools` and
   `lanes/r9-input/tools`) and the oracle runtime `target/t3-ref/runtime-f870c41` into this
   worktree's own `target/` (user decision 2026-10-05 #1; never symlink). Retarget paths,
   lanes and ports (16000–16999; never 3773). Every lane server sets
   `T3CODE_TELEMETRY_ENABLED=false` (telemetry is excluded, issue X39; the shipped server sends
   PostHog events by default) — `lane-backend.sh` exports it. Copy the pinned Hermes tools the same way after
   confirming main still pins the same Hermes revision.
   Decision U23 (2026-10-05): commit the verification tools under
   `examples/t3-code/tools/` with the same relative paths, so a fresh clone can run every
   ticket's acceptance commands. Before committing, remove absolute user paths and anything
   secret from them; built outputs (lane `.app` copies, evidence, the oracle runtime) stay in
   `target/`.
8. **Docs.** Update `README.md`, `AGENT-HANDOFF.md` and `STATUS.md` to the new location, the
   new base SHA, and the check counts. Keep `EXACT2-GAPS.md` content; only correct items that
   this ticket proves changed.

Excluded: removing workarounds that main made unnecessary (`20261005-main-fix-adoption`),
any new feature, any framework edit.

## Context and guidance

Parent specification: [spec](../spec.md). Research: [research](../research.md).
Line numbers in this plan are from the mc-orch tree on 2026-10-05; later tickets move code, so find it by symbol.
Source behavior: the clone's own `README.md`, `AGENT-HANDOFF.md` (round 11), and the
committed `STATUS.md` and `EXACT2-GAPS.md` ("Already fixed on exact2 main", "Rebase notes").
Reference pin for the fixture backend: T3 Code `f870c419fc` runtime (`runtime-f870c41`); the
feature tickets move the pin to `1e2ecbd975` or later.
Library revision: `20261005-platforms-v3`. Selected topics: foundations (build commands,
diagnostics are compiler output only), platforms (macOS build before driving; resize,
keyboard/focus, pointer/menu, storage/relaunch), testing-and-debugging (`--json` agent runs;
static evidence is not runtime evidence; record source/build identity), capabilities
(unlisted native integrations are unknown in the library).
Consumer framework revision: branch base `c1522fdac` before this ticket; the new pin is
chosen at `prepare` (origin/main was `9d442ecba` on 2026-10-05, 1354 commits ahead).
Read `CLAUDE.md`, `rules/RULES.md` and `rules/DEFERRED.md` before editing (repository
instructions). Pinned Bun is at `~/.bun-1.4.2/bin` on this Mac; `bun install
--frozen-lockfile` is needed in a fresh worktree.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| recorded decision | User decision 2026-10-05 #1 (move to t3-code, copy tools to `target/`) and evening decision (integration branch) | none | Recorded | `spec.md` Confirmed requirements |
| task (local, no PR) | [20261005-round12-wrapup](closed/20261005-round12-wrapup.md) | none (untracked tree) | Implemented; unrun UI checks explicitly recorded under the latest user direction | round-12 continuation record |
| recorded decision | Branch reset of local `daehyeon/t3-code` from `a9f9e58ec` to the main pin | none | User confirms at `prepare` | pending |

## Issue assessment at preparation

Checked sources and time: {{at prepare}}.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| X14 parked native reply (`EXACT2-GAPS.md`) | Data-module native call survives a let-go answer, macOS | Clone docs, checked against main `d2cb661eb` | unknown until reproduced | Reproduce on the pin (scope item 6) |

## Implementation notes

- The import is mechanical. Keep it in its own commit so reviewers can separate it from the
  compile and behavior fixes.
- Keep the clone's file header convention that names the reference source.
- `client.ts` (1455 lines) and `app.contract` (1327 lines) are near the 1,500-line cap. Do not
  grow them here.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Unit and type checks | Fresh `bun install --frozen-lockfile` | `bun test examples/t3-code`; strict `tsc` per README; `cargo run -q -p contract -- build examples/t3-code/app.contract -o <scratch>/t3.plan` | All tests pass (count ≥ the round-12 count, recorded); `tsc` clean; build exit 0 | macOS host machine | logs |
| Native tests | Xcode 27.0 | `cargo test -p t3-code-macos --lib`; every AppKit binary with the README recipe (`swift-all.sh`) | All green; counts recorded per binary | macOS 26.6.2 | `swift-all.log` |
| Repository gates | `git add -A` | The five checks from `CLAUDE.md` | All pass | macOS | logs |
| App launches and connects | Isolated fixture backend on port 16xxx (isolated HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_*, T3CODE_HOME; fixture provider) | `EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle --run`, then `bun scripts/agent.mjs macos --json tree state logs` and pair, select a thread, send, answer an approval | Paired; reply streams; approval resolves; no refused operation in `logs` | macOS, 1280×840 | `--json` transcript, screenshots |
| Input semantics | — | AppKit `r8-keys`, `r9-input`, `r10-connect`, `composer`, `snapshot` | Green; any changed expectation explained | macOS | logs |
| X14 decision | Fixture backend; the round-11 reply-ownership scenario (15 turns with concurrent draft edits, README) | Run without the framework edit | No reply is lost with `T3ReadGate.swift` in place; the result without the gate is recorded in issue X14 (reproduced, or not — then the user decides whether to close X14) | macOS | trace + X14 update |
| UI verification limit | Latest user direction | Preserve prior evidence; no repeated pixel matrix or fidelity fix loop | Unrun UI checks remain explicitly unverified and do not block import preparation | macOS | round-12 continuation record |
| File cap | — | `bun scripts/caps.mjs` after `git add -A` | Pass | — | log |
| Docs and records present | Fresh clone of the integration branch | `ls examples/t3-code/{EXACT2-GAPS.md,STATUS.md,README.md,AGENT-HANDOFF.md}` and the plan directory | All present | — | listing |
| No telemetry from lanes | One lane backend during the launch-and-connect row | `ps eww <pid>` of the lane server; `lsof -i -a -p <pid>` during the session | `T3CODE_TELEMETRY_ENABLED=false` set; no connection to the PostHog or OTLP hosts named in issue X39 | macOS | env excerpt, `lsof` log |

Task-owned source paths: `examples/t3-code/**`, root `Cargo.toml`, `Cargo.lock`,
`.gitignore` lines for the example.
Required environment: Xcode 27.0, pinned Bun 1.4.2, pinned Hermes, the reference oracle
runtime copy, no running T3 Code (Nightly) during drives.

## Progress

2026-10-07 (records sync): the import is done. Under the PR workflow of 2026-10-06 the clone lives at `examples/t3-code` on `feat(example)/t3-code`, and every task PR since (#142–#222) is based on it and merged into it. The branch-reset and import steps below are history.

Preparation started 2026-10-06; import and branch reset not executed.

- Fetched `origin/main`; proposed pin:
  `e79be39d49ea3d5d609ed8fed6aadc8c9a243be1`.
- Preserved the Oct 3 snapshot on `daehyeon/t3-code-oct3` at `a9f9e58ec`,
  and the prior session's complete committed history on
  `daehyeon/t3-code-pre-main-20261006` at `21b0b3d3c`.
- Round-12 source archive and one-pass checks are recorded in the preceding ticket.
  mc-orch source and historical evidence remain in place.
- Both bases use Hermes pin `6badada762121682b5481b6124e6c3a991ae6046`.
- Import recipe remains scope items 2–8: only app files and root workspace registration;
  retain the plan, STATUS and EXACT2-GAPS; exclude generated declarations, the temporary
  UI checklist, and all old framework changes. Audit copied tools before committing them.
- Waiting only for the explicitly required confirmation in scope item 2 before resetting
  the integration branch to the proposed pin. No push or external publication performed.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

#99 goes to `main` at the end, as the user scheduled (2026-10-07). Before it goes: drop `.exact/`, `STATUS.md`, `AGENT-HANDOFF.md` and `EXACT2-GAPS.md`; export the clone as one commit on current `main` so the evidence never enters main's history; rewrite the `EXACT2-GAPS X<n>` citations in source and README to GitHub issue numbers; build and drive there; report the async lane's `--workspace` build time before and after; move the three `docs/agent-pitfalls.md` entries to their own PR.

2026-10-08 (records sync): #99 also waits for main adoption round 7, which waits for main fix of X67 (main's examples test
overflows the compiler's 2 MiB test-thread stack on the clone; without the fix the merged branch cannot pass `cargo test`).
Round 7 brings in the `now()` → `performanceNow()` rename (main `9731c8056`) and drops `panels.contract` and
`settings-panels.contract`, which main's examples test refuses (unless the root rewrite did these first). In the cleanup,
cite #100, #117 and #276 as declined upstream: the app's own code is the design there.
