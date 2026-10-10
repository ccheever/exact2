# macOS: let an app declare its menu bar items (context-menu submenus landed in #223)

**Status:** Open
**Systems:** Contract, host/apple macOS, menus
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/141

## Current scope

Add selected Contract menu extensions: chordless items, placement and stable removal identifiers; preserve standard menus/Develop in development. Check localization/Help/enabled state/agent reachability without a new operation. Submenus/Paste-as-Text/Speech already landed.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

## Summary
On macOS the host builds a fixed menu bar (app, File, Edit, View, Go, Window, Develop). An app adds items to it only through buttons that declare a ⌘ chord; the host files each one by its chord. An app cannot add an item without a chord, a menu (Help), a standard role item (Speech), or remove a host item. A Contract context menu becomes a native `NSMenu` with check marks, but a row that opens another menu is not a submenu. Ask: (1) a declared application menu, or app control over the host's items, and (2) submenus in context menus.

## Why it matters
A coding-agent desktop app has its own menu bar: "Check for Updates…" (no chord) in the app menu, "Paste as Text" (⇧⌘V) in Edit, Speech in Edit, Actual Size / Zoom In / Zoom Out in View, a Help menu, and no developer menu in release. Its row context menus have submenus (Copy ▸ path / link / id) and check marks.
Today an app must write host code that finds the host's menus by title ("Edit", "View", "Develop") at run time, edits them, and builds its own `NSMenu`s for nested context menus. A host change or another UI language breaks this without notice, and the agent cannot reach those custom menus.

## Current behavior (exact2 4c893fef6)
Menu bar of the minimal app, read with System Events from a launched development build (`bun exact.mjs mac --run`; separators, which System Events reports as `missing value`, are shown as `–`; the Window menu is left out):
```
Apple, ExactMac, File, Edit, View, Window, Develop
ExactMac: About X26 App Menu | – | Settings… | – | Services | – | Hide X26 App Menu | Hide Others | Show All | – | Quit X26 App Menu
File: Paste as Text | – | Close Window | Close All
Edit: Undo | Redo | – | Cut | Copy | Paste | Delete | Select All | – | AutoFill | Start Dictation… | Emoji & Symbols
View: Show Tab Bar | Show All Tabs | – | Zoom In | Actual Size | – | Enter Full Screen
Develop: Reload | Open Project… | App Info… | Save Trace
```
| Sub-capability | Verdict | Observation |
|---|---|---|
| Item from a ⌘ button | already-supported | `Settings…` (⌘,) in the app menu; `Zoom In` (⌘=) and `Actual Size` (⌘0) in View (`contract-grammar.md:1033-1042`). |
| Item placed where the app wants | reproduced | `Paste as Text` (⇧⌘V) is filed under File, not Edit; placement follows the chord table only. |
| Item without a chord, a new menu, a role item | reproduced | No form exists: `contract vocab menu` / `menubar` / `menuitem`: "not a tag or an attribute"; `app.schema.json` has no menu key. "Check for Updates…", Help, Speech cannot be declared. |
| Remove host items | reproduced | Host and AppKit items stay (File ▸ Close All, View ▸ Show Tab Bar, Edit ▸ AutoFill). `Develop` hides only with the launch variable `EXACT_DEV_MENU=0` (`DevMenuMac.swift:88`); the bar is then `Apple, ExactMac, File, Edit, View, Window`. |
| Empty Go menu | already-supported | `Go` is absent when no button files into it. |
| Context-menu check marks | already-supported | `aria-checked` sets `NSMenuItem.state` (`MenusMac.swift:382`). |
| Context-menu submenus | reproduced | `MenusMac.swift:368-390` builds one flat `NSMenu` and never sets `submenu`; LLP 1021:615 lists submenus as not done ("First consumer brings the nesting rules"). A `role="menuitem"` row with `popovertarget="copy-menu"` becomes a plain item. |
| Agent chooses a context-menu item | already-supported | Under the agent the menu opens as its popover: `tap row contextmenu`, `tap m-pin`, `tap m-copy-path` run the actions (`press view 13 (pin)`, `press view 20 (did)`). |

## Expected behavior
The web has no application menu; Electron's `Menu.setApplicationMenu(template)` / `Menu.popup` is the closest model. ARIA allows nested menus (`role="menu"` inside a `menuitem` with `aria-haspopup`).
- **Proposal A (macOS).** An app-declared template (in `app.json` or Contract): menus, items with `label`, `role` for standard items (about, services, hide, hideOthers, unhide, quit, close, undo, redo, cut, copy, paste, pasteAndMatchStyle, delete, selectAll, startSpeaking, stopSpeaking, togglefullscreen, window, help), `accelerator`, `enabled`, `checked`, `visible`, submenus and separators; a click runs a Contract action and says whether the accelerator triggered it. The template replaces the host's menus, so Develop and Go are present only if declared (Develop always kept in development builds, if maintainers prefer).
- **Proposal B (smaller).** Keep the chord filing, and add: an explicit menu name on a ⌘ button (for example `menu="Edit"`), items without a chord, a Help menu, and a way to drop host items by stable identifier.
- **Submenus.** A `role="menuitem"` whose `popovertarget` names another `role="menu"` popover (or `aria-haspopup="menu"`) becomes an `NSMenuItem.submenu` on macOS and a nested `UIMenu` on iOS; the web and the agent open the nested popover.
- iOS: not applicable to the menu bar (iPadOS menu bar is a possible later step). Web and Linux: no menu bar.

## Reproduction
Minimal app (`app.contract`; `app.ts` has no sources):
```contract
component X26Menu
  state log = ""
  state pinned = false
  action did(what: string)
    log = `${log} ${what}`
  action pin
    pinned = not pinned
  view
    column testId="root" padding=20 gap=8
      button "Settings…" press=did("settings") aria-keyshortcuts="Meta+," testId="settings"
      button "Zoom In" press=did("zoom-in") aria-keyshortcuts="Meta+=" testId="zoom-in"
      button "Actual Size" press=did("actual") aria-keyshortcuts="Meta+0" testId="actual"
      button "Paste as Text" press=did("paste-text") aria-keyshortcuts="Shift+Meta+v" testId="paste-text"
      button "Row" contextPopover="row-menu" testId="row"
      column id="row-menu" popover="auto" role="menu" aria-label="Row actions"
        button "Pinned" press=pin aria-checked=pinned role="menuitemcheckbox" popovertarget="row-menu" popovertargetaction="hide" testId="m-pin"
        button "Copy ▸" popovertarget="copy-menu" role="menuitem" testId="m-copy"
        button "Delete" press=did("delete") popovertarget="row-menu" popovertargetaction="hide" role="menuitem" testId="m-delete"
      column id="copy-menu" popover="auto" role="menu" aria-label="Copy"
        button "Copy path" press=did("copy-path") popovertarget="copy-menu" popovertargetaction="hide" role="menuitem" testId="m-copy-path"
      text log testId="log"
```
| Step | Command / action | Host | Actual | Expected |
|---|---|---|---|---|
| 1 | `bun scripts/exact.mjs new target/repro/x26-app-menu`, paste files, `contract build --json`, `bun exact.mjs mac` | – | `[]`, exit 0 | same |
| 2 | `bun exact.mjs mac --run`, then `bash app/menus.sh <pid>` | macOS 26.6 | bar above; Paste as Text in File; Develop shown | app-declared bar (proposal) |
| 3 | `EXACT_DEV_MENU=0 bun exact.mjs mac --run` | macOS | no Develop | – |
| 4 | Right-click "Row" in the launched app (code read: `MenusMac.swift:368-390`) | macOS | flat menu, "Copy ▸" is a plain item | "Copy" with a submenu |
| 5 | `agent macos "tap row contextmenu" "tap m-pin" … "tap m-copy-path"` | macOS | actions run (log ` copy-path actual`) | same (supported) |

### Evidence
The files are attached at the end of this issue, in folded sections.
- `evidence/X26/repro.log`: revision, build, the System Events menu listing (with and without `EXACT_DEV_MENU=0`), vocab and schema checks, agent JSON and logs, and the quoted `MenusMac.swift` lines.
- `evidence/X26/app/`: `app.contract`, `app.ts`, `app.json`, `menus.sh` (reads the menu bar of a pid with System Events).

## Acceptance criteria
- An AppKit test (or an agent `menus` read, to be defined) lists `NSApp.mainMenu` titles, items, key equivalents and enabled state; for a declared template they equal the template, with no host-only items.
- A declared item without a chord runs its action; an item whose accelerator the page already handled does not run its action twice.
- Step 4 shows "Copy" with a submenu holding "Copy path"; choosing it runs `did("copy-path")`. The same nesting opens as nested popovers on the web and under the agent.

## Notes
- Not tested: step 4 with a real right-click (it would need real input); the submenu verdict is from the host source and LLP 1021. iOS `UIMenu` nesting not checked.
- Decision needed before implementation: template in `app.json` vs. Contract, whether Develop stays in development builds, and the nesting rules LLP 1021 §5 defers to "the first consumer".
- Related: LLP 1021 (menus), `contract-grammar.md:1033-1042` (the Mac's menu bar).

---

<details><summary>Evidence: <code>repro.log</code></summary>

````text
exact2 4c893fef65f1bf8d6e4be6df9265e2ae49790637 (2026-10-05), macOS 26.6.2

$ bun scripts/exact.mjs new target/repro/x26-app-menu   # then app/app.contract, app/app.ts
$ bun exact.mjs contract build app.contract --json
[]
$ bun exact.mjs mac   # exit 0

$ bun exact.mjs mac --run; bash app/menus.sh <pid>   # System Events reads the menu bar (read-only)
Apple, ExactMac, File, Edit, View, Window, Develop
ExactMac: About X26 App Menu | missing value | Settings… | missing value | Services | missing value | Hide X26 App Menu | Hide Others | Show All | missing value | Quit X26 App Menu
File: Paste as Text | missing value | Close Window | Close All
Edit: Undo | Redo | missing value | Cut | Copy | Paste | Delete | Select All | missing value | AutoFill | Start Dictation… | Emoji & Symbols
View: Show Tab Bar | Show All Tabs | missing value | Zoom In | Actual Size | missing value | Enter Full Screen
Window: Minimize | Minimize All | Zoom | Zoom All | Fill | Center | missing value | Move & Resize | Full Screen Tile | missing value | Remove Window from Set | missing value | Bring All to Front | Arrange in Front | missing value | Show Previous Tab | Show Next Tab | Move Tab to New Window | Merge All Windows | missing value | missing value | X26 App Menu
Develop: Reload | Open Project… | App Info… | Save Trace

$ EXACT_DEV_MENU=0 bun exact.mjs mac --run; menu bar items:
Apple, ExactMac, File, Edit, View, Window

$ bun exact.mjs contract vocab menu | menubar | menuitem
`menu` is not a tag or an attribute
`menubar` is not a tag or an attribute
`menuitem` is not a tag or an attribute
$ grep -n -i menu scripts/app.schema.json
234:          "description": "The name a native host shows: the menu bar, the app switcher, Finder.",

$ bun exact.mjs agent macos "tap row contextmenu" "tap m-pin" "tap row contextmenu" "tap m-copy" "tap m-copy-path" "type root key Meta+0" logs
{"at":[210,133],"contextmenu":true,"delivery":"platform","tapped":10,"target":"row","carrier":"macos","mode":"agent","epoch":1,"incarnation":1,"clock":0}
{"at":[210,151],"tapped":13,"delivery":"platform","target":"m-pin","carrier":"macos","mode":"agent","epoch":2,"incarnation":1,"clock":0}
{"at":[210,133],"contextmenu":true,"delivery":"platform","tapped":10,"target":"row","carrier":"macos","mode":"agent","epoch":2,"incarnation":1,"clock":0}
{"at":[210,169],"tapped":15,"delivery":"platform","target":"m-copy","carrier":"macos","mode":"agent","epoch":2,"incarnation":1,"clock":0}
{"tapped":20,"at":[210,187],"delivery":"platform","target":"m-copy-path","carrier":"macos","mode":"agent","epoch":3,"incarnation":1,"clock":0}
{"key":"Meta+0","typed":1,"value":"","target":"root","delivery":"platform","carrier":"macos","mode":"agent","epoch":4,"incarnation":1,"clock":0}
t=0 boot: 22 nodes, epoch 1
t=0 press view 13 (pin) → epoch 2 (+0 −0 ~1)
t=0 press view 20 (did) → epoch 3 (+0 −0 ~1)
t=0 press view 6 (did) → epoch 4 (+0 −0 ~1)
  Text#22 [log] " copy-path actual"

# Source: host/apple/Sources/ExactKit/Mac/MenusMac.swift:368-390 (menu(of:from:)) builds one flat NSMenu: sectionHeader from aria-label, separators from hr,
#   item.state from aria-checked; no item.submenu is ever set.
    func menu(of pop: NodeView, from source: NodeView? = nil) -> NSMenu {
        let menu = NSMenu()
        menu.autoenablesItems = false
        let once = Picked()
        // The popover's `aria-label` titles the menu ("Open location in").
        if let heading = pop.props["accessibilityLabel"], !heading.isEmpty { menu.addItem(.sectionHeader(title: heading)) }
        for case let row as NodeView in pop.container.subviews {
            if row.props["contextPreview"] == "true" { continue } // a context menu's preview (§5.1)
            if row.props["semanticTag"] == "hr" { menu.addItem(.separator()); continue }
            guard row.isButton else { continue }
            let item = NSMenuItem(title: title(of: row), action: #selector(pick(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = Pick(row, in: pop, from: source, presentation: presentation(of: pop),
                                          title: item.title, once: once)
            item.state = row.props["accessibilityChecked"] == "true" ? .on : .off
            // As a chooser's: a hidden or inert row is shown, never chosen.
            item.isEnabled = !row.disabled && !row.inert && shown(row, in: pop)
            item.image = image(of: row)
            menu.addItem(item)
        }
        return menu
    }
    /// One menu's items share this: the first item taken is its only one.

# llp/1021-menus.rfc.md:615-616:
- **Submenus** — `UIMenu` nests and ARIA allows it; nothing here needs
  it. First consumer brings the nesting rules.
# host/apple/Sources/ExactKit/Mac/DevMenuMac.swift:102-182 builds the fixed bar (app, File, Edit, View, Go, Window, Develop); :88 enabled = EXACT_DEV_MENU != "0"
````
</details>

<details><summary>Evidence: minimal app sources (<code>app/</code>)</summary>

`app.contract`

````contract
component X26Menu
  state log = ""
  state pinned = false
  action did(what: string)
    log = `${log} ${what}`
  action pin
    pinned = not pinned
  view
    column testId="root" padding=20 gap=8
      button "Settings…" press=did("settings") aria-keyshortcuts="Meta+," testId="settings"
      button "Zoom In" press=did("zoom-in") aria-keyshortcuts="Meta+=" testId="zoom-in"
      button "Actual Size" press=did("actual") aria-keyshortcuts="Meta+0" testId="actual"
      button "Paste as Text" press=did("paste-text") aria-keyshortcuts="Shift+Meta+v" testId="paste-text"
      button "Row" contextPopover="row-menu" testId="row"
      column id="row-menu" popover="auto" role="menu" aria-label="Row actions"
        button "Pinned" press=pin aria-checked=pinned role="menuitemcheckbox" popovertarget="row-menu" popovertargetaction="hide" testId="m-pin"
        button "Copy ▸" popovertarget="copy-menu" role="menuitem" testId="m-copy"
        button "Delete" press=did("delete") popovertarget="row-menu" popovertargetaction="hide" role="menuitem" testId="m-delete"
      column id="copy-menu" popover="auto" role="menu" aria-label="Copy"
        button "Copy path" press=did("copy-path") popovertarget="copy-menu" popovertargetaction="hide" role="menuitem" testId="m-copy-path"
      text log testId="log"
````

`app.json`

````json
{
  "$schema": "../../../scripts/app.schema.json",
  "name": "X26 App Menu",
  "short_name": "X26 App Menu",
  "id": "com.example.x26-app-menu",
  "start_url": "/",
  "display": "standalone",
  "app": {
    "id": "com.example.x26-app-menu",
    "name": "X26 App Menu"
  },
  "host": {
    "ios": {
      "minimumOS": "17.0",
      "deviceFamily": [
        "iphone",
        "ipad"
      ]
    },
    "macos": {
      "minimumOS": "14.0",
      "window": {
        "width": 900,
        "height": 700
      }
    },
    "web": {}
  },
  "deploy": {
    "store": {
      "web": "0",
      "macos": "0",
      "ios": "0",
      "linux": "0"
    }
  }
}
````

`app.ts`

````ts
import type { Answer, Sources } from './app.contract.d.ts';

export const appId = 'com.example.x26-app-menu';
export const grants = '';

const sources: Sources = {};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
````

`menus.sh`

````sh
P=$1
osascript -e "tell application \"System Events\" to tell (first process whose unix id is $P) to get name of every menu bar item of menu bar 1"
n=$(osascript -e "tell application \"System Events\" to tell (first process whose unix id is $P) to count menu bar items of menu bar 1")
for m in $(seq 2 $n); do osascript -e "tell application \"System Events\" to tell (first process whose unix id is $P) to tell menu bar item $m of menu bar 1 to return (name as text) & \": \" & (my j(name of every menu item of menu 1))" -e 'on j(l)
set AppleScript'"'"'s text item delimiters to " | "
set s to l as text
return s
end j'; done
````

</details>

## Discussion at transfer

### daehyeon-mun — 2026-10-07T08:09:39Z

## Decision needed for the rest
#223 landed context-menu submenus: a native `NSMenu` submenu on macOS, a nested `UIMenu` on iOS, and nested popovers on the web and under the agent. #226 files ⇧⌘V/⌥⇧⌘V under Edit right after Paste and adds Edit ▸ Speech, as Apple's HIG and TextEdit do. That part needs no decision.

**[Design]**: an accepted LLP decides otherwise, or the API has to be chosen first. It needs a ruling before implementation.

**Open:** an app-declared menu bar.
- Where the template lives (`app.json` or Contract).
- Chord-less items, a Help menu, and dropping host items (Show Tab Bar, AutoFill, Close All).
- Whether Develop stays in development builds.

LLP 1021 §5 defers this.

**Options:**
- **A.** A full template that replaces the host's menus (Electron's `Menu.setApplicationMenu`).
- **B.** Keep the chord filing, and add a `menu=` name on a ⌘ button, chord-less items, a Help menu, and removal of host items by stable identifier.

**Recommendation:** B, in Contract, because items run Contract actions and the agent can reach them. Develop stays in development builds. A duplicates what the chord filing already does well.

**Cost:** B medium.

### ccheever — 2026-10-08T08:07:36Z

**Decision: Choose declarative menu extensions in Contract.**

Keep open with the bounded scope below.

Items invoke Contract actions, so Contract is the right home. Keep the host's standard menus and add chordless items, explicit menu placement and stable removal identifiers.

Keep Develop in development builds. Test localized menus, Help, enabled state and agent reachability. Context submenus and Paste-as-Text/Speech already landed; do not rebuild them.
