---
name: 20261008-x63-textarea-undo-menu
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: https://github.com/ccheever/exact2/issues/315
reproduced_on: b896050d7 (main, agent mode, before filing)
---

# X63: Edit › Undo and ⌘Z do nothing in a plain `textarea` on macOS

Moved to main `issues/20261009-macos-textarea-native-undo.md` (2026-10-09); tracked there.

## Summary

On macOS, Exact's `textarea` keeps its own undo manager, but the host's Edit menu sends `undo:` and `redo:` to the
first responder with a nil target. No `NSTextView` implements `undo:`, so the action reaches `NSWindow`, which undoes
the **window's** manager, where none of the text's steps are. ⌘Z, ⇧⌘Z, Edit › Undo and Edit › Redo change nothing in a
plain `textarea`; only a Markdown editor (`markup="markdown"`) undoes.

## Why it arose

The real-input batch (#298, bug 3) found ⌘Z, ⇧⌘Z and Edit › Undo doing nothing in T3 Code's composer and in the
Appearance prompt preview, both Exact textareas taken by the clone's `T3ComposerEditor` (the editor's undo grouping and
the `PromptPreviewEditorTests` undo test act on the text view's manager directly, so they passed). The clone's own
Edit › Undo (`R8KeysMenus.swift`) also sent `undo:` to nil.

## Clone workaround

Not blocking: the clone already lays the reference's menu bar over the host's (`R8KeysMenus.swift`) and retargets
Edit › Undo there; [fix-misc-batch](../../tasks/closed/20261008-fix-misc-batch.md) (#306) makes that item (and Edit › Redo)
act on the focused text view's own manager, which matches the reference (Electron's `undo`/`redo` roles act on the
focused editor). Remove that routing when the host undoes its textareas.

## Evidence and history

- Reproduced against the host's own `TextArea` on main `fa965d3e2` (a Swift scaffold built like `MarkupEditorTests`,
  without `markup`: after typing, `undo:` through the responder chain is handled by `NSWindow` and the text stays), and
  with real keys on the clone's base `c0475fbaa` (#298 bug 3, the real-input batch on `b7761f556`: the composer and the
  prompt preview kept the text).
- Checked on 2026-10-08 against #125, #275 and #276: they assume undo already reaches the field; this is a host bug under
  all of them.
- Filed as [#315](https://github.com/ccheever/exact2/issues/315) on 2026-10-08, reproduced on main `b896050d7` in agent
  mode before filing (evidence under `file-x48-x68/` on `t3-code-evidence`).
