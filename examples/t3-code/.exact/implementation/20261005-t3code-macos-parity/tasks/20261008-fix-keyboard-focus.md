---
name: 20261008-fix-keyboard-focus
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-fix-keyboard-focus'
pr_url: https://github.com/ccheever/exact2/pull/310
verified_commit: PENDING
---

# Menus and dialogs by real pointer and keyboard: Custom snooze, the More menu, focus rings, Escape order

## Outcome

Five clone bugs from the real-input batch ([#298](https://github.com/ccheever/exact2/pull/298)) are fixed to match
T3 Code `1e2ecbd975`. The numbers are #298's:

- **4** ([dialog-shortcut-focus](closed/20261008-dialog-shortcut-focus.md), #255): Custom snooze could not be
  opened from the sidebar row's snooze menu by the real pointer. Moving onto "Custom…" ended the row's hover, the
  click only closed the menu, and ↓ did not enter the menu.
- **5** (#255): Custom snooze showed no focus ring at open, and none after five Tabs.
- **6** (#255): with ⌘K open over Custom snooze, the first Escape closed the dialog underneath, not the palette.
- **13** ([pr-header-actions-and-stacks](closed/20261005-pr-header-actions-and-stacks.md), #262): the PR header's
  More menu, opened from the keyboard, left the focus on "…", and ↑/↓ did nothing.
- **16** (#262): the Close pull request dialog showed no focus ring.

Added scope (coordinator, 2026-10-08): **every clone popover menu** now has one shared keyboard pattern, after Base
UI's Menu (`menu-keys.contract`, `KeyMenu`):

- ↑/↓ move through the enabled items and wrap.
- Home and End go to the first and last items.
- A letter goes to the next item whose label starts with it.
- When Enter or Space on the trigger opens the menu, the first item takes the focus.
- ↓ and ↑ on a closed trigger open the menu at its first and its last item (coordinator ruling, 2026-10-08:
  the reference's Base UI menus open from the trigger on ↓/↑, and the user's rule is to match the original).
- When a pointer opens it, the popup takes the focus.
- The menu is modal while it shows, so Escape closes it before the page's own Escape shortcut runs.

That covers the hand-off and Check out menus from #293, merged before this branch.

Contract cannot open a popover from a key handler (X66). A popover menu therefore opens on ↓/↑ through
`KeyMenuOpen`: two invisible `popovertargetaction="show"` invokers over its trigger, armed with
`aria-keyshortcuts` only while the trigger holds the focus. This is the approach fix-provider-auth-state found
for its sign-in method menu. A menu that its owner mounts from state opens from the trigger's own key handler.

## Framework decisions (Charlie, 2026-10-08)

The coordinator reviewed this task against Charlie's decisions on the filed issues:

- **#282 (`showModal`/`close` from an action).** When it lands, the focus plumbing for bugs 16, 6 and 5 goes:
  - the `ask` blur before a confirmation and the dialog's `opener` focus-back;
  - the `covered` Escape and the root `sidebarDialogFocus` gate.
- **#283 (`focusin`/`focusout`).** `KeyMenuWatched`'s `inside` moves to focusin/focusout, with a test on
  `relatedTarget`.
- **#280 (date, time and select as macOS Tab stops).** Custom snooze's date, time and select join the Tab
  order when that fix lands. The tests here do not pin the current Tab cycle.
- **#302 (`outline: none`).** It covers only `input` and `textarea`. The double ring on a keyboard-focused menu
  row (AppKit's ring plus the highlight) is a declared difference, not covered by X61.
- **#278 (accessibility roles).** The `aria-modal` on the 22 popover menus doubles as shortcut scoping. The
  Escape-order pin in `menu-keys.test.ts` stays.

## Scope and exclusions

Included:
- The cause of bugs 4, 5, 6, 13 and 16, and a fix that matches the reference.
- One regression test per bug that fails on the base.
- A live row for each bug, run with real input under the shared lock, base and branch.
- Before/after evidence.
- The shared pattern on the 36 `KeyMenu` call sites. The Settings kit menus (`CnMenu`, `SkPopup`, `ScopeMenu`,
  `CoreMenu`, `TraitsMenu`) carry it to every Settings menu that uses them.
- ↓/↑-to-open on every menu trigger:
  - 31 popover triggers have `KeyMenuOpen`. Six gained a positioned wrapper: Keybindings and Scheduled Tasks
    rows, the Scheduled Tasks environment, `SettingsSelect`, `GhostSelect`, and the snooze clock.
    `R7DevSelect`'s box became positioned.
  - 12 state-driven menus open from their trigger's key handler. The device rail's three menus share one handler;
    the title submenu and the diff turn submenu are submenus.

Excluded:
- The title menu's Custom… (#299, title-custom-snooze). This branch gives only the title menu and its submenu the
  shared arrows (`shell-panels.contract`), merged on top of #299.
- Menus that are not clone popover menus:
  - The composer's slash and @ menus already had their own arrows.
  - The thread, draft and tab context menus are native `NSMenu`s.
  - The Settings icon picker is a Popover in the reference.
  - Listbox selects with a search field (model picker, branch picker) have their own arrows.
- Framework edits. X66 is reported, not changed.
- ↓/↑ on the sign-in method menu's trigger (`providers-setup.contract` `ProviderAccountRow`).
  fix-provider-auth-state owns that trigger's press, Space and Return, and builds its own invisible invoker
  there. Expected merge work: whoever lands second gives the `KmItem` ids that PR's `idPrefix` and lets its
  invoker's press bump this menu's `keyed`.
- The Icon submenu row in Connections (`EnvironmentIconMenu`). It is a row inside a menu, where ↓ moves to the
  next row; a submenu opens on → in Base UI.
- X52 (#280: date, time and select as macOS Tab stops) and X53 (#282: a modal opened from state), filed limits.

## Causes and fixes

| Bug | Cause (reproduced) | Fix | Reference |
| --- | --- | --- | --- |
| 4 (pointer) | The row showed its actions, the snooze clock among them, only while `hovering`. The menu's popover hangs from the clock. Moving the pointer off the row onto the menu ended the hover: the actions collapsed, the clock moved, and the popover moved with it (agent: Custom… from x 158.88 to 242). The release landed outside the menu, which light-dismissed it. On the branch, a pin on the popup's focus alone still failed with the real pointer: a real mouse-down takes the focus from the popup before the click lands (attempt 1 below) | `ThreadRow`: `derive shown = hovering or snoozeFocus or snoozeHover`. The row stays shown while the focus is inside the menu (`KeyMenuWatched` `inside`) or the pointer is on one of its rows (`SnoozeMenuItem` `pointer`) | The sidebar's snooze menu keeps the row's hover actions shown while it is open |
| 4 (↓) | The popup took no focus and had no key handling. The focus stayed on the clock, so ↓ reached nothing | `KeyMenuWatched` around the rows. Each row has an `id`, the clock counts Enter and Space (`snoozeKey`), and the rows highlight on focus (`lit`) | Base UI Menu: `useListNavigation` (`loopFocus`), `useTypeahead`, `focusItemOnOpen: "auto"` |
| 5 | With no focus owner in the menu, the dialog opened with nothing that AX could name as focused. The 1 ms first-stop focus (`sidebarDialogFocus`) did not show, and five Tabs showed no ring. This is inferred from the AX reads and the batch record; the host was not traced | The menu popup now owns the focus (`retainFocus`), so the dialog's first-stop focus lands on Date and time. It shows a ring after a keyboard open. After a pointer open it shows none until the first key, as `:focus-visible` behaves on the web. Then Tab: Cancel, Snooze, Close, each with a ring | Base UI Dialog `initialFocus`; the browser's `:focus-visible` |
| 6 | The dialog's Cancel declares `aria-keyshortcuts="Escape"`. The host's key route answers shortcuts first, so with the palette over the dialog, Escape ran the dialog's shortcut under the palette | `SidebarSnoozeDialog` gets `covered=paletteOpen` (`app-window.contract`, `sidebar-overlays.contract`) and declares no Escape while covered, so the palette's own Escape closes it. The root `sidebarDialogFocus` task's gate gains `and not paletteOpen`, so it runs again as the palette closes and the dialog takes back the focus at Date and time (1 line changed, 0 added) | Base UI's stacked dialogs: Escape closes the topmost; the palette's `finalFocus` |
| 13 | The menu had no focus target and no keys. A painted popover gives the focus to an `autofocus` node inside it, and there was none, so "…" kept the focus | `KeyMenu(menuId="pr-more-keys", modal=true)` over every enabled row, the hand-off rows included. "…" counts Enter and Space (`moreKey`) | Base UI Menu, as for 4 |
| ↓/↑ (ruling) | No key handler can show a popover: `showPopover` is not a host command (X66). The base's triggers took ↓ as nothing | Popover menus: `KeyMenuOpen` (two invisible show-invokers over the trigger, armed by its `focus`/`blur`), reporting the end to the owner, which bumps `keyed` with `kmBump`. A negative count asks `KeyMenu` for the last item (`kmEnd`). State-driven menus: the trigger's key handler opens the menu. When the owner focuses it, it focuses the `-first`/`-last` box; when the data module mounts it, the menu reads `kmEndKeyed(end)`, which the trigger's keys set and its taking the focus clears | Base UI Menu over Floating UI's `useListNavigation` (`openOnArrowKeyDown`); WAI-ARIA menu button |
| 16 | `autofocus` waits while a control holds the focus (the HTML rule; the host applies it only while the first responder is the window or the page). "…" held it, given back by the closing menu, so Cancel's `autofocus` never applied | Every control that asks for a confirmation lets go of the focus first (`ask`: `blur()` then `local("pr-ui-ask", …)`). That covers the primary button, the More rows and Approve workflows. Cancel then takes the focus. Cancel and Escape give it back to the control that opened the dialog (`opener`: primary, Approve workflows, or "…") | Base UI AlertDialog `initialFocus` and `finalFocus` |

Files:
- `menu-keys.contract`, new: `KmItem`, `kmTarget`, `kmOpenKey`, `kmArrow`, `kmBump`, `kmEnd`, `kmKeyEnd`, `kmOpenTarget`,
  `kmEndKeyed`, `KeyMenu`, `KeyMenuWatched`, `KeyMenuOpen`.
- `sidebar-row.contract`, `sidebar-overlays.contract`, `app-window.contract`, `app.contract` (bugs 4–6).
- `pages-pr-actions.contract` (bugs 13 and 16, and the More menu's hand-offs).
- The shared pattern:
  - `pages-prs.contract`, `pages-pr-handoffs.contract`, `pages-pr-quick.contract`, `pages-pr-stack.contract`
  - `shell-panels.contract`, `chat.contract`, `shell-details.contract`, `requests.contract`, `timeline-plan.contract`
  - `settings-kit.contract`, `settings-b-kit.contract`, `settings-rows.contract`, `settings-projects.contract`,
    `settings-scheduled.contract`, `settings-keybindings.contract`, `settings-a-hosts.contract`
  - `connections.contract`, `connections-routes.contract`, `providers-setup.contract`, `r7-device.contract`
  - `legacy-sidebar.contract`, `snapshot.contract`, `pages-usage.contract`, `pages-usage-prices.contract`,
    `pages-hero.contract`
  - `r4-git.contract`, `r4-surfaces.contract`, `r4-surfaces-files.contract`, `r6-polish.contract`,
    `r6-device.contract`, `diff.contract`, `markdown.contract`, `r8-keys-table-menu.contract`

Tests:
- `menu-keys.test.ts`, new: 47 tests.
  - The pattern's wiring and `kmTarget`'s definition (↓/↑ with wrap, Home/End, typeahead).
  - `kmBump` evaluated as written: every bump differs from the last, with the end in its sign.
  - Every menu's `KeyMenu`, with its modality.
  - Every popover trigger that counts its keyboard openings has a `KeyMenuOpen` for its own popover and is armed
    by its own focus (31 triggers).
  - The state-driven openers.
  - Every `menuitem` in a keyboard-menu file has an `id`.
  - The bug 4, 6, 5, 13 and 16 pins.
- `dialog-focus.test.ts`: two expectations follow bug 6, the covered Escape and the new gate.

`app.contract`: 1,478 lines after merging `732f0e3f3`, the same as the base (0 net root lines; one root line changed).

Plan: 4,920 slots and 16.08 MB, against the base's 3,931 slots and 15.23 MB. The difference is each menu's focus
state and its invokers.

## Menus covered

The pattern is on 36 `KeyMenu` call sites.

Popover menus (`modal=true`, 22):
- Sidebar row snooze.
- Pull requests: Sort, provider, Filters and its submenu.
- PR detail: More, Check out, the stack menu, the row stack popover.
- Usage: environment; Usage prices: apply.
- Hero: project picker.
- Legacy sidebar options; SnapShots sound.
- Details git options; add surface.
- Timeline plan card.
- The Settings kits (`CnMenu`, `SkPopup`, `ScopeMenu`, `CoreMenu`, `TraitsMenu`). Through them: connections and
  environment rows, routes, provider sign-in, project scripts and selects, scheduled tasks, keybindings, hosts and
  device selects.

State-driven menus (`modal=false`, 14):
- Thread title menu and its submenu.
- Diff scope and turns.
- Files: crumb menu and editors.
- Linked PR menu.
- Details: scripts and editors.
- Device rail: text, rotate, more.
- Markdown table copy.
- Approval menu.

The trigger of a state-driven menu that the data module opens lets go of the focus first, so the popup can take it.

↓/↑ on a closed trigger:
- All 31 popover triggers that count keyboard openings, through `KeyMenuOpen`. Not the sign-in method menu
  (fix-provider-auth-state's) or the Icon submenu row.
- The 12 top-level state-driven menus, from their trigger's key handler.
- The table Copy menu mounts at the window, outside its trigger's tree. ↓/↑ open it with its popup focused,
  and the next ↓ or ↑ reaches the first or the last item.

## Acceptance and reproduction

| Row | Result | Proof | Blocker |
| --- | --- | --- | --- |
| 4: regression test fails on the base | pass | `menu-keys.test.ts` "#298 bug 4 …" (the base has no `shown` pin, no `KeyMenuWatched`, no row ids) | — |
| 4: real pointer onto Custom… opens the dialog (real input, normal launch) | pass | [ri-01](https://raw.githubusercontent.com/ccheever/exact2/7b9b5ae796f850f58fd70351d7c6f15fec1dc041/fix-keyboard-focus/ri-01-bug4-pointer-onto-custom.png). Base: the menu jumps right and the click closes it, no dialog. Branch (build 7): the menu stays put, and the click opens Custom snooze with the focus on Date and time ([record](https://raw.githubusercontent.com/ccheever/exact2/17d6ad4776619ade65b128e4d6c5d2863321e831/fix-keyboard-focus/real-input-record.txt)) | — |
| 4: ↓ in the pointer-opened menu (real input) | pass | [ri-02](https://raw.githubusercontent.com/ccheever/exact2/004b9b512b8fcfbe86eefeeab25a217e7614edb8/fix-keyboard-focus/ri-02-bug4-arrows-in-pointer-menu.png). Base: the ring stays on the clock. Branch: ↓ ↓ moves to In 3 hours, End to Custom…, and Return opens Custom snooze | — |
| 4: agent pair | pass | [01](https://raw.githubusercontent.com/ccheever/exact2/72fde7663d58257d47a959b824e90f2ff8c552b0/fix-keyboard-focus/01-snooze-pointer-onto-custom.png) and [02](https://raw.githubusercontent.com/ccheever/exact2/3f4ae034ab8fe0c749b43e082f06fcfa68fddea2/fix-keyboard-focus/02-snooze-menu-arrows.png): Custom… stays at x 158.88 (base: moves to 242). The keys go hour, three-hours, custom; Escape returns to the clock ([transcript](https://raw.githubusercontent.com/ccheever/exact2/8ea00a8e206f665d53ef69c82c93cf6524b6bc2a/fix-keyboard-focus/pairs-transcript.txt)) | — |
| 5: regression test fails on the base | pass | `menu-keys.test.ts` "#298 bugs 6 and 5 …" and "#298 bug 4 …" (the popup owns the focus) | — |
| 5: rings in Custom snooze (real input) | pass | [ri-03](https://raw.githubusercontent.com/ccheever/exact2/c0063602d8db0c92e0e1803bbaf4b01a5555d972/fix-keyboard-focus/ri-03-bug5-dialog-rings.png). Base: no ring at open or after five Tabs. Branch: opened from the keyboard, a ring on Date and time, then Tab: Cancel, Snooze, Close, Date and time. Opened by the pointer: the focus is on Date and time with no ring; Tab shows a ring on Cancel, Snooze, Close; Escape returns to the row | — |
| 6: regression test fails on the base | pass | `menu-keys.test.ts` "covered by the palette …" and "a sidebar dialog takes the focus … again", plus `dialog-focus.test.ts` | — |
| 6: ⌘K over Custom snooze, Escape (real input) | pass | [ri-04](https://raw.githubusercontent.com/ccheever/exact2/3e87b1f004f3cc9b47c348d2a23dec2ca474449f/fix-keyboard-focus/ri-04-bug6-escape-order.png). Base: the first Escape closes the dialog under the palette. Branch: it closes the palette, and the dialog keeps a ring on Date and time. The second Escape closes the dialog, with the ring on the row. Agent: [03](https://raw.githubusercontent.com/ccheever/exact2/bdf903ef2daf2402427ab0ae1c6780ebd64d5d87/fix-keyboard-focus/03-escape-order.png) | — |
| 13: regression test fails on the base | pass | `menu-keys.test.ts` "Enter or Space on "…" counts a keyboard opening …" | — |
| 13: More by keyboard (real input, #132) | pass | [ri-05](https://raw.githubusercontent.com/ccheever/exact2/3d384d021b07267ef82d632ccf0b6061b63ccd2a/fix-keyboard-focus/ri-05-bug13-more-menu-keys.png). Base: the focus stays on "…" (AX), and ↓ ↓ and End do nothing. Branch: Return puts a ring on Refresh; ↓ ↓ goes to Explain this PR, End to Close pull request; Return opens the dialog. A pointer open focuses no item; Escape returns to "…". Agent: [04](https://raw.githubusercontent.com/ccheever/exact2/baeabb36509cedc219f71390f92dc8feb3685644/fix-keyboard-focus/04-more-menu-keys.png) | — |
| 16: regression test fails on the base | pass | `menu-keys.test.ts` "the confirmation takes the focus at Cancel …" | — |
| 16: Close pull request? focus (real input) | pass | [ri-06](https://raw.githubusercontent.com/ccheever/exact2/44b02d1f0242389c493711ad9248cbf33145133c/fix-keyboard-focus/ri-06-bug16-close-dialog-focus.png). Base: the focus stays on "…" behind the dialog, and Tab goes to Edit title behind it. Branch: opened from the keyboard, a ring on Cancel, Tab to Close, Escape back to "…". After a real click, Cancel is focused (no ring after a pointer open). Agent: [05](https://raw.githubusercontent.com/ccheever/exact2/a591ec840e4c26dc66984d14e4e5667d6b93d7f4/fix-keyboard-focus/05-close-dialog-focus.png) | — |
| Shared pattern on every menu (agent, base vs branch) | pass | [menus-before](https://raw.githubusercontent.com/ccheever/exact2/74aaea0cd86f7ab37859afe90a48053400de3570/fix-keyboard-focus/menus-before.txt): on the base, the focus never leaves the trigger on any of 9 menus. Branch, after merging `732f0e3f3` ([menus-after-final](https://raw.githubusercontent.com/ccheever/exact2/d2c6ec83ccd14cb340060d84b9a4077780ebadf7/fix-keyboard-focus/menus-after-final.txt)): Enter focuses the first item on 10 menus. ↓ and End move through the items, Escape returns to the trigger, and the title menu's →/← enter and leave its submenu. The Usage environment menu's Escape keeps the page. The row stack popover focuses its popup, then ↓ reaches the first layer | — |
| ↓/↑ on a closed trigger opens the menu at its first or last item (ruling) | pass (agent) | [06](https://raw.githubusercontent.com/ccheever/exact2/72d540ec9d216bdbc7024f6548cb0d2426a8085e/fix-keyboard-focus/06-arrow-down-opens-more.png): on the base, ↓ on the closed "…" does nothing; on the branch, it opens More with Refresh focused, and ↑ opens it at Close pull request. [arrows-after-final](https://raw.githubusercontent.com/ccheever/exact2/f737afbe121bdb78d5c25d111397a3c059323ec9/fix-keyboard-focus/arrows-after-final.txt) lists the rest. First and last both pass on the snooze, Sort, Filters, More, Check out, thread title and Settings › Source Control select menus (a wrapped trigger: the menu opens under it). The diff scope menu passes as well: ↓ first and ↑ last, and a press focuses the popup. The Settings scope menu was checked on ↓ only. Regression tests: `menu-keys.test.ts` "↓ and ↑ on a closed trigger …" (3 tests, which fail on the base) | Agent only. The real-input session was spent before the ruling, and agent keys go through the same host key route ("delivery": "platform"). The details "Open in" menu was not checked with rows: it lists none in this lane |
| Visual and protocol parity with the oracle | not run | — | user decision 2026-10-06: the desktop oracle and trace tools are not built |

## Residuals

- **↓/↑ need invisible invokers** (X66). The table Copy menu is the one menu where ↓/↑ focus its popup, not an
  item.
- **Keyboard-focused rows show the AppKit focus ring as well as the highlight.** The reference shows only the
  highlight; its items are `outline-none` with a highlighted background. This is a declared difference (Charlie,
  2026-10-08): #302 (X61) covers only `input` and `textarea`.
- **The sign-in method menu's ↓/↑** come from fix-provider-auth-state's invoker. The merge wiring is under Scope.
- **Escape in the diff scope menu closes the diff panel with it.** The menu is state-driven and not modal, so the
  panel's own Escape shortcut wins. Not checked on the base; nothing here changed either shortcut.
- **A data-mounted menu's end is cleared when its trigger takes the focus, not on `pointerdown`.** A `pointerdown`
  would take the press from the window's light-dismiss count (#290). Consequence: if the trigger already holds the
  focus from an earlier keyboard opening, and the menu closed without moving it, a pointer press opens the menu at the
  item the keys last asked for.
- **The row stack popover**, opened from the keyboard, focuses the popup rather than the first row: its rows load
  after it opens. ↓ then reaches the first row.
- **State-driven menus** (`modal=false`) are not modal. Whether a page's own Escape shortcut runs before such a menu's
  Escape was not checked on every page. The title menu closes on Escape and gives the focus back to the title (agent).

## Progress

2026-10-08:
- Implemented on `feat(example)/t3-code-fix-keyboard-focus` from `ec32c8c37`, with #293 (merged into the base),
  `732f0e3f3` (#303 and #306) and `84a52dde0` (#290) merged.
- Agent pairs, base `ec32c8c37` (`t3-code-evidence-base`, under its build lock) against the branch.
- Real input under the shared lock, 07:58:34Z–08:28:43Z: base and branch, every bug. One retry, for bug 4's pointer
  row.
- Draft PR #310 opened.
- Then the coordinator's review applied Charlie's decisions and ruled on ↓/↑-to-open (match the reference).
  - Built ↓/↑ on every menu with the invisible invoker.
  - Revised X66 to say this works only through such an invoker, is the popover sibling of #282, and also serves
    #290 and #307.
  - Checked ↓/↑ in agent mode.

## Attempts and evidence

| Attempt | Revision | Checks and outcomes | Evidence | Remaining |
| --- | --- | --- | --- | --- |
| repro (agent) | base `ec32c8c37` | 9 menus: the focus stays on the trigger through ↓, End and Escape. Snooze: Custom… moves from x 158.88 to 242 under the pointer | menus-before, pairs-transcript | — |
| first cut | `77b76eaee` | snooze pin on the popup's focus, More menu, dialog focus through a root task | — | root lines over the budget; replaced by blur-then-autofocus (0 net) |
| every menu | `13473beee` | `KeyMenu` on 36 call sites. Agent: Escape in the Usage environment menu went back a page, because Usage's Escape shortcut ran before the menu's | — | fixed next: painted popover menus are `aria-modal` while they show (`542ee58f0`) |
| real input 1 | build 6 (`542ee58f0`) | 6, 13 and 16 pass; 4's keys and 5 pass. **4's pointer row fails**: the real mouse-down on Custom… takes the focus from the popup, the pin drops, the row collapses, and the menu moves out from under the release | real-input-record | fixed next |
| real input 2 | build 7 (`2fa851b7b`: the pointer on a menu row also pins it) | 4's pointer row passes; the pointer-opened dialog's rings pass | ri-01, ri-03, real-input-record | — |
| before (real input) | base `ec32c8c37`, lane copy | every bug reproduced; Custom snooze could be opened only by an AX press | ri-01–ri-06 | — |
| X66 repro | contract CLI at `4bc1fc9ff`; main `9314e7a81` | `showPopover` refused (`type-unknown-command`), `toggle` refused (`lower-unknown-attr`) | [x66-repro](https://raw.githubusercontent.com/ccheever/exact2/488156a47931ae37a228b3665b0078660c046237/fix-keyboard-focus/x66-repro.txt) | — |
| ↓/↑ 1 (agent) | build 9 | Popover menus pass, first and last: snooze, Sort, Filters, More, Check out. **The thread title menu focused its popup, not an item.** The key action set `titleEnd` and read it back in the same action, but a Contract action reads the state as it began. Reading the same way, the approval and script menus' toggles moved the focus only on close | arrows-after-final | fixed next: the end comes from the key itself; a toggle tests `not open`, the state as it began |
| ↓/↑ 2 (agent) | builds 10–11 | Title first/last pass; the Settings select (wrapped trigger) and the scope and diff scope menus pass. The details "Open in" menu never took the focus, on a press either: the trigger kept it, so the popup's `autofocus` waited. Fixed in build 11 (the trigger lets go first, the menu reads its end as it mounts). In this lane the menu then lists no editors, so it has no rows to check | arrows-after-final, 06 | not checked with rows |
| merge #290 | `84a52dde0` | EXACT2-GAPS rows X65 (#290's) and X66 kept side by side. `usage-pooled.test.ts` failed: #290's light dismiss counts every press at the window, and the six `pointerdown` handlers that cleared a data-mounted menu's end took those presses | — | fixed: the end clears on the trigger's `focus` |
| ↓/↑ 3 (agent) | build 12 | Diff scope after the merge: ↓ first, ↑ last, a press focuses the popup. Escape closes the panel with the menu (residual) | arrows-after-final | — |
| checks | CHECKS-REV | CHECKS | — | — |

Real-input apparatus:
- Lane copies of the macOS client ("T3 Code (Lane FKF base/after)"), each launched with `env -i` and its own home
  directories (HOME, CFFIXED_USER_HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_*) and `T3CODE_TELEMETRY_ENABLED=false`.
- Paired to a private lane server on 127.0.0.1:16181 (`daehyeonmun2021/playground`).
- Pointer by cliclick; keys as HID events, each sent only after checking that the lane app is frontmost; focus read
  from the AX tree.

The build-7 copy is a new ad-hoc identity, so a keychain prompt asked for the lane keychain's item. It was denied. The
stale item was deleted from that lane keychain only, and the copy was re-paired through Add environment, with no
capture while the code was on screen. The user's keychain search list is unchanged.

## Next action

The coordinator reviews and merges the draft PR. X66 is unpublished; the coordinator files it after review.
