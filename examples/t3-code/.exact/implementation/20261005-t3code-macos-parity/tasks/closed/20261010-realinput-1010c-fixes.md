---
name: 20261010-realinput-1010c-fixes
plan: 20261005-t3code-macos-parity
implementation: done
verification: partial
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-realinput-1010c-fixes
pr_url: https://github.com/ccheever/exact2/pull/399
verified_commit: df2cc67e19d30813b547e7099cf9811a94bd4df5
---

# Failures of the attended real-input session of 2026-10-10 (realinput-1010c)

## Outcome

The attended session (`realinput-1010c`, normal launches of lane copies, real keys and pointer, the user present for
sign-ins, notifications and the Korean input source) passed most rows. These steps failed under real input, although
their agent-mode and unit checks passed. This task fixes each as the reference does and writes exact real-input steps
for the next session. (#349's own failures are in its fix round, not here.)

## Findings

| Id | From | Reference (T3 Code `1e2ecbd975`) | Clone under real input | Evidence |
| --- | --- | --- | --- | --- |
| RC-2 | #374 CO-7 | After a Shift+click removes a model, the picker trigger names the remaining model. | The accessible label is right ("Claude Sonnet 5.5"), but the painted trigger text stays stale ("Claude Fable 5.1, C…") in a box sized for the new label (2 of 2). | [C](https://raw.githubusercontent.com/ccheever/exact2/4227ea6597bae1028438390c0516437d80da45c3/realinput-1010c/C-co7-shift-pick.png) |
| RC-3 | #378 FW-3 step 3 | With the pointer moved into Search authors: Escape closes Author, ↓ moves nothing, a second Escape closes Filters. | The last Escape does not close Filters; only a click outside does (2 of 2). | [G](https://raw.githubusercontent.com/ccheever/exact2/e8fabfeed724dce27c432773cd615219967a8fea/realinput-1010c/G-fw3-fw4-filters.png) |
| RC-4 | #364 steps 1-3 | The skill chip's popover closes on Escape; in Settings' prompt sample, Escape closes only the popover; the chip's accessible element is an enabled button. | After clicking the heading and then the chip, Escape leaves the popover open (2 of 2); in Settings, Escape closes the popover and Settings (2 of 2); the chip's "Skill Frontend Design. Show details" element reports itself disabled. | [H](https://raw.githubusercontent.com/ccheever/exact2/5df0a1d5d26d8b1de7ce55614125bfc0f66a6b7a/realinput-1010c/H-skill-chip.png) |
| RC-5 | #375 PG-3 | Moving the pointer from the (i) up into the unpriced popover keeps it open. | A ~4 pt gap between the (i) and the popup closes it on the way. | [I](https://raw.githubusercontent.com/ccheever/exact2/a0aabaf843cfa53420383227b9844aa36dfb324d/realinput-1010c/I-pg3-pg4.png) |
| RC-6 | #354 step 2 | In the Add profile menu, ↓ walks the rows from Blank profile to Chrome and the other browsers. | ↓ opens on Blank profile but never reaches Chrome (3 tries); the row ⋮ menu and the launcher chevron work. | [K](https://raw.githubusercontent.com/ccheever/exact2/b5e68e2a741418544b1608af7933ba5677f55e30/realinput-1010c/K-profiles.png) |
| RC-8 | #360 (check) | Diff › Changes shows the thread's project (the lane's `work` repo). | On the #360 bundle the Changes view showed the enclosing exact2 checkout ("feat(example)/t3-code vs origin/main", +576k) for a lane project; and a click on the composer did not focus it. Check whether this is the lane's project path (inside the exact2 tree) or a clone bug; fix if the clone resolves the wrong repository. | [D](https://raw.githubusercontent.com/ccheever/exact2/d10b10619a6d71683e6eea6ccd6e2bbf53f3a604/realinput-1010c/D-pa12-cmd-return.png) |

## Scope and exclusions

Included: RC-2 to RC-6 and RC-8 (RC-8 starts as a check). RC-1, RC-7 and RC-9 are in [realinput-1010c-native](20261010-realinput-1010c-native.md). Excluded: #349's failures (its own fix round); the
reaction-pill tooltip of #261 (waits for main #327, #322).

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| RC-2..RC-6 | a unit or AppKit test of the cause where one can be written; agent drive where agent input reaches it; exact real-input steps for the next session (they stay open until it runs) | before / after image where agent mode shows it |
| RC-8 | reference comparison; fix only a real difference | text |

## Why agent input passed where real input failed

Every agent `type <target> key <K>` first makes the target the first responder when it takes one (AgentMac.swift
`type`), so an agent key always lands on a node the drive chose. A real key lands wherever AppKit's focus is. And the
agent's window is never the key window (the R9 input log reads `key=false` on every agent click), so an agent click
on a text view never moves AppKit's first responder, while a real click does. The rows below are those two paths.
To reproduce a real key with the agent, this task sends keys to a target that takes no focus (`type chip-popover key
Escape`, `type pr-filters-layer key Escape`, `type browser-profiles-setting key ArrowDown`): the key then goes to the
focus the clicks left, as a keyboard's does.

## Cause and fix

- **RC-2 (model trigger after a Shift+click).** X64 (#316): on macOS a paragraph drawn from a text raster keeps its old
  raster when an update shrinks it below the raster size (16,384 device pixels). The trigger's one-line label at 14 pt
  is above it with two models ("Claude Fable 5.1, Claude Sonnet 5.5", about 232 × 20 pt) and below it with one, and the
  label was one text node across both, so the window kept painting the old raster clipped to the new box while the
  tree, the capture path and the accessible name had the new text (the session's image). The fix is X64's documented
  workaround, already used by the Providers list: the label is `each shown in [label] key=shown`, a new text node per
  label. The agent's drive shows the cause, not the paint (agent captures draw the right text either way): the label's
  node id stays 195 through both labels before and changes with each label after. The key holds the composer's resting
  size as well (`` key=`${xs}:${shown}` ``, review round): a resting composer in a wide window keeps the label but draws
  it at 12/400 in at most 10.5rem for 14/500, so the same text can shrink below the raster size. The review's example,
  a two-model label (about 18,560 device px at 14 pt, 12,700 at 12 pt), cannot rest (a fan-out exists only on a draft,
  `fanoutSelections` returns null with a thread, and only a thread's composer rests, `canRest`); a long single-model
  label (a custom model's name above about 205 pt at 14 pt) can. EXACT2-GAPS X64 names this consumer; the workaround
  goes when main #327 (which fixes #316) is adopted.
- **RC-3 (Filters' last Escape).** FW-3 (#378) drops the focus with `blur()` when the pointer left the submenu's row
  (Base UI leaves it on BODY). On macOS `blur()` makes the window itself the first responder, and the host's popover
  Escape (`MenusMac.key`) closes a popover only while the focus owner is a view inside the window's content, so a real
  second Escape reached nothing and Filters stayed open. #378's agent drive passed because its `type <row> key Escape`
  focused the row first. The dropped focus now lands on a 1-pt rest box inside the Filters popover (`pr-filters-rest`,
  `tabindex=-1`, `aria-hidden`), outside both KeyMenus and the submenu's row: no row is lit, ↓ moves nothing (no `key`
  handler hears it there), no ring (not pressable), and the next Escape reaches the host's popover Escape, which closes
  Filters and gives the focus back to the Filters button, as the reference does (CDP: BODY, ↓ nothing, Escape →
  `BUTTON Filters`). The host limit is EXACT2-GAPS X73 (review round), with the other `blur()` callers checked against
  it by reading: `PrPageBack` and `UsageKeys` blur and leave their page with nothing shown, and the menu and dialog
  openers blur for a popup that takes the focus by `autofocus` and closes on its own Escape shortcut, so none leaves an
  auto popover open that only the host's Escape closes.
- **RC-4 (the skill chip's details).**
  - Escape after a click elsewhere: the details closed on Escape only through the native text view's `cancelOperation`,
    which runs only while that text view is the first responder. In the session the heading's click (a paragraph takes
    the focus) left the focus off the text view when the chip was clicked, so Escape reached nothing. The agent drive
    shows the same state (focus on the heading's text node after the chip's press) because its window is never key; on
    the feature-branch tip an Escape at that focus leaves the details open, as in the session. Base UI's Popover
    dismisses on an Escape anywhere in the document. The window now has a hidden Escape shortcut button for the details
    (`chip-popover-escape`, the pattern of `palette-escape` and the diff menus' Escape buttons), which a key reaches
    from any focus in the window before a field or text view does.
    The host gives an Escape to the oldest shortcut button that declares it (ShortcutsMac `nodes()`: the lowest node
    id). The first version put the button in the details layer, made when the chip opened, so a holder made before it
    kept the key: with a right panel open (Diff, Files, Browser) its toggle holds Escape (`r4-surfaces.contract`), and
    Escape closed the panel and left the details open (review round; T3 Code binds no Escape to the right panel,
    `rightPanel.close` is mod+w, so its Popover takes the key). The button is now its own component,
    `ChipPopoverEscape`, the window's first node (made with the window, so older than every holder made later: the
    right panel's toggle, the notice stack's "Hide other notices", a surface menu's backdrop beside the composer), and
    declares Escape only while the details show (`held`).
    It gives Escape up while the SSH prompt, the palette, the model picker, the composer's options, the project picker
    or a confirmation holds it (`held=false`; the SSH prompt because the button now sits outside the prompt's inert
    background), and T3 Code's palette over the details closes alone (realinput-1010c H, step 5).
  - Escape over Settings: Settings' Back holds an `aria-keyshortcuts="Escape"` unless something owns Escape
    (`escapeOwned`); the chip's details were not one, and shortcuts run before any text view, so Escape closed Settings
    (and the details with it). `escapeOwned` now includes `chipOpen` (the window's `chipShown`): the first Escape closes
    the details, the second leaves Settings, as the reference (CDP: Escape closes the popup, the hash stays
    `#/settings/appearance`, the focus returns to the chip).
  - The accessible element: `NSAccessibilityElement` reports `isAccessibilityEnabled() == false` unless told otherwise
    (checked in a one-file probe), so Accessibility Inspector listed "Skill Frontend Design. Show details" as disabled.
    The element now calls `setAccessibilityEnabled(true)`.
- **RC-5 (the unpriced popover's gap).** Not changed: no clone cause found. The clone's geometry is the reference's
  (layout read-back: the (i) at 425,122 12 × 12 pt, the popup at 288.8,92 284.8 × 26 pt; the reference 424.6,120.5 and
  288.2,90.5 284.8 × 26 px), and the clone's hover column (`UnpricedPopup`, `hover=enterPop`) carries the 4 pt
  `padding-bottom` that fills the gap, so a pointer in the gap is inside the popup's own hover area. Agent hover
  (hit-tested) passes: the journal reads `hover out view 808 (enterSeg)`, `hover in view 1019 (enterPop)` and the popup
  stays. The reference over CDP keeps the popup open with the pointer resting 1.5 s in the 4 px gap (floating-ui's
  rect between trigger and popup), and closes it when the pointer moves away. A real pointer is delivered by the
  host's tracking areas, not by a hit test; their known differences are X62 (#322: one hovered node, tracking clipped
  by ancestors, overlapping areas trading the hover), which main #327 replaces with hit-tested hover. Which tracking
  event went missing in the session is not known; the batch steps below capture it (Save Trace's journal lists every
  hover in/out the window delivered).
- **RC-6 (Add profile's ↓).** The Add profile menu's keyboard rows (`SkPopup items`) held only Blank profile, so ↓ from
  it wrapped to itself, and the source rows had no KmDoor (the pointer's highlight could not hand them the focus
  either). `bpAddItems` lists Blank profile, then every source when they are listed and importable, the enabled
  MenuItems in order, as Base UI's list navigation walks them (reference over CDP: Blank profile, Chrome, Brave).
  The disabled rows ("Looking for browsers…", "No supported browsers found", the profile limit, "Connect to an
  environment…") are not stops, as Base UI skips disabled items.
- **RC-8 (check).** A lane artifact, not a clone bug
  ([rc8-diff-repository.txt](https://raw.githubusercontent.com/ccheever/exact2/7e6f9788240f3277580e1d9ccd54e2c759361373/realinput-1010c-fixes/rc8-diff-repository.txt)):
  with HOME set to the lane home, the embedded server's workspace root is that home (`T3LocalBackend.swift` `cwd: NSHomeDirectory()`, as the
  reference's packaged `backendCwd = homeDirectory`); the lane project outside it is refused ("Review diff preview cwd
  must stay within the configured workspace root.", in the lane server's trace), and the Diff asks again at the server's
  cwd as DiffPanel does (`r11-device-diff.ts`, `DiffPanel.tsx:283-303`); the lane home is no repository, so git found the
  enclosing exact2 checkout. The same lane setup gives the same result in T3 Code. The note "a click on the composer
  did not focus it" is not reproducible by the agent (its window is never key, so AppKit never moves the first responder
  on an agent click into a text view: before and after both keep the heading's focus); the batch steps below check it
  with the R9 input log, which records each press's hit view, responder and key-window state.

## Found, not changed (framework)

- `MenusMac.key` (the host's popover Escape) answers only when the window's first responder is a view inside the
  viewport, and `blurElement` (`blur()`) makes the window itself the first responder, so on macOS a popover cannot be
  closed by Escape after `blur()`; on the web, Escape closes an auto popover whatever has the focus (BODY included).
  The clone works around it (RC-3); recorded as EXACT2-GAPS X73 with the other `blur()` callers checked against it;
  not filed (the coordinator files).
- The host gives an Escape to the oldest shortcut button that declares it (ShortcutsMac `nodes()`, the lowest node id),
  where the web's popovers and dialogs give it to the newest layer; RC-4's button is placed as the window's first node
  for that reason. Not a gap row: the clone states the order it needs in each Escape owner (`held`, `escapeOwned`).
- Not changed (outside this task's rows): the clone's right panel toggle closes the panel on Escape
  (`r4-surfaces.contract`), while T3 Code's keymap binds none to the right panel (`rightPanel.close` is mod+w,
  `packages/shared/src/keybindings.ts`); whether another path in T3 Code closes it on Escape was not checked.
- On an agent drive the window is never key (`key=false` in the R9 input log), so a `tap … mouse` into an NSTextView
  never moves the first responder; rows about a text view's focus after a click need real input.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| RC-2 | implemented, verified by the mechanism; the painted label open (real input, batch step 1). The label's node id stayed 195 through both labels before and is a new node per label after (1156, 1176). Agent window captures draw the right text in both builds, so the stale paint itself is not reproducible by the agent. Review round: the key also holds the resting size (`` `${xs}:${shown}` ``; unit test; no agent-visible change, and the long-label resting case is batch step 1) | [rc2-label-node.txt](https://raw.githubusercontent.com/ccheever/exact2/9cc32a9ffd5f381f4940a4aee9dba88a226f52e4/realinput-1010c-fixes/rc2-label-node.txt), [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/78e51eceb34715d8a31e4c0056f53eece887f7e5/realinput-1010c-fixes/drive-record.txt) |
| RC-3 | pass (agent, keys at the focus the clicks left): Filters › Author by the pointer, the pointer into Search authors, Escape, ↓, Escape. Before: focus none after the first Escape, Filters still open after the second. After: focus on the rest box (node 4277), ↓ moves nothing, the second Escape closes Filters and the focus is the Filters button (4269), as the reference over CDP (BODY, nothing, `BUTTON Filters`). Real keys: batch step 2 | [rc3-filters-second-escape.png](https://raw.githubusercontent.com/ccheever/exact2/c000b19f72e89e5bf01e8d0e3b1bb058ce992f6b/realinput-1010c-fixes/rc3-filters-second-escape.png), [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/78e51eceb34715d8a31e4c0056f53eece887f7e5/realinput-1010c-fixes/drive-record.txt) |
| RC-4 | pass (agent, same keys): composer, heading, chip, Escape: before the details stay open, after they close and the prompt is unchanged. Settings sample chip, Escape: before Settings closed too, after only the details close and Settings stays, as the reference. The enabled element: pass (AppKit test fails on the tip's Swift, passes here). Real keys and Accessibility Inspector: batch step 3. Review round, the right panel open (Files): before (`c8668fb34`) Escape closed the panel and left the details open (the details' Escape node 978, the panel toggle 931: the host takes the lowest id); after, the details' Escape is node 3, so Escape closes the details and the panel stays, a second Escape closes the panel; the no-panel and Settings cases still pass. Real keys: batch step 3 (d) | [rc4-composer-chip-escape.png](https://raw.githubusercontent.com/ccheever/exact2/1279971ecc47b77ec18cfaeb36884b0b98b04d6e/realinput-1010c-fixes/rc4-composer-chip-escape.png), [rc4-settings-chip-escape.png](https://raw.githubusercontent.com/ccheever/exact2/0f1443d8195f2df47b92dbaaf8ee5f58a8b8170d/realinput-1010c-fixes/rc4-settings-chip-escape.png), [rc4-ax-enabled.txt](https://raw.githubusercontent.com/ccheever/exact2/b888d3b28ecdfbc4dca80665c709ddbffd781b66/realinput-1010c-fixes/rc4-ax-enabled.txt), [rc4-panel-chip-escape.png](https://raw.githubusercontent.com/ccheever/exact2/89f69cc5723933e1c9c8b25b456b55870cafdc07/realinput-1010c-fixes/rc4-panel-chip-escape.png), [drive-record-review.txt](https://raw.githubusercontent.com/ccheever/exact2/480638d7a61a98d21bb7b684e50949c5d07c8f7d/realinput-1010c-fixes/drive-record-review.txt) |
| RC-5 | open (real input, batch step 5): no clone cause found; nothing changed. The clone's geometry matches the reference's and its hover box covers the gap; agent hover passes; the reference stays open with a pointer resting in the gap | [rc5-gap.txt](https://raw.githubusercontent.com/ccheever/exact2/8b03fa528eee611deff7b779febceb6962ecf46e/realinput-1010c-fixes/rc5-gap.txt), [ref-gap.mjs.txt](https://raw.githubusercontent.com/ccheever/exact2/9938387890154c243ccee478ac12a308ca90b3fe/realinput-1010c-fixes/ref-gap.mjs.txt) |
| RC-6 | pass (agent, keys at the focus): focus Add profile, ↓ opens on Blank profile, ↓, ↓: before the focus stays on Blank profile (2684 three times), after Blank profile, Chrome, Brave (2695, 3169, 3171), as the reference over CDP. Real keys: batch step 4 | [rc6-add-profile-down.png](https://raw.githubusercontent.com/ccheever/exact2/19d49db164118b6599b13676fa185ba59ad99d37/realinput-1010c-fixes/rc6-add-profile-down.png), [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/78e51eceb34715d8a31e4c0056f53eece887f7e5/realinput-1010c-fixes/drive-record.txt) |
| RC-8 | check done: a lane artifact, no clone change (the reference behaves the same with the lane's HOME). The composer-click note: not checkable by the agent; batch step 6 | [rc8-diff-repository.txt](https://raw.githubusercontent.com/ccheever/exact2/7e6f9788240f3277580e1d9ccd54e2c759361373/realinput-1010c-fixes/rc8-diff-repository.txt) |

Drives: one before drive (`t3-code-ri1010c-before`, the feature-branch tip `f0aaaa56d`, built for this task: the shared
evidence base `950e8e2e5` predates #354, #364, #374, #375 and #378) and one after drive of this branch's bundle, same
ops ([drive.sh.txt](https://raw.githubusercontent.com/ccheever/exact2/0685361b82e216160a0f02bab3cdf5363104d61b/realinput-1010c-fixes/drive.sh.txt), [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/78e51eceb34715d8a31e4c0056f53eece887f7e5/realinput-1010c-fixes/drive-record.txt)). The reference ran on the lane
(backend 16520, CDP 16521) with the browser fixture copied into the lane home for RC-6.

## Tests

- `realinput-1010c-fixes.test.ts` (new): RC-2's label keyed on the resting size and the text, RC-6's `bpAddItems` and
  its source rows' ids.
- `menu-keys.test.ts` FW-3: `closeSub` focuses `pr-filters-rest` (no `blur()`), the rest box is the popover's first child,
  outside the row that holds the submenu and both KeyMenus.
- `composer-chip-popover.test.ts`: the hidden Escape button (`ChipPopoverEscape`, `held`), the window's first node
  after `main` (review round), its `held` terms, `escapeOwned` with `chipOpen`, and the window passing
  `chipOpen=chipShown`.
- AppKit `macos/tests/composer` `chippress.swift`: the chip's button is enabled (56 tests, 0 failures; 1 failure with the
  tip's `modules/apple`).

Checks: see the PR ("Checks"); the review round's final checks ran on `7a58514b4`.

## Real-input batch steps

Launch this branch's bundle normally, active and key, window 1280×840, from the worktree
`/Users/daehyeonmun/orca/workspaces/exact2/t3-code-realinput-1010c-fixes` (lane `realinput-1010c-fixes`: its `codex/` and
`claude/` hold the Usage transcripts, `browser-home/` the browser fixture). Leave `HOME` alone for step 6 (RC-8); every
other step works with or without a lane `HOME`.

```sh
A=/Users/daehyeonmun/orca/workspaces/exact2/t3-code/target/t3-audit L=$A/lanes/realinput-1010c-fixes
PATH=$HOME/.bun-1.4.2/bin:$PATH EXACT_APP_DIR=$PWD/examples/t3-code T3_LOCAL_HOME=$L/clone-t3-home T3_LOCAL_PORT=16522 \
  T3CODE_TELEMETRY_ENABLED=false T3_LOCAL_RUNTIME_DIR=$A/runtime/t3-0.0.46-nightly.20261005.2667-darwin-arm64 \
  CODEX_HOME=$L/codex CLAUDE_CONFIG_DIR=$L/claude XDG_CONFIG_HOME=$L/xdg/config XDG_DATA_HOME=$L/xdg/data \
  XDG_STATE_HOME=$L/xdg/state XDG_CACHE_HOME=$L/xdg/cache T3_BROWSER_IMPORT_HOME=$L/browser-home \
  R9_INPUT_LOG=$L/logs/r9-input.log bun host/apple/build.mjs t3-code-macos --bundle --run
```

Dismiss the update and mobile-app notices. Run each step in the dark appearance (the session's) and once in light for
step 1.

1. **RC-2, the model trigger.** On the "work" New thread draft, click the model trigger, then the Claude rail. Note the
   original model (for example "Claude Fable 5.1"). Shift+click "Claude Sonnet 5.5": the trigger reads "<original>,
   Claude Sonnet 5.5". Shift+click the original's row: the trigger reads exactly "Claude Sonnet 5.5", nothing of the old
   label painted or clipped, in a box that fits it. Shift+click "Claude Opus 5.5", then Shift+click it again: "Claude
   Sonnet 5.5" again, cleanly. Pass: the painted label equals the accessible name (Accessibility Inspector on the
   trigger) after every click. Fail: record the painted text. Then, only if a model whose label is wider than about
   205 pt is at hand (a custom model with a long name; add none for this): on a thread with enough messages to scroll,
   with Settings › General "Collapse composer on scroll" on and the window wide, choose that model, scroll the timeline
   until the composer rests (its controls shrink), then back to the end: the label paints whole at both sizes.
2. **RC-3, Filters' last Escape.** Pull Requests › click Filters › click Author ("Search authors" has the caret) › click
   inside "Search authors" › Escape: Author closes, Filters stays, no row lit, no ring › ↓: nothing moves › Escape: Filters
   closes and the Filters button has the ring. Repeat with State: click Filters › click State › move the pointer onto
   "Closed" › Escape › ↓ › Escape: the same. (#378's FW-3 steps 1-3 still apply.)
3. **RC-4, the chip's Escape.** (a) In the composer type `$frontend-design now`. Click the heading "What should we build
   in work?", then click the "Frontend Design" chip: the details open. Press Escape: they close and the prompt is
   unchanged. Click after "now" (the caret there), click the chip, press Escape: closed, the caret still after "now"
   (a press elsewhere in the text closes the details by itself). Click the chip, press ⌘K: the
   details stay above the palette; Escape closes only the palette; Escape again closes the details. (b) Settings ›
   Appearance › click the prompt sample's chip › Escape: the details close and Settings stays; Escape again: Settings
   closes. (c) Accessibility Inspector on the composer's text area: the child button "Skill Frontend Design. Show details"
   reads Enabled: true. (d) The right panel open (review round): open a Diff, Files or Browser panel (⌘⌥B or the
   header's panel button), click the heading, click the chip, press Escape: the details close and the panel stays;
   Escape again closes the panel (the clone's own panel Escape). Repeat with the notice stack expanded if one shows.
4. **RC-6, Add profile by keys.** Settings › Integrations › Browser profiles: Tab (or click then Escape) until "Add
   profile" has the ring, press ↓: the menu opens on Blank profile. ↓: Chrome. ↓: Brave, then Arc, Safari, Firefox, and
   ↓ from Firefox wraps to Blank profile; ↑ goes back. Return on Chrome opens the import wizard (Cancel it; do not
   import). Escape closes the menu.
5. **RC-5, the unpriced popover's gap (diagnostic).** Usage › Cost › 30 days: "… · API estimate (i)". Rest the pointer
   on the (i) until "API estimate excludes 20.0% unpriced records." shows. (a) Move straight up slowly, about 1 pt a step,
   into the popup: note whether and where it closes. (b) The same in one quick move. (c) Re-open it and enter the popup
   from its left edge (never through the gap). (d) From inside the popup move down into the 4 pt gap above the (i) and
   rest 2 s. Right after the first case that closes it, choose Develop › Save Trace (⌥⌘T) and keep the file:
   `bun scripts/agent.mjs trace <file>` lists each `hover in/out view N` the window delivered (808 is the (i), 1019 the
   popup's hover box in this lane). The reference stays open in (a), (b) and (d) (a CDP pointer resting 1.5 s in the gap
   keeps it).
6. **RC-8, the Diff's repository and the composer's click.** With `HOME` not set to a lane home (or with the project
   inside the lane home): on a thread of the "work" project (ri1010c-4's "lane thread" kind), open the Diff (⌘⌥B › D)
   and choose Changes: it diffs the `work` repository (feature/audit, its own files), not the exact2 checkout. Then
   click into the diff's comment box (or the heading), then click once in the composer's empty area below the
   placeholder: the composer takes the caret. If it does not, attach `$L/logs/r9-input.log` (each press's hit view,
   responder and `key=` window state).

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Before drive | tip `f0aaaa56d` (worktree `t3-code-ri1010c-before`) | RC-3, RC-4 (composer and Settings) and RC-6 reproduced with keys sent at the focus the clicks left | [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/78e51eceb34715d8a31e4c0056f53eece887f7e5/realinput-1010c-fixes/drive-record.txt) |
| After drive (the one live drive) | this branch's bundle (`9a7605e02`) | every step passed the first time | the links above |
| Reference | Electron reference on the lane, CDP | Settings chip Escape, Filters' Escape sequence, Add profile ↓, the unpriced gap | the composed images, [rc5-gap.txt](https://raw.githubusercontent.com/ccheever/exact2/8b03fa528eee611deff7b779febceb6962ecf46e/realinput-1010c-fixes/rc5-gap.txt) |
| Review round: before and after drives | before: the PR's previous head `c8668fb34` (worktree `t3-code-ri1010c-before`); after: this branch's bundle (`5fa6501bc`) | RC-4 with the Files panel open reproduced before and passes after; the no-panel and Settings chip cases pass in both; each drive was re-run once after a first run stopped at a `tree` op (after: an empty right panel holds no Escape, so the script opens Files; before: the op named the new `chip-popover-keys` box, which that build does not have) | [drive-record-review.txt](https://raw.githubusercontent.com/ccheever/exact2/480638d7a61a98d21bb7b684e50949c5d07c8f7d/realinput-1010c-fixes/drive-record-review.txt), [drive-review.sh.txt](https://raw.githubusercontent.com/ccheever/exact2/22075885833065b8b32df735820339571ce0942e/realinput-1010c-fixes/drive-review.sh.txt) |

## Not done / not verified

- RC-2 (the painted label), RC-3, RC-4, RC-6 with real keys and pointer: open until the batch steps run (agent mode
  cannot be the key window or send keys without choosing a target).
- RC-5: open; the cause is not found and nothing changed. Batch step 5 records the hover events a real pointer delivers.
- RC-8's composer-click note: open (batch step 6, with the R9 input log).

## Next action

Run the real-input batch steps; then the coordinator reviews and merges.

## Delivery

Merged on 2026-10-10 as `df2cc67e1` (#399, squash) after an independent review and its repair round. The real-input
batch steps ran in `realinput-1010e` on the bundle of `a1ade42f9` (lane ri1010e-3)
([notes](https://raw.githubusercontent.com/ccheever/exact2/a18ab23126a27ce4804478282e641fb8734d3f02/realinput-1010e/E4-399-notes.txt)).
New rows went to [realinput-1010e-followups](20261010-realinput-1010e-followups.md).

- RC-2 passed in dark and light: the painted label equals the accessible name after every Shift+click
  ([dark](https://raw.githubusercontent.com/ccheever/exact2/f42813d709681e41d5d88f4bfa2198acbfcf3bd3/realinput-1010e/E4-RC2-dark-shift-clicks.png)).
  The long single-model sub-step was skipped: no model label wider than about 205 pt is available. The unit test covers
  the key.
- RC-3 passed for Author and State
  ([Author](https://raw.githubusercontent.com/ccheever/exact2/70c2e0e5927630f2907f40dae7221c8e6d6ece86/realinput-1010e/E4-RC3-author.png)).
- RC-4 (b), (c) and (d) passed. In (a) the prompt stays unchanged and the ⌘K order passed, but the caret failed (2 of 2):
  a click on the chip moves the caret to 0, after Escape no caret shows, and typing lands before the chip
  ([typed](https://raw.githubusercontent.com/ccheever/exact2/7a248636ea42d3fecaf35fdcfe546194628c52f8/realinput-1010e/E4-RC4a-2-FAIL-typed-x-lands-before-chip.png)).
  Moved to RE-3.
- RC-5: the diagnostic ran. No cause was found, a bad state was seen twice, and no trace was written (the session
  pressed ⌥⌘T, which is not the binding). Moved to RE-6.
- RC-6 passed: ↓ walks Blank profile, Chrome, Brave, Arc, Safari, Firefox and wraps; Return on Chrome opens the wizard
  with the fixture's profiles only; Escape closes the menu
  ([keys](https://raw.githubusercontent.com/ccheever/exact2/2d9e11dfd24b7e2ef6730f8b9202d4792cbc5f94/realinput-1010e/E4-RC6-1-arrow-keys.png)).
  New: after Cancel by mouse in the wizard the focus goes nowhere. Moved to RE-4.
- RC-8 passed: Diff › Changes shows the `work` repository (feature/audit → main, +6 −1), and one click in the composer's
  empty area gives it the caret
  ([Diff and composer](https://raw.githubusercontent.com/ccheever/exact2/f593e58600f3bfd30f87cefada94f96ef0234f2c/realinput-1010e/E4-RC8-diff-work-repo-comment-then-composer.png)).
