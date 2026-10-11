# Reconcile the live Markdown editor DOM after multi-line replacement

**Status:** Closed
**Resolution:** Live DOM reconciliation and cache invalidation, complete-source Select All replacement, and browser regression coverage for editing and undo.
**Systems:** Web Markdown editor, Shared editor integration, JS and wasm web hosts
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P1
**Related:** LLP 1045 D1 and D5, host/web/markup-editor.js:119 and 218

Replacing a multi-line selection leaves deleted lines in the source reported to the app, while removing them from the visible editor. Undo can then throw and empty the editor DOM.

Verified in headless Chrome against a freshly built JS web fixture, after waiting for the real Markdown editor to mount:
```contract
component App
  state draft = "one\n\ntwo"
  action changed(value)
    draft = value
  view
    column
      textarea id="editor" testId="editor" markup="markdown" value=draft input=changed
      text draft testId="echo"
```
Click the editor, press Cmd+A, type `replacement`. The live DOM contains only `<div class="md-line">replacement</div>`, but `editor.value` and the bound echo are `replacement\n\ntwo`. Cmd+Z throws `NotFoundError: ... insertBefore ... not a child of this node`; the source becomes the old document while the DOM becomes empty. Redo remains empty. A richer document also retains deleted link/fence fragments.

Build the fixture with `bun host/web-js/build.mjs markdown-stress --contract <fixture.contract> --out <directory> --render none`, serve that directory, and wait until the editor is a contenteditable DIV before driving it. This reproduction does not depend on typing during module loading or injecting a new value into the editor.

In `reconcile`, structural changes are read from the cached `lines` array, including nodes the native edit detached, rather than the current DOM. The incremental renderer subsequently treats retained cached tail nodes as insertion anchors even when they are no longer children.

Reconstruct the source from live native DOM changes with the appropriate hidden-syntax mapping. Reconcile or invalidate cached line identity before diffing; never use a detached node as an anchor. Preserve IME and native typing behavior.

Acceptance: real browser selection/replacement across paragraphs, blank lines, formatting and lists gives identical visible text, source and app value. Undo/redo restores consistent DOM/source without errors. Exercise composition and source-mode round trips through the same glue; shared Rust editor tests alone do not cover this failure.

Fix verified in Chrome, Firefox and WebKit through the production glue and shared wasm. Browser tests cover paragraph/blank-line replacement, formatted links and fences, task lists and quotes, native edits that omit `beforeinput`, composition, preserved native typing nodes, controlled value echoes, undo/redo, and source-mode transfer. Structural reconciliation reads the live DOM and invalidates the line cache; Select All replaces the complete source, including hidden syntax. The Markdown stress app was also built and driven through both JS and wasm web hosts.

## Recovery audit (2026-10-08)

The browser Edit-menu/mobile Select All path also spans hidden syntax; a partial heading selection still preserves the heading and following body. Independent Chrome and WebKit probes cover native Select All, undo and composition. Boundary text is inspected from each end of the DOM rather than scanning the whole document during selection changes.
