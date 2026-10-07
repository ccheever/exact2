---
name: 20261007-terminal-real-drag
plan: 20261005-t3code-macos-parity
implementation: verified
verification: passed
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-terminal-real-drag
pr_url: null
verified_commit: null
---

# The two findings left open by real-input-checks, run to ground

## Outcome

[20261007-real-input-checks](closed/20261007-real-input-checks.md) (PR #213) left two findings open: a real
mouse drag in the terminal selected nothing in either build, and the branch's relaunch showed "Environment
disconnected" for about 30 s where the base reconnected within 8 s. Both were found to come from how the drive
was run, not from the app. With the cause removed, both behave as T3 Code does in both builds. No app code
changes; X8 is adopted and moves to `issues/closed/`.

## 1. Terminal drag selection with a real mouse

**Cause.** The earlier drives started the drag while T3 Code's selection popup (the native Add to chat / Copy
menu that a selection or a double-click opens, `T3TerminalActions.show` → `NSMenu.popUp`) was still open. The
drive's Escape came from `cliclick kp:esc`, which did not close the menu: the accessibility tree still listed
"Add to chat" and "Copy" after it (14:57:09). A press while a native menu tracks goes to the menu, which closes
and swallows it, so the page saw no `pointerdown` and the drag selected nothing. The very first Before drags had
a second fault: they started at window x 215, which is the sidebar (the drive scaled a displayed-image
coordinate wrongly). T3 Code shows the same popup through Electron's native menu (`ThreadTerminalDrawer.tsx`,
`localApi.contextMenu.show`), so a press that closes it is consumed there too: this is parity, not a bug.

**How it was found.** A bare window hosting `T3TerminalView` (the AppKit test setup) with a real HID drag posted
from outside selected and opened the popup, so the view and WebKit were not at fault. A traced lane build (a
temporary log of every left-mouse event with its hit view and first responder, and of every page↔native
message; not committed) then showed, in the app: down, drags and up all hit the `WebView`, the page sent
`selection` messages that grew with the drag, and `selection-ready` at release. A drag whose press arrived while
the menu was open had no `leftMouseDown` in the trace at all.

**Re-check with a real drag** (lane bundle copies, the same lane server, 2026-10-07 14:56–14:58, under the drive
lock; Before is `t3-code-evidence-base` at `4f523ef5c`, After is `feat(example)/t3-code` at `887b2491b`, whose app
code equals `ca398fe0c`): with no menu open, a `cliclick` drag inside the terminal and a drag that leaves the
window below and comes back both select and open Add to chat / Copy, in both builds; an `orca computer` Escape
closes the menu (the tree no longer lists its items).

No test was added: no app code changed, and the existing AppKit drag test
(`macos/tests/terminal/pointer.swift` `testDragSelectsText`) covers the view. The terminal AppKit binary passes
(38 tests, 1 skipped, 0 failures).

## 2. "Environment disconnected" after a relaunch

**Cause: a macOS Keychain prompt, specific to the lane.** All lane copies are ad-hoc signed with different code
signatures but share one Keychain item (`com.exact.t3code.macos.access-token`, account `origin + environment`):
the item is created by whichever copy paired first, and another copy reading it waits for SecurityAgent's "wants to
use your confidential information" prompt. The unified log shows `SecurityAgent[48862]` spawned at 14:07:52, the
second the branch copy relaunched into its 30 s disconnect (the item had been created by the Before copy at 13:48);
and in a later loop the frontmost process during each slow launch was SecurityAgent (pid 82612 in the log), not the
app. Once the copies were in the item's access list (it lists all three lane copies), every relaunch connected in
about 1 s.

**Measurements** (real held ⌘Q, relaunch of the same bundle copy, polled every ~0.5 s for a thread row with no
"Environment disconnected"):

| Run | Branch copy | Base copy |
| --- | --- | --- |
| first relaunch (14:07) | ~30 s (Keychain prompt) | 8 s |
| loop 1 (14:50–14:53) | > 60 s ×3 (SecurityAgent frontmost) | 1.0 s ×3 |
| loop 2 (14:54–14:55), alternating | 1.0 s ×3 | 1.0 s ×3 |

Not an app difference. A user's single signed app owns its item; a person moving between differently signed
builds of the clone (a development and a release build) would see the same prompt once.

## Evidence

| Scenario | Image |
| --- | --- |
| Terminal real drag inside, and out of the window and back (Before / After) | ![drag](https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/real-input-checks/08-terminal-real-drag-select.png) |

## Checks

Docs only. `bun scripts/caps.mjs` (after `git add -A`); the terminal AppKit binary (38 tests, 1 skipped, 0
failures). `bun test examples/t3-code` 2298 tests, 0 fail, 1 skip; strict tsc (`--target ES2023 --lib ES2023,DOM`) clean; `contract build` 2543 slots, 45 resources.

## Next action

Review the PR.
