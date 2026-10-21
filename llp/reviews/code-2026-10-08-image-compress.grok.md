I'll review this as a read-only pass against Amendment A1 and the round-3 blockers. First I'll read the project rules and the spec, then the full diff from `origin/main`.The review stays in this session: read-only, no files written. Next I’ll take the commit list and read Amendment A1 plus the round-3 notes.The branch is eight commits, about 3,800 lines. I’ll read Amendment A1 and the round-3 reviews, then the implementation against those blockers.Amendment A1 and the round-3 notes are in. I’ll read the codec, the commit gate, and the waiter next, since those are where the blockers were.The round-3 fixes are in the tree; I’m checking the remaining race and refusal paths against the spec so the report cites exact lines.Nothing here blocks landing. The four round-3 blockers are fixed in this tree. One should-fix remains, on a path Amendment A1 does not mention.

## Round 3

**(1) A completion that arrives as the waiter gives up.** Fixed. The right to write stays until `take_task` takes the settlement (`vendor/ibex/crates/ibex2/src/task.rs:1236`), not when the pool job ends. After `abandon_image_work`, the waiter treats `Written` or a queued completion as a result and waits another 5 seconds (`js/src/storage.rs:121`). On the exact-js model the owner does not take that completion until the waiter returns, so the old race — gate already gone, abandon sees `Nothing`, guest hears failure, JPEG is on disk — cannot happen. `an_image_right_lasts_until_its_completion_is_taken` covers the gate lifetime, not that interleaving; the structure does not leave a window for it.

**(2) Unload or shutdown writes later.** Fixed. `Session::drop` (`js/src/storage.rs:193`) and `RuntimeState::shutdown` (`task.rs:870`) abandon first. The write holds the gate lock across the atomic write (`vendor/ibex/crates/ibex2/src/stdlib/fs_image.rs:125`), so abandon either marks the call before any rename or waits out the write in progress. `Module::unload` drops the session before it returns (`js/src/lib.rs:559`), so a new session cannot start until that has finished. A write already inside the lock still finishes, which is what A1.5 says.

**(3) A background step whose wait ran out stalls the queue.** Fixed. `background_round` calls `__exact_background_failed` (`js/src/background.rs:99`), and that `landed()`s the head so the rejection runs and the next operation starts (`js/src/prelude.js:1401`). `a_background_compression_the_wait_gave_up_on_rejects_and_the_queue_moves` fails the answer, writes the next file, and checks that the late JPEG does not replace it. That test fails without this path.

**(4) Animated WebP `ANMF` frame size.** Fixed. Width-minus-one is at +14 and height-minus-one at +17 (`host/web/storage-image.js:115`). The fixture in `host/web/storage-image.test.mjs` builds a real 16-byte `ANMF` with duration `0xfffff` and expects the 32×32 frame, not the duration. The old +20/+23 read fails that test.

## Should-fix

**A let-go compress that overruns the wait strands the module queue.** `js/src/turns.rs:151` and `js/src/prelude.js:1352`.

`finish_let_go` is how an answer that was torn down (interrupt, budget, or the runner dropping it) finishes storage it already issued. On timeout it calls `__exact_let_go("failed", message)`. That marks the call lost and deletes it. It never calls `landed`. `message` is ignored. `head` stays, the promise never settles, and later `storage.fs` operations sit in `queue` until unload.

This is the same stall round 3 fixed for background work, on the other teardown path. It needs the codec to run past the 30 second wait while that answer is already let go: the search stops starting trials at 20 seconds, but a thumbnail decode that started before the deadline cannot be cancelled. A phone JPEG that finishes inside the wait is delivered and the queue moves. A large PNG or WebP whose one decode overruns 30 seconds, during a navigation away, leaves every later save on that module stuck. The awaited path (`__exact_storage_failed`, `js/src/lib.rs:897`) and the unawaited path both `landed`. This one does not, and no test drives it.

Fail the in-flight operation through `landed`, as `__exact_background_failed` does, and keep draining. Do not only stamp the call lost.

## The rest

The search matches A1.2 and the recorded trials: half-up quality, stop before the fourth shrink, exact byte length, `q/100`, and the one-pixel floor. Apple and web both fill white, drop source metadata, and scale to the floored size. ImageIO tone-maps via `kCGImageSourceDecodeToSDR`. The `Owned` wrappers release a +1 each; dictionaries use the retaining callbacks; the autorelease pool pops on drop, including on `?`. I do not see a CF leak or a use after free.

Hosts agree on the refusals that matter. The prelude rejects a bad option with `TypeError` before the queue. Non-`app:/` paths, including `doc:/`, are refused before the document and native-disk branches on the wasm realm, the JS target, the Rust-source carrier, and `data/host`. Linux, Windows, and Android install no codec and return `unsupported` before `read_capped`. The web reads the header before `createImageBitmap`, refuses HEIF/AVIF, encodes outside the path lock, and uses `storage-environment.js`'s captured `now`. Codes in `storageCode` match A1.4; `compressImage: invalid:` from the Rust checker is not one of those names and surfaces as `failed`, which the JS API never produces because the prelude checks first.

`scripts/app.mjs` is 1499 lines and `js/src/prelude.js` is 1434. Vendored Ibex files are over 1500 and excluded by `scripts/caps.mjs`. `docs/reference.md` and `docs/contract-for-agents.md` match the code, including Linux `unsupported`.
