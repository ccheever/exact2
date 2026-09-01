# Linux rejects normally formatted JSON envelopes

**Status:** Closed
**Resolution:** Linux now decodes bounded typed envelope JSON independent of key order, whitespace, and escapes, accepts live/additive fields, and rejects duplicate recognized fields.
**Systems:** Linux host
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1023.001

Linux extracts envelope fields with exact substring searches such as
`"exact":1` and `"url":"..."` (`host/linux/src/fetch.rs:56-68,151-163`). A
valid indented JSON envelope with spaces after colons is rejected as “no exact
version” and the host silently falls back to its baked plan. Escapes and other
normal JSON spellings fail similarly.

Replace the scanner with a bounded typed JSON decoder. Ignore unknown additive
fields as LLP 1023.001 requires, refuse duplicate recognized fields, and add
compact, pretty-printed, escaped, reordered, live-tier, malformed, duplicate,
and oversized fixtures.
