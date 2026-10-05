---
name: 20261005-clone-on-exact2-main
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

# Clone builds, runs and is verifiable on current exact2 main

## Outcome

The newest clone (the round-12 tree that `20261005-round12-wrapup` finishes) is in git on the
integration branch `daehyeon/t3-code`, based on a pinned exact2 `main` revision. It passes the
clone's checks and the repository's five checks, launches as a bundled macOS app, pairs to an
isolated fixture backend, and runs the core conversation workflow under the agent driver. The
lane verification tools run from this worktree. The round-11 pixel matrix, re-shot on the new
base, has no regression that is not fixed or declared in `EXACT2-GAPS.md` with an issue
link.

## Scope and exclusions

Included:

1. **Preserve the old state.** Keep the current local commit `a9f9e58ec` (Oct 3 snapshot,
   `STATUS.md`, `EXACT2-GAPS.md`, and the framework edit `js/src/parking.rs`) on a local branch
   `daehyeon/t3-code-oct3`. Keep the mc-orch worktree untouched as evidence.
2. **Initialize the integration branch** at a pinned `origin/main` SHA (record it) plus the
   root workspace registration only: the Apple crate as a workspace member outside
   `default-members`, its `Cargo.lock` lines, and the `.gitignore` lines for
   `examples/macos/t3-code/app.contract.d.ts` and generated assets. Push `daehyeon/t3-code`
   (user decision 2026-10-05). Moving the local branch head away from `a9f9e58ec` is a branch
   reset: confirm with the user at `prepare` even though the push was approved.
3. **Import the tree** from the mc-orch worktree with
   `rsync -a --no-links --exclude UI-PARITY-TODO.tmp.md --exclude app.contract.d.ts` after
   checking for symlinks. Do not carry `js/src/parking.rs` or any other framework file.
   Then restore `EXACT2-GAPS.md` and `STATUS.md` from commit `a9f9e58ec`
   (`git show a9f9e58ec:examples/macos/t3-code/EXACT2-GAPS.md`, same for `STATUS.md`): the
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
   [issue X14](../issues/20261005-x14-parked-native-reply.md): the steps and output, or "not
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
   `examples/macos/t3-code/tools/` with the same relative paths, so a fresh clone can run every
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
| task (local, no PR) | [20261005-round12-wrapup](20261005-round12-wrapup.md) | none (untracked tree) | Its acceptance passed in mc-orch | pending |
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
| Unit and type checks | Fresh `bun install --frozen-lockfile` | `bun test examples/macos/t3-code`; strict `tsc` per README; `cargo run -q -p contract -- build examples/macos/t3-code/app.contract -o <scratch>/t3.plan` | All tests pass (count ≥ the round-12 count, recorded); `tsc` clean; build exit 0 | macOS host machine | logs |
| Native tests | Xcode 27.0 | `cargo test -p macos-t3-code-apple --lib`; every AppKit binary with the README recipe (`swift-all.sh`) | All green; counts recorded per binary | macOS 26.6.2 | `swift-all.log` |
| Repository gates | `git add -A` | The five checks from `CLAUDE.md` | All pass | macOS | logs |
| App launches and connects | Isolated fixture backend on port 16xxx (isolated HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_*, T3CODE_HOME; fixture provider) | `EXACT_APP_DIR=$PWD/examples/macos/t3-code bun host/apple/build.mjs macos-t3-code-apple --bundle --run`, then `bun scripts/agent.mjs macos --json tree state logs` and pair, select a thread, send, answer an approval | Paired; reply streams; approval resolves; no refused operation in `logs` | macOS, 1280×840 | `--json` transcript, screenshots |
| Input semantics | — | AppKit `r8-keys`, `r9-input`, `r10-connect`, `composer`, `snapshot` | Green; any changed expectation explained | macOS | logs |
| X14 decision | Fixture backend; the round-11 reply-ownership scenario (15 turns with concurrent draft edits, README) | Run without the framework edit | No reply is lost with `T3ReadGate.swift` in place; the result without the gate is recorded in issue X14 (reproduced, or not — then the user decides whether to close X14) | macOS | trace + X14 update |
| No visual regression | Same backend fixture as round 11 | Matrix re-shot at 1280×840 and 840×620, light and dark, with the copied tools | Every cell whose score moved by more than 0.3 is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | matrix table in `AGENT-HANDOFF.md` |
| File cap | — | `bun scripts/caps.mjs` after `git add -A` | Pass | — | log |
| Docs and records present | Fresh clone of the integration branch | `ls examples/macos/t3-code/{EXACT2-GAPS.md,STATUS.md,README.md,AGENT-HANDOFF.md}` and the plan directory | All present | — | listing |
| No telemetry from lanes | One lane backend during the launch-and-connect row | `ps eww <pid>` of the lane server; `lsof -i -a -p <pid>` during the session | `T3CODE_TELEMETRY_ENABLED=false` set; no connection to the PostHog or OTLP hosts named in issue X39 | macOS | env excerpt, `lsof` log |

Task-owned source paths: `examples/macos/t3-code/**`, root `Cargo.toml`, `Cargo.lock`,
`.gitignore` lines for the example.
Required environment: Xcode 27.0, pinned Bun 1.4.2, pinned Hermes, the reference oracle
runtime copy, no running T3 Code (Nightly) during drives.

## Progress

Planned. No branch.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

After `20261005-round12-wrapup` passes: `prepare` (choose the main pin, confirm the branch
reset, push the integration branch), then `implement`.
