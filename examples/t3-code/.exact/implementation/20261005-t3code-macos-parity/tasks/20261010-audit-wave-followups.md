---
name: 20261010-audit-wave-followups
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-audit-wave-followups
pr_url: https://github.com/ccheever/exact2/pull/372
verified_commit: null
---

# Differences the audit fix agents found outside their tasks

## Outcome

The first audit fix wave (2026-10-10) reported four differences from the reference that were outside each agent's task.
Each was seen on the base and the branch alike. This task fixes them as the reference does.

## Findings

| Id | Reference (T3 Code `1e2ecbd975`) | Clone | Found by | Steps |
| --- | --- | --- | --- | --- |
| FU-1 | On a server thread with no messages, the composer is docked at the bottom with "Send a message to start the conversation." | The composer sits in the middle of the window, as on a draft. | composer-provider-state-and-details (#356) | Make a thread with `thread.create` only (no turn); open it in both apps. |
| FU-2 | The right panel's open state is per thread: a fresh draft opens with the panel closed. | A fresh draft shows the launcher when the panel was open on the previous thread. | right-panel-launcher-and-files (#355) | Open the right panel on a thread, then ⌘N; compare. |
| FU-3 | ⌘↩ in a Files preview's comment draft saves the comment (as in the Diff). | Same cause as PA-12 before #360: `r4-surfaces-files.contract` passes the draft's focus ops but does not track them, so ⌘↩ reaches the composer's Send instead. | diff-panel-parity (#360) | Files › a source file › comment on a line › type › ⌘↩. Agent mode first; real keys in the batch. |
| FU-4 | Settings' provider page remounts on every route change, so a provider's open "Add custom model" field closes. | Two root-level provider links in a row while Settings stays open (A → B → A) can show A's still-open Add field again. | settings-escape-and-nav (#361) | Open Add on provider A; follow two provider links from inside Settings; compare. |
| FU-5 | Escape in the command palette opened over Settings closes only the palette. | Escape closes the palette and Settings too (agent keys, base and #364's branch alike): #361's `escapeOwned` has no palette term. | settings-appearance-and-skill-chip (#364) | Open Settings, ⌘K, Escape; compare. Real keys in the batch. |

## Scope and exclusions

Included: the five rows. Excluded: framework changes; anything already in another open task.

## Context and guidance

- FU-1: reference `ChatView.tsx` (empty-thread composer placement and hint), clone `chat-canvas-layout.ts`,
  `composer-resting-layout.ts`.
- FU-2: reference right-panel state keyed by thread (`rightPanel` store), clone `right-panel-tabs.ts`,
  `r4-surfaces-panel.ts`.
- FU-3: the PA-12 fix in #360 (`diff.ts` `noteDraftFocus`, `draftHoldsCommandEnter`) is the pattern; apply it to the
  Files preview draft (`r4-surfaces-files.contract`, `r4-surfaces-files.ts`).
- FU-5: `app-settings.contract` `escapeOwned` (#361) and the palette's Escape in `palette.contract`.
- FU-4: #361 keys the field by the Models block key (`modelAdding == block.key`); clear it on a provider route change.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| FU-1 | agent drive on a no-turn server thread | before / after / reference image |
| FU-2 | agent drive: panel open, ⌘N | before / after / reference image |
| FU-3 | Bun test as #360's PA-12 test; agent ⌘↩; real ⌘↩ in the batch | before / after image |
| FU-4 | Bun test of the route change | text |
| FU-5 | agent drive: Settings, ⌘K, Escape (Settings stays); real Escape in the batch | before / after / reference image |

## Cause and fix

- **FU-1.** The clone treated every empty timeline as the draft hero (`app-main.contract` `overlaid = length(data.messages) > 0`).
  The reference's `isDraftHeroState` (`ChatView.logic.ts resolveDraftHeroState`) holds only for a local draft. Now
  `ChatColumn` derives `hero = length(data.messages) == 0 and data.threadId == ""`. A server thread with no messages docks
  the composer over MessagesTimeline's empty state, "Send a message to start the conversation." (text-sm,
  muted-foreground/30, centred). Like `hideEmptyPlaceholder: threadDetailLoading`, the hint waits while the detail loads
  (new snapshot field `threadLoading`) and while a run works. The canvas measures the docked overlay too
  (`app.contract` chatCanvas).
- **FU-2.** The window kept one `rightPanel` flag for every thread, so a panel opened on one thread showed the launcher
  on the next one. The reference keys `isOpen` by thread (`rightPanelStore` `byThreadKey`, `show` with no surface is the
  launcher). Now:
  - The client's `surface-show` opens the thread's panel even with no surface (`r4-surfaces-panel.ts`).
  - `shell.panel` reports the panel's `key` and `launcher`.
  - The window holds its open state for the key it was opened on (`app.contract` `rightPanelAt`; `rightPanel` is now a
    derive).
  A fresh draft (⇧⌘N) opens closed. The thread keeps its launcher when you come back.
- **FU-3.** It had two causes.
  - The cause the record named: the Files draft sent its focus ops but `fileComment` ignored them. The PA-12 pattern now
    covers it. `diff-file-comments.ts` keeps the focus with the panel key it was taken on. `r4-surfaces-files.ts`
    `fileDraftHoldsCommandEnter` counts it only while that panel shows the draft's file as source with the draft's line
    (the host sends no blur when a view leaves the tree, LLP 1008). While it holds, `composer-presentation.ts` gives
    Send no chords.
  - Found by the live drive's first try ([image](https://raw.githubusercontent.com/ccheever/exact2/a11b799d820316e82b2b731b40049adaa245373c/audit-wave-followups/fu3-first-try-stale-draft.png)):
    ⌘↩ inserted the "app.ts L3" chip, but the card stayed as an empty draft. The save ran on `localChanged`, and the
    chip's text change is the composer's `draft` write on the same send. That write replaced the save before the save
    closed the draft. `chatLocal` now sends `surface-files-comment-*` on its own queued mutation, `fileCommentChanged`.
    The Comment button used the same send, so it had the same exposure.
- **FU-4.** A root-level provider link left the Settings view's `modelAdding` in place. The reference keys
  `ProviderSettingsPanelContent` by the link's environment and instance, so a link to another target remounts the field.
  `app-settings.contract` gains `fn providerLinkVisit`. The root's `openProviderSettings` uses it to count the links that
  change the Providers page's target. A link to the page's own target does not count, as the same key does not remount.
  The field counts only on the visit it was opened on (`fn modelAddingOn`, `modelAddingLive`).
- **FU-5.** #361's `escapeOwned` had no palette term, so Back kept `aria-keyshortcuts="Escape"` beside the palette's own
  Escape. Now `SettingsWindow` takes `paletteOpen`, and an open palette owns Escape (`escapeOwned`), as its Base UI
  Dialog prevents it in the reference.

No framework code changed, and no new declared difference (`EXACT2-GAPS.md` unchanged).

## Acceptance results

The lanes are `audit-wave-followups` (reference, after) and `audit-wave-followups-before`, base port 16320, each with a
fresh `--storage` per drive. The "Audit empty thread" was made with `thread.create` only (`tools/setup-thread.sh` on
spare port 16323). The clone and reference homes have codex and claudeAgent switched off, so nothing can be sent.
- Before: the never-edited worktree `t3-code-evidence-base` at `950e8e2e5`. That is the feature tip before #355–#366.
  Each finding was also seen at `eba445c44`.
- After: the code of `e3443c45c`. The final head adds only merges of #352, #368, #369 and #370.
- Reference: the Electron T3 Code `1e2ecbd975` over CDP.

Each run is one agent drive ([drive record](https://raw.githubusercontent.com/ccheever/exact2/487098b41d7c94fc3b4bd82b5f8a70f8b3da4f68/audit-wave-followups/drive-record.txt)).
Images read before | after | reference.

| Row | Result | Proof |
| --- | --- | --- |
| FU-1 | pass | [empty server thread](https://raw.githubusercontent.com/ccheever/exact2/7650f57e9a292b58360a3c360379d70f7513574b/audit-wave-followups/fu1-empty-server-thread.png): the composer is docked at the foot under "Send a message to start the conversation.", as in the reference. Before, it was centred. |
| FU-2 | pass | [fresh draft](https://raw.githubusercontent.com/ccheever/exact2/e76b2f01345d288c3e0678bc8021f00488307822/audit-wave-followups/fu2-fresh-draft.png): launcher opened on the thread, then ⇧⌘N. The draft opens with the panel closed. Before, the draft showed the launcher. [Back on the thread](https://raw.githubusercontent.com/ccheever/exact2/d078f6089c7e2b1fe5644b00b98350b8d71a8c3c/audit-wave-followups/fu2-back-on-thread.png): its launcher is still open, as in the reference. The reference's store read `{<thread>: {isOpen: true, surfaces: []}}` with no entry for the draft. |
| FU-3 (Bun, agent) | pass | Unit tests like PA-12's (below). [Agent ⌘↩](https://raw.githubusercontent.com/ccheever/exact2/9f28d8e229e6ed1b4e66c768c8d020283febb62f/audit-wave-followups/fu3-cmd-enter-saves.png): the inline card "audit note" and the composer's "app.ts L3" chip. Before, the draft stayed open with its text. Tree with the draft focused: `send-message` keys were `Meta+Enter Meta+Alt+Enter` before and `''` after. |
| FU-3 (real keys) | open | Needs real input. See "Real-input batch steps". |
| FU-4 | pass (text) | [Bun test before/after](https://raw.githubusercontent.com/ccheever/exact2/a2b05e146fd920ee1620ce3017be6b5550b59279/audit-wave-followups/fu4-tests-before-after.txt). A → B → A closes the field opened on A, and a link to the page's own target keeps it (`providerLinkVisit` and `modelAddingOn`, run from the Contract source). Fails before, passes after. |
| FU-5 (agent) | pass | [Settings, ⌘K, Escape](https://raw.githubusercontent.com/ccheever/exact2/e37177d86499241ddc15d2f5cf97d34e816f8fd2/audit-wave-followups/fu5-palette-escape.png): Settings stays with the palette closed, as in the reference. Before, the window went back to the chat. |
| FU-5 (real keys) | open | Needs real input. See "Real-input batch steps". |

## Tests

`audit-wave-followups.test.ts` (10 tests; 9 fail against `eba445c44`'s sources):
- **FU-1:** the hero and overlay derives, the placeholder and its conditions, and `threadLoading` on a server thread
  versus a draft.
- **FU-2:** the panel view's `key` and `launcher` across a thread, its draft and another thread (show, a surface, hide,
  show again, close all), and the window's keyed `rightPanelAt`.
- **FU-3:** Send's chords over begin, blur, focus, hide, show and save (the chip "app.ts L3"). A focus report with no
  draft holds nothing. A draft that left the tree without a blur holds nothing: another file, another thread, the
  editor, a cancel. The queued `fileCommentChanged` send.
- **FU-4:** the two fns, run as JavaScript.
- **FU-5:** `escapeOwned` has the palette term, and the palette keeps both of its Escape buttons.

`settings-escape.test.ts` now reads the field through `modelAddingLive` and the visit stamp.

## Checks

These ran on `6b140f514`, the head after a second merge of `origin/feat(example)/t3-code` (#368, #369, #370). That
merge had conflicts in `app.contract`, `app-window.contract` and `app-settings.contract`, resolved keeping both sides:
the incoming `scheme` prop and Settings update pill, and this branch's `providerVisit`, `paletteOpen` and
`modelAddingLive`. The same checks had all passed on `e32ff7949`, the merge of #352.

| Check | Exit | Result |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 3,948 pass, 1 skip, 0 fail, 277 files |
| strict `tsc` on `examples/t3-code/app.ts` (README command) | 0 | no errors |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 5,968 slots, 47 resources, 103,605 nodes; `app.contract` is 1,273 lines |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 3,521 pass, 0 fail, 34 ignored |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |

No Rust or Swift changed, so `cargo test -p t3-code-macos --lib` and the AppKit binaries were not run. The bundle was
built twice (the first live try, then after the FU-3 send fix). There was one live agent drive per build.

## Real-input batch steps

FU-3 and FU-5 with real keys, in one session (screen unlocked; lane `audit-wave-followups`, a build of this branch).
`A` = `/Users/daehyeonmun/orca/workspaces/exact2/t3-code/target/t3-audit`, `L` = `$A/lanes/audit-wave-followups`.
1. From `/Users/daehyeonmun/orca/workspaces/exact2/t3-code-audit-wave-followups`, with the lane's isolation as
   `$A/clone-drive.sh` sets it, run: `PATH=$HOME/.bun-1.4.2/bin:$PATH EXACT_APP_DIR=$PWD/examples/t3-code
   T3_LOCAL_HOME=$L/clone-t3-home T3_LOCAL_PORT=16322 T3CODE_TELEMETRY_ENABLED=false
   T3_LOCAL_RUNTIME_DIR=$A/runtime/t3-0.0.46-nightly.20261005.2667-darwin-arm64 CODEX_HOME=$L/codex
   CLAUDE_CONFIG_DIR=$L/claude XDG_CONFIG_HOME=$L/xdg/config XDG_DATA_HOME=$L/xdg/data XDG_STATE_HOME=$L/xdg/state
   XDG_CACHE_HOME=$L/xdg/cache bun host/apple/build.mjs t3-code-macos --bundle --run`.
2. Dismiss the mobile-app notice. Open "Audit empty thread", then press the header's "Toggle right panel". Choose Files,
   expand `src` and open `app.ts`.
3. Hover line 3 and click its "+". Type `audit note` and press ⌘Return with real keys. It passes when the card "audit
   note" shows under line 3, the draft is gone, and the composer gets the "app.ts L3" chip. Nothing is sent, because the
   lane's clone home has its providers switched off.
4. Open Settings (the sidebar's gear), press ⌘K, then press Escape with real keys. It passes when the palette closes and
   Settings stays open. Press Escape again: Settings closes.

## Next action

The coordinator runs the real-input batch steps, then reviews and merges the PR.
