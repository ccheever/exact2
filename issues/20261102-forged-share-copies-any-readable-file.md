# A forged Android share copies any file the app can read

**Status:** Closed
**Systems:** Android extras (`ExactActivity.environment`), cold share (`host/linux/src/picker.rs`), share claim (`runner/src/runner/picker/shared.rs`)
**Severity:** P1
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** Another app can cold-start an Exact Android app with an `x_share` extra and have this process copy any file it can read into `app:/tmp/shared/`. A declared size of one byte still copies the whole file, past the 64 MiB cap. A path of `/proc/self/fd/N` for `N` greater than 2 is taken with `File::from_raw_fd` and closed when that file is dropped.

**Cause.** `environment` writes `EXACT_SHARE` from `Shares.request`, then the `x_*` loop overwrites it when the extra is `x_share`. `cold_share` parses that value on Android. `Pending::from_json` with no directory accepts a `path` as given. `Claims::claim` uses the JSON `size` when it is present, and drops a file only when that number is above `FILE_CAP`. `Claimed::copy_with` then `std::fs::copy`s the path, or `std::io::copy`s the already-open descriptor, and does not stop at the declared size. `hold` adopts every `/proc/self/fd/N` with `N` greater than 2.

**Reproduction.** Cold-start the app with `x_share` set to `{"target":{"action":"/","params":{"files":[{"name":"files","accept":"*/*"}]}},"share":{"files":[{"path":"<a file this app can read>","name":"note.txt","size":1}]}}`. The file's full contents land under `app:/tmp/shared/`. The same extra with `"path":"/proc/self/fd/3"` takes that descriptor.

**Fix.** Accept `EXACT_SHARE` only from `Shares.request`, which opens streams through the intent's URI grant. Ignore a JSON `size` when measuring the copy, and refuse a copy that grows past `FILE_CAP`. Adopt a descriptor only for an fd this process just detached.

**Acceptance.** An `x_share` extra copies nothing. A real share sheet still copies the granted streams, and a stream larger than 64 MiB is dropped rather than stored.


**Resolution.** Android ignores external `x_share` extras and accepts only streams opened through the intent URI grant. The copy uses measured bytes, stops past the 64 MiB limit, and removes an oversized partial output. Detached descriptors come only from the app's own share handoff.

**Verification after the fix.** The integrated runner regression passed for accurate byte counts and a source that grows past the cap after admission. Android canvas Kotlin compiled against the installed SDK; manifest and launch-environment regressions passed. A real device share-sheet interaction was not exercised.
