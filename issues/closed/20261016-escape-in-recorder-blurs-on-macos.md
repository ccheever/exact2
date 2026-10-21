# macOS: Escape in a keycapture field whose placeholder changes blurs it, where the web keeps it

**Status:** Closed
**Resolution:** fixed: a placeholder write that restarts a focused field's editing is no blur or change (NodeView.keepingEditing)
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

`-[NSTextField setPlaceholderAttributedString:]` on a field that has the
field editor and no text ends its editing and begins it again
(`-[NSCell _restartEditingWithTextView:]`), so the editor shows the new
placeholder. The end reached `controlTextDidEndEditing`, which sent the
field's `blur`; the recorder's `blur` ended `taking` and unmounted the row.
With text in the field AppKit does not restart, which is why a recorded key
kept the focus, and why the earlier wire-batch test (a field with a value)
heard no blur.

## Fix

`NodeView.keepingEditing` (FieldEditingMac.swift) wraps the placeholder
write: while AppKit restarts the editing of a field that keeps the focus, its
end is no blur and no `change`. If the editing does not come back, the end is
heard as before. `KeyCaptureEscapeMacTests` covers the recorder's shape in a
session and the wire batch on an empty field.
