# Child action parameters are captured by props

**Status:** Closed
**Resolution:** Lifted child actions now closure-convert lexical captures while declared parameters correctly shadow them.
**Systems:** Contract compiler
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1017.000

Component inlining adds props to `child_subst`, then rewrites child action
bodies with that substitution (`contract/syntax/src/inline.rs:124-190`). The
statement substitution does not remove names bound by the action's own
parameters (`:493-553`).

If a child has prop `value` and action `capture(value: string)`, using the
child with `value="prop"` rewrites the action's parameter reference to the prop.
The program compiles, but a change event writes `"prop"` instead of its event
payload. This is silent semantic corruption.

Make substitution binder-aware: action/function parameters, each items, match
bindings, and every other lexical binder must shadow outer substitutions. Add
a runtime fixture that dispatches a distinct payload and observes it.
