---
name: 20261010-realinput-1010e-followups
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-realinput-1010e-followups
pr_url: @@PR@@
verified_commit: null
---

# Findings of the real-input session realinput-1010e

## Outcome

The session `realinput-1010e` ran the real-input steps of #398 (realinput-1010c-native) and #399 (realinput-1010c-fixes)
on the bundle of `a1ade42f9`, plus #308's retry. Passed: RC-1 step 1, RC-2, RC-3, RC-4 (b)–(d), RC-6, RC-8. This task
takes what did not pass, and two new observations. Each row is compared with the reference first; a row that matches it
closes as such.

## Findings

| Id | From | Clone under real input | Evidence |
| --- | --- | --- | --- |
| RE-1 | #398 RC-1 step 3 | Not run: the reference shows its permission helper only when packaged (`app.isPackaged`), and the audit's reference runs from source. The open question is whether the reference's first click on its helper (System Settings front) only activates it (Electron's documented `acceptFirstMouse` default, false) or also clicks through. | [E1](https://raw.githubusercontent.com/ccheever/exact2/5113cc7d755271906d63b7efc8c8472eacbbd96d/realinput-1010e/E1-398-rc1-step3-reference.txt) |
| RE-2 | #398 RC-1 step 2 | A drag from the helper's row starts on the first press, but T3 Code becomes the front app only on release. After the next click on System Settings the helper closes and does not come back; the record expects it to stay docked (after step 1's click it did come back). | [notes](https://raw.githubusercontent.com/ccheever/exact2/d2a861e62ae350a3dffff37583c00397edf95c72/realinput-1010e/E3-398-notes.txt), [input log](https://raw.githubusercontent.com/ccheever/exact2/255928f3810222129fdb0f65f89944bf99d44aea/realinput-1010e/E3-398-rc1-r9-input.log), [after drag](https://raw.githubusercontent.com/ccheever/exact2/3c0dc690cb41698843e4f6317d39e542dce78d4e/realinput-1010e/E3-398-rc1-drag-after-redacted.png) |
| RE-3 | #399 RC-4 (a) | With the caret after "now" (position 20), a click on the skill chip moves the caret to 0, before the chip. After Escape closes the details no caret is painted, and typing gives `x$frontend-design now` (2 of 2). | [typed](https://raw.githubusercontent.com/ccheever/exact2/7a248636ea42d3fecaf35fdcfe546194628c52f8/realinput-1010e/E4-RC4a-2-FAIL-typed-x-lands-before-chip.png), [no caret](https://raw.githubusercontent.com/ccheever/exact2/88d59b512246d6a67f2d2ad00e26df0aceca3ce7/realinput-1010e/E4-RC4a-2-FAIL-no-caret-after-now.png), [input log](https://raw.githubusercontent.com/ccheever/exact2/dddf6ac3548562932718caa1adbe9b38e29bbfa0/realinput-1010e/E4-RC4-r9-input.log) |
| RE-4 | #399 RC-6 (new) | After Cancel by mouse in the browser import wizard, the focus goes nowhere instead of back to Add profile. | [notes](https://raw.githubusercontent.com/ccheever/exact2/a18ab23126a27ce4804478282e641fb8734d3f02/realinput-1010e/E4-399-notes.txt) |
| RE-5 | #308 | PR #168's Code tab in the GitHub lane answered "The server returned HTTP 503." in three sessions (1010d, 1010e), so the gutter drag never ran. | [E2](https://raw.githubusercontent.com/ccheever/exact2/addf3a13effcf856f8ce10eec7fbd867fff1e6f2/realinput-1010e/E2-308-503.png) |
| RE-6 | #399 RC-5 | On a fresh Usage view the unpriced popover stays open on slow steps, a quick move and rests across the gap, but entering from the (i)'s left edge closes it as soon as the pointer leaves the (i). Twice earlier, a bad state: any 1–2 pt move on the (i) closed it for about 250 ms, and later hover stopped opening it until a click and Escape. Not reproduced on three tries. The session pressed ⌥⌘T for a trace, which is not the binding, so no trace was written. | [bad state](https://raw.githubusercontent.com/ccheever/exact2/5eeb9e3df58a0cada2dc728cd6f24f03438d06f1/realinput-1010e/E4-RC5-bad-state-flicker-on-2pt-move-within-i.png), [closes on first step](https://raw.githubusercontent.com/ccheever/exact2/0457d3dfcc1099f87db9802703ca17a711eb602c/realinput-1010e/E4-RC5a-bad-state-closes-first-step.png) |

## Steps

- RE-1: settle the reference's first click without packaging all of T3 Code if a smaller probe answers it. Option A: a
  minimal Electron app on the reference's own Electron binary (`target/t3-ref/src-1e2ecbd975`'s `node_modules/electron`)
  that shows a `BrowserWindow` the way the reference's helper does (`showInactive`, the same options) beside another
  front app, and logs whether a single click reaches the page. Option B: run the reference packaged (its app folder in a
  copy of `Electron.app/Contents/Resources/app`, so `app.isPackaged` is true) in an audit lane. The real click itself
  is a real-input row; write its exact steps. If the reference's first click only activates, return `acceptsFirstMouse`
  false on the clone's helper row and close button (match the original; the user's rule since #398).
- RE-2: read the reference helper's show and hide rules (the docking condition, `!frontmost && !isFocused`, and what a
  drag does) and compare them with the clone's after a drag session ends. Fix where the clone differs. Real-input steps.
- RE-3: compare with the reference's composer (a click on a skill chip: where the caret goes, and after its popover
  closes). Fix the clone's caret on chip click and after Escape. A unit or agent test where agent input reaches it.
- RE-4: compare the reference's wizard (a Base UI Dialog returns the focus to its trigger on close). Fix if different.
- RE-5: find which request answers 503 (the lane server's log) and whether the reference answers the same in the same
  lane config. A lane setup problem is fixed in the lane's setup notes for the next session; a clone bug is fixed here.
- RE-6: read the hover code for the bad state (timers, the ~250 ms close, a stuck hover flag) and the left-edge entry.
  Fix what the code shows. The next session's trace step uses `kill -USR1 <pid>` or the development build's ⌥⇧T
  (CLAUDE.md), not ⌥⌘T.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| RE-1..RE-6 | reference comparison first; a unit or AppKit test of the cause where one can be written; agent drive where agent input reaches it; exact real-input steps for the next session (they stay open until it runs) | before / after image where agent mode shows it |

## Next action

Start now.
