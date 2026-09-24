# A Markdown blank line shifts the selection by one

**Status:** Closed
**Resolution:** Not a defect: Chrome Range.toString() over the blank-line BR is empty, not a newline; real editor selection across the BR copies no source unit. Verified in headless Chrome on 2026-09-24; no code change.
**Systems:** Markdown editor, Web host
**Severity:** P2
**Author:** Grok 4.7 for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1045

An empty source line is a `<br>` so the caret has a box (`host/web/markup-editor.js`). `toSource` measures with `Range.toString()`, and a range that covers that `<br>` includes a newline. That newline is not in the line's source. The real `\n` is only the join between lines. `reconcile` uses `textContent`, which does not count the `<br>`.

For source `a\n\nb`, the blank line starts at 2. A range that covers only the `<br>` reports 3, the start of `b`. Delete and reconcile then disagree about the caret. A collapsed caret the browser reports as `(line, 0)` stays correct, because `toDom` places before the `<br>`. Anything that includes the `<br>` does not.

Measure the visible text, and do not count the caret's `<br>` as a source unit. Done when a selection across a blank line maps back to the same source offsets `reconcile` writes.
