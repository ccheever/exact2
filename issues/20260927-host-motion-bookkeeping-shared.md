# Paint-owner and presence bookkeeping is written twice (Apple, Linux) and has diverged; move it below the hosts

**Status:** Open
**Systems:** `host/apple/src/paint.rs`, `host/apple/src/presence.rs`, `host/linux/src/paint_motion.rs`, `host/linux/src/presence.rs`, runner or kernel `motion`
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1062, LLP 1063; Charlie accepted the recommendation 2026-09-27

Both Rust hosts implement the same policy: paint-owner adoption, boot adoption, the "stays `currentcolor`" rule, scheme re-targeting, `node_key`, layout observation and `observe_box`. They already differ: Apple resolves appearance per view (`paint.rs:48`), and Linux uses one `self.dark` (`paint_motion.rs:28`).

**Do:** move the policy into the runner or the kernel's motion module, so a host only presents values. Keep Apple's per-view appearance as the semantics, and have Linux follow it. Every future host (Android) then inherits one copy. Keep the crate order: each crate depends on strictly less than the one above it.

From Charlie's rulings of 2026-09-27 on the review of Seth's PR #47.
