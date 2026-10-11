# macOS: ⌘Z and Edit › Undo do nothing in a plain `textarea` (the undo reaches the window's manager, not the field's)

**Status:** Open
**Systems:** host/apple macOS, text editing
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/315

## Current scope

Route plain textarea keyboard/Edit-menu undo and redo to its own manager with correct enabled state. Preserve Markdown history and per-editor authoritative-value reset. The web agent Meta+z limitation is not a browser undo failure.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

On macOS, a plain `textarea` keeps its own undo history: `TextArea` overrides `undoManager` with its own manager (`textUndo.manager`, `host/apple/Sources/ExactKit/Mac/TextAreaMac.swift:17`), so an authoritative value write can reset that editor's history alone. The host's Edit menu sends `undo:` and `redo:` to the first responder with a nil target (`host/apple/Sources/ExactKit/Mac/DevMenuMac.swift:147-148`).

`NSTextView` does not implement `undo:`. The action therefore travels up the responder chain to `NSWindow`, which undoes the window's own manager. None of the text's steps are there. ⌘Z, ⇧⌘Z, Edit › Undo and Edit › Redo do nothing in the field.

The Markdown editor (`markup="markdown"`) does undo. `TextArea.performKeyEquivalent` sends ⌘Z and ⇧⌘Z straight to the editor's own manager, but only `if markup != nil` (`TextAreaMac.swift:85-92`).

So every plain `textarea` in an Exact macOS app ignores the platform's undo, unless the app routes its own menu. In a browser a `<textarea>` undoes on ⌘Z and Edit › Undo, and so does an `NSTextView` in any AppKit app.

The consumer is T3 Code's composer and its Appearance prompt preview. Both are Exact textareas, and a real-input pass found ⌘Z, ⇧⌘Z and Edit › Undo doing nothing in either. The clone now lays its own Edit menu over the host's and sends Undo and Redo to the focused text view's manager. Every app would have to do the same.

### Current and expected behavior

- **Current (macOS):**
  - Type `hello` into a plain `textarea`, then press ⌘Z. The value stays `hello`.
  - The agent's chord goes through `NSApp.mainMenu?.performKeyEquivalent`, the path the menu's key equivalent takes. Its reply has no `value`, which means the menu took the chord.
  - The same steps in a `textarea markup="markdown"` leave it empty.
- **Expected:** ⌘Z and Edit › Undo undo the focused textarea's last edit, and ⇧⌘Z and Edit › Redo redo it, as in a browser's `<textarea>` and in `NSTextView`. For example, `TextArea` could answer `undo:`/`redo:` on its own manager, or the Edit items could act on the first responder's `undoManager`.
- **Web:** the web host's `textarea` is a real `<textarea>`, so the browser owns its undo. The agent's web `Meta+z` is a CDP key event without Chrome's editing command, so it cannot show this: the plain field keeps `hello` there too, and only the Markdown editor, which handles the chord itself, undoes. That row is recorded as a limit of the driver, not as evidence.

### Reproduction and evidence

App made with `bun scripts/exact.mjs new <dir>`, with no data sources:

```text
component X63app
  state draft = ""
  state md = ""
  action write(value: string)
    draft = value
  action writeMd(value: string)
    md = value
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding=24 gap=12 background-color="light-dark(#ffffff, #111111)" color="light-dark(#111111, #eeeeee)"
      textarea testId="ta" value=draft input=write aria-label="Plain" width=320 height=60
      text `plain: [${draft}]` testId="echo"
      textarea testId="md" markup="markdown" value=md input=writeMd aria-label="Markdown" width=320 height=60
      text `markdown: [${md}]` testId="echo-md"
```

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Plain textarea, ⌘Z | `bun exact.mjs mac`; `bun exact.mjs agent macos --size 420x300 "type ta hello" "clock settle" "tap ta" "type ta key Meta+z" "clock settle" tree` | macOS 26.6.2, Apple Silicon | main `b896050d7` | the chord's reply is `{"typed":2,"key":"Meta+z",…}` (the menu took it); then `TextInput#2 [ta] value="hello"`, `plain: [hello]` | `value=""`, `plain: []` | [record](https://raw.githubusercontent.com/ccheever/exact2/690838bdab6e3cfdf7b646666a8c2530fd26bca4/file-x48-x68/x63-record.txt), [screenshot](https://raw.githubusercontent.com/ccheever/exact2/e482386f1dcc93903e9cbc248435c3fac8b35dbb/file-x48-x68/x63-macos.png) |
| Markdown textarea, ⌘Z (control) | same session: `"type md hello" "clock settle" "tap md" "type md key Meta+z" "clock settle" tree` | same | same | `TextInput#4 [md] value=""`, `markdown: []` | (undoes) | same |

![macOS window after the drive: the plain field still reads hello; the Markdown field is empty](https://raw.githubusercontent.com/ccheever/exact2/e482386f1dcc93903e9cbc248435c3fac8b35dbb/file-x48-x68/x63-macos.png)

`TextAreaMac.swift` and `DevMenuMac.swift` were last changed in `5b2b77339`; the lines cited above are main `b896050d7`'s.

### Acceptance criteria

- In the repro, ⌘Z after typing in `ta` empties it on macOS, through the agent's chord and with a real key. ⇧⌘Z brings `hello` back.
- Edit › Undo and Edit › Redo are enabled exactly when the focused textarea can undo or redo, and act on it.
- The Markdown editor keeps its own behavior. An authoritative `value` write still clears that editor's history only.
- iOS: the keyboard's undo and the shake/three-finger gestures act on a UIKit textarea's own history, if `TextAreaIOS` keeps a separate manager too (not checked here).

### Constraints and related work

- Workaround: an app lays its own Edit menu over the host's and sends Undo and Redo to the focused text view's `undoManager`. This needs native code in every app.
- Not the same as #125 or #275. Those ask for app control over undo steps (ending an undo group, `setRangeText` as one step, a cancelable `beforeinput` with `historyUndo`/`historyRedo`). All of them assume that the platform's undo already reaches the field. This bug is that it does not.
- Not tested: iOS, a real key press (the screen was locked; the agent's chord takes the main menu's key-equivalent path), and an `input` (single-line field).
- Related: #141 (app-declared menu bar items).
