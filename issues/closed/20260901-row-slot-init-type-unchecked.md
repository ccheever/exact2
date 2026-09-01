# Row initializers skip slot type validation

**Status:** Closed
**Resolution:** Row slot initializers now validate their declared type before a new row is published.
**Systems:** Runner
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1005

Global boot evaluates each slot initializer and checks conformance, returning
`SlotType` on mismatch (`runner/src/runner.rs:453-477`). New `each` rows
evaluate and insert owned-slot initializers without checking the declared type
(`runner/src/instance.rs:479-490`).

A validated or externally produced plan can therefore publish malformed row
state and fail later when expressions assume the declared type.

Apply the same conformance check while realizing a new row, before publishing
the instance update. Add a malformed-plan test with a row initializer whose
value disagrees with its slot type.
