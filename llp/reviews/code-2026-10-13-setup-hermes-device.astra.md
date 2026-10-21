# Code review: setup installs the iOS device Hermes bundle, round 1, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `a5631876a6ad70837b79c8d4396518ab8cf450c851734e2ea5c5c0211a723390`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: fix (1 MAJOR, 1 MINOR); Grok: ship (noted the stale doc).
- **Disposition:** each target its own installer run; the probe asks the Xcode useXcode() selects; docs/reference.md updated

---

- **MAJOR — [scripts/exact.mjs:75](/private/tmp/rv-hermes/scripts/exact.mjs:75): device failure can prevent simulator installation.** All cross targets share one installer invocation. The installer sorts them, puts `aarch64-apple-ios` first, and returns on its first error. With empty caches or a pin bump, a device-specific download failure therefore skips both simulator bundles. The host survives, but existing simulator setup regresses. Install the device bundle separately or continue after individual target failures.

- **MINOR — [scripts/exact.mjs:80](/private/tmp/rv-hermes/scripts/exact.mjs:80): SDK detection misses the build’s Xcode fallback.** When Command Line Tools is selected and `/Applications/Xcode.app` exists, this probe skips the device bundle. Apple builds call `useXcode()` and can build for that device. Use the same selection logic before probing. The [changed test](/private/tmp/rv-hermes/scripts/app.test.mjs:1109) supplies booleans directly, so it never exercises this detection.

The two focused tests pass. In-memory checks confirmed correct Intel targets, no probe on Linux, graceful handling of missing `xcrun`, and continued setup after cross-install failure. `--check` remains unchanged: separate optional bundle rows, offline verification, and the correct device install command.

Setup adds one serial download and validation when needed; the cached device archive is 6.35 MB. The probe took 1.64–1.70 seconds here, with Xcode cache writes blocked by the sandbox. Fresh installation timing was not measured. No files changed; the worktree remains clean.

VERDICT: fix
