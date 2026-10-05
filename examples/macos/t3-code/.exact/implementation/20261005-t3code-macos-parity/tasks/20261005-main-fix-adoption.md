---
name: 20261005-main-fix-adoption
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
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

Parent specification: [spec](../spec.md). Source: committed `EXACT2-GAPS.md` and the clone's
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
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged first: group 1 changes shared files that this ticket also edits (review finding: sequence it first) | pending |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |

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

Planned.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after the clone-on-main PR merges.
