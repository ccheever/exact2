RFC 0491 W0-A draft artifact — hand-verified inventory/draft, provenance commands inline; generated authorities under tests/protocol/exwf/ own their boundaries and this pack never restates them. Registered in exact-verify.json as `kernel-refresh-w0a-pack` (RFC 0491 W0-B).

# WS-F dead-code candidates

This is an inventory, not a deletion authorization. The structured rows, spans, estimated LOC, evidence, and verification flags are in `ws-f-dead-code-candidates.json`.

## W0-B disposition (2026-08-23)

RFC 0491 W0-B ("land WS-F prerequisite-free deletions") acted on this inventory. The findings table below is the W0-A snapshot and is kept verbatim; the JSON rows and line spans are likewise historical after the deletions.

- **Deleted (zero live in-repo consumer, re-verified at deletion time):** the frozen pager ABI `exact_motion_pager_answer_turn` (Rust export + hand-written header declaration; Apple keeps calling `_v2`); the Rust `MotionClockRegistry` with its registry-only support types (`MotionClockConsumerKind`, `MotionClockConsumerToken`, `MotionClockTick`, `MotionClockError`, `SurfaceMotionClock`; `MotionTickPhase`/`MOTION_TICK_ORDER` preserved for WS-B(4)/RFC 0490 M2); the legacy `MotionPager` and its tests (the nine nearby physics constants stay in place, unmoved — the live `GestureArena` reads them, and W0-B patches no constants); the benchmark batch setters (`exact_batch_create_views`, `exact_batch_set_style_size`, `exact_batch_set_transform`, their header declarations and the two batch-only bench cases); and the dead selection state API (`SelectionState`, `plain_text_for_selection`, `project_selection`, the `lib.rs` re-export; `SelectionViewRange` and `project_document_range` stay live). Net −869 lines.
- **Not deleted (not prerequisite-free):** numerical protocol v1 (Phase 2 orders the RFC 0495 A-B receipt/evidence schema before protocol-v1/trace deletion); the text-measure callback generations (live fallback chain; WS-E replaces them); the v1 `InteractionArena` and the direct-setter path (live migration candidates); `MotionDescriptorKind::Driver` (occupies the motion schema digest roster and the full variant set is unproved — Phase 2).

## Candidate findings

| Candidate | Current finding |
| --- | --- |
| numerical protocol v1 | No production encoder selection was found, but compatibility parsers and tests remain; deadness is unverified. |
| five text-measure callback generations | None is dead: current hosts register generations and `ffi.rs` retains the fallback chain. |
| frozen pager ABI | No repository production call to `exact_motion_pager_answer_turn` was found; Apple calls `_v2`, but external C-ABI consumers are unproved. |
| Rust `MotionClockRegistry` | Repository population is test-only; the public rlib surface leaves external-consumer deadness unproved. Its type/implementation is 125 lines (`motion.rs:1591-1715`). |
| legacy `MotionPager` | Repository population is test-only; the public rlib surface leaves external-consumer deadness unproved. Its implementation is 190 lines (`motion.rs:237-426`). |
| v1 `InteractionArena` | Still constructed by production interactive-navigation FFI; a live-migration candidate, not dead. |
| dead variants / digest-only descriptor | `MotionDescriptorKind::Driver` occupies the digest roster and is rejected by the snapshot validator; the RFC does not name the remaining alleged variants, so the full candidate is unverified. |
| benchmark batch setters | No repository production calls found, but public C-ABI external consumers are unproved. |
| selection state API | No repository production Rust caller found, but the public rlib surface makes deadness unproved. |
| direct-setter path | Apple still calls seven direct write functions; a live-migration candidate, not dead. |

`ExactMotionClockRegistry` at `ios/ExactApp/ExactApp/Renderer/ExactMotionClock.swift:412` is live Swift production code and is explicitly **not** a candidate. This exclusion is also machine-readable in the JSON.

The zero-consumer and live-consumer searches are preserved verbatim in the JSON `provenanceCommands`. LOC reproductions are:

```sh
awk 'NR>=1591&&NR<=1715{n++} END{print n}' kernel/src/motion.rs
awk 'NR>=237&&NR<=426{n++} END{print n}' kernel/src/motion.rs
awk 'NR>=78&&NR<=236{n++} END{print n}' kernel/src/motion.rs
awk 'NR>=3487&&NR<=3584{n++} END{print n}' kernel/src/ffi.rs
awk 'NR>=762&&NR<=1668{n++} NR>=4376&&NR<=4408{n++} END{print n}' kernel/src/ffi.rs
```

## Discrepancies and unresolved proof

- RFC 0491's roughly 245-line Rust `MotionClockRegistry` claim does not match the current source boundaries: the broad supporting block `1512-1715` is 204 lines, while the type/implementation at `1591-1715` is 125 lines.
- The v1 `InteractionArena` and the direct-setter write path are live today. They are valid breaking-window migration candidates, not verified current dead code.
- “Dead enum variants” is not sufficiently named in the RFC to prove as a set. The break package must enumerate each concrete variant and its producer/consumer search.
