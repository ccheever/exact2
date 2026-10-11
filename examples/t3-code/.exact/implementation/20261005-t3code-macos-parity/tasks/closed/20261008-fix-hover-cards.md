---
name: 20261008-fix-hover-cards
plan: 20261005-t3code-macos-parity
implementation: done
verification: partial
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-fix-hover-cards'
pr_url: https://github.com/ccheever/exact2/pull/307
verified_commit: 7fe081148
---

# Tooltips and hover cards stay inside the window and stay open under the pointer

## Outcome

Five clone bugs from the real-input batch (#298) share one cause and one fix:

| Batch bug | Task | Symptom |
| --- | --- | --- |
| 1 | [provider-settings-upkeep](20261005-provider-settings-upkeep.md) (#251) | The update icon's tooltip, in the provider list and in the editor, opens above the icon and is cut off by the card |
| 2 | [this-machine-network-access](20261005-this-machine-network-access.md) (#248) | The "N scopes" popover opens above and is cut off by the Authorized clients list |
| 14 | [pr-header-actions-and-stacks](20261005-pr-header-actions-and-stacks.md) (#262) | The base branch's hover card closes as the pointer moves into it |
| 10 | [usage-pooled-view](20261005-usage-pooled-view.md) (#263) | The segment's hover popover closes as the pointer moves into it, so the email cannot be revealed (moved here from #290, coordinator 2026-10-08) |
| 9 | [settings-model-picker](20261007-settings-model-picker.md) (#246) | No tooltip on a model row that is unavailable on another environment (moved here from fix-misc-batch, coordinator 2026-10-08) |

The reference (T3 Code `1e2ecbd975`) portals every `TooltipPopup` and `PopoverPopup` to the body (`ui/tooltip.tsx`,
`ui/popover.tsx`) and places them with Base UI's side, align, sideOffset 4 and collision flip; an `openOnHover`
popover closes through `safePolygon()` and its `closeDelay` (`PopoverTrigger.js`, Base UI 1.5.0), so the pointer can
cross into it.

## Cause

- **Bugs 1 and 2: clipping.** The tooltip (`CnTooltip`) and the scopes card were absolute children of their trigger,
  inside a `scroll` (the provider list, the editor, the Authorized clients list) and, for the list, `PvGroup`'s
  `overflow="hidden"`. Both scrolls clip. The side was right (top, as the reference); the container was not.
  Contract has no hover-opened top-layer popover (`interestfor` is refused, LLP 1021 §5.1), and `position: fixed`
  is not in the vocabulary, so the clone must draw them at the window level itself.
- **Bugs 14 and 10: the pointer cannot reach the card (X62, framework: [#322](https://github.com/ccheever/exact2/issues/322),
  fixed by [#327](https://github.com/ccheever/exact2/pull/327) (open)).** Probed with a one-file app and a real
  pointer (`issues/20261008-x62-hover-outside-the-box.md`): on macOS a `hover` handler is an `NSTrackingArea`
  clipped by every ancestor's bounds, and the presenter keeps one hovered node. A card drawn beside its trigger
  lies outside the trigger's box, so the trigger hears a leave as the pointer moves into the card (the card then
  unmounts or goes inert) and the card itself hears no pointer. The web counts overflowing descendants
  (`pointerenter`/`pointerleave`), so the same Contract works there. The clone also had no close delay: Contract
  timers are root-only (`task`), so the base's comment "the reference's 120 ms close delay is not timed".
  Once #327 is adopted, only this X62 part of the layer's reason changes: the layer stays, as it also fixes the
  clipping of bugs 1 and 2 and matches the reference's portal.
- **Bug 9.** The row had `title=row.reason`, a native title the macOS host never shows. The reference's
  `ModelListRow` wraps a disabled row in a `Tooltip` with `TooltipPopup side="left"` (`disabledReason`, text from
  `useScopedModelAvailability.ts`: "This model is unavailable on … Select that environment to choose its model
  separately."), outside the picker popover that would clip it.

## What was built

- **The hover layer** (`hover-layer.contract`, new). `HoverTip` (key, kind, words or lines, side, align, the
  trigger's frame, a size estimate, testId), `hoverTipAtFrame`, `hoverDelay(kind)` (Base UI's closeDelay: scopes
  100, freshness 120; the Usage popover has none but needs a 50 ms crossing; a tooltip 0), `hoverFlip` (Base UI's
  flip, best fit) and `HoverLayer`: placed on top, bottom, left or right of the trigger with sideOffset 4, the
  card's offset part of its hover box (no gap to cross), only the popup taking the pointer. `HoverText`
  (TooltipPopup's look) and `HoverScopes` (AccessScopeSummary's tooltipStyle card, its 250 ms delay a fade).
- **T3Window** (`app-window.contract`) holds the state: `hoverTip`, `hoverOn` ("trigger" or "card"), the clock mark
  at the last enter, the close-delay end; `hoverShown` (shown while hovered and the clock has not moved, else until
  the close delay ends); the action `hoverTipAt(tip, part, inside)`. It provides `hoverTipAt`, `hoverTip` and
  `hoverShown`, and draws `HoverLayer` after `WindowOverlays` in the SSH-inert row (kind "usage" excepted).
- **The root** (`app.contract`) keeps only the hover clock, +10 lines over the base: `hoverWait`, `hoverEnd`,
  `task hoverTick` (+10 every 10 ms while a close delay runs), `task hoverSurface` (+10 when Settings, its route,
  the utility page, a modal or the selected pull request changes: a tooltip whose trigger went with no leave ends),
  `hoverTicked`, `hoverHold(end)`. app.contract: 1,488 lines after merging `0e2901aec`.
- **Bug 1:** `ProviderVersionAdvisory` (`providers-upkeep.contract`) hands the icon's frame to the layer (tip, side
  top, centred); a press closes it (closeOnClick).
- **Bug 2:** `NetScopes` (hover and focus), `NetDot`, and `PairingLinkRow`'s expiry and share-URL tooltips
  (`connections-network.contract`): every tooltip inside the Authorized clients scroll area, the same clipping.
- **Bug 14:** `PaFreshnessMark` (`pages-pr-actions.contract`) hands the mark's frame to the layer (kind freshness,
  side bottom, align start); T3Window draws `PaFreshnessCard` from the open pull request (`prDetail.actions`,
  `prAct`), so its buttons work from the layer. `PaOutlineButton` reports its hover to the card (`keep`), as the
  host hands the hover to the button. The pressed popover is unchanged; its buttons' testIds are now
  `pr-freshness-popover-<method>` and the hover card's `pr-freshness-hover-<method>`.
- **Bug 10:** the Usage page draws the segment popover once, in its scroll content (`UsagePopups` in
  `usage-pooled.contract`, `id="usage-content"`), at the segment's rect taken when it opens (`place`), its card
  inside every box around it so the host tracks the pointer on it; it stays mounted for the segment last shown, so
  the fade-out keeps its place, and a fresh one fades in after Base UI's 300 ms. The segment is the trigger, the
  popover and its email the card, through the window hover state (kind usage, 50 ms crossing); Escape and "Use
  reset" close it at once. `pinned` and the press are unchanged (#290 owns them). A press outside the page (the
  window's count, #290's `outside`) closes a hover-opened popover too, as Base UI's outside press does however it
  opened (`hoverAt`, found by the real-pointer session: an enter can reach a segment the pointer is not on, X62).
- **Bug 9:** `ModelRow` (`model-picker.contract`) hands the row's frame and reason to the layer: a tooltip, side
  left, centred, flipping right where the left has no room; its width estimate is the words at 12 px up to 20rem.
- **Tests:** `hover-layer.test.ts`, 11 tests reading the sources (as `dialog-focus.test.ts` does); all 11 fail on
  `ec32c8c37`'s sources and pass here.

## How to use the hover helper

- Inject it: `inject hoverTipAt: action` (and `rem` when the estimate needs it) in any component under T3Window.
- A trigger's hover (or focus) calls
  `hoverTipAt(hoverTipAtFrame(key, kind, text, lines, side, align, frame(triggerId), estimate, testId), "trigger", inside)`.
- A card's own hover, and any hover node inside the card, calls the same with `"card"`.
- Kinds: `"tip"` closes with its trigger's leave; `"scopes"` (100 ms), `"freshness"` (120 ms) and `"usage"` (50 ms
  crossing) stay for their close delay; add a kind to `hoverDelay` and, if T3Window must draw new content, a branch
  under `HoverLayer` in `app-window.contract`.
- Sides: top, bottom, left, right, each flipping to the opposite side as Base UI's flip does (`estimate` is the
  height for top and bottom, the width for left and right).
- A surface that draws its own card (the Usage page, in its scroll content) reads `hoverTip`/`hoverShown` and is
  skipped by T3Window's layer (kind "usage").

## Acceptance

| Row | Result | Proof |
| --- | --- | --- |
| Bug 1: the list and editor update icons' tooltip whole, above the icon (agent) | pass | [01](https://raw.githubusercontent.com/ccheever/exact2/e780f6b871a414f7981cfdf0a9536cffe80d1b77/fix-hover-cards/01-update-icon-tooltip-list.png), [02](https://raw.githubusercontent.com/ccheever/exact2/e780f6b871a414f7981cfdf0a9536cffe80d1b77/fix-hover-cards/02-update-icon-tooltip-editor.png); [drive record](https://raw.githubusercontent.com/ccheever/exact2/e780f6b871a414f7981cfdf0a9536cffe80d1b77/fix-hover-cards/agent-drive-record.txt) |
| Bug 2: "5 scopes" whole, above the count; the pointer can cross into it and it stays; it closes 100 ms after leaving (agent) | pass | [03](https://raw.githubusercontent.com/ccheever/exact2/e780f6b871a414f7981cfdf0a9536cffe80d1b77/fix-hover-cards/03-scopes-card.png), [04](https://raw.githubusercontent.com/ccheever/exact2/e780f6b871a414f7981cfdf0a9536cffe80d1b77/fix-hover-cards/04-scopes-card-pointer-inside.png) |
| Bug 14: the freshness card stays as the pointer crosses into it and while on its button; closes after leaving (agent) | pass | [05](https://raw.githubusercontent.com/ccheever/exact2/e780f6b871a414f7981cfdf0a9536cffe80d1b77/fix-hover-cards/05-freshness-card.png), [06](https://raw.githubusercontent.com/ccheever/exact2/e780f6b871a414f7981cfdf0a9536cffe80d1b77/fix-hover-cards/06-freshness-card-pointer-inside.png), [07](https://raw.githubusercontent.com/ccheever/exact2/e780f6b871a414f7981cfdf0a9536cffe80d1b77/fix-hover-cards/07-freshness-card-pointer-on-button.png) |
| Bug 10: the Usage popover stays as the pointer crosses into it; its email's tooltip shows; closes after leaving (agent) | pass | [08](https://raw.githubusercontent.com/ccheever/exact2/e780f6b871a414f7981cfdf0a9536cffe80d1b77/fix-hover-cards/08-usage-popover.png), [09](https://raw.githubusercontent.com/ccheever/exact2/e780f6b871a414f7981cfdf0a9536cffe80d1b77/fix-hover-cards/09-usage-popover-pointer-inside.png) |
| Bug 9: an unavailable row's reason as a tooltip left of the row, beside the picker (agent) | pass | [10](https://raw.githubusercontent.com/ccheever/exact2/e780f6b871a414f7981cfdf0a9536cffe80d1b77/fix-hover-cards/10-unavailable-model-row.png) |
| The same rows on the merged heads `6ef66bc93` and `5752b7be2` (after #290) (agent) | pass | [settings](https://raw.githubusercontent.com/ccheever/exact2/92e5b5006c23973be989618a2faa9d6e273293d1/fix-hover-cards/merged2-steps-settings-after.ndjson.txt), [pr](https://raw.githubusercontent.com/ccheever/exact2/92e5b5006c23973be989618a2faa9d6e273293d1/fix-hover-cards/merged2-steps-pr-after.ndjson.txt), [usage](https://raw.githubusercontent.com/ccheever/exact2/92e5b5006c23973be989618a2faa9d6e273293d1/fix-hover-cards/merged2-steps-usage-after.ndjson.txt); earlier: [settings](https://raw.githubusercontent.com/ccheever/exact2/e780f6b871a414f7981cfdf0a9536cffe80d1b77/fix-hover-cards/merged-steps-settings-after.ndjson.txt), [pr](https://raw.githubusercontent.com/ccheever/exact2/e780f6b871a414f7981cfdf0a9536cffe80d1b77/fix-hover-cards/merged-steps-pr-after.ndjson.txt), [usage](https://raw.githubusercontent.com/ccheever/exact2/e780f6b871a414f7981cfdf0a9536cffe80d1b77/fix-hover-cards/merged-steps-usage-after.ndjson.txt) |
| Regression tests failing on the base | pass: 11 fail on `ec32c8c37`, pass here | `hover-layer.test.ts` |
| Real pointer (2026-10-08, 10:41-11:22Z): bug 1, the list and editor icons' tooltip above the icon, whole; gone after the glide away | pass | [real-01](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-01-list-icon.png), [real-02](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-02-editor-icon.png); [settings log](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-settings-after.ndjson.txt); [session record](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-session-record.txt) |
| Real pointer: bug 2, "5 scopes" on the count, still there after the glide up into it and 900 ms, gone 500 ms after the glide out | pass | [real-03](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-03-scopes-card.png), [real-04](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-04-pointer-in-scopes-card.png), [real-05](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-05-scopes-card-left.png) |
| Real pointer: bug 14, the freshness card on the mark, still there in the card and on "Update with rebase", gone 500 ms after the glide out | pass | [real-11](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-11-freshness-card.png), [real-13](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-13-pointer-on-update-with-rebase.png), [real-14](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-14-freshness-card-left.png); [pr log](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-pr-after.ndjson.txt) |
| Real pointer: bug 10, the Usage popover opens on the segment, stays after the glide into it (900 ms), inert 400 ms after the glide out; a click on the email reveals it | pass | [real-21](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-21-usage-popover.png), [real-22](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-22-pointer-in-usage-popover.png), [real-24](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-24-email-revealed.png) (the lane fixture's address), [real-25](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-25-usage-popover-left.png); [usage log](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-usage-after.ndjson.txt) |
| Real pointer: bug 10, the email's tooltip on the email | fail: the email's hover box and the card's hover trade the hover on each move and the tooltip unmounts (X62's third case, #322); waits for #327 (STATUS.md) | [real-23](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-23-pointer-on-email.png); `mail-events` in the usage log |
| Real pointer: #290's rows: a press pins the segment; a press in the sidebar closes it; a press, then a second press on it with the pointer resting, closes it and it stays closed after a 2 pt move; at 840x620 the legend row's press shows it below the row (card y 203, row 169-197) and a second press closes it | pass on `7fe081148` (the sidebar press failed on `5752b7be2`: fixed here, see attempt 6) | [real-26](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-26-pinned.png), [real-27](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-27-after-sidebar-press.png), [real-28](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-28-second-press-resting.png), [real-29](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-29-legend-pinned.png), [real-30](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-30-legend-second-press.png) |
| Real pointer: bug 9, the unavailable GPT-6 Luna row's tooltip left of the row, the reference's words; gone after the glide away | pass | [real-31](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-31-unavailable-row.png) |
| #290's rows (agent, `5752b7be2`): a press on the segment pins it; a press elsewhere on the page closes it (#290's light dismiss, the hover state too); hover, then a press pins it and a second press on the hovered segment closes it and it stays closed (the hover waits for a new enter); at 840x620 the legend row's press shows it anchored below that row, not over it, and a second press on the row closes it | pass (agent) | [handed-over log](https://raw.githubusercontent.com/ccheever/exact2/92e5b5006c23973be989618a2faa9d6e273293d1/fix-hover-cards/handed-over-steps-usage-after.ndjson.txt), [26](https://raw.githubusercontent.com/ccheever/exact2/92e5b5006c23973be989618a2faa9d6e273293d1/fix-hover-cards/handed-26-after-press-elsewhere.png), [27](https://raw.githubusercontent.com/ccheever/exact2/92e5b5006c23973be989618a2faa9d6e273293d1/fix-hover-cards/handed-27-second-press-hovered.png), [28](https://raw.githubusercontent.com/ccheever/exact2/92e5b5006c23973be989618a2faa9d6e273293d1/fix-hover-cards/handed-28-legend-pinned.png), [29](https://raw.githubusercontent.com/ccheever/exact2/92e5b5006c23973be989618a2faa9d6e273293d1/fix-hover-cards/handed-29-legend-second-press.png) |
| #281's rule (coordinator, 2026-10-08: "consumption cannot depend on a press handler being present"), real pointer: a press on `PaFreshnessCard` over the Timeline tab (on its text: no padding point lay over a tab) presses nothing under it; a press on the Usage popover's padding (499, 276) over the weekly limit's Codex segment | freshness card: pass (the tabs unchanged, the card stays). Usage popover: fail, as expected before [#327](https://github.com/ccheever/exact2/pull/327): it pinned the segment under it (`usage-seg-0-1-0` expanded); waits for #327 (STATUS.md) | [real-12b](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-12b-press-on-card-padding.png), [real-22b](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-22b-press-on-popover-padding.png) |
| Framework limit recorded | X62, filed as [#322](https://github.com/ccheever/exact2/issues/322), fixed by [#327](https://github.com/ccheever/exact2/pull/327) (open) | `issues/20261008-x62-hover-outside-the-box.md` |

## Real-input batch steps

They ran in one session on 2026-10-08, 10:41-11:22Z (results in "Acceptance"; the session record:
`t3-code-evidence:fix-hover-cards/real-session-record.txt`). They are one automated drive per lane, `target/fix-hover-cards/real.mjs` in this worktree
(uncommitted apparatus, with `drive.mjs`, the lanes and `lane-app/`): the app runs in agent mode, so the driver sets
the screen up and reads the tree back, but every hover is a real pointer (cliclick moves in 3 pt steps, 25 ms apart,
on the window's screen place from `orca computer list-windows`) and real time passes with `clock +N real` after each
move, so a close delay that runs out closes the card. Before the real moves the drive sets the app frontmost through
System Events (NSRunningApplication.activate is refused while another app is frontmost) and clears the agent's
pointer with an agent contact, because in agent mode the host's hit-test of a resting pointer after a layout uses
the agent's pointer, never the cursor (MouseChainMac.restingPointer).

1. Take `…/t3-code/target/t3-ui-parity/lanes/.realinput-lock` (owner "fix-hover-cards: real pointer rows"); the
   screen unlocked; nothing covering the app window.
2. `export PATH="$HOME/.bun-1.4.2/bin:$PATH"`; in `target/fix-hover-cards`: the lanes are running
   (`gh/` server on 16151: `T3_GITHUB_LANE_SHARED=…/t3-code/target/t3-ui-parity/github-lane bun gh/gh-start.mjs` if
   not; `cd usage-lane && bun lane.mjs start`), fresh pairing links (`bun gh/gh-pair.mjs`;
   `cd usage-lane && bun lane.mjs pair a && bun lane.mjs pair b`).
3. `bun real.mjs after out-real <worktree> settings`, then `… pr`, then `… usage`. Read back from
   `out-real/real-<part>-after.ndjson` and the `after-real-*.png` captures:
   - settings: `provider-advisory-list-codex-tip` and `provider-advisory-editor-codex-tip` present after the glide onto
     each icon, their `tip-box` above the icon and whole in the window; absent after the glide away (bug 1).
     `pairing-link-scopes-<id>-popup` present on the count; still present after the glide up into the card and 900 ms
     of real time; absent 500 ms after the glide out (bug 2).
   - pr: `pr-freshness-hover` present on the mark; present after the glide down into the card and onto "Update with
     rebase" (900 ms each); absent 500 ms after the glide out (bug 14). Nothing is pressed.
   - pr (#281): a real press on the card's padding (or, failing a padding point over a control, its text) over a tab:
     `tabs-after` equals `tabs-before` (no tab pressed) and the card stays.
   - usage (#281): a real press on the popover's padding over the next card's bar: no segment there gets
     `aria-expanded`, and the popover stays.
   - usage (#290's rows): a real press on a segment pins it; a real press in the sidebar ("No projects yet") closes it; a press,
     then a second press on the same segment with the pointer resting, leaves it closed after 800 ms; at 840x620 a press
     on the legend row shows the popover anchored so that it does not cover that row (`legend-anchor` `covers: false`),
     and a second press on it closes it.
   - usage: `usage-seg-pop-0-0-0` `inert: false` after the glide into the popover (900 ms); the email's tip present
     on the email; after the real click the email shown (the screenshot); `inert: true` 400 ms after the glide out
     (bug 10). Then
     `model-row-tip-codex:gpt-6-luna` present on the unavailable row, its `tip-box` left of the row; absent after the
     glide away (bug 9).
4. Release the lock (only with this owner note). Kill the app pids the drive printed if one is left.

## Attempts and evidence

| Attempt | Revision | Checks and outcomes | Evidence | Remaining |
| --- | --- | --- | --- | --- |
| probe | one-file app, framework of `c0475fbaa` | Real pointer (cliclick) under the lock, 06:12-06:32Z: a card outside its trigger's box gets no tracking and the trigger hears a leave as the pointer moves in; overlapping hover nodes alternate each move; a window-level card hears the pointer | X62 issue file | — |
| 1 | `5476de8b2` | Layer state in the root (+22 root lines); dev drives found the editor icon under a toast, the authorized clients section behind network access, the pressed popover's duplicate button testId (fixed) | dev drives (not kept) | root lines |
| 2 | `a33cf290f`, merge `3fa874f59` (base `ec32c8c37`) | Root block compressed to +17 lines; coordinator: at most +10 | — | root lines |
| 3 | `53bece1c7` | The state in T3Window, the root keeps the hover clock (+10); usage (bug 10) and model rows (bug 9) on the helper; left/right sides. `bun test examples/t3-code` 3,144 pass / 1 skip / 0 fail; strict tsc clean; contract build clean; five checks all exit 0 | [pairs 01-10](https://raw.githubusercontent.com/ccheever/exact2/e780f6b871a414f7981cfdf0a9536cffe80d1b77/fix-hover-cards/), [agent-drive-record](https://raw.githubusercontent.com/ccheever/exact2/e780f6b871a414f7981cfdf0a9536cffe80d1b77/fix-hover-cards/agent-drive-record.txt), [drive.mjs](https://raw.githubusercontent.com/ccheever/exact2/e780f6b871a414f7981cfdf0a9536cffe80d1b77/fix-hover-cards/drive.mjs.txt) | real input |
| real input | `53bece1c7` | Lock taken 07:44Z; the first real-pointer drive showed nothing: `loginwindow` over the screen, `CGSSessionScreenIsLocked = Yes` (07:51Z). Lock released at once; no further real input. `real.mjs` dry-run (`REAL_DRY=1`) to its last step for all three lanes | [real.mjs](https://raw.githubusercontent.com/ccheever/exact2/e780f6b871a414f7981cfdf0a9536cffe80d1b77/fix-hover-cards/real.mjs.txt) | the real-input batch |
| 4 | merge `6ef66bc93` (base `fa46ad5d0`, #306: `model-picker.contract` merged cleanly, X63 kept beside X62) | `bun test examples/t3-code` 3,146 pass / 1 skip / 0 fail; strict tsc clean (app.ts, the new test); contract build 3,941 slots, 46 resources; `cargo test -p t3-code-macos --lib` 13 pass; `app.contract` 1,488 lines (+10 over `fa46ad5d0`); five checks: build ok, test 3,521 passed / 0 failed / 34 ignored (94 binaries), clippy ok, fmt ok, caps within, boot ok; agent drives repeated on the merged build: every row passes | merged step logs | real input; #290's merge |
| 5 | `0f6f00892`, `55e6eddc0`, merge `5752b7be2` (base `84a52dde0`: #290, #303) | Base UI's open delay on the hover clock (nothing drawn before it: #263 item 11); the legend row anchors the Usage popover; a press on the pinned trigger and #290's light dismiss drop the hover state too; #290's pin, outside press, ground and confirm focus kept; #290's assertions follow the merged lines. `bun test --timeout 60000 examples/t3-code` 3,153 pass / 1 skip / 0 fail; strict tsc clean; contract build 3,950 slots, 46 resources; `cargo test -p t3-code-macos --lib` 13 pass; `app.contract` 1,488 (+10 over `84a52dde0`); five checks on `5752b7be2`: build ok, test 3,521 passed / 0 failed / 34 ignored (94 binaries), clippy ok, fmt ok, caps within, boot ok; agent drives repeated: all rows pass | merged2 logs, handed-over log and 26-29 | real pointer: the screen locked again (09:23Z) |
| 6 | `7fe081148`, merge `653dfc98a` (base `0e2901aec`: #312, #308) | Real-pointer session under the lock (10:40:48-11:22:30Z), on the build of `5752b7be2` (settings, pr) and of `7fe081148` (usage): bugs 1, 2, 9, 14 pass; bug 10's popover passes and its email's tooltip fails (X62, #322); #290's three rows pass; #281's press passes on the freshness card and fails on the Usage popover (before #327). On `5752b7be2` the sidebar press left the pinned popover open: the window counted the press, but a usage segment had an enter while the pointer crossed the sidebar's resize handle ("hover in view 1306 (sidebarHover)", then "hover in view 1130 (enterSeg#7408)", five times), so the hover-opened state kept it. Fixed in `7fe081148` (a press outside the page drops a hover-opened popover, as Base UI's outside press does); the same press then closed it. X62 cited as #322 in the code comments. After the merge: `bun test examples/t3-code --timeout 60000` 3,243 pass / 1 skip / 0 fail; the macOS build of `653dfc98a` exit 0; `app.contract` 1,488 lines (+10 over `0e2901aec`) | [session record](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real-session-record.txt), [real.mjs](https://raw.githubusercontent.com/ccheever/exact2/f487ebe706da46aaec1be841ba6f6eb7d6cc4f2e/fix-hover-cards/real.mjs.txt), real-01 to real-31 | the email tooltip and the Usage padding press wait for #327 |

## Handed over by #290 (coordinator, 2026-10-08)

Real-pointer rows that #290 traced to hover, now on this helper (see "Acceptance": they pass by the agent on
`5752b7be2` and by the real pointer on `7fe081148`): (1) a real press in the sidebar drops the pin, but #263's hover state kept the popover: here the
hover state follows real leaves (the popover is drawn where the host tracks it) and #290's light dismiss drops it
too; (2) a press on the pinned segment with the pointer resting on it: `toggle` drops the hover state as it unpins,
and the hover waits for a new enter (Base UI's blockMouseMove); (3) a press on a narrow legend row, which the popover
hung over: the legend row now anchors it (`place(id, "legend")`). #290's item 11 (a still-transparent popover over
the next card takes the click) is answered by the open delay: nothing is drawn before Base UI's 300 ms.
#290 also saw free `pointermove` stop after a segment's hover action called a root action (unconfirmed). The
real-pointer session did not log `pointermove`; hover events kept coming after the segment's enters (the usage
log's `mail-events`).

## Next action

2026-10-08 (records sync, `t3-code-records-reclassify`): merged into `feat(example)/t3-code` as #307 (`3c8c11ef2`); the record moved to `tasks/closed/`. Its deferred real-input rows stay in `STATUS.md` "Next real-input batch".

Two real-pointer rows wait for [#327](https://github.com/ccheever/exact2/pull/327) (open,
fixes X62 #322 and #281): the Usage popover's email tooltip, and the press on the Usage popover's padding
(STATUS.md, "Next real-input batch"). When #327 is in the base, rerun `real.mjs … usage`; no code change is planned
for it here.
