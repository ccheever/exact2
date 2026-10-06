---
name: 20261005-thread-commands-and-keys
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-thread-commands-and-keys
pr_url: https://github.com/ccheever/exact2/pull/165
verified_commit: null
---

# Deleting a thread cleans its worktree, and the missing shortcuts work

## Outcome

Deleting a thread asks whether to delete its worktree when no other thread uses it, and removes
it with the reference's order, errors and refresh. The keyboard shortcuts that the reference
binds by default or offers as commands, and that the clone lacks or handles differently, work as
in the reference: steer the first queued message, edit the last queued message, open the host
control, cycle the host, thread previous and next, open in the last-used editor, copy the PR link
or the thread ID, toggle the right panel on a draft, and the overlay and theme chords inside
Settings.

## Scope and exclusions

**G5 Delete the worktree too?** Flow of `deleteThread` (reference):

1. Optional "Delete thread?" confirm (setting `confirmThreadDelete`; done in the clone).
2. Orphan check: the target has a non-empty `worktreePath`, no *surviving* thread uses the same
   path (in a bulk delete, threads already deleted in this run do not survive), the project is
   not a Scratch project, and the project's `worktreeOnDelete` cleanup rule is off (when it is
   on, the server cleans up and the app does not ask or remove).
3. Dialog (Cancel / Confirm, destructive): title "Delete the worktree too?", text "This thread is
   the only one linked to this worktree:" and the last path segment. The answer decides the
   worktree only; the thread is deleted either way.
4. Then in order: detach live provider sessions (only if the thread has a runtime), close
   terminals with `deleteHistory` (gap TN3; `20261005-terminal-drawer` adds the call, so leave one hook), delete the
   thread, navigate to the fallback thread, then `vcs.removeWorktree{cwd: workspaceRoot, path,
   force:true}` and `vcs.refreshStatus{cwd}`. Failure toasts (stacked): "Failed to delete
   worktree" with "Could not remove <name>. <message>"; "Worktree deleted, but Git status
   refresh failed". A cleanup failure never turns the deletion into an error.
5. An archived thread (not in the shell) deletes directly with no prompt.

**G14 and A14 keys** (defaults from `packages/shared/src/keybindings.ts:21-111`; the server's
resolved list reaches the clone in `server.getConfig`):

| Command (default) | Reference rule | Clone today |
| --- | --- | --- |
| `thread.steerQueuedMessage` (⇧⌘↩, not in a terminal) | Steers the first queued message when the provider allows it; consumes the key only then; key repeat is consumed and ignored | no handler |
| `thread.editQueuedMessage` (⌥↑, composer focus) | Not on a draft; only when the caret is at the very start (else the key moves the caret); not while editing or busy; expands the queue and edits the **last** queued message; repeat ignored | no handler |
| `composer.host` (⇧⌘H) | Opens the host ("Run on") control | no handler |
| `composer.cycleHost` (no default key; offered in Settings as "Composer: Cycle Host") | On a draft with several machines and no locked environment: moves to the next machine, stepping from a pending switch, wrapping; repeat ignored | not in `keybindingCommands` (`keybinding-settings.ts:107`) |
| `thread.previous` / `thread.next` (⇧⌘[ / ⇧⌘]) | Moves along the sidebar's order; **stops at the ends**; with no current thread: previous = last, next = first; unknown current = nothing; repeat ignored | wraps (`keyboard-dispatch.ts:128-132`) |
| `editor.openFavorite` (⌘O) | Opens the **last-used** editor if still available (stored), else the first available | first available only (`keyboard-dispatch.ts:160`, `palette-commands.ts:190`; last editor kept in memory only, `shell-details.ts:85`) |
| `thread.copyReference` (⇧⌘C) | Copies the open panel's PR link, else the thread's current PR link, else the linked PR URL; toast "PR link copied" / "Failed to copy PR link"; else the thread ID ("Thread ID copied"); nothing while the panel URL is not ready | thread ID only (`settings-rest-commands.ts:240-243`) |
| `rightPanel.toggle` (⌥⌘B) | Works on a draft too | skipped on drafts (`keyboard-dispatch.ts:164-168`) |
| Overlay chords in Settings (⌘K, ⌘P, ⇧⌘F, ⌥⌘A theme, ⌥⇧⌘A appearance, ⌘U usage) | Handled app-wide by the palette provider, so they work in Settings | Settings registers only back, forward, theme editor (`keyboard-dispatch.ts:110-116`) |

Also audit (small): Back/Forward in the reference follow browser history over all routes; the
clone keeps thread/draft entries plus Settings (`keyboard-dispatch.ts:57-69`). List the clone's
routes (Usage, Pull Requests, PR detail) and fix gaps found. Thread-jump order (⌘1–9) must equal
the oracle's sidebar order for pinned, active, working, snoozed and settled rows.

Excluded: `chat.new` picker (done: `app.contract:1081-1093`; its legacy-sidebar branch belongs to
`20261005-legacy-sidebar`); terminal and preview keys (`terminal.*`, `script.*.run` wait for the
`20261005-terminal-drawer`, `20261005-terminal-layout` and `20261005-terminal-integrations`; `preview.*` is the excluded Browser); queue-edit attachments
(`20261005-composer-fidelity`); the Delete-thread confirm itself.

## Context and guidance

Parent specification: [spec](../spec.md). Reference (T3 Code `1e2ecbd975`):
`apps/web/src/hooks/useThreadActions.ts:422-600`, `apps/web/src/worktreeCleanup.ts:11-47`,
`apps/web/src/components/Sidebar.logic.ts:487-511,867-892`, `.../ConfirmDialogHost.tsx:40-110`,
`packages/contracts/src/git.ts:170-175`, `packages/shared/src/projectSettings.ts:225-230`;
`apps/web/src/components/ChatView.tsx:7612-7830,6799-6840`, `.../chat/QueuedRunsControl.tsx:185-215`,
`apps/web/src/editorPreferences.ts:15-48`, `packages/shared/src/threadReference.ts`,
`.../CommandPalette.tsx:505-580`, `apps/web/src/routes/_chat.tsx:121-130`, commit `c5a0c78b7c`.
Library revision: `20261005-platforms-v3`. Selected topics: layout-and-interaction (keyboard focus
tests, offscreen row targeting), accessibility (keyboard access; dialog labels; focus return is
not covered by the library), state-and-data (an action reads a snapshot; await each command),
testing-and-debugging. Key chords reach the app as hidden buttons with `aria-keyshortcuts`
(`keyboard-dispatch.ts`, `settings-shortcuts.contract`); the library does not cover them or
native key monitors (**unknown in the library**; X25 and `R8KeysMenus.swift` are the basis).
Important constraint: such a button hears the chord *before* the focused text view
(`EXACT2-GAPS.md` X25), but ⌥↑ must act only with the caret at the start; the native composer
view (`T3Composer.swift`, "Keys the text view never sees", `:147`) has to report the caret and
decide, as `T3ComposerIntent.swift` does for ⌘↩. Under Korean 2-Set `R10Connect.swift` re-issues
chords by key code; keep that working.
Reuse: `remove` / `deleteMany` (`sidebar-commands.ts:153,511`), the delete dialog
(`sidebar-commands.ts:460`), `domain.ts:458` (cleanup rules), `isScratch` (`r4-git-env.ts:45`),
`provider-session.detach` (`palette-commands.ts:172`), `cc:queued-steer`
(`composer-controls-commands.ts:213-224`), `beginQueuedEdit` (`composer-controls-queue.ts:90`),
`environmentOptions` and `runOnEnvironment` (`r4-git-env.ts:37,72`), `favoriteEditor`, `pushToast`.
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it
by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | 20261005-clone-on-exact2-main | pending | Merged | pending |
| merged task PR | 20261005-desktop-oracle-and-trace | pending | Merged | pending |
| merged task PR | 20261005-composer-fidelity | pending | Merged (⌥↑ edit starts the queue edit that carries attachments) | pending |
| scheduling preference | 20261005-main-fix-adoption | pending | Merged first | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |

## Issue assessment at preparation

Checked sources and time: {{at prepare}}; draft records only.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X25](../issues/20261005-x25-keyboard-keyup-code-capture.md) | `KeyboardEvent.repeat`, capture phase | `EXACT2-GAPS.md` X25 | nonblocking (workaround: native key monitors; filter repeats with `NSEvent.isARepeat` — confirm in an attended held-key check that the result matches the reference) | Prove with a held key |
| [X15](../issues/20261005-x15-non-latin-key-equivalents.md) | Chords under Korean 2-Set | X15 | nonblocking (workaround: `R10Connect.swift`) | Attended check of each new chord 2026-10-07: #110 closed by main #168, which covers declared chords and the host's command items only; `R10Connect.swift` and the key-code fallbacks stay (adopt-main-fixes-input). |
| [X20](../issues/20261005-x20-rich-text-editing.md) | Caret read from the composer | X20 | nonblocking (workaround: native text view reports the caret) | none |
| [X9](../issues/20261005-x09-root-component-across-files.md) | `app.contract` cap | 1,327 of 1,500 | nonblocking until the cap | Put new dispatch kinds in `settings-shortcuts.contract` |
| [X21](../issues/20261005-x21-two-way-websocket.md) | Sending `vcs.removeWorktree` | X21 | nonblocking (workaround: Swift transport) | none |

## Implementation notes

- Port `getOrphanedWorktreePathForThread` and `formatWorktreePathForDisplay` as
  `worktree-cleanup.ts`, `resolveAdjacentThreadId` and `deleteSelectedThreadEntries` into the
  sidebar logic file, `resolveThreadReferenceCopyTarget` as `thread-reference.ts` (headers name
  source and changes; no React). Keep the existing inline fallback only if the ported
  `getFallbackThreadIdAfterDelete` tests pass against it.
- Persist the last editor in `t3-code.json`; the stored value is used only while still in
  `availableEditors`.
- Each new command needs a dispatch entry, a `kind` in `settings-shortcuts.contract`, and a label
  (generic rule "Composer: Cycle Host"). Add commands through the table-driven command map that
  `20261005-hot-file-split` makes in `keyboard-dispatch.ts`.
- Consume a chord only when the reference consumes it (`preventDefault` only on success).

## Acceptance and reproduction

Every row, attended or not, runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and
`T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773; see
`20261005-embedded-server-runtime`). "Oracle" is `target/t3-ui-parity/electron-oracle.mjs`; "trace" is
`target/t3-ui-parity/trace-proxy.mjs` with `trace-diff.mjs`.

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Worktree prompt | Disposable repo; thread A on a new worktree (clone's worktree flow); cleanup rule off | Delete A | Dialog with the title, text and name above; Cancel keeps the worktree, thread still deleted | macOS 1280×840 and 840×620, light and dark | pixel pair vs oracle; `git worktree list` |
| Worktree removal | Same | Confirm | Trace order: detach (if runtime), `thread.delete`, `vcs.removeWorktree{force:true}`, `vcs.refreshStatus`; directory gone | macOS | trace diff; `git worktree list`; disk check |
| Shared and bulk | A and B on one worktree | Delete A, then B; then select both and delete | A: no prompt; B: prompt. Bulk: one prompt after the first deletion | macOS | trace |
| Skips | Scratch thread; cleanup rule on; archived thread | Delete each | No prompt, no `removeWorktree` call (rule on: server cleans) | macOS | trace |
| Cleanup failure | Make the path undeletable | Confirm | Toast "Failed to delete worktree" with the message; thread gone | macOS | screenshot; logs |
| Logic ports | — | `bun test` ports with original names: `worktreeCleanup.test.ts` (9), `Sidebar.logic.test.ts` "deleteSelectedThreadEntries" and "resolveAdjacentThreadId" cases and "getFallbackThreadIdAfterDelete", `threadReference.test.ts` (6) | Pass | host machine | log |
| Delete dialog keyboard and motion | Same worktree thread | Open the dialog; Tab; Enter; reopen; Escape; set prefers-reduced-motion | Focus enters the dialog and returns to the sidebar afterwards; Enter and Escape results equal the oracle's; the dialog appears without movement under reduced motion | macOS | `tree --ax`; film (`over 300 every 30`) in both modes; `(attended session)` for real keys |
| Queue keys | Running thread, 2 queued messages | ⇧⌘↩; then ⌥↑ with caret at start, then mid-text | Steer command for the first; edit of the last; mid-text ⌥↑ moves the caret | macOS, AppKit `r8-keys`/`composer` binaries for chords; real chords `(attended session)` incl. Korean 2-Set | binaries + notes |
| Host keys | Draft with two machines | ⇧⌘H; bind and press Cycle Host repeatedly, then hold it | Control opens; machines step and wrap; a held key steps once | macOS | `--json` drive; `(attended session)` for the held key |
| Thread keys | Six threads | ⇧⌘[ at first, ⇧⌘] at last; ⌘1–9 | Nothing at the ends; order equals the oracle's | macOS | screenshots; oracle comparison |
| Editor and reference | Two editors available | Open with the second via the menu; ⌘O; ⇧⌘C with and without a PR | Second editor opens; PR link or thread ID with the toast wording | macOS | trace; clipboard read |
| Panel and Settings chords | Draft; Settings open | ⌥⌘B on the draft; ⌘K, ⌘P, ⌘U in Settings | Panel toggles; overlays open above Settings | macOS | screenshots |
| Clone checks | `git add -A` | Usual list, `bun scripts/caps.mjs`, five checks | Green; every moved matrix cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

States: dialog default focus, Escape cancels, Enter confirms (check against the oracle), disabled
chords when the target is absent, error toasts, keyboard focus return after the dialog.
Reduced motion: dialog appears without movement.
Task-owned source paths: `sidebar-commands.ts`, new `worktree-cleanup.ts`, `thread-reference.ts`
(+ tests), `keyboard-dispatch.ts`, `settings-shortcuts.contract`, `keybinding-settings.ts`,
`keybinding-view.ts`, `snapshot-shortcut.ts`, `settings-rest-commands.ts`, `palette-commands.ts`,
`modules/apple/T3Composer.swift` hunks, `macos/tests/r8-keys`.
Required environment: Xcode 27.0, pinned Bun 1.4.2, oracle desktop build, disposable git repo.

## Progress

Implemented on `feat(example)/t3-code-thread-commands-and-keys` (base `9670b0723`), 2026-10-06. Verification: unverified.

- G5 (`worktree-cleanup.ts`, `sidebar-delete-logic.ts`, `sidebar-commands.ts` `remove`/`deleteThreads`): ports of
  getOrphanedWorktreePathForThread, formatWorktreePathForDisplay, deleteSelectedThreadEntries and
  getFallbackThreadIdAfterDelete. The orphan check counts surviving, unarchived threads; Scratch projects and the
  `worktreeOnDelete` rule (environment `storageCleanup` or the project's `worktreeCleanup` override) skip it. The
  question is the sidebar dialog `delete-worktree` (SettingsConfirm, destructive; title "Delete the worktree too?",
  text "This thread is the only one linked to this worktree:" and the last segment). Answers resume the paused run
  (`sidebar:dialog-confirm` / `sidebar:dialog-cancel`); Cancel still deletes the thread. Order: provider-session.detach
  per session (only with a runtime, projection fetched for a thread that is not open), the terminal hook
  (`setCloseThreadTerminals`, no-op until 20261005-terminal-drawer), thread.delete, fallback navigation (top thread of
  the project in the thread sort; with none left, the home route `/` as the reference does: a draft in the most
  recently active project), then `vcs.removeWorktree{cwd: workspaceRoot, path, force: true}` and
  `vcs.refreshStatus{cwd}`; failures are stacked toasts and never fail the deletion. Bulk deletes continue after a
  failure (first failure toasted) and ask once per orphaned worktree. The legacy sidebar's direct deletes use the same path.
- Keys (`thread-keys.ts`, one `MAIN_ROWS` entry): ⇧⌘↩ steers the first queued message only when the provider steers
  (the row button's hard-coded chord is gone); ⌥↑ is a hidden `t3-composer-key` button the native composer presses
  only with a collapsed caret at 0 (`T3ComposerQueueKey.swift`, one line in `T3Composer.handle`); the queue list
  shows while an edit is open. ⇧⌘H opens the strip's "Run on" menu and `composer.cycleHost` (new in
  `keybindingCommands`, label "Composer: Cycle Host") moves a draft to the next machine, wrapping. Repeats are
  consumed and ignored by the host for every dispatch button.
- `keyboard-dispatch.ts`: previous/next via resolveAdjacentThreadId (no wrap; none with an unlisted current thread or
  the model picker open), ⌥⌘B on drafts, the palette provider's chords (⌘K ⌘P ⇧⌘F ⌥⌘A ⌥⇧⌘A ⌘U) in Settings,
  Back/Forward over Usage and Pull Requests pages (`keyboardDispatch` gets `utilityPage`), last-used editor for ⌘O.
- ⇧⌘C (`thread-reference.ts`): resolveThreadReferenceCopyTarget and resolveThreadCurrentPullRequestLink (stack
  layering through `resolveChains`); the open PR surface's URL first; toasts "PR link copied" / "Thread ID copied"
  with the value. The palette row is "Copy PR link" or "Copy thread ID".
- Last editor: `shell.lastEditor` in t3-code.json (`shell-prefs.ts`), used only while still available.
- Audit: the PR detail selection inside the Pull Requests page is not a history entry (the page is). Thread-jump
  order is unchanged (pinned, active, working, snoozed, settled rows as the snapshot lists them); oracle comparison not run.

Not run or left: oracle and trace-diff rows (not built); the worktree dialog in the app UI (its only entry is the
native context menu, which the agent answers as dismissed: unverified (attended)); real chords ⌥↑, ⇧⌘↩, held keys and
Korean 2-Set (unverified (attended)); queue keys live (no signed-in provider: unverified (needs sign-in, on hold));
⇧⌘H / Cycle Host live (one machine in the lane); reduced-motion and focus-return rows of the dialog.

Open question for the user: whether the agent may get a way to pick from the sidebar's native menu (test
apparatus, needs approval), so the worktree dialog can be driven live.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 | `72bd6a8d8` | `bun test examples/t3-code` 1867 pass / 0 fail (base 1829); strict tsc clean; contract build 2324 slots, 43 resources; `cargo test -p t3-code-macos --lib` 10 pass; AppKit binaries 0 failures except r8-keys (2) and r9-input (4) focus tests, which pass on rerun and fail the same way at the base; mermaid not runnable (lane server has no web Mermaid build); caps and the five checks pass (cargo tests 2927 pass); macOS bundle builds | Live drive (one BEFORE on the base app, one AFTER): at the first thread the dispatch list has no `shortcut-thread.previous` and ⇧⌘[ keeps "Locked C" (base: wraps to "Plain notes"); ⌥⌘B on a draft opens "Open a surface"; ⌘K in Settings opens the palette above it; ⌘U leaves Settings for Usage with Back/Forward live. G5 against the lane server and real `git worktree`s through the clone's own sidebar path (menu pick stood in): Confirm → thread.delete, vcs.removeWorktree{force:true}, vcs.refreshStatus, feature-a gone from `git worktree list`; shared feature-b: first delete no dialog, last thread's Cancel keeps it; locked feature-c → "Failed to delete worktree" toast, thread deleted | attended rows above |
| 2 | home route fix | Deleting a project's last open thread goes to the home route `/` (a draft in the most recently active project) as the reference does, not a draft in the deleted thread's project; `bun test examples/t3-code` 1868 pass / 0 fail; strict tsc clean; no live drive (coordinator) | Bun test "deleting a project's last open thread goes home" | — |

## Next action

Review the PR; attended session for the native-menu dialog, real chords and Korean 2-Set; then `verify`.
