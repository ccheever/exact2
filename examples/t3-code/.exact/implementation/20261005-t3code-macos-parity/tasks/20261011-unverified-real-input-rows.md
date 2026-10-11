---
name: 20261011-unverified-real-input-rows
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

# Rows never verified under real input: make the ones that can run ready for one final session

## Outcome

STATUS "Known differences" row 11 (the user asked on 2026-10-11 to fix what can be fixed now). The rows below never ran
under real input. Rows that wait for main PR #327 (X62) stay out: #261's reaction pill tooltip, #307's Usage popover
email tooltip and the press on its padding, #311's PR-link card.

| Id | Row | Why it never ran |
| --- | --- | --- |
| UV-1 | #311, the Code tab's age tip: under a real pointer it did not show on the label while the author tip on the same card did ([image](https://raw.githubusercontent.com/ccheever/exact2/1e242b9575b0549cf907453414401cb7e99b1372/realinput-1009/age-tip-real-pointer.png); "Hermes formats its date; with the reaction tooltip, to investigate") | Not investigated |
| UV-2 | #311, the Code tab's viewed-error tip (a fixture that fails only the viewed read), the short and withheld tips (a file the sandbox does not have), the viewed-here tip (a non-GitHub host) | No fixture |
| UV-3 | #311, the commit link in a comment (onto it, away, a click) | The fixture comment needs a linked SHA |
| UV-4 | #346, a muted tab behind another keeps its muted indicator, and its media plays on when shown again | Fixed in `b6b3e417e`, AppKit-tested, not driven by real input |

Details: [pr-links-previews-and-routing](closed/20261005-pr-links-previews-and-routing.md) "Real-input batch steps" A and B;
[browser-surface-automation](closed/20261005-browser-surface-automation.md) "Real-input batch steps" 4; STATUS "Next
real-input batch".

## Steps

1. UV-1: reproduce in agent mode with the lane's GitHub server (`tools/github-lane`) and read the hover path; compare with
   the author tip on the same card and with the reference (CDP). If the cause is the clone's, fix it; if it is X62 (main
   #327), say so with the evidence and leave it for round 9.
2. UV-2, UV-3: build the fixtures. GitHub writes only in the sandbox `daehyeonmun2021/playground` (a comment with a linked
   SHA, a short or withheld file); the viewed-error tip and a non-GitHub host through the lane's fixture proxy if the clone
   can be pointed at one without code changes, else a test plus the reason the row cannot run live. Keep the fixture
   scripts under `target/` and name them in the record.
3. Drive each row once in agent mode (hover by hit test) and write exact "Real-input batch steps" for UV-1..UV-4 (lane,
   fixture commands, steps, what passes).
4. A PR is needed only if code or tests change; otherwise the PR carries the record.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| UV-1 | cause found; fix with tests, or X62 evidence | before / after / reference images if fixed |
| UV-2..UV-4 | fixtures ready; agent-mode drive; real-input steps written | agent-mode images |

## Next action

Start now.
