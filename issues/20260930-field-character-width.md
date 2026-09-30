# A text field's 20 characters are wider on native than on the web

**Status:** Open
**Systems:** kernel, Apple host, Linux host
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-30
**Related:** issues/closed/20260930-native-input-stretches.md, LLP 1069.001

A text field at `field-sizing: fixed` (the default) is 20 characters wide, as HTML's `<input>` is. The kernel measures that as the width of twenty `0`s in the field's font (`kernel/src/arena.rs` `text_runs_after`, `NodeType::TextInput`). Chrome uses the font's average character width instead. With the same font (`system-ui`, 16 px, macOS 27), Chrome's content box is **175 px** and the macOS host's is **195.31 px** (20 × 9.77, the width of `0`). 175 / 20 = 8.75 px matches SF Pro's OS/2 `xAvgCharWidth` (1120 of 2048 units at 16 px), which is what Blink's `LayoutTextControlSingleLine` multiplies by when the font has a valid one. A textarea (20 columns) differs the same way: Chrome 183 px, macOS 195.31 px.

Measured 2026-09-30 with a fixture of fields in block, flex, absolute and percentage containers, driven on web, macOS and Linux (`agent.mjs <host> --plan inputs.plan layout`). After the stretch fix, every case agrees in kind; only this width differs.

To match: the text measurer needs one more question, the font's average character width (CoreText: the OS/2 table through `CTFontCopyTable`; cosmic-text: the face's OS/2 table), with `0`'s width as the fallback when a font has none, as Blink does. The kernel would then multiply by the field's size (20) where it now measures the string.
