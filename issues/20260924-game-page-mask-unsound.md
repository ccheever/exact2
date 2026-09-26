# Replacing a public page mask exposes uninitialized components through safe Rust

**Status:** Open
**Systems:** Game, ECS storage
**Severity:** P1
**Author:** Codex for Charlie Cheever
**Date:** 2026-09-24

`game/engine/src/storage/pages.rs:63` exposes `Page::mask` as a public field.
Although the slice is immutable, a caller can replace the slice on its local
`Page`. `Page::runs` uses that replaceable mask as proof that slots contain live
`C` values before constructing `&[C]` with `from_raw_parts` (lines 91–109).
There is no unsafe operation required of the caller.

Reproduced against the real `exact-game` crate with this safe code:

```rust
use exact_game::{Transform, World};
let mut world = World::new(60, 0);
world.spawn(Transform::default());
let pages = world.pages::<Transform>();
let mut page = pages.iter().next().unwrap();
let before: usize = page.runs().map(|(_, run)| run.len()).sum();
page.mask = &[u64::MAX; 8];
let after: usize = page.runs().map(|(_, run)| run.len()).sum();
assert_eq!((before, after), (1, 512));
```

Only one component was initialized, but all 512 slots become readable. The
probe uses `Transform`, whose absent zeroed slots happen to be valid values.
For a general component containing a `String` or another type with nonzero
validity requirements, manufacturing these references violates Rust's memory
safety rules. No memory corruption payload or Miri run was needed for this
demonstration; the public API has invalidated the unsafe block's precondition.

Make the initialization mask private and expose it through a read-only getter;
the unsafe slice construction must always use storage-owned initialization
metadata. Keep the legitimate read-only page iteration API. Verify that safe
callers cannot substitute a mask and that sparse pages containing owned values
only expose initialized slots.

Reviewed at `35cb7ac053cc98e6fb205d65adae8fb5d61e3de7`; reproduction output:
`/tmp/exact2-review-20260924/game-mask.log`.
