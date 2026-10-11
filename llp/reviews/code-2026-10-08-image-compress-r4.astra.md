Two findings remain.

1. **Blocker — retirement can still revoke a successor’s write.** [js/src/storage.rs:131](/private/tmp/imgrv/wt-c4/js/src/storage.rs:131), [js/src/turns.rs:61](/private/tmp/imgrv/wt-c4/js/src/turns.rs:61)

   The retirement check and runtime-wide `abandon_image_work()` are not synchronized. A concrete interleaving is:
   - Waiter W reaches its deadline, reads `retired == false`, and is preempted before abandonment.
   - The owner forgets W’s call, sets retirement, and delivers its completed operation. The queue starts compression B.
   - W resumes and revokes B’s newly registered gate.

   B then fails with `timeout` despite having its own budget remaining. Acquire/Release ordering does not close this window. Consequently, round 3’s successor-cancellation finding is **not fully resolved**.

   Serialize retirement with checking and revoking rights, or bind abandonment to the specific operation being awaited. Add a barrier-controlled regression that pauses W after the retirement check and starts B before releasing W. The existing test joins W before forgetting its call, so it cannot catch this race. A1.9’s equal-wait-duration justification overlooks the different start times.

2. **Should-fix — the Core Foundation value-callback static has the wrong Rust type.** [data/src/image/apple.rs:37](/private/tmp/imgrv/wt-c4/data/src/image/apple.rs:37), [data/src/image/apple.rs:178](/private/tmp/imgrv/wt-c4/data/src/image/apple.rs:178)

   `DictionaryCallBacks` occupies six pointer-sized fields, appropriate for key callbacks. Apple’s value-callback structure has only five: 40 bytes rather than 48 on supported 64-bit platforms. This matches both the installed SDK and [Core Foundation’s declaration](https://github.com/swiftlang/swift-corelibs-foundation/blob/main/Sources/CoreFoundation/include/CFDictionary.h).

   Every normal Apple compression constructs `&kCFTypeDictionaryValueCallBacks`, forming an oversized Rust reference before conversion to a raw pointer. That violates [Rust’s reference-validity requirements](https://doc.rust-lang.org/reference/behavior-considered-undefined.html), even though Core Foundation consumes only the actual five fields. Declare separate, correctly sized callback types and corresponding FFI parameters.

The other earlier fixes hold on inspection: background and let-go queue advancement is restricted to abandoned compressions; subsequent let-go writes retain ownership; deadline/header checks, pre-Blob size checks, non-Apple imports/`SERIAL`, and GIF/WebP rectangle selection are corrected. The inherited answer-path timeout behavior is honestly scoped in A1.5 and recorded in `QUEUE.md`; I am not reporting it again.

The search matches [Bluesky’s loop](https://github.com/bluesky-social/social-app/blob/main/src/lib/media/image/compress.ts), including the documented rounding deviations. I found no additional defect in CF retain/release accounting, CommitGate write locking, metadata/orientation handling, host routing, worker placement, Linux refusal, or error codes.

Validation passed: five web image unit tests, caps, boot, and diff whitespace checks. In-memory probes against the actual prelude confirmed ordinary background/let-go writes retain their queue head and abandoned compressions release it correctly. Native/Hermes integration, browser integration, and device tests were inspected but not executed under the read-only constraints. No files were changed.

**The waiter-retirement race blocks landing.**