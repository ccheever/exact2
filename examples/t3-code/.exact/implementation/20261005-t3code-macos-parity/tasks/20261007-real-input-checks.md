---
name: 20261007-real-input-checks
plan: 20261005-t3code-macos-parity
implementation: verified
verification: passed-with-open-findings
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-real-input-checks
pr_url: https://github.com/ccheever/exact2/pull/213
verified_commit: 9cd6c49cb
---

# The real-input checks left open by #206 and #207

## Outcome

The rows that PRs #206 (interface font size) and #207 (adopt main fixes, round 3) left for a real hand
are driven with real input: Orca computer use (`orca computer`, events posted to the app by pid) and
HID events from this Mac (`cliclick`, and two small CGEvent posters for a held key and the wheel), each
read back from the window picture, the accessibility tree or the saved preferences file. Before is the
untouched base `t3-code-evidence-base` at `4f523ef5c` (the tip before #206 and #207); After is this
branch at `ca398fe0c` (#203, #206, #207, #212 merged). No app code changed.

## Scope

| # | Check | From |
| --- | --- | --- |
| 1 | Settings › Appearance › Interface font size set to 20 through the real dropdown, real quit, relaunch of the same bundle copy | #206 rows "Persistence across a real relaunch", "Attended size change with real input" |
| 2 | Real middle click on a right-panel tab (Files, with Diff open) | #207 / X8, `RightPanelTabsInput.swift` hit test |
| 3 | Focus ring on custom pressable buttons under real Tab presses | #189 / X47 |
| 4 | X8 rows left manual: cursor shapes, drags out of the window and back, terminal pointer under a real hand | X8 file, "Still not agent rows" |
| 5 | Steer/queue morph under a really held ⌘ (not filmed in #207: needs a held modifier, X25) | #207 final round |
| 6 | #207's lost final-round record | `20261007-adopt-main-fixes-r3.md` |

Excluded: sign-in terminal links (user hold on sign-in tasks), following a terminal or chat link (opens
the user's browser or editor), ⌘C/⌘V into the shared pasteboard beyond an empty-clipboard check (the
user's clipboard is an outside effect), IME and Korean 2-Set (input method, not pointer).

## Lane

Not committed (`target/lane-ric`): reference server `1e2ecbd975` (`apps/server/dist/bin.mjs serve`) on
127.0.0.1:16531 with isolated HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_*, T3CODE_HOME, telemetry off; a
fake Codex (`bin/codex` running the reference's own `codexCollabMockPeer.mjs`, copied, with a
`model/list` answer of one model "GPT-5.5", the user agent `codex_cli_rs/0.160.1` and a scripted reply;
`holdTurnOpen` for the steer film); a fixture repository `projects/alpha` (README, `src/a.ts`,
`src/long.ts`, two uncommitted edits so Diff has content). Pairing links were minted with
`bin.mjs pair --base-dir <lane>/t3home` and pasted, never printed.

Each app ran as a bundle copy with its own bundle id (`com.exact.t3code.laneric.before` / `.after`,
ad-hoc signed), launched with `open -n` and addressed by pid, so its preferences domain and data root are
the lane's. The user's `com.exact.t3code.macos` domain hashed `acc150dd…` before and after; the lane's
Keychain item (`127.0.0.1:16531`) was deleted; the clipboard was empty before and after. Sessions ran
under `.t3-live-drive-lock`: Before (13:47–13:59), After (14:00–14:08), and one Before retry (14:09–14:10)
for the two Before rows the first session ran with the wrong start point (focus ring from a blank area;
terminal drags at x 215, which is the sidebar). The screen was unlocked throughout.

Tool notes for the next real-input drive: `orca computer type-text` typed every character twice into the
pairing field and the terminal (single key presses and `cliclick t:` typed once; `paste-text` was used
for links); `orca computer scroll` did not scroll the settings page (a CGEvent wheel did); window points
are screenshot pixels × 0.5, and a displayed-image coordinate must be scaled first.

## Results

| # | Before | After | Verdict |
| --- | --- | --- | --- |
| 1 | The dropdown sets 20 px (`t3-code.json` `fontSizeInterface: 20`); only the preview line grows; after a held ⌘Q (the clone's default quit mode is hold) and relaunch the setting still reads 20 and the UI stays at the 16 px layout | The dropdown sets 20 and the whole UI scales (sidebar, settings, composer); after a held ⌘Q and relaunch `fontSizeInterface` is 20 and the UI is at 20 again | **pass** |
| 2 | Middle click on Files: **Diff** closes, Files stays (the wrong-tab bug) | Middle click on Files: Files closes, Diff stays | **pass** |
| 3 | Click General, Tab, Tab: accessibility focus moves General → Appearance → Keybindings → SnapShots; no ring is drawn | The same focus moves and each focused row draws the ring | **pass** (#189 adopted) |
| 4a | Cursor shapes (`screencapture -C` under a real pointer): terminal I-beam, drawer edge up-down resize, chat arrow, theme editor header open hand, grip crosshair | Same five | **pass** (parity) |
| 4b | Theme editor header dragged to a point left of and above the window, released outside: the panel follows and stays inside the window (top 8 pt) | Same | **pass** (parity) |
| 4c | Terminal: real double-click selects "73" with the Add to chat / Copy popup; right-click opens Add to chat / Copy / Paste; Escape closes | Same | **pass** |
| 4d | Terminal: a real drag (in the terminal, out of the window and back; `cliclick` and `orca computer drag`) selects nothing | Same; ⌘C after it leaves the clipboard empty | **fail, pre-existing** (open finding below) |
| 4e | — (not driven) | Sidebar rail dragged with a real pointer resized the sidebar; a real double-click on the rail reset it | pass |
| 5 | Held ⌘ for 1.5 s over the send button while a turn runs: queue → steer → queue, two icons cross-fade mid-way | The same swap morphs one path (morphicons) | **pass** |

Open findings, with their exact state:

- **Terminal drag selection does not work with a real pointer, in both builds** (4d). The agent's drag
  (`macos/tests/terminal/pointer.swift` `testDragSelectsText`, #207's live agent drag) selects; a real
  drag does not, while a real double-click and right-click on the same view do. Not a regression of this
  branch. Cause not found in this task: a recognizer or a host node taking `mouseDragged` before the web
  view is the first suspect (`NodeViewMac.mouseDragged`'s mouse chain); the next step is a real-pointer
  session that logs the page's pointer events (`t3Terminal.debug()`). Recorded in `QUEUE.md`.
- **After a relaunch the After build showed "Environment disconnected" for about 30 s** (the Before
  relaunches reconnected within 8 s), and the snapshot (so the 20 px layout) arrived with the connection:
  the data source's first answer waits for the transport's `status`, which runs on the transport's queue.
  Seen once; whether the reconnect delay belongs to this build is not established. A seeded launch with no
  server showed the same in #206 ("did not read the saved preferences in the unpaired state").

Toast copy morph (an error toast's Copy button) was not filmed: no error toast with a description was
reachable in the lane without an outside effect (Pull Requests with no `gh` shows an empty state, not a
toast).

## Evidence (before/after, one image per scenario)

| Scenario | Image |
| --- | --- |
| 1 Interface font size, real dropdown and relaunch | ![font](https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/real-input-checks/01-interface-font-size-relaunch.png) |
| 2 Real middle click on the Files tab | ![middle](https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/real-input-checks/02-middle-click-files-tab.png) |
| 3 Focus ring under real Tab | ![focus](https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/real-input-checks/03-focus-ring-tab.png) |
| 4a Cursor shapes | ![cursors](https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/real-input-checks/04-cursor-shapes.png) |
| 4b Theme editor dragged out of the window | ![theme](https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/real-input-checks/05-theme-editor-drag-out-of-window.png) |
| 4c/4d Terminal under a real pointer | ![terminal](https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/real-input-checks/06-terminal-real-pointer.png) |
| 5 Steer/queue under a held ⌘ | ![steer](https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/real-input-checks/07-steer-queue-morph.png) |

## Records updated

`20261005-interface-font-size.md` (its two real-input rows), `20261007-adopt-main-fixes-r3.md` (its final
round restored, and its real-input rows), `issues/README.md` (X8, X47), the X8 issue file.

## Next action

Review the PR. Debug the terminal's real-pointer drag selection (QUEUE.md).
