# Code review: setup installs the iOS device Hermes bundle, round 1, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `a5631876a6ad70837b79c8d4396518ab8cf450c851734e2ea5c5c0211a723390`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: fix (1 MAJOR, 1 MINOR); Grok: ship (noted the stale doc).
- **Disposition:** each target its own installer run; the probe asks the Xcode useXcode() selects; docs/reference.md updated

---

I'll review commit `e38d64bbf` as a read-only defect check: the iOS device Hermes install path, setup side effects, the `--check` report, and the test. No checkout, restore, or file edits.The request is a defect review of that one commit, with a ship-or-fix verdict. I'll inspect the diff and the surrounding setup, check, and test paths without changing the worktree.The device bundle is installed only on a Mac where `xcrun` can see the `iphoneos` SDK, and that change is safe to ship. Host install, Linux, and a missing SDK stay on the previous path. `setup --check` still reports the device row and does not start failing because of it.

`hermesSetupTargets` keeps every cross target from `hermesCrossTargets`, then drops `aarch64-apple-ios` unless the third argument says a device build can run. The live call in `setup` probes with `xcrun --sdk iphoneos --show-sdk-path` and treats a non-zero status as "no SDK". The published asset `hermes-vanilla-aarch64-apple-ios.tar.gz` is 6.06 MB and its sha256 matches the pin in `vendor/ibex/crates/hermes-lean-sys/build_support.rs`.

On a Mac with that SDK, setup adds one download to the existing cross `cargo run` (the installer binary is already built by the host install just above it). Without the SDK, including a Mac with no Xcode, the probe is false, the device triple is omitted, and the simulator triples install as they did before. On Linux and Windows, `hermesCrossTargets` returns nothing and `iphoneosSdk` returns false before spawning, so setup does not call `xcrun` or download an Apple bundle. On Intel, the simulator triple stays `x86_64-apple-ios`; the device triple is still `aarch64-apple-ios`, which is the triple `host/apple/build.mjs --device` actually links.

A failed cross download does not roll back the host bundle. The host install is an earlier `cargo run` that throws on failure. The cross run is a separate process: each pin is published only after that archive validates, and a later error leaves other digests on disk. Setup logs the failure and continues into `cargo fetch` and the report. The installer walks targets in a `BTreeSet`, so `aarch64-apple-ios` is fetched before `aarch64-apple-ios-sim` and `aarch64-apple-tvos-sim`. If that first download fails, those simulator triples are not attempted in the same run. Re-running setup retries them. Simulator bundles already in the cache are a different digest and are not deleted.

`setup --check` never calls `hermesSetupTargets`. It still walks `hermesCrossTargets`, so on Darwin the device row is still there, optional (`need` is set, so a miss does not fail the process), and the fix line is the installer command without `--check` or `--offline`. After a successful setup on a machine with the SDK, that row is an offline verification of the bundle just installed. Check runtime is unchanged: that row was already checked before this commit.

The test locks the gate both ways: Darwin arm64 without the flag stays simulator-only, with the flag it is simulator, then `aarch64-apple-ios`, then tvOS simulator, and Linux stays empty even when the flag is true. It passes the boolean in, so it does not execute `xcrun`. The Intel list is the same filter with `x86_64-apple-ios` in front; that path is not asserted, and it does not need a separate branch.

`docs/reference.md` still says setup installs only the simulator bundles and that a device build names its own command. The script behavior above is what actually runs.

VERDICT: ship
