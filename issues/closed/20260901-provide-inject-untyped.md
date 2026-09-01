# Provide/inject is not checked against the inject's declared type

**Status:** Closed
**Resolution:** Provide and inject composition now enforces duplicate names and value type compatibility.
**Systems:** Contract compiler
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1017 P4a

Prop arguments on a source `Use` are unified with the target prop type. `provide name = expr` is only inferred. The inliner fills injects by name. The expanded root has no `Use` nodes left, so a number provided for `inject accent: string` is not compared to `string`.

If that value lands in a positional `text`/`image`/`iframe` argument, lower also skips `check_prop_value` (named attrs do not). The runner bridge will stringify a number into a `Str` prop, so `text 1` boots as `"1"` instead of `lower-attr-type`. A bool still fails at apply (`PropKind`).

Fix: type each provide against every inject it fills (or type the subst'd value against the inject `Ty` during expand). Run `check_prop_value` on positional props the same as named ones. Fixture: `provide accent = 1` into `inject accent: string` used as `text accent`.
