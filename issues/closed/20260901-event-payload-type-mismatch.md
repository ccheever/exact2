# Event handlers ignore payload type mismatches

**Status:** Closed
**Resolution:** Handler payloads now unify with action parameters across every event kind and reject mismatches at the handler.
**Systems:** Contract compiler, Runner
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1006

The type pass knows the event payload types (`change`, `key`, and `message` are
strings; `hover` is boolean) but discards failed unification with an explicitly
typed action parameter (`contract/types/src/lib.rs:1215-1225`). Analysis checks
only arity. Thus `input change=changed` with `changed(value: number)` compiles;
the first real input is refused by the runner as `ArgumentType`.

Turn failed event/action unification into a compile-time diagnostic at the
handler attribute, and add fixtures for every event kind plus an unannotated
parameter that should infer successfully.
