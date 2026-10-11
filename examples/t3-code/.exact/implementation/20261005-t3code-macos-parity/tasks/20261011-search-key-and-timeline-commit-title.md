---
name: 20261011-search-key-and-timeline-commit-title
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

## Next action

Start now.
