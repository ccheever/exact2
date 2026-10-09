I'll review the image-downsample change against Amendment A1 and the two earlier review rounds. First I'll read the project rules and the spec, then the full diff.The branch is ten commits over `origin/main`. I'll read Amendment A1 and the two earlier code reviews, then the implementation diffs.A1.5 now limits the timeout claim to `compressImage` and records the older answer-path hazard in `QUEUE.md`. I'll check that claim against the waiter, the gate, and the three queue paths.The queue paths match the scoped claim so far. Next I’ll check the codec, the search, and the web header reader for anything the last fixes changed or left wrong.The round-2 fixes are in the tree. I’ll finish the unread tests, the 1,500-line cap, and the host wiring, then write the ranked review.Line counts are under the cap (`prelude.js` 1446, `scripts/app.mjs` 1499). Next I’ll re-read the codec, the gate, and the tests that claim the round-2 fixes.Nothing in this round blocks landing. The round-1 and round-2 fixes are in the tree and hold together, and the answer-path scoping is an honest description of `origin/main`.

## Round 1 and round 2

Only a compression whose right to write was taken away is failed and lets the queue move, on the background and let-go paths. An ordinary stalled write still holds its queue.

- `CommitGate::abandon` (`vendor/ibex/crates/ibex2/src/stdlib/fs_image.rs:54`) turns `Nothing` into `Abandoned` and then returns the current state, so a second waiter on the same call reports the right already taken. `Written` stays `Written`. The write holds that mutex across `write` / `sync` / `rename` (`fs_image.rs:129` and `app_fs_unix.rs:141`).
- The waiter (`js/src/storage.rs:121`) fails with `compressImage: timeout: the storage wait ran out; nothing was written` only when the result is `Abandoned` and the queue has no completion. `Written`, or any already-queued completion, extends five seconds.
- Background (`js/src/background.rs:96`) and let-go (`js/src/prelude.js:1356`, `js/src/turns.rs:155`) call `failAbandonedImage` (`prelude.js:1408`), which requires both that message and `head.method === "compressImage"`. Anything else still sets `background.stalled` or marks the let-go call lost, as before.
- On a matching let-go failure the prelude returns `"settled"` before the loop that would mark the call lost, and `finish_let_go` keeps delivering the rest of that chain. `a_let_go_chain_keeps_its_other_storage_after_a_discarded_waiter_gave_up` is that case: the first waiter’s outcome is discarded, the chain still writes `data/second`, and `slow.jpg` stays `next`.
- Web: the 20 s deadline is checked after the header and before `createImageBitmap` (`host/web/storage-image.js:171`). `blob()` checks the byte length before `new Blob` (`host/web/storage-fs.js:216`). A negative BMP width is not absolute-valued (`storage-image.js:99`); the fixture expects `null`.
- The reference now says the native call runs off the JS thread (`docs/reference.md:675`).

The answer path is unchanged in the way Amendment A1 claims. Against `origin/main`, `__exact_storage_failed` (`prelude.js:1418`) still settles whatever head belongs to that call and still sets `call.lost`. The only edit is that it no longer forces the code `"failed"`, so `storageCode` can read `compressImage: timeout:`. A1.5 (`llp/1069.002-media-picker.rfc.md:302`) and `QUEUE.md:1085` both say an ordinary awaited write whose wait runs out still lets the queue move while it may yet land. That matches the diff. It is not a defect in this change.

The search matches A1.2, including the recorded trials, the half-up quality step (`(hi + lo).div_ceil(2)` agrees with `Math.round` for these integers), the stop before the fifth size, and the one-pixel floor. Apple’s `Owned` takes a +1 and `CFRelease`s once; dictionaries use the retaining callbacks; the pool pops on every return; orientation 5–8 is applied before the search and does not change the pixel product. Linux, Windows, and Android install no codec, so `unsupported` happens after `stat` and before `read_capped`. Non-`app:/` paths are refused before the `doc:/` branch on the Rust host and in `storage-request.js`. `prelude.js` is 1446 lines and `scripts/app.mjs` is 1499; both are inside the 1,500-line cap (`scripts/app.mjs` is one edited line, not a new one). Vendored Ibex is excluded by `scripts/caps.mjs`.

## Nit

**Web size check can reject an image whose real rectangles are under 64 Mi pixels.** `host/web/storage-image.js:78`

```javascript
const larger = (a, b) => a && b ? [Math.max(a[0], b[0]), Math.max(a[1], b[1])] : null;
```

A1.5 says GIF uses the larger of the logical screen and the first image, and WebP the larger of the VP8X canvas and the first frame. This takes the component-wise maximum instead. A GIF screen of 50000×2 and a first image of 2×50000 is 100,000 pixels either way, under the cap, but the check sees 50000×50000 and returns `too-large` (`storage-image.js:86`, and the same helper at lines 114 and 116). It cannot accept a decode larger than both rectangles, so this is a false refusal, not a pixel-cap bypass. Real frames sit inside the canvas, which is why the fixtures still pass.

Pick the rectangle with the greater pixel count, then run `positive` and the 64 Mi pixel check on that rectangle.

## Landing

Nothing blocks. I would not hold the branch on the web size nit.
