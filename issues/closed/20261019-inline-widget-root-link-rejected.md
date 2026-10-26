# Lock Screen inline widget: a root link renders "Open <app>"

**Status:** Closed
**Resolution:** fixed by bbd846b41: the Swift inline admission reads the wire's button as the compiler reads a pressable
**Systems:** widgets (ExactSnapshot), Apple host wire, contract snapshot compiler
**Severity:** P2
**Author:** Claude (Fable 5.1), from the Brooks shop-app lane's report
**Date:** 2026-10-19

## Symptom

An `accessoryInline` widget whose root is a `link` holding one `text` compiles, but the widget renders the fallback "Open <app>".

## Cause

The two sides disagree on the node's name. A `link` lowers to `NodeType::Pressable` (`contract/lower/src/tags.rs:132`), and the compiler's inline admission matches the kernel enum, so `Pressable` descends like a `View` (`contract/cli/src/snapshot/inline.rs:47-55`). The Apple wire names that type `"button"` (`kind_for`, `host/apple/src/host.rs:1475`); nothing emits `"pressable"`. Swift's `admitsInline` (`host/apple/Sources/ExactSnapshot/SnapshotDriver.swift:123-138`) descends only into `"view"` and `"pressable"`, counts the `"button"` root as an unsupported node, logs "accessoryInline requires one text/time leaf and at most one image" and draws the fallback (`SnapshotDriver.swift:191-193`). The `"time"` case there is dead the same way: `time` arrives as `"text"` with `semanticTag == "time"`.

## Fix

Make the Swift admission match the compiler's: descend into `"button"` as the wire name of a pressable. `SnapshotDriverTests.swift` builds its nodes with kind `"view"` only; add a `"button"` root.

## Fixed (2026-10-19)

`admitsInline` descends into `"button"`, the wire's name for a pressable, and counts a text leaf as `"text"` alone (`time` arrives as a text with `semanticTag == "time"`), so a root `link` holding one line admits as the compiler admits it. `SnapshotDriverTests.testInlineGuardRejectsEveryNonContainerLeaf` holds a `button` root with one text and refuses a `button` of `type="progress"`.
