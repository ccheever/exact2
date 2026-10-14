---
name: 20261014-pr-list-place-and-thread-picker
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-pr-list-place-and-thread-picker'
pr_url: ''
verified_commit: ''
---

# The Pull Requests list keeps its place, and Link to thread opens its own picker

## Outcome

The user asked on 2026-10-14 to fix everything fixable from `realinput-1011c`'s known differences and the kept
workarounds; the coordinator's triage gave this task these four rows. The user decided on 2026-10-11 that every item is
built now, matching the reference T3 Code (no further decision needed unless the reference itself is ambiguous).

| Id | Clone (the T3 tip `609f392d5`) | Reference (check it first) |
| --- | --- | --- |
| PLS-1 | Under real input every window refocus (a click back, or ⌘Tab back) empties the Pull Requests list for a moment ("No pull requests") and brings it back at its top ([PLS-1-refocus-pair.png](https://raw.githubusercontent.com/ccheever/exact2/3a72de6a243dde7070d3aef5f3a1220fcd701e2a/realinput-1011c/b12-github-lane/pairs/PLS-1-refocus-pair.png); `realinput-1011c` B12, the GitHub lane added as a second environment). Not X58: real input sends no `prefer`; cause not traced | `useLiveRefresh` reads the list again on the window's `focus`; the rows stay (`answered ?? carried`) and the reader stays where they were |
| SCHEME-REMOUNT | A live Light/Dark switch mounts every surface split by `when scheme == "dark"` again (PagesCover's pages, the chat header, the composer dock and strip, the palette, the checkout dialog and more), so the Pull Requests list jumps to its top and a field there loses its caret | One React tree for both schemes: the colours are CSS variables; a theme switch repaints in place and keeps the scroll |
| PR-PICKER | The pull request page's More › "Link to thread" opens the ⌘K palette's "Choose a thread" page (the palette's 576 pt frame, its footer of key hints) | `PullRequestThreadLinks` `display="picker"`: its own `Dialog` (`DialogPopup className="max-w-md" showCloseButton={false}`), an sr-only `DialogTitle` "Link pull request to a thread", a `Command` (`aria-label="Choose a thread"`) whose input says "Search threads or projects..." and whose list is `max-h-80 overflow-y-auto`; "No active threads found." when nothing matches; a linked thread is marked (check, "Linked") and cannot be chosen |
| R10-SELECT | The checkout dialog's field (PullRequestThreadDialog) and the device tab's rename field select their text on open through the hook `t3-select-on-open` (`R10Connect.swift`), a module workaround kept since main has `setSelectionRange` (`48755525c`) | `PullRequestThreadDialog`: in the frame after it opens, `focus()` then `select()` on the reference field; `RightPanelTabs` rename input: focused with its text selected |

## Steps

1. Reference first: the four rows in `target/t3-ref/src-1e2ecbd975` (below); the reference app on this lane (16540,
   CDP 16541) for the picker and for a theme switch with the list scrolled.
2. PLS-1: trace what re-asks or re-keys the list when the app becomes active (the transports' `applicationActive`, the
   window's `focus`, `windowReturns`), reproduce it in a Bun test from the transport's own events, and keep the rows and
   the scroll.
3. SCHEME-REMOUNT: collapse every `when scheme == "dark"` / `else` pair (and `when dark`) into one subtree whose
   colours are `light-dark()` or follow the live `scheme`; main re-applies an svg's `light-dark()` paint on an
   appearance change (`9223aad88`) and refreshes text rasters (#494). Flip the scheme with the list scrolled; if a
   glyph's paint still sticks, record a one-file repro with Chrome as the oracle for the coordinator.
4. PR-PICKER: the picker as its own dialog (palette page `pr-link-thread`, panel `picker`), reusing the palette rows'
   data and logic, with FocusGuard and DialogOpenFocus.
5. R10-SELECT: the dialog's (and the rename field's) open action calls `focus(field)` then `setSelectionRange(field, 0,
   length(text))`; the hook and its handler in `R10Connect.swift` go.
6. Tests; one agent drive of the before build (the T3 tip built in this worktree first) and one of the after build;
   before / after / reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| PLS-1 | Bun test replaying the transport's `_retryDue` on the shell streams (focused and a background environment) through the Pull Requests page: the same rows and question before and after; agent drive (list scrolled, a refocus): the same scroll node and offset | before / after text record |
| PLS-1 under a real refocus | real-input steps (click back, ⌘Tab back) | — |
| SCHEME-REMOUNT | Bun test: no scheme split left in any contract; agent drive: list scrolled, `prefer prefers-color-scheme dark`, the same scroll node and offset, glyphs repainted | before / after / reference images |
| SCHEME-REMOUNT under a real appearance switch | real-input steps | — |
| PR-PICKER | Bun test of the dialog's structure; agent drive: More › Link to thread | before / after / reference images |
| PR-PICKER under real keys and pointer | real-input steps | — |
| R10-SELECT | Bun test (the open action, no hook left); agent drive: the checkout dialog opens with its reference selected | before / after text record |

## Reference

- PLS-1: `apps/web/src/routes/_chat.pull-requests.tsx` (`capableEnvironments`: every environment whose server config
  says `pullRequests`, not gated by the shell's synchronizing state; `listData = answered ?? carried`),
  `apps/web/src/hooks/useLiveRefresh.ts`, `packages/client-runtime/src/state/shell.ts` (`foregroundResubscriptions`:
  on a wakeup the shell resubscribes on the same session from its in-memory cursor and keeps its snapshot).
- SCHEME-REMOUNT: `apps/web/src/hooks/useTheme.ts` (one tree, the `dark` class on the root), `apps/web/src/index.css`.
- PR-PICKER: `apps/web/src/components/pullRequest/PullRequestThreadLinks.tsx` (`display === "picker"`, `ThreadPicker`),
  `components/ui/dialog.tsx` (`DialogPopup`, `DialogViewport`), `components/ui/command.tsx` (`Command`,
  `CommandInput`, `CommandList`, `CommandItem`), `components/ui/dialog-styles.ts`.
- R10-SELECT: `apps/web/src/components/PullRequestThreadDialog.tsx` (the `useEffect` on `open`:
  `requestAnimationFrame(() => { focus(); select(); })`), `apps/web/src/components/RightPanelTabs.tsx` (the rename
  input).
- Earlier records: `tasks/closed/20261011-pr-list-scroll-and-sidebar-sweep.md` (PS-1, X58), `EXACT2-GAPS.md` X58,
  `tasks/20261013-r12-kept-workarounds.md`, `STATUS.md` "Known differences" (PLS-1), `realinput-1011c` B12 notes.

## Scope and exclusions

Application changes stay under `examples/t3-code`. No framework edits, provider messages, live GitHub writes, or publication are included. The parent's latest direction replaces repeated pixel matrices with bounded behavioral checks; the consolidated real-input session still owns the rows below that require real input.

## Context and guidance

Parent: [specification](../spec.md). Implementation guidance: Exact library `20261005-platforms-v3`, foundations, components, state/data, layout/interaction, accessibility, platforms, and testing/debugging. The repository's own current APIs and `docs/testing.md` govern the checks. Reference checkout was confirmed at `1e2ecbd975`; inspected its Pull Requests route, foreground shell resubscription, and `PullRequestThreadLinks` picker.

## Dependencies

- Prepared branch and task record: `dd96ae28fdcb9d4999f0b96f4fd2cbbdb237e164`, based on T3 `609f392d5`.
- Public field selection `48755525c` and live SVG appearance paint `9223aad88` are ancestors of this branch, confirmed with `git merge-base --is-ancestor`.
- No new framework capability is required.

## Implementation notes

- A background shell's `_retryDue` marker previously reached `applyShell` as a snapshot without a sequence. The failed decode removed the environment from the PR query set. Foreground retries now resubscribe from the retained cursor without discarding the shell/config. Pending retries survive superseded acknowledgements and subscribe replies, including a native subscription that already changed. The next answer retries until it adopts the returned subscription id. Each adopted key clears independently; a new generation or successful bootstrap clears old retries. Ordinary subscribe failures still request bootstrap.
- The focused connection's temporary `ready=false` now carries the last displayed PR list for that environment. Switching to another environment cannot carry those rows.
- Removed the light/dark subtree copies throughout the app. The surviving controls receive the live scheme or `light-dark()` paint, so a scheme change does not recreate them.
- Link to thread uses a separate, centered 448-point picker with the reference title, query, bounded list, linked/disabled rows, empty state and focus guards. It uses the existing data and command path, with picker-specific initial highlighting. Enter excludes disabled and negative-index rows, including a query that finds only already-linked threads.
- Checkout and device rename select through `FieldOpenSelect`: first layout calls `focus`, then `setSelectionRange`. Removed `t3-select-on-open` from the manifest and native module.
- Also removed R9's obsolete scheme-remount caret observer. A native regression showed it overwrote select-on-open when reopening the same text. The updated R9 test protects the intended selection without changing its composer, IME, or transcript behavior.
- Existing tests that required two appearance branches now require one while retaining their checks of Escape, focus return, popover dismissal and layout order.

## Attempts and evidence

Resumed locally on 2026-10-14 with the stopped session's implementation intact. Environment: Apple Silicon, macOS 26.6.2, Bun 1.4.2, Rust 1.97.0.

| Check | Result |
| --- | --- |
| `bun test ./examples/t3-code` | 4,874 passed, one existing grammar-generator skip, zero failed. |
| Strict `tsc` on `examples/t3-code/app.ts`, README command | Passed after the review repair. |
| `contract build examples/t3-code/app.contract` | Passed. The native build recompiles the final Contract. Authored-dialog/color warnings remain. |
| R9 AppKit test binary | 13 passed, including reopened select-all, composer focus/composition and transcript restoration. |
| R10 AppKit test binary | Three passed: non-Latin chords, menu equivalents and wake topics. |
| Final macOS bundle | `EXACT_APP_DIR="$PWD/examples/t3-code" bun host/apple/build.mjs` passed after both review repairs. Cargo 169.3 s; Swift 1.9 s; bundle packaged and signed. |
| Final app startup | `agent.mjs macos --storage pr-list-review-final --size 1280x840 --json "clock data" tree state logs` passed with a fresh store and local-server configuration unset. Settled at epoch 5 with no pending work or surface refusals. The welcome field received focus. No connected/provider workflow was driven. |
| Native public selection fixture | `FieldOpenSelect` selected UTF-16 range `0:4` for `한글🙂`, retained it on an appearance flip, and selected it again after reopening. |
| R9 regression before/after | With the old observer, reopening `set` then selecting all ended at caret `3:3`. After removal it stayed selected at `0:3`. The committed R9 test covers the same sequence. |
| PLS baseline replay | The four PLS tests fail against an untouched archive of `dd96ae28f`; all four pass with this implementation. |
| Review repairs | The superseded-subscribe preservation test failed before the `letGo` rethrow. A later test reproduced the native-applied subscribe losing its retry and now proves next-sync recovery, live shell updates and a later foreground cursor. Superseded acknowledgement and generation reset also pass. The real picker query plus actual Contract derives reproduced Enter choosing an already-linked row; linked-only, empty, enabled and mixed results pass after the target guard. Actual subscribe failures still request bootstrap. |
| Independent review | The final retry lifecycle and picker target repairs were reviewed with no remaining blocking finding. Reviewer ran 195 focused tests successfully. |
| `bun scripts/boot.mjs` | Passed. |
| `git add -A -- examples/t3-code` followed by `bun scripts/caps.mjs` | Passed with the new test and task record staged. |
| Root Cargo build | `cargo build --all-targets --keep-going` passed. The coordinator stopped the subsequent broad checks to run them once on the integrated tree; no full root-suite pass is claimed here. |

Local-only logs are under `/tmp/t3-pr-list-*`; temporary native fixtures and the baseline source archive are under `target/pr-list-runtime`, `target/t3-pr-list-tests`, and `target/pr-list-before`. These are diagnostics, not published proof artifacts. Final logs are `all-tests-recovery.log`, `tsc-recovery.log`, `build-recovery.log`, `app-final.jsonl`, and `caps-final.log` under that prefix. An earlier capture build correctly refused a source change; the final build above ran with stable runtime sources after both review repairs.

## Progress and remaining verification

Implementation is ready for verification. Verification remains **unverified** pending the integrated review and consolidated input session. No before/after/reference image comparison is claimed in this resume.

Still required for final parity acceptance:

1. Actual click-back and Command-Tab refocus with the PR list scrolled, checking retained rows and offset.
2. A real appearance switch on the scrolled list, including glyph repaint and field caret.
3. The dedicated picker's actual keyboard/pointer and focus-return behavior against the reference.
4. Checkout and device rename select-on-open in the complete connected workflow.

## Next action

Implementation is ready for verification. The coordinator owns review/publication, integrated checks and the consolidated real-input session. Keep this ticket active and unverified until those acceptance checks pass.
