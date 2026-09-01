# EXWF decoder ignores nonzero padding

**Status:** Closed
**Resolution:** EXWF decoding now requires every payload-alignment padding byte to be zero.
**Systems:** Kernel
**Severity:** P3
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1001 §4

LLP 1001 specifies zero padding in EXWF operation payloads. The decoder in
`kernel/src/wire/frame.rs:112-124` advances over padding bytes without checking
their values, so corrupted or noncanonical frames can pass digest verification
and decode successfully.

Reject the first nonzero padding byte with a stable decode error and add
fixtures for every payload alignment width. Export should continue to write
only zero padding.
