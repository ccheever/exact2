# Grid `span` is `u16` in the kernel and truncates on the wire

**Status:** Closed
**Resolution:** Grid spans now validate the full wire domain and reject zero or overflowing values before encoding.
**Systems:** Kernel
**Severity:** P3
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1001 (one write path)

`GridLine::Span` is a `u16` (`kernel/src/style.rs`) but the wire writes `n as i16` and reads `i16::unsigned_abs` (`kernel/src/wire/codec.rs`). Spans `> i16::MAX` wrap; a structured `SetStyle` with `Span(40000)` is accepted (no apply-time range check; `MAX_GRID_TRACKS` only bounds track *lists*) and round-trips as `Span(25536)`. That is silent data loss on the one write path the spec says is shared. `Span(0)` is likewise representable and never refused.

Fix: encode span as `u16` (or refuse `n > i16::MAX` / `n == 0` at both decode and validate). A wire fixture with `Span(32)` is not enough; the overflow is the claim that is untested.
