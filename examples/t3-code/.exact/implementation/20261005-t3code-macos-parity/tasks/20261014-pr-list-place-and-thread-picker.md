---
name: 20261014-pr-list-place-and-thread-picker
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: pending
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

## Next action

Build the four rows; drive before and after; open the draft PR.
