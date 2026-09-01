# Image `src` containment does not resolve symlinks; fonts and the web arm do

**Status:** Closed
**Resolution:** Apple image loading now resolves symlinks and confines local sources to the canonical asset root.
**Systems:** Apple host
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1020 fold (web arm `localDocument` resolves symlinks), LLP 1011

`NodeView.resolveSource` on both presenters `standardizedFileURL`s then prefix-checks against `EXACT_ASSETS`. `Text.fontURL` and `WebArm.localDocument` resolve symlinks on the root and the candidate first, then prefix-check. A relative `imageSource` through a symlink under the asset root reads outside it.

Same class as the LLP 1020 fold, left on images.

Fix: resolve symlinks on both the asset root and the candidate before the prefix check, matching `Text.fontURL`. A test that a symlink under `EXACT_ASSETS` pointing at `/etc/passwd` (or a file outside the app dir) is refused.
