# Structured styles bypass wire validation

**Status:** Closed
**Resolution:** Generated style-domain validation now applies equally to structured input, wire decode, and export.
**Systems:** Kernel
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1001 §§3-4

The structured `SetStyle` path validates finiteness and transitions but not the
shape constraints enforced by wire decode (`kernel/src/txn.rs:216-225`). In
particular, `GridTracks` exposes an unrestricted public Vec while wire ingress
rejects more than 32 tracks. Structured ingress accepts 33 tracks; debug export
then hits the encoder's `debug_assert`, while release export writes an envelope
its own decoder refuses. More than 255 tracks also truncates the encoded count.

The same split admits structured `Dimension::Auto` for padding even though
wire decode refuses that value; layout silently lowers it to zero.

Put codec/domain validation in one shared validator called by structured
apply, wire decode, and export. Test that every value accepted through one
ingress can export and decode through the other, including track limits and
row-specific dimension vocabularies.
