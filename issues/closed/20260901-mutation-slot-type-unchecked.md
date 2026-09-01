# Mutation slots are not type validated

**Status:** Closed
**Resolution:** Plan validation now requires every mutation result slot to be global and to match its option result type.
**Systems:** Plan, Runner
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1005

The plan format records a mutation result type and an arbitrary slot, but plan
semantic validation never verifies that the slot is global and has type
`option<T>` (`plan/tables/format.json`, `plan/src/lib.rs:197-288`). The builder
documents the invariant without enforcing it.

At runtime both immediate and fulfilled answers check only the inner `T`, then
write `Some(value)` directly into the named slot. A decoder-valid plan can
therefore violate the slot table's declared type or overwrite a row-owned
placeholder that is never observed.

Validate the mutation-to-slot relation in both builder and decoder semantics,
and retain a defensive runner check before assignment. Add malformed-plan
fixtures for wrong type, non-option type, and row-owned slot.
