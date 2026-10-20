# Native-view props named like events bind to the box silently; transform on a box is deferred

**Status:** Closed
**Resolution:** fixed by bbd846b41 (event-named props refused on a module tag) and aefedf88d (a box transform of translate, rotate and scale lowers to those rows)
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

On a module tag a handler the module cannot send (anything but `press`, `change`, `hover`, `focus`, `blur`, `key`, `submit`, `load`, `message`) is refused as `lower-native-attr` with the module hint; `analyze-unknown-action` on a module tag carries the hint too; the recipe's example uses `source`. Tests: `contract/lower/src/native.rs`, `contract/cli/tests/it/corpus.rs::a_module_tag_keeps_svg_names_and_names_the_fix_for_known_ones`. The transform half is below.

## Fixed (2026-10-19), the transform half

The transform half (symptom 2). `contract/lower/src/box_transform.rs` rewrites a box's `transform` attribute, before `check_svg`, into the rows it equals when its value is `translate(…)` (`translate`, `translateX`, `translateY`), `rotate(…)` (`rotate`, `rotateZ`) and `scale(…)` (one factor, or two equal ones), each at most once, in CSS's individual-property order; `none` is no row. It reads the literal wherever the lowering sees one: inline, through a class, and in each arm of a choice (`transform=(c ? "scale(0.8)" : "none")`, `class=(c ? Scaled : Shifted)`), with `none` on an arm that lacks a row, as a class choice's rows are built. Everything else keeps the `lower-attr-tag` refusal, now ending with a clause naming the value's own fault: `matrix`, skews, `scale(x, y)` with x ≠ y, `scaleX`/`scale3d`, 3D functions, a function repeated, functions in another order, and a bound string; a `transform` beside an explicit `translate`, `rotate` or `scale` row on the same element is refused too, naming the row. SVG elements keep the whole property. Tests: `contract/cli/tests/it/svg.rs` (`box_transforms_lower_to_their_rows_inline_through_classes_and_choices`, `box_transforms_are_refused_inline_and_through_classes`). `contract vocab transform`, `docs/contract-for-agents.md`'s refusal table and `QUEUE.md`'s entry say what is accepted and what stays deferred.

Symptom 1 (the native-view handler props) is still open.
