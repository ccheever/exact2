# Code review: exact2's menu rule (a popover a menu row opens, opened from the menu's own invoker), round 1, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `unknown`.
- **Method:** one brief (sha256 `9a35f02fdd0cd203144f5014764e3da02e29209928e177bd0853c71d2a49096e`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** all findings taken in bb2bb7835b

**VERDICT: WRONG**

The web rule and the iOS lift do what the ruling asks. On a real Mac menu they do not: choosing the row runs after the menu entry is already gone, so the popover never opens and is never re-anchored. That is the production path for a menu-shaped `role="menu"`, including a context menu.

1. **Blocker** — A native Mac menu row never opens its non-menu popover. `host/apple/Sources/ExactKit/Mac/MenusMac.swift:300`

`show` re-sources only while an open `role="menu"` entry still contains the row, then refuses a hidden source (`:300` and `:307`). A real menu is an `NSMenu` (`:327`). `pick` (`:844`) only records the choice. `track` (`:116`) runs `closed` first, and that `close` (`:349`, `:439`, `:444`) drops the entry and sets the menu popover `isHidden`. The queued `press` (`PresenterMac.swift:873`) then calls `show` with the row. The holding menu is gone, the row's ancestor is hidden, and `show` returns. The same sequence is a context menu (`MenusMac.swift:392`): `menu(of:)` is built with no invoker, and the long-pressed node is only the entry's source, which `close` has already removed. A menu-shaped popover that is not `role="menu"` used to be a submenu (`submenu(of:)` at `:678` now requires the role) and now takes this item path, so it no longer opens either. `MenuRowPopoverMacTests.swift:10` forces the painted presentation and calls `show` while the entry is still open (`:57`). It never picks an `NSMenu` item.

2. **Should-fix** — A web row that opens with `commandfor` still nests, so the menu takes the popover with it. `host/web/menu-rows.js:15`

The listener only matches `button[popovertarget]`. `commandfor` / `command="show-popover"` (and `toggle-popover`) is the same click default action, so `preventDefault` would stop it, but the row never gets there. `Invocation.swift:39` already treats `commandfor` as the open and ignores `popovertarget` when it is set, so iOS `items` and Mac `show` apply the rule to that spelling and the web host does not. The menu-rule paragraph (`docs/contract-for-agents.md:1318`) and `lower-menu-row-popover` only name `popovertarget`. Nothing in the repo uses `commandfor` to show a popover from a menu row (native-fixture's bar row is `show-modal`).

3. **Should-fix** — `host/web/frame-messages.test.mjs` loads `commands.js` and will not resolve the new import. `host/web/frame-messages.test.mjs:9`

The page does `import {commands} from '/commands.js'`, and `host/web-js/commands.js:8` is a static `import "./menu-rows.js"`. This server answers `/menu-rows.js` with the HTML document. The module fails, `mount` never runs, and the test waits on iframes that are never created. `commands.test.mjs` serves the file; this allowlist was not updated. `field-range` and `markup-editor` fall back to `host/web/` and still resolve it.

Checked, and not filed:

- `preventDefault` on `click` cancels the button's default action in the HTML activation behavior, which is specified as always coming from a `click`. Enter and Space on a `<button>` synthesize that `click`, so the capture listener sees them. `setTimeout` runs after the row's own `press` (the listener does not stop propagation; the wasm `press` is synchronous on `click`) and after `pointerup`, which is when Firefox and WebKit light-dismiss. A source outside the menu makes the menu close inside `showPopover` and leaves the new popover up.
- A submenu's `exactOpener` is the row that opened it (`menu-rows.js:21`). A popover from that submenu therefore anchors in the parent menu: the submenu closes, the parent stays. iOS threads the original invoker (`MenusIOS.swift:1453`, `ContextMenusIOS.swift:298`) and `lift` anchors to that view (`MenusIOS.swift:743`). Both keep the menu you chose from from dismissing the popover. `contextAt` records the long-pressed node (`navigation.js:941`). `popoverCommand` does not, so an action-opened menu has no `exactOpener` until something clicks it.
- The JS-target import does not enlarge the wasm boot graph. `boot.mjs` still allows only `glue.js` and `navigation.js`. The wasm host injects `menu-rows.js` with a script tag after paint (`glue.js:21`). The JS target imports it from `commands.js` the same way it already imports `prompt-glue.js` and `alert-message.js`, and `stageHostGlue` copies it beside the runtime.
- `submenu(of:)` now requires `role="menu"` on both Apple hosts, on top of the old shape test. In-repo menus that are menus already have the role. On iOS a non-menu target becomes an action and `agentShow`s from the opener even when the row has no `press` handler. Confirmations still go to `openConfirmation`.
- The note is literal-only, on the expanded tree (component uses are inlined before `notes`). `in_menu` staying set through a non-menu popover nested in a menu matches the web rule's `closest`. native-fixture's "Open in…" row (`app.contract:454`, target `role="alertdialog"`) will be noted; that matches the written rule. Notes rank with ordinary warnings and count toward the 20-warning cap; one such row does not crowd it out.
- Line counts are under the 1500 cap: `MenusIOS.swift` 1492, `glue.js` 1499, `warnings.rs` 1499.
