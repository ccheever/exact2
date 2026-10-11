# Linux keeps focus on a native button that opens a dialog; the web moves it into the dialog

**Status:** Open
**Systems:** host/linux (dialog presentation, focus)
**Author:** Claude (Opus 5.5), for Charlie Cheever
**Date:** 2026-10-08
**Severity:** P2
**Related:** LLP 1104 r11 (buttons native by default); host/web-js/conformance/native-buttons (synthetic-native-buttons)

Strict conformance with the Linux reference (`conform.mjs --synthetic --linux --strict`) fails at `synthetic-native-buttons tap open-dialog`: after the tap, the wasm oracle marks the dialog `focused` and the button not, as `showModal()` does on the web (focus moves to the dialog or its first focusable descendant); the Linux reference keeps the button `focused` and the dialog unfocused. Found by the async lane at be15451e (first in ca2ae925..be15451e).

The web's behaviour is the standard (HTML's dialog focusing steps). The Linux presenter should move focus into a dialog it opens modally, as the web does, and return it to the opener when the dialog closes.
