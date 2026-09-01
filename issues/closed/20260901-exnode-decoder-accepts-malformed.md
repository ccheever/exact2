# EXNODE decoder accepts malformed envelopes

**Status:** Closed
**Resolution:** EXNODE decoding now validates section structure, scalar domains, topology, and trailing data before allocation or publication.
**Systems:** Kernel
**Severity:** P3
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1001

The EXNODE decoder is described as a validating envelope, but
`kernel/src/export.rs:227-360` accepts multiple noncanonical or inconsistent
forms: nonzero reserved header data; duplicate, overlapping, or header-pointing
sections; payload trailing bytes; unknown row flag bits; noncanonical booleans;
duplicate props; nonfinite frames/float props; and parent/depth topology that
does not agree.

Production currently does not consume this decoder outside tests, so exposure
is low, but accepting these forms makes future persistence/debug consumers
unsafe to add under the documented contract.

Validate directory topology and exact section consumption, then validate all
decoded scalar and tree invariants before returning. Add one mutation fixture
per refusal class and stable decoder errors.
