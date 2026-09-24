# A legal windowed-list measurement is applied, then the runner is poisoned

**Status:** Open
**Systems:** Runner, Lists
**Severity:** P2
**Author:** Grok 4.7 for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1010

`list_viewport_within` rejects a row taller than `f32::MAX` before it touches the tree. It allows a height equal to `f32::MAX`. `ListWindow::measure` then stores those heights and only afterwards checks that their sum fits in a layout coordinate (`runner/src/instance/window.rs`). Two mounted rows reported at `f32::MAX` pass the preflight and fail the sum.

That error is an `InstanceError::List`, not `InvalidCollectionFeedback`. `update_tree` has already put the mutated tree back, and it poisons on every such error (`runner/src/runner/lists.rs`). Virtualized collections preflight the extent and return `InvalidCollectionFeedback` without poisoning (`runner/src/runner/collection.rs`). By the time the sum is refused, `window.top`, `port`, and `origin` have also been overwritten.

Check the extent before writing the window, and leave the runner usable when the report is only too large. Done when two `f32::MAX` row heights are a typed refusal, the previous window is still the one on screen, and a later commit still runs.
