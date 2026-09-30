# Seven vendored Taffy patches are marked "to upstream" and none has been sent

**Status:** Open
**Systems:** vendored Taffy
**Severity:** P4
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** vendor/taffy/EXACT-PATCHES.md (patches 10, 11, 12, 14, 17, 18, 20), LLP 1074 §0.1

`EXACT-PATCHES.md` marks patches 10 (percentage padding basis), 11 (the cache keyed on every input), 12 (`aspect-ratio` as CSS), 14 (an item's contribution clamps before its margin), 17 (replaced elements never stretch to a grid area or insets), 18 (one absolute-box solver) and 20 (`Position::Static` and the containing block) as upstream material. Each is written against upstream 0.14.0 with Chrome-differential cases the upstream test suite could take. None has been prepared as a pull request or sent (DioxusLabs/taffy). Both reviewers of LLP 1074 said the same: worth doing, as separate maintenance work, one patch per PR, with fixtures. Until then every Taffy upgrade re-applies them by hand.
