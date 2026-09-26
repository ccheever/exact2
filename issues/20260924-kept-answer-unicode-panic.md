# Non-ASCII text in a persisted kept answer panics during boot

**Status:** Open
**Systems:** Runner, Persistence
**Severity:** P2
**Author:** Codex for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1027 D4

`runner/src/runner/kept.rs:84–91` rejects odd byte lengths, then slices the
input string at every two-byte offset while decoding hexadecimal. An even
byte length does not establish UTF-8 character boundaries. For example,
`"€x|00"` reaches `unhex("€x")` and panics at `&s[0..2]` rather than returning
`None`. This decoder consumes host-persisted text during resource startup;
corrupt persisted data should be ignored, not prevent the app from booting.

Reproduced through the real public `Runner::boot_with_delivery` API. Compile
a plan with a `saved` resource returning `shape Saved { text: string }`, mark
that resource as a store reader (`plan.resources[0].reader = true`), and boot
with a data source whose `ready()` is false and the saved pair:

```rust
vec![("exact.kept.saved".into(), "€x|00".into())]
```

The boot panicked at `kept.rs:90`: `end byte index 2 is not a char boundary;
it is inside '€'`. `catch_unwind` in the diagnostic harness caught it; an
abort/trap build cannot recover this way. The precondition is a malformed
kept value in host storage, not an ordinary valid resource reply.

Decode hexadecimal from byte pairs with ASCII validation, returning `None`
for malformed content. Exercise both halves of the saved representation,
non-ASCII inputs of odd and even byte length, and successful boot after a bad
saved answer is discarded.

Reviewed at `35cb7ac053cc98e6fb205d65adae8fb5d61e3de7`. Full public-API probe:
`/tmp/exact2-review-20260924/game-probe/src/bin/runner-review.rs`;
output: `/tmp/exact2-review-20260924/runner-review.log`.
