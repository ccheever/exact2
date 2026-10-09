---
name: 20261010-realinput-1010-fixes
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Two bugs the 2026-10-10 real-input batch found in merged audit fixes

## Outcome

The real-input batch `realinput-1010` (normal launches of lane copies, real keys and pointer) passed #361's and #357's
rows and most of #355's and #359's. Two steps failed. This task fixes both, as the reference does.

## Findings

| Id | Reference (T3 Code `1e2ecbd975`) | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| RI-1 (#355, PA-2) | On "Open a surface", a letter opens its surface (B, T, F, D, P, L, M), also after the panel was closed and reopened. | Real arrows and Return work, but after closing the Files tab and pressing ⌘⌥B again, a real F does nothing (`f`, `F`, a clean repeat). The launcher still had the keys: ↓ moved its highlight right after. | "Audit work thread"; ⌘⌥B; ↓×3 Return (Files opens); close the Files tab; ⌘⌥B; F. | [r1-pa2-letter-f.png](https://raw.githubusercontent.com/ccheever/exact2/ae3a0188f58b0c0983e0ffd0dc63aae82c684325/realinput-1010/r1-pa2-letter-f.png) |
| RI-2 (#359, PG-9) | A click on the helper's "T3 Code" row reveals the app in Finder, and Finder comes to the front (the helper then hides, as it does whenever System Settings is covered). | The click opens Finder's window with the app selected, but Finder stays behind System Settings and the helper does not hide (two clicks). | SnapShots › Set up › Allow (Screen Recording); click the helper's "T3 Code" row. | [r4-pg9-step2-row-click-reveal.png](https://raw.githubusercontent.com/ccheever/exact2/7ddcff1e7ff21eb8c634f1225254ce733388f2a4/realinput-1010/r4-pg9-step2-row-click-reveal.png), [readback](https://raw.githubusercontent.com/ccheever/exact2/f175842b4a97dca827ac518775eca74bff5a1b3e/realinput-1010/r4-pg9-step2-readback.txt) |

| RI-3 (#359, PG-9, check) | The reference's helper closes when its grant poll sees the grant; the setup row reads the same API. | In the attended run the user saw the Screen Recording helper close and T3 Code come front, while the setup row kept "Allow" until the end (the macOS relaunch case). Check that the helper's grant poll and the setup row use the same check, as the reference does; change nothing if the reference behaves the same. | Attended only: revoke, Allow, drag the row into Screen Recording; read the helper and the row. | [screen-recording read-back](https://raw.githubusercontent.com/ccheever/exact2/447a52027fee82452706d39f1568c8611bdc1938/realinput-1010/r5-pg9-attended-screen-recording.png) |

## Context and guidance

- RI-1: `R8KeysLauncher.consume` and the launcher's `ui("key", k)` (`shell-panels.contract`, `r8-keys*.ts`,
  `#355`'s focus-on-mount). The arrows reach the launcher after the reopen; check how a letter is consumed versus an
  arrow after a reopen (a stale focused-node id or a consumed-once flag). Reference: `RightPanelTabs.tsx` `handleKeyDown`.
- RI-2: `modules/apple/T3PermissionHelper.swift` (the row's click: `NSWorkspace.activateFileViewerSelecting`). The panel
  is non-activating; Finder may need an explicit activation (`NSRunningApplication` for `com.apple.finder`
  `activate()`), as the reference's `shell.showItemInFolder` brings Finder front. Reference:
  `apps/desktop/src/permissions/MacPermissionHelper.ts`.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| RI-1 | Bun/AppKit test of the reopen path; agent drive (⌘⌥B, close, ⌘⌥B, F opens Files); real F in the next batch | before / after image |
| RI-3 | code comparison with `MacPermissionHelper.ts`; unit test of the shared check | text |
| RI-2 | AppKit test that the click activates Finder; real click in the next batch (Finder front, helper hidden) | text (front app read-back) |

## Next action

Build after the in-flight wave (it touches the launcher and the helper only).
