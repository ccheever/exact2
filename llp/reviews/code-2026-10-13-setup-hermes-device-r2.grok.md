# Code review: setup installs the iOS device Hermes bundle, round 2, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `47c307f2aee9480e44c9b78d6c77d4464f963588ad2802bf7824a3aecae45745`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** ship from both.
- **Disposition:** landed

---

I'll review only the Round 2 fix in that diff, in a throwaway worktree, and leave the tree untouched. First I'm reading the review instructions and the patch itself.The repo is already at `76a4462f3`. I'll read the fix diff and the surrounding installer code without touching the worktree.The three Round 1 notes are in this diff. I'll check the installer loop and the Xcode probe against `useXcode`, and run the setup-target test without changing the tree.The three Round 1 fixes hold. A failed device download no longer skips the simulators, the SDK probe follows the Xcode selection the Apple build uses, and the setup sentence in the reference matches that install set.

`setup` now runs the Hermes installer once per cross target and keeps going when a run exits non-zero (`scripts/exact.mjs:412`). The installer still puts every requested triple in one `BTreeSet` and returns on the first error (`vendor/ibex/crates/hermes-lean-sys-installer/src/main.rs:72`), which is why `aarch64-apple-ios` used to be fetched before either simulator and a device failure skipped both. Each invocation now carries only one `--target`, so that failure stays inside the device run. The host bundle is still the earlier install, the one that aborts setup, and a bad download is confined to that triple's digest directory.

The probe asks `xcrun --sdk iphoneos` with the developer directory `useXcode()` would select (`scripts/exact.mjs:81`). An explicit `DEVELOPER_DIR` is left alone. When it is unset and `xcode-select -p` names the Command Line Tools, and `/Applications/Xcode.app/Contents/Developer` exists, that path is passed only to this `xcrun`. On this machine `xcode-select` already points at Xcode 27, which has the iphoneos SDK, so the default target list includes `aarch64-apple-ios`. Pointing `DEVELOPER_DIR` at the Command Line Tools drops that triple; pointing it at Xcode 27 keeps it.

`docs/reference.md:35` now says setup installs the iOS and tvOS simulator bundles and the iOS device bundle where that SDK is installed. The focused setup-target test passed. The worktree was left clean.

VERDICT: ship
