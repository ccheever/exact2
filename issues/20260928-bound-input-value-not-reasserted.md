# A bound `value` that an input action leaves unchanged keeps the typed text

**Status:** Fixed (this PR)
**Systems:** Web host (`glue.js` `writeValue`), Apple host (native text fields), runner (prop diffing)
**Severity:** P2
**Author:** Claude Fable 5.1, building Ocho for Eliot Hertenstein
**Date:** 2026-09-28
**Related:** LLP 1045 D5 (the changed-middle write), LLP 1008 (the native text field)

Ocho's launcher has `input value=typed input=typedChanged`, and `typedChanged`
consumes a complete two-digit code: it picks the row and sets `typed = ""`.
Before the keystroke `typed` was `""`; after the action it is `""` again, so the
commit carries no change to the `value` row and the host writes nothing — the
field keeps showing `03` while the app believes it is empty. A React-style
controlled input re-asserts the bound value after every input event; here the
diff is against the last committed value, not against what the element shows.

Reproduced on the web with `scripts/agent.mjs web --app ocho "tap new-session"
"type launcher-code 03"` before the workaround landed: the row for `03` was
picked, the step advanced, and the field still read `03`.

The workaround in Ocho mounts a fresh input per step (`when launchStep == 1` …
`3`, each with its own id and `autofocus`), so the element itself is new.

A fix in the host: after dispatching an `input` event, if the element's bound
`value` (the app's, as last committed) differs from `el.value`, write it — the
row is the app's, and an unchanged row means "what I had", not "whatever you
typed". The Apple text field arm has the same question.
