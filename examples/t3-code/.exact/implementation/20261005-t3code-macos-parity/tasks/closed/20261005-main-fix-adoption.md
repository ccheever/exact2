---
name: 20261005-main-fix-adoption
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-main-fix-adoption
pr_url: https://github.com/ccheever/exact2/pull/155
verified_commit: null
---

# The clone uses main's fixes instead of its workarounds

## Outcome

Every clone workaround for an exact2 limit that `main` has since fixed is removed, and the
user-visible behavior is the same or closer to the reference. `EXACT2-GAPS.md` is re-checked
on the pinned main: each remaining gap has current evidence, and each fixed one moves to
"Already fixed on exact2 main" with the workaround removal recorded.

## Scope and exclusions

Included — workarounds the clone docs list as fixed on main (`EXACT2-GAPS.md` "Already fixed
on exact2 main"; draft plan `p0-cleanup`):

| Fixed on main | Clone workaround to remove | Proof |
| --- | --- | --- |
| `pointer-events: none` on boxes, visible-overflow hit testing (`652a6c865`, `c44607c7d`) | about 20 `inert=true` uses and negative-margin parents | coordinate tap on each affected control hits the intended target |
| `cursor` keywords on macOS (`c6136f39d`) | none or partial; add the reference cursors (sidebar resize edge, links, buttons) | `layout <id>` shows the cursor row; real-input check S1 |
| Popover top/bottom placement with `position-area` (`2c6b551ba`) | fixed placement code | popover opens on the reference side near window edges, both sizes |
| Key events with modifiers, `preventDefault()`, bubbling, `tabindex` (`f35b3eafc`, `8a0afbeab`, `d672f9372`) | native key hooks that only existed for these (keep hooks still needed for X15/X20/X25) | row keyboard context menu (handoff gap #1 "rows have no keyboard context menu") works; AppKit key tests green |
| Awaiting another answer's fetch (`f96641ddd`) | any app-side wait shim | the reply-ownership scenario still passes |
| `title` maps to a native tooltip (`2bfebe63e`) | authored tooltip boxes that only mimic `title` | tooltip text and delay match the reference |

Plus: re-check every remaining `EXACT2-GAPS.md` item (X3–X30) on the pin with a minimal
reproduction or the clone's current behavior, and update its "Current state" line.

Excluded: framework changes (missing support is filed as an issue through `issue-open`); gaps that are
still open (they stay documented with their workaround).

## Context and guidance

Parent specification: [spec](../../spec.md). Source: committed `EXACT2-GAPS.md` and the clone's
README section "Framework limits worked around" (mc-orch tree).
Library revision: `20261005-platforms-v3`. Selected topics: layout-and-interaction (a layout box
is not proof of a tappable center; assert behavior), testing-and-debugging ("button that does
not respond" recipe: tree/state/layout/logs before and after `tap`), accessibility (focus and
labels), design (complete UI states).
Do not open framework source while planning; at implementation, the repository instructions
apply.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | #147 | Merged first: group 1 changes shared files that this ticket also edits (review finding: sequence it first) | merged into `feat(example)/t3-code` (`7f692c9a1`); this branch is rebased on it |
| merged task PR | [20261005-clone-on-exact2-main](../20261005-clone-on-exact2-main.md) | pending | Merged | pending |

Scheduling preference (not a prerequisite): finish before the feature tickets that edit the
same Contract files (sidebar, popovers, tooltips) to limit conflicts.

## Issue assessment at preparation

Checked sources and time: {{at prepare}}. Each still-open X item gets a plan issue record only
when a ticket depends on it.

## Implementation notes

Remove one workaround per commit with its proof, so a regression can be bisected.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Each removal keeps behavior | Fixture backend with the round-11 thread fixture | Per row above: agent `tap` at the control's coordinates, `layout`, `state`, screenshots | Same result as before removal or closer to the oracle | macOS 1280×840 and 840×620 | `--json` transcripts, before/after png |
| Row keyboard context menu | Sidebar with threads and a draft row | Focus a row, press the reference's context-menu key | Menu opens with the reference items | macOS | transcript + oracle shot |
| Gap document current | — | Review `EXACT2-GAPS.md` | Every X item has a current-state line dated on the pin | — | diff |
| Regression gates | — | Clone checks, AppKit binaries, matrix at both sizes and appearances | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs, matrix |

Task-owned source paths: Contract and Swift files that hold the workarounds,
`EXACT2-GAPS.md`, `README.md`.

## Progress

Implemented 2026-10-06 on exact2 main `c12832e82` (the feature branch at `da40e6590`, after
hot-file-split #147, floating-device-player #146 and legacy-sidebar #143). Verification: unverified.
One workaround per commit:

| Fixed on main | Removed or adopted | Proof on the pin |
| --- | --- | --- |
| `pointer-events: none` on boxes, inherited, visible-overflow hit testing (`652a6c865`, `c44607c7d`) | `inert=true` on 20 overlay, tooltip, hover-card, drag-ghost, fade and probe layers → `pointer-events="none"`; the composer controls row's margin -10 / padding 10 and the footer's `- 10` | ExactKit `ClipMacTests` (in 795 tests, 0 failures); live: `layout model-picker` x 0 → -10 (outside its row) and a mouse press at (3,14) still opens the model menu, before and after |
| `cursor` keywords (`c6136f39d`) | pointer on the shared button styles, links, sidebar and legacy-sidebar buttons; `w-resize` sidebar rail; `col-resize` right panel edge | `CollectionMacTests.testCSSCursorKeywordsReachAppKit`; real pointer: unverified (attended) |
| `position-area` (`2c6b551ba`) | SnapShot Accessibility data and Usage unpriced popovers use `position-area="top"` | `PositionAreaTests`, `ChooserMacTests.testTheMenuPopsUpByItsPositionArea`; live: not reached (needs a SnapShot attachment / usage data) |
| Key modifiers, `preventDefault()`, bubbling, `tabindex` (`f35b3eafc`, `8a0afbeab`, `d672f9372`) | the draft row's hidden `aria-keyshortcuts="Shift+F10"` button → its own `key` handler; the right-panel tab's native Shift+F10 monitor → the tab buttons' `key` handler | `r12-sidebar.test.ts` (Shift+F10 rows), AppKit `contextmenu` 11/0 (new pass-through test), `r12-sidebar` 3/0, `r11-upstream` 3/0; live: not reached (see attempts) |
| Awaiting another answer's fetch (`f96641ddd`) | none: `readDetail` keeps per-answer reads because a let-go answer's replies are still dropped (#109) | `cargo test -p exact-js --test it an_answer_awaiting_another_answers_fetch` 1/0 |
| `title` → native tooltip (`2bfebe63e`) | `title` where the reference has it: pending question toggle and Dismiss, project group settings, license Project source, device rail text-size and more-actions triggers | host `PresenterMac` sets `toolTip` from `title`; hover tooltip itself unverified (attended) |

Kept: the window-level sidebar rail and edge tips (paint order under the chat column, not only
hit testing), every workaround for #100–#141 (listed in `EXACT2-GAPS.md` "Current state on the
pin"), the rename field's Escape monitor (#140: window shortcuts are heard before a field's key
handler). The row keyboard context menu already existed (r12-sidebar); the handoff gap line was stale.

Not run: the 840×620 size and dark appearance live, the oracle and trace-diff rows
(desktop-oracle-and-trace will not be built), real pointer/keyboard checks (cursor shapes,
tooltip delay, physical Shift+F10): unverified (attended).

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-06) | `0c9a3749c` on `da40e6590` | `bun test examples/t3-code` 1321/0 (base 1321); strict tsc clean; contract build 2182 slots, 43 resources; `cargo test -p t3-code-macos --lib` 10/0; AppKit `contextmenu` 11/0, `r12-sidebar` 3/0, `r11-upstream` 3/0; ExactKit 795/0 (2 skipped); five checks pass (cargo test 2928/0, 18 ignored; clippy, fmt, caps, boot); macOS bundle builds | drive records below; PR screenshots | live draft-row and tab Shift+F10 not reached |

Live drives (one BEFORE on `t3-code-evidence-base` `da40e6590`, one AFTER on this branch; same
steps, lane server on 16120 with the fixture project added by `t3 project add`). A first pair
stopped at the Projects step's disabled "Import 0 projects" (`serve` ignores
`--auto-bootstrap-project-from-cwd`), and an earlier partial BEFORE drive ran about a minute
without the drive lock before it was stopped. Records of the final pair (AFTER; BEFORE equal
except where noted):

```
ok pair · ok no import candidates: continue without importing · wizard gone: true
layout model-picker: {"x":-10,"y":2,"w":167.53,"h":28}      (BEFORE: "x":0, inside the padded row)
ok mouse press model-picker at 3,14 {"tapped":186,"at":[3,14],"delivery":"platform"}
model menu open after the edge press: true                   (BEFORE: true)
ok hover new-thread → tooltip "New thread (⇧⌘O)" shown        (BEFORE: same)
ok press add-project under the tooltip layer {"tapped":71,"at":[14,14]} → Add project dialog
FAIL type a draft: view 147 is hidden or inert (the Add project dialog stayed open; drive design)
```

## Next action

Review the PR. A later attended session checks the cursors, the `title` tooltips and a physical Shift+F10 on a draft row and a right-panel tab.
