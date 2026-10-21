I'll review the image-downsample change against Amendment A1 and the round-1 findings. First I'm reading the repo guidance, the spec, and the full diff.The review is read-only and stays in this checkout. Next I’ll list the commits against `origin/main` and read Amendment A1 plus the round-1 notes.Round 1’s blocker was a timeout that advanced every storage op. I’ll read the last commit against the gate, waiter, and prelude paths that were supposed to narrow that.The round-2 review is still open. I’ll confirm the answer-path queue behavior and the round-1 fixes against the current code before writing the report.Round-1 findings are fixed for the cases those reviews named. One should-fix remains on let-go, and it does not block landing.

## Round 1

- **Background timeout advanced every storage operation** (Astra, blocker). Fixed. `background_round` only enters the failure path when the message is `IMAGE_ABANDONED` (`js/src/background.rs:96`), and `failAbandonedImage` also requires `head.method === "compressImage"` (`js/src/prelude.js:1407`). Any other timeout still sets `background.stalled` and leaves the head. `CommitGate::abandon` returns `Abandoned` only on the Nothing→Abandoned transition (`vendor/ibex/crates/ibex2/src/stdlib/fs_image.rs:53`), so a leftover gate is not reported as a new loss.
- **Let-go compress stranded the queue** (both). Fixed for the tested shape: a superseded awaited compress, with the next write coming from a later live answer. `__exact_let_go("failed")` settles that compress, `finish_let_go` drains, and `a_let_go_compression_the_wait_gave_up_on_does_not_strand_the_queue` fails if the JPEG overwrites `slow.jpg`.
- **Web 64 MiB check, decode-after-deadline, non-Apple test imports.** Fixed. `blob()` checks `byteLength`/`size` before `new Blob` (`host/web/storage-fs.js:216`). `late()` runs after the header read and before `createImageBitmap` (`host/web/storage-image.js:170`). The Apple-only imports and timeout tests are `cfg`'d.

**Answer path, checked on purpose.** `__exact_storage_failed` (`js/src/prelude.js:1417`) still `landed()`s every timed-out operation of that call, including `"storage continuation timed out"` for an awaited `atomicWriteFile`. That is what `origin/main` did; this change only stopped forcing the code `"failed"`. It matches A1.5's sentence that an answer fails as any step whose wait ran out does. Background and let-go are the paths that used to stall, and only an abandoned compression now fails those. An ordinary awaited write can still be overtaken. That window is older than this branch, and A1.5 keeps it.

## Should-fix

**A let-go call with a second operation already queued loses that operation's completion, and the queue stops behind it.** `js/src/prelude.js:1355`, `js/src/turns.rs:151`, `js/src/lib.rs:891`.

`failAbandonedImage` settles the compress and issues the next operation. `__exact_let_go` then sets `lost` on every let-go call and deletes it, including the call that now owns the operation just issued:

```1352:1362:js/src/prelude.js
  global.__exact_let_go = function (failed, message) {
    if (failed && head && headOwner() && headOwner().letGo) failAbandonedImage(String(message));
    var owed = false;
    calls.forEach(function (c) {
      if (!c.letGo) return;
      if (failed) { c.lost = true; storing.delete(c); }
      if (c.storage > 0 && !c.lost) owed = true;
      else { c.replied = true; calls.delete(c.id); }
    });
```

`finish_let_go` then `break`s without `deliver_storage_one` (`js/src/turns.rs:159`). The next `__exact_let_go("")` sees nothing owed. A later answer waits at ticket `WAITING`, and `resume` delivers nothing for that ticket (`js/src/lib.rs:891`), so the head never clears.

Failing input: an answer calls `compressImage` without awaiting, then `writeFile` without awaiting, and the runner supersedes it while the compress is head (1 ms wait). The compress is abandoned, the write is started, the call is deleted, and a later answer's write stays in `queue` behind a completion nobody takes. The new let-go test does not catch this: its successor is a different live answer, which is not `letGo` and waits at ticket 0.

Fix: if this call still owns `head` after `failAbandonedImage`, do not mark it `lost`. The next `finish_let_go` can then wait and `deliver_storage_one`. Add that two-op let-go case next to the existing test.

## Nits

1. **No regression for an ordinary stall.** `js/tests/it/compress.rs:147`. Astra asked for a stalled non-compress write that must not be overtaken. The three timeout tests still pass if every timeout advances the queue, because their head is `compressImage`. A 1 ms wait, an awaited or background `writeFile` that sleeps in the write, and a second write queued behind it, asserting the second does not land, would lock the predicate.

2. **A negative BMP width is accepted.** `host/web/storage-image.js:98`. A1.5 abs's a negative height (top-down) and says any other non-positive side is undecodable. `Math.abs` is applied to both sides, so width `-8` is treated as 8 and can reach `createImageBitmap`. Abs only the height; reject a non-positive width. A huge absolute value still hits the pixel cap, so this is not a decode-the-world hole.

3. **`docs/reference.md:675` says compressImage is "run off the JS thread."** That is the native pool. On the web the canvas runs in the page or the module worker, which that same paragraph then describes. One clause is enough.

## Checked, no defect

Search matches A1.2 and `compress-trials.json`: `Math.round((hi+lo)/2)` and `(hi+lo).div_ceil(2)` agree for these sums; the fourth shrink is not tried; sizes from 4000 are 4000, 3200, 2560, 2048. Apple ImageIO references in `data/src/image/apple.rs` are +1 into `Owned` and released on drop, including through `?`; the dictionary uses retaining callbacks; the pool pops on drop; encode copies no source properties; bitmap info `5` is `kCGImageAlphaNoneSkipLast`. The gate is taken at `ibex2_async_begin`, held across `write`/`sync`/`rename`, and cleared at `take_task`. Linux returns `unsupported` before `read_capped` when a codec is absent; a file over 64 MiB is `too-large` from `stat`. Web, JS target, and Rust-source carriers refuse a non-`app:/` path before the document branch. `prelude.js` is 1445 lines and `scripts/app.mjs` is 1499, both within the cap. Vendored Ibex is outside it.

Nothing blocks landing. The let-go hole above is the one fix worth making before or just after.
