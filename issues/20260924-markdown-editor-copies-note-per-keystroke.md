# Markdown editor copies the whole note on every keystroke

**Status:** Open
**Systems:** Markdown editor, Web host
**Severity:** P2
**Author:** Grok 4.7 for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1045

Plain typing is left to the browser and read back on `input` (`host/web/markup-editor.js`). `reconcile` joins every line's `textContent` and `put`s the full source into the wasm buffer. `put` writes UTF-16 one code unit at a time. `render` then builds a comparison string for every line before the prefix/suffix diff. A long note pays a full copy plus a full pass per character.

The module is fetched only when a Markdown textarea mounts, so this is not a boot cost. It is a hitch while editing Interview or Fieldnotes. The shared editor already knows the selection and the inserted text from `beforeinput`; the read-back does not have to resend the document.

Send the changed range, or reuse the source the wasm already holds, and stop rewriting every line key when the line did not change. Done when a multi-thousand-line note's ordinary typing does not copy the whole source or rebuild every line's key.
