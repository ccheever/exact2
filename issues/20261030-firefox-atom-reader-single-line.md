# In Firefox, Markdown Stress's single-mode atom reader is a line taller than in Chrome

**Status:** Open
**Systems:** web host reader pills (`host/web/atoms.js`), the JS target in Firefox, cross-browser conformance (`host/web-js/conformance/known-firefox.json`)
**Severity:** P3
**Author:** Claude (Opus 5.5), from #1148's conformance runs
**Date:** 2026-10-30

## Symptom

After `tap toggle-single`, Markdown Stress's left-to-right atom reader (`atom-reader`, `atomText` drawn with `atoms="citation mention skill"`) is 133 px tall in Firefox and 107 px in Chrome. `conform.mjs markdown-stress --browser firefox --strict` fails on `layout.atom-reader.h` and, below it, `layout.markup-node.y` at each of the five single-mode steps: `tap toggle-single`, `tap markup-editor`, `tap format-bold`, `tap format-italic` and `tap format-heading`. `known-firefox.json` names neither.

## Measured

- On origin/main at `78ae0e76ac`, with `--build --build-js` and a wasm root of its own: `atom-reader.h` is 107.00 in Chrome and 133.00 in Firefox at all five steps, and `markup-node.y` is 1003.39 against 1028.40 (1058.39 against 1083.40 at `tap format-heading`).
- The reader's `x`, `y` and `w` agree within the 4 px tolerance, so its box is as wide in both. At its 26 px line height, Firefox breaks its text into one more line.
- WebKit reports no difference on either box (#1148's branch, where this reader is main's).
- The reader is drawn only in single mode (`if single` in `apps/markdown-stress/app.contract`). It came with #997. Its text is two paragraphs with six pills and no icons, among them two citations with no space between (`[B](cite:2)[C](cite:3)`).

## Cause

Not yet known: which line Firefox breaks differently, and why. Comparing the reader's line boxes in the two engines would show it.

## Notes

- The five Firefox `layout.reading-column.h` entries in `known-firefox.json` are measured with this extra line in the column. A fix moves each by 26 px.
- #1148 puts its right-to-left fixture above this reader, so that none of its boxes moves with this difference.
- Repro: `bun host/web-js/conform.mjs markdown-stress --browser firefox --strict --build --build-js --wasm-root /tmp/<yours> --out /tmp/<yours>-out`, with Homebrew's OpenSSL first on `PATH` (`docs/agent-pitfalls.md`).
