# Native-view props named like events bind to the box silently; transform on a box is deferred

**Status:** Open
**Systems:** contract lowering (native.rs, analyze handlers, svg.rs transform), docs/recipes/native-views.md
**Severity:** P2
**Author:** Claude (Fable 5.1), from the Brooks shop-app lane's report
**Date:** 2026-10-19

## Symptoms

1. A native-view module prop named like an event (`playing=someAction`) binds as a box handler that never fires, with no diagnostic; `src` is refused with `lower-native-attr` (since `d40504c29b`), but `docs/recipes/native-views.md:14` still writes `video-view src=…`.
2. `transform="translate(0px, 25px) scale(0.8)"` on a box is refused with the individual-properties message (`BOX_TRANSFORM_REFUSAL`, `contract/lower/src/vocab.rs:128`; LLP 1055.000 D5, `QUEUE.md`), though the individual `translate`, `rotate` and `scale` rows carry it on every host.

## Causes

1. `leftover()` (`contract/lower/src/native.rs:61`) binds every known attribute to the box first; `refused()` (`:81-120`) diagnoses text, form and other-element props but not handlers. A module can emit only `press, change, hover, focus, blur, key, submit, load, message` (`NativeModule.swift:427, 1142`); any other event name on a module tag is dead. `analyze-unknown-action` on a module tag lacks the module hint.
2. The ruling deferred the general property (matrices, skews, arbitrary order), not the subset the individual properties already express.

## Fix

Refuse, on a module tag, a handler the module cannot emit, with the module hint; add the hint to `analyze-unknown-action` there; fix the recipe. Lower a box `transform` that is a `translate`, `rotate`, `scale` list in CSS's individual-property order to those rows, and keep the refusal for the rest.

## Fixed (2026-10-19), the handler half

On a module tag a handler the module cannot send (anything but `press`, `change`, `hover`, `focus`, `blur`, `key`, `submit`, `load`, `message`) is refused as `lower-native-attr` with the module hint; `analyze-unknown-action` on a module tag carries the hint too; the recipe's example uses `source`. Tests: `contract/lower/src/native.rs`, `contract/cli/tests/it/corpus.rs::a_module_tag_keeps_svg_names_and_names_the_fix_for_known_ones`. The transform half is on the branch `worktree-agent-adcf2f4883b2a1c52`.
