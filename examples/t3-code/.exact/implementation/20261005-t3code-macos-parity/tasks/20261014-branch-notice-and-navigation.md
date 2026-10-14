---
name: 20261014-branch-notice-and-navigation
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-branch-notice-and-navigation
pr_url: null
verified_commit: null
---

# Branch notice and navigation

## Outcome

BR-SWITCH: show the reference's branch-change notice before continuing a local-checkout
thread on a different branch, and restore its prior branch with confirmation for a dirty
checkout. STF-2: a clone toast's Open project leaves Settings and opens the project's draft.
G4-11: relaunch opens Home's most recently active project draft instead of the last thread.

## Scope and exclusions

Application changes only. No framework changes, provider messages, publication, merge,
or global OS input. The isolated verification server uses port 16842 and this worktree's
`target/branch-navigation/server`; it does not use port 3773 or the user's `.t3`.
The implementation does not claim full pixel or real-input parity.

## Context and guidance

Parent [specification](../spec.md). Prepared scope is the `branch-notice-and-navigation`
entry of the first clone fix wave, authorized by the user's continuation on 2026-10-14.
The initial worktree was clean at `609f392d52a5c3f65cda2f50e5e5824a01a4184e`.

Reference checkout verified at `1e2ecbd9758830669684b494d4398f626b0576e0`:

- `apps/web/src/components/BranchToolbar.logic.ts:257`: local checkout mismatch.
- `apps/web/src/components/ChatView.logic.ts:922`: key, content gate and session dismissal.
- `apps/web/src/components/ChatView.tsx:6990`: reveal state and restore operation;
  `7357`: dirty checkout gate; `7408`: notice; `11391`: confirmation wording.
- `apps/desktop/src/window/DesktopWindow.ts:370,707` and
  `apps/desktop/src/electron/ElectronProtocol.ts:27`: each desktop launch loads `/`.
- `apps/web/src/routes/_chat.index.tsx:36`: replace that route with the most recently active
  project's draft. This confirms G4-11 before changing persistence.

Library revision `20261005-platforms-v3`: foundations, state/data, layout/interaction,
platforms, testing/debugging. Actual Contract syntax and behavior are checked against this
checkout. macOS AppKit desktop is the required target. TypeScript type-system guidance
and concise writing guidance were applied.

## Dependencies

- Recorded decision: user continuation authorizes this bounded first-wave implementation.
- Existing composer notice stack, workspace status stream, generic AlertDialog, fleet project
  navigation, and Home project ordering are present at the base commit.
- No new framework dependency. Root Cargo checks and independent integration review belong
  to the coordinating lane.

## Implementation notes

- Ported `resolveLocalCheckoutBranchMismatch` into `composer-controls-branch.ts`. The notice
  requires a server thread in the local checkout, two different known branches, and draft
  content. Once revealed it remains through clearing the draft; resolution or leaving the
  thread resets revelation. Dismissal is scoped to the environment/thread/branch pair for
  the client session. The status stream stays active with the workspace card closed.
- Restore sends `vcs.switchRef`. As in the reference, it dispatches `thread.metadata.update`
  only if the server returns a different normalized ref name. A dirty checkout first opens
  "Switch to <branch>?" with Cancel and Switch branch. Cancel sends no Git write. A confirmed
  action is tied to the exact mismatch so a changed checkout cannot inherit consent.
- Restore captures environment, generation, origin, project and thread across awaits. A late
  answer cannot issue metadata for the prior thread on a newly focused environment.
- Repository and streamed Git-status caches are scoped to environment, connection generation and
  origin. A second server with the same repository path cannot inherit a clean status or restore
  consent; restore remains unavailable until its own status arrives.
- The existing error toasts distinguish failure to switch from failure to update metadata
  after switching. Successful restore focuses the composer.
- Toast navigation clears Settings, its theme dialog and a utility page before sending the
  existing thread/project command. The fleet route already handles cross-environment drafts.
- Disk loading no longer restores session selections. Saved draft text, attachments and
  uncertain operation identity remain intact. Live environment switching still uses the
  in-memory selections.
- The generic confirmation component now accepts an explicit action label; existing callers
  keep Confirm, and branch restore uses Switch branch.

## Acceptance and reproduction

Use Bun 1.4.2 and the checkout's pinned Rust toolchain.

```sh
bun test examples/t3-code/composer-controls.test.ts examples/t3-code/client.test.ts
bun node_modules/typescript/bin/tsc --noEmit --strict --target ES2023 --module ESNext --moduleResolution bundler --skipLibCheck --lib ES2023,DOM examples/t3-code/app.ts
cargo run -q -p contract -- build examples/t3-code/app.contract -o target/branch-navigation.plan
EXACT_APP_DIR="$PWD/examples/t3-code" bun host/apple/build.mjs t3-code-macos --bundle
```

Runtime recipe: create an isolated Git project with `main` and `feature`, and an empty server
thread whose metadata branch is `feature`. Open it while checkout is `main`, enter draft text,
and restore. Repeat with uncommitted changes: Cancel must leave `main`; Switch branch must
checkout `feature` and retain the changes. Trigger an isolated tracked clone while Settings
is open, press Open project, then relaunch on the same agent store. Expect Settings closed
and a draft on both project navigation and relaunch. Do not send the draft to a provider.

## Progress

Implementation is ready for verification after the final native build and bounded agent
capture. Independent review and real-input cases remain with the parent workflow.

## Attempts and evidence

- macOS 26.6.2 (25G83), Apple Silicon; Bun 1.4.2; Rust 1.97.0.
- Focused Bun tests: 98 passed, 0 failed, 451 assertions. These cover local/worktree/unknown
  mismatch cases, intent gating, reveal reset, dismissal, clean restore, dirty confirmation,
  stale confirmation, normalized metadata, two failure toasts, environment changes during
  switch/ID calls, and Home draft startup with saved drafts intact.
- Related composer/Git/clone tests: 55 passed, 0 failed, 240 assertions across the matching
  three files. This includes existing Studio environment project navigation behavior.
- Full application Bun suite: 4,868 passed, 1 existing grammar-generation skip, 0 failures,
  117,829 assertions across 328 files. A historical relaunch-selection expectation was updated
  to the now-confirmed Home startup rule; saved draft content remains asserted.
- Caps and staged diff whitespace checks: passed.
- Strict application TypeScript: passed. An exploratory command including test files failed
  because this checkout has no Bun test type declarations and has existing fixture typing
  errors; the repository's documented strict `app.ts` command passed.
- Contract build: passed, producing a 32 MB plan, with existing authored-colour warnings.
  The first attempt found unsupported default-prop syntax; explicit labels fixed it.
- A native baseline attempt was invalidated by concurrent source editing during capture.
  A later bake was stopped at its recorded PID to add the review's stale-context guard.
  A test-only relaunch expectation update also invalidated a capture because the baker includes
  test files. Final sources and tests are now frozen. No native success is claimed from these
  interrupted captures. Final native result is recorded below.
- The native build of `f12352a80` passed. Independent review then reproduced a same-path
  cross-environment Git-status leak; the cache scope and pending consent were corrected.
  Focused composer/client/shell/diff tests now pass 123 tests and 584 assertions, including
  changed-environment, generation, origin, and reconnect-consent regressions. Strict application
  TypeScript and the full suite pass after this correction. A final native rebuild follows.
- Local raw outputs are under `target/branch-navigation/evidence` and task-named temporary
  build/test logs. Scratch scripts and credentials are not tracked.

## Next action

Run independent verification against the implementation commit and complete the bounded
native/real-input acceptance that is not covered by unit tests. Do not close this ticket or
claim the parent plan complete from these implementation checks.
