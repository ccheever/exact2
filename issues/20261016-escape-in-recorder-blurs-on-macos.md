# macOS: Escape in a keycapture field whose placeholder changes blurs it, where the web keeps it

**Status:** Open
**Systems:** Apple host (fields, key routing)
**Severity:** P2
**Author:** Claude (Opus 5.5)
**Date:** 2026-10-17
**Related:** realinput-1017k F-2, FN33

## Problem

A shortcut recorder as the T3 Code clone writes it: an `aria-modal` row,
shown while `recording or taking`, holding an `input` with `autofocus`,
`keycapture` (Escape calls `capture("escape")`, which ends `recording`;
`taking` keeps the row up) and `blur` (which ends `taking`). Escape changes
only the field's `placeholder` ("Press shortcut" to "Unassigned"). On the web
the field keeps the focus and the row stays (`agent web`). On macOS the field
hears `blur` right after the Escape's commit, the row unmounts and the focus
is on the window (`agent macos` and real keys alike, with or without
VoiceOver):

    tap add-keybinding · tap field-new · type recorder-new key Escape · logs
    keycapture view 22 (keys#5) → epoch 5
    blur view 22 (took#4) → epoch 6

Recording a key instead ("k", which also changes the value) keeps the focus.
In realinput-1017k F-2 the clone lost the field to the window on the same
Escape with VoiceOver on (keys alone kept it, FN33), so this may be the same
blur under different timing.

## Cause

Not found. Ruled out: a placeholder change on a focused field fires no blur
(an XCTest wire batch), and the prevented Escape does not reach `menus.key`
or `dialogs.key`. Next: break on `FocusEvents` blur for the field during the
Escape's dispatch to see who resigns the field editor.

## Fix

Whatever resigns the field: a prevented Escape must leave the focus where it
is, as the web does.

