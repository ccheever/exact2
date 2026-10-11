---
name: 20261011-search-key-and-timeline-commit-title
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-search-key-and-timeline-commit-title
pr_url: https://github.com/ccheever/exact2/pull/439
verified_commit: null
---

# ⌘⇧F under real keys, and the Timeline's commit titles

## Outcome

The user asked on 2026-10-11 to fix these after `realinput-1011a` (STATUS "Known differences" and "Found, not in
scope").

| Id | Clone | Reference (check it first) |
| --- | --- | --- |
| SK-1 | Under real keys ⌘⇧F opened no project search, from the composer or from the timeline ([keys](https://raw.githubusercontent.com/ccheever/exact2/f4d5690d51737e4a76136d207d7d91c2d975f115/realinput-1011a/DT7-palette-search-keys.txt)), though `mod+shift+f` is bound to `projectSearch.toggle` and #431's agent drive opened it. A real key goes through the main menu's key equivalents and the app's key monitors before the window; #434's review found that an agent key goes straight to the window | ⌘⇧F opens the project content search from anywhere in a thread |
| SK-2 | Other bindings may be lost the same way | List every keybinding the reference has (`keybindings`) and which of them a real key would lose on the clone's path; fix them together |
| TC-1 | A pull request Timeline commit title stays invisible until the pointer is over it ([image](https://raw.githubusercontent.com/ccheever/exact2/2610719c6267c52ddaaa45274a12a74110dadfdc/realinput-1011a/UV1-timeline-ages-no-tip.png)) | The reference draws the title at rest |

## Steps

1. Reference first (CDP on the reference lane; source under `target/t3-ref/src-1e2ecbd975`): SK-1 and SK-2's bindings;
   TC-1's commit row at rest and under the pointer, its colours light and dark.
2. SK-1: find what takes a real ⌘⇧F before the binding (a main menu item with that key equivalent, a module's key
   monitor, `R10Connect.swift`'s re-issued chords, the composer's text view's own handling) by reading the source and
   by an AppKit-level test that sends the key through `NSApplication.sendEvent` (the real key's path) on the base and
   on the branch. Fix it so the binding runs, as the reference's does.
3. SK-2: the same for every binding that path loses; a test that fails if one is lost again.
4. TC-1: find whether the title's style hides it at rest (a hover-only colour or opacity) or the host does not redraw it
   (like X82: the agent's plain `screenshot` redraws every text and can hide that; use `screenshot … window` or
   `sample`). Fix the clone's side; if it is the framework's, record the measurement and leave the filing to the
   coordinator.
5. Tests; one agent drive; before / after / reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| SK-1, SK-2 | reference comparison first; AppKit test through `NSApplication.sendEvent`; agent drive | before / after text records |
| TC-1 | reference comparison; tests; agent drive (`screenshot … window`) | before / after / reference images, light and dark |
| SK-1 and TC-1 under real keys and pointer | real-input steps for the short final session | — |

## Reference

- SK: `packages/shared/src/keybindings.ts` DEFAULT_KEYBINDINGS (85 rules; the clone's `keybindingDefaults` is the same
  list): `mod+shift+f` `projectSearch.toggle` `when: "!terminalFocus"`, opened from anywhere in a thread
  (CommandPalette.tsx). The renderer sees the key before Electron's menu.
- TC-1: `PullRequestTimelineTab.tsx` CommitEvent: the whole row is a `<button>` whose title is `text-foreground` at rest
  and `group-hover:text-primary`. The lane server's own build of the web client in headless Chrome (PR #168, sandbox
  `daehyeonmun2021/playground`): the title at rest `oklab(0.274 …)` light / `oklab(0.97 0 0)` dark, under the pointer
  `oklch(0.488 0.217 264)` / `oklch(0.571 0.21 264)`.

## Cause and fix

**SK-1.** The clone's own real-key path is right. A temporary in-process diagnostic (removed before committing) sent a
CGEvent-backed ⇧⌘F through `NSApplication.sendEvent`, the path a key from the window server takes (local monitors, the
key window's `performKeyEquivalent`, the main menu, the window): it opened the project search with the focus on the
window, in the composer, and after DT-7's own ⌘K / Tab / ⇧Tab / Esc sequence; `R8_KEYS_LOG` saw each key. The only ⇧⌘F
item in the bar is the host's hidden File › Search Project Contents. What differs under real keys is before the app:
this Mac has symbolic hot key 157 = ⇧⌘F enabled (SkyLight `CGSGetSymbolicHotKeyValue`), absent from System Settings'
shortcut table and not one AppKit handles in the app (`_NSKeyboardUIHandleSymbolicHotKey` has no case for 157), so a key
the window server routes there never arrives as a keyDown. Spotlight's "Search With Spotlight" service declares ⇧⌘F too
but is not in the Services menu, and Services key equivalents run after the clone's monitor and menu anyway (a bundled
probe: ⇧⌘L's service ran only after the monitor and the window's `performKeyEquivalent`). No clone change; the
real-input steps below confirm whether the key reaches the process and whether the reference loses it too. Record:
[sk-record.txt](https://raw.githubusercontent.com/ccheever/exact2/fd60e224177735cdc00c9d68a821046681234006/search-key-and-timeline-commit-title/sk-record.txt).

**SK-2.** Of the reference's bindings, the app's path loses none: every ⌘ chord goes through the same monitor →
`ShortcutHost.perform` → its dispatch button (or a visible button) → the hidden menu item. Before the app, on this Mac:
⇧⌘F (symbolic hot key 157, above); ⇧⌘3 / ⇧⌘4 (`usage.period.month` / `.quarter`, Usage page only) are the screenshot
hot keys 28-31, disabled here but enabled on a stock Mac, where every app loses them; ⇧⌘A, ⇧⌘M, ⇧⌘L are Services key
equivalents (Terminal's man page index and man page, Safari's Search With Google) that come after the app's monitor and
menu, so they are kept. The guard test fails if a reference ⌘ binding of an open thread loses its dispatch button.

**TC-1.** The macOS host's raster pump. A wide text (over 16,384 device pixels) is rastered; `updateLayer` asks
`TextRasterizer.ensure` with `urgent` only while the text is on screen, a worker's raster made while it was off screen
stays `textRasterPending` for the pump to present, and the pump (`refreshVisibleText`) takes its candidates from
`selection.paragraphs`, which hold selectable texts only; no text under a button is selectable (`textSelectable`).
So every commit title (in the row's `button`) stayed blank from its first mount until the hover colour invalidated it;
the sha and age (small, drawn directly) and the lifecycle row's texts (not in a button) showed. This is X82's mechanism
with a first-mount trigger, not only a live scheme switch (recorded in X82's row, `EXACT2-GAPS.md`; the framework
filing is the coordinator's). Clone fix: the commit row is a `column` whose texts sit beside the button, and the button
lies over the whole row (`position: absolute`, full size, `z-index: 2`, transparent), so the texts are selectable to
the host and drawn while every press, hover and drag is still the button's; the texts are `aria-hidden`, the button's
label "View commit <id>" the row's one name, as the reference's. `user-select: text` on the title was refused by the
compiler (not portable to iOS and Linux).

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| SK-1 | in-app path pass, cause outside the app: before and after (the same key code) ⇧⌘F through `NSApplication.sendEvent` opens the project search from the window, the composer and after DT-7's sequence; the real key's loss traced to the system's enabled symbolic hot key 157 (⇧⌘F). Under real keys: open, step 1 | [sk-record.txt](https://raw.githubusercontent.com/ccheever/exact2/fd60e224177735cdc00c9d68a821046681234006/search-key-and-timeline-commit-title/sk-record.txt) |
| SK-2 | pass: every reference binding listed against the app's path and this Mac's hot keys and Services; none lost in the app; guard test (4 pass) | [sk-record.txt](https://raw.githubusercontent.com/ccheever/exact2/fd60e224177735cdc00c9d68a821046681234006/search-key-and-timeline-commit-title/sk-record.txt) |
| TC-1 | pass (agent drive, `screenshot … window`, light and dark, reference): before, every commit title blank at rest and drawn only after hover; after, drawn at rest in the ink colour, blue under the pointer, as the reference; a mouse click on the title and on the row's middle opens the Code tab on that commit | [light](https://raw.githubusercontent.com/ccheever/exact2/f1a0fff3b8840975e292d53ac7704604e426ecd8/search-key-and-timeline-commit-title/tc1-light.png), [dark](https://raw.githubusercontent.com/ccheever/exact2/5bc3bc8bc7bbaf99b5a1b45efc9b13628f157534/search-key-and-timeline-commit-title/tc1-dark.png), [press](https://raw.githubusercontent.com/ccheever/exact2/e65626aeeb9a9dd653ed3bc54a4a01ba0bbb683a/search-key-and-timeline-commit-title/tc1-after-press.png) |
| SK-1 and TC-1 under real keys and pointer | open: real-input steps 1-4 | — |

Drives (agent mode, 1280×840, the GitHub lane's primary server on 16323 paired through Settings › Connections, PR #168):
before on the evidence worktree at `ff1eb4ceb`, after on this branch's bundle (`91ccd8abb`'s app code); the reference is
the lane server's web client in headless Chrome 1280×840 at 2×.

## Real-input batch steps

Launch this branch's bundle as a lane copy (its own bundle id, the lane home outside any git checkout) with
`R8_KEYS_LOG=<lane>/r8-keys.log` in its `open -n --env`; a thread open (Verification fixture). Never send a message.

1. **SK-1, composer.** Click in the composer; press ⇧⌘F. Expected (reference): the project content search opens
   ("Search in <project>"). Then `grep 'key=f flags=1179648' <lane>/r8-keys.log`: no line means the key never reached the
   app (the system took it); a line means it did, and its `items=` and `responder=` are the next clue.
2. **SK-1, window.** Click the timeline (focus off the composer); ⇧⌘F; the same two reads.
3. **SK-1, control.** The same ⇧⌘F in the reference T3 Code (Alpha) lane window with a thread open, and ⇧⌘F in Xcode's
   editor if it is installed (Find in Project). If neither opens anything, ⇧⌘F is held by the system on this Mac
   (symbolic hot key 157) for every app: record it, nothing to fix in the clone. Also press ⇧⌘M in the clone's composer:
   the model picker opens (a Services chord the app keeps).
4. **TC-1.** Connect the GitHub lane (Settings › Connections), Pull Requests › #168 › Timeline, the pointer away from
   the list: every commit row shows its title at rest; on a row the title turns blue; a click on the title opens the
   Code tab on that commit. Settings › Appearance › Dark and back to the Timeline: the same in dark.

## Tests

- `search-key-and-timeline-commit-title.test.ts` (4): SK-2, every ⌘ binding of the reference's defaults that holds in an
  open thread keeps its dispatch button and chord, with nothing focused and from the composer (23 asserted; commands a
  visible control or the composer answers, and those needing a terminal, preview, model picker, Usage page, history or
  Undo notice, are named in the file); SK-1, ⇧⌘F is `projectSearch.toggle`'s one button from the composer, with nothing
  focused and with the palette open; TC-1, the commit row's button is last in the row, over it, holds no text, and the
  title keeps the reference tones, its texts `aria-hidden`.

## Checks

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 1, then 0 | the first run's one failure was `help-tags.test.ts`: the new overlay button had an `aria-label` and no text, so AppKit would show its label as a help tag; `title=""` added; rerun 4543 pass, 1 skip, 0 fail (310 files) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | after the `title=""` change too; `app.contract` 1,398 lines |
| `cargo test -p t3-code-macos --lib`, AppKit binaries | not run | no Rust or Swift changed (the diagnostic Swift never reached a commit) |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 3,679 passed, 0 failed |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |
| `host/apple/build.mjs t3-code-macos --bundle` | 0 | once, before the live drive (the app code of `91ccd8abb`); `title=""` came after it |

The five Cargo/boot checks ran on `034a87fb9`; the final head adds only the overlay button's `title=""`, its test line
and this record (`examples/t3-code` is outside the Cargo default members).

## Not done / not verified

- SK-1 under real keys: whether the key reaches the process, and whether the reference loses it too (steps 1-3). Blocker:
  real input is the coordinator's final session.
- TC-1 under a real pointer (step 4). Same blocker.

## Seen while building, not a finding row

- The Timeline's "Pull request opened" row shows the actor's avatar in the clone; the reference's LifecycleEvent shows
  the pull request glyph (IconMarker) — visible in the TC-1 images.

## Next action

Coordinator: review #439; run real-input steps 1-4 in the next short session.
