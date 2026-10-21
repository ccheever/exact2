I'll review this as a read-only pass: start with the agent docs and the branch history, then read the spec and the full diff against origin/main.The branch is eleven commits ahead of `origin/main`. Next I’ll read Amendment A1 and the three earlier review rounds, then the diffs those rounds asked to fix.Round 3 left a discarded waiter that could revoke a later compression, plus a non-Apple `SERIAL` warning. I’ll read the retirement commit and the queue paths those reviews named.The round-4 retirement path is the open question. I'll re-read that waiter against the spec and the remaining diffs before ranking findings.Nothing in this change blocks landing. Round 3 is fixed on the path a reviewer can actually hit, and the answer-path queue advance is honestly scoped as `origin/main` behaviour.

## Should-fix

**A retired waiter can still take a compression's right.** `js/src/storage.rs:131` loads the flag, then `js/src/storage.rs:142` calls `abandon_image_work` with no second look. `vendor/ibex/crates/ibex2/src/task.rs:622` clones every gate under `image_work`'s mutex and abandons them after dropping it. `js/src/turns.rs:61` sets the flag before `__exact_forget` / `finish_let_go`, which is what makes the common case safe: a waiter still blocked in `context.wait` wakes, sees the flag, and returns `storage continuation retired…` without touching a gate. `Runner::fulfill` (`runner/src/runner/commit.rs:1227`) drops that outcome because the park is already gone, so it does not settle the successor or strand the queue. The live let-go waiter is `Session::continuation` (`js/src/turns.rs:158`), which has no flag and still abandons at its own deadline.

The hole is the gap after the load and before that mutex. Interleaving:

1. Answer A's waiter comes out of `wait(25ms)` with the 30s deadline already due, reads `retired == false`, and is descheduled.
2. The runner lets A go. `forget_calls` sets the flag. `finish_let_go` runs the chain and the next `compressImage` reaches `begin_image_work` (`boundary_abi.rs:897`).
3. A's waiter resumes. `abandon_image_work` takes every current gate, including the chain's new one (`Nothing` becomes `Abandoned`).

The chain's JPEG is never written. Its own waiter then delivers `compressImage: timeout: the wait for it ran out before it was written`. That contradicts A1.5 (`llp/1069.002-media-picker.rfc.md:301`): a retired waiter takes no right, so it cannot take the right of a compression the let-go chain started after it. The same window also kills the in-flight compression the new waiter had just started watching, instead of giving that waiter its own 30s. It fails closed (no write after a guest-visible success, and the queue still moves). It does not skip an abandonment the live waiter owes.

`js/tests/it/compress.rs:246` does not cover this. It joins the first waiter to a timeout before the superseding `answer()`, so deleting the flag check would not fail it. A1.9 says as much.

Fix: pass the flag into `abandon_image_work` and load it while holding `image_work`, before any gate is mutated. `begin_image_work` uses that same mutex, and the flag is stored before a new gate can be registered, so a gate created after retirement cannot be abandoned. A check immediately before the call, still outside the mutex, leaves the window. A test that keeps the first waiter blocked inside `wait` across the let-go, then asserts the chain's later `compressImage` still writes, would lock the deterministic half.

## Nit

**`host/web/storage-image.js:66` still says GIF and WebP use "the larger" of the two rectangles.** `larger` at line 78 is the rectangle of more pixels (`a[0] * a[1] >= b[0] * b[1]`), which is what the `50000×2` vs `2×40000` fixture asserts. The comment is the wording round 3 misread as a component-wise max. Say "more pixels" in the header comment too.

## What holds

Round 1's queue predicate is still in place. `failAbandonedImage` (`js/src/prelude.js:1407`) settles the head only when the head is `compressImage` and the message is `compressImage: timeout: the storage wait ran out`. Background (`js/src/background.rs:96`) and let-go (`js/src/prelude.js:1356`, returning `"settled"` before any call is marked lost) both go through that. An ordinary stalled write still sets `background.stalled` or stays lost, and the queue waits. The answer path (`__exact_storage_failed`, `prelude.js:1418`) still settles whatever the head is. A1.5 (`rfc:305`) and `QUEUE.md:1085` both say that is unchanged `origin/main` behaviour, safe for `compressImage` only because its right to write is taken away, and that the answer's remaining queued operations are left with no owner. That scoping matches the code.

Round 3's other two fixes hold. `SERIAL` and `WAIT` are `#[cfg(target_vendor = "apple")]` (`js/tests/it/compress.rs:9`). Header rectangles are by pixel count, and a tie keeps the first rectangle, which has the same pixel count for the cap.

The search matches A1.2 and the recorded Bluesky trials: `lo = 0`, `hi = 101`, stop when `shrinks >= 4` before the next size, `Math.round((hi+lo)/2)` agrees with `(hi+lo).div_ceil(2)` on these integers, quality `<= 13` resets and shrinks by `floor(dim * 0.8)`, and a side is at least 1. Apple FFI (`data/src/image/apple.rs`) owns +1 Core Foundation refs and releases them once, dictionary callbacks retain, the autorelease pool pops on drop, Get-rule strings are not released, the pixel product is `u64`, orientation 5–8 is transposed, and the JPEG is encoded with only the lossy-quality property. Linux installs no codec and refuses `unsupported` before the capped read. Rust-source requests pass no gate. The web checks the header before `createImageBitmap`, refuses HEIF/AVIF by not recognizing them, and checks the 20s deadline after the header and before each trial. Docs (`docs/reference.md:652`, `docs/contract-for-agents.md:850`) match that.

`scripts/caps.mjs` counts `split('\n').length` and fails only above 1500. `scripts/app.mjs` is exactly 1500. `js/src/prelude.js` is 1447. No other non-vendored source in the diff is over the cap.

Nothing blocks landing. The retirement race is fail-closed and needs a preemption between the flag load and the `image_work` lock; it should be closed before this is treated as done, and it does not need to hold the branch.
