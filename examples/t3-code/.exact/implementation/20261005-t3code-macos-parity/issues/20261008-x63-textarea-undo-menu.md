---
name: 20261008-x63-textarea-undo-menu
plan: 20261005-t3code-macos-parity
status: published
kind: framework-gap
blocks: []
upstream_url: https://github.com/ccheever/exact2/issues/315
reproduced_on: b896050d7 (main, agent mode, before filing)
---

# X63: Edit › Undo and ⌘Z do nothing in a plain `textarea` on macOS

**Status (reclassified 2026-10-08):** Bucket 2: #315 is fixed in open main PR #327 (ccheever, full). T3 waits, then removes #306's Edit › Undo routing.

## Summary

On macOS, Exact's `textarea` keeps its own undo manager (`TextArea.undoManager`, `textUndo`, so an
authoritative value write resets only that editor's history). The host's Edit menu sends `undo:` and
`redo:` to the first responder (`DevMenuMac.swift`: `Selector(("undo:"))`, target nil). No `NSTextView`
implements `undo:`; the action travels the responder chain to `NSWindow`, which undoes the **window's**
manager, where none of the text's steps are. The menu item reads enabled (the text view can undo), and
choosing it, or pressing ⌘Z, changes nothing. Only a Markdown editor (`markup="markdown"`) undoes,
because `TextArea.performKeyEquivalent` routes ⌘Z/⇧⌘Z straight to its own manager when `markup != nil`.

Every `textarea` of an Exact macOS app without its own menu routing is affected. The web host's
`<textarea>` undoes as the browser does.

## Why this issue arose

The real-input batch (#298, bug 3) found ⌘Z, ⇧⌘Z and Edit › Undo doing nothing in T3 Code's composer
and in the Appearance prompt preview, both Exact textareas taken by the clone's `T3ComposerEditor`
(the editor's undo grouping and the `PromptPreviewEditorTests` undo test act on the text view's manager
directly, so they passed). The clone's own Edit › Undo (`R8KeysMenus.swift`) also sent `undo:` to nil.

## Where it lives (main fa965d3e2)

- `host/apple/Sources/ExactKit/Mac/TextAreaMac.swift`: `override var undoManager: UndoManager? { textUndo.manager }`;
  `performKeyEquivalent` handles ⌘Z/⇧⌘Z only `if markup != nil`.
- `host/apple/Sources/ExactKit/Mac/DevMenuMac.swift`: `edit.addItem(withTitle: "Undo", action: Selector(("undo:")), keyEquivalent: "z")`
  and Redo likewise, target nil.
- AppKit: `NSTextView.instancesRespond(to: "undo:")` is false; `NSWindow`'s is true, and it acts on
  `window.undoManager`.

## How to reproduce

Against the host's own `TextArea` (the same scaffold as `MarkupEditorTests.editor`, without `markup`),
compiled with every `host/apple/Sources/ExactKit` file and the app's Rust core as the README's
`timeline-keyboard` recipe does (`-package-name ExactKit`):

```swift
let node = NodeView(id: 3, kind: "textarea", presenter: Presenter())
node.frame = NSRect(x: 0, y: 0, width: 400, height: 200)
node.makeTextArea(); node.props["value"] = ""; node.handlers = ["input"]; node.applyTextArea(); node.layoutTextArea()
let window = NSWindow(contentRect: node.frame, styleMask: [.titled], backing: .buffered, defer: false)
window.contentView = node
let f = node.textArea as! TextArea
window.makeFirstResponder(f)
f.insertText("typed", replacementRange: NSRange(location: NSNotFound, length: 0))
f.tryToPerform(Selector(("undo:")), with: nil)   // what Edit › Undo sends
```

| Step | Actual (main fa965d3e2) | Expected |
| --- | --- | --- |
| after typing | `typed`; the text view's manager `canUndo` true, the window's false | — |
| `undo:` through the responder chain (Edit › Undo, ⌘Z through the menu) | handled (by `NSWindow`), text still `typed` | text empty, as `<textarea>` in Chrome after ⌘Z |
| `TextArea.performKeyEquivalent(⌘Z)` | false, text still `typed` | — (the menu path is enough) |

In an app: a one-field view with `textarea`, type, press ⌘Z or choose Edit › Undo: nothing changes, and
Edit › Undo stays enabled. Real keys on the clone's base build (#298 bug 3, the real-input batch on
`b7761f556`): the composer and the prompt preview kept the text.

## Not the same as #275, #276 or the rest of #125

Checked on 2026-10-08 (`gh issue view 275|276|125 -R ccheever/exact2`):

- **#125** (closed by #209, the clipboard part) asked, among the textarea editing primitives, for "a way to end
  the current undo group" and for `historyUndo`/`historyRedo` visible as `beforeinput` input types. Its undo
  half is about *controlling* undo steps from the app, not about undo reaching the editor at all.
- **#275** (open) carries that remainder: `selectionchange` on a field, `setRangeText` as one undo step, and a
  cancelable `beforeinput` with `historyUndo`/`historyRedo`. All three assume the platform's undo already acts
  on the field; none says that it does not.
- **#276** (open) is the policy question of atomic inline ranges (chips) over a plain string. Unrelated.

X63 is a host bug under all of them: the textarea's undo history is fine (the editor registers steps, and its
own `undoManager` can undo them), but the user's ways in (⌘Z, ⇧⌘Z, Edit › Undo, Edit › Redo) never reach it,
because `TextArea` overrides `undoManager` and does not answer `undo:`/`redo:`, so the action lands on `NSWindow`
and its separate manager. Fixing #275 would not fix this, and fixing this needs none of #275.

## Requested support

Edit › Undo, Edit › Redo, ⌘Z and ⇧⌘Z act on the focused textarea's own history on macOS (for example,
`TextArea` implementing `undo:`/`redo:` on its manager, or the Edit items targeting the first responder's
`undoManager`), as a browser's `<textarea>` does; and on iOS whatever the shake/three-finger gestures and
the keyboard's undo use, if the UIKit editor keeps its own manager too.

## Clone-side

Not blocking: the clone already lays the reference's menu bar over the host's (`R8KeysMenus.swift`) and
retargets Edit › Undo there; [fix-misc-batch](../tasks/20261008-fix-misc-batch.md) makes that item (and
now Edit › Redo) act on the focused text view's own manager, which matches the reference (Electron's
`undo`/`redo` roles act on the focused editor). Remove that routing when the host undoes its textareas.

## Filed upstream (2026-10-08)

Filed as [#315](https://github.com/ccheever/exact2/issues/315) ([Bug] macOS: ⌘Z and Edit › Undo do nothing in a plain `textarea` (the undo reaches the window's manager, not the field's)), reproduced on main `b896050d7` in agent mode before filing (evidence under
`file-x48-x68/` on `t3-code-evidence`). Under the framework vs T3 split (user, 2026-10-08) the fix is framework work;
the clone's side waits for main fix of #315, then an adoption round.

- **Fixed by [#327](https://github.com/ccheever/exact2/pull/327)** (open on main, 2026-10-08); this resumes in the main-adoption round after it merges. Then #306's own Edit › Undo and Redo routing in `R8KeysMenus.swift` (to the focused text view's undo manager) goes.
