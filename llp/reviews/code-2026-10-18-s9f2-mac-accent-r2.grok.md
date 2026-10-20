# Code review: LLP 1121 stage 9 F2, the Mac's accent cells in a key window, round 2, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the change and round 1.
- **Method:** one brief (sha256 `1e6aeda52da10daf8523a0ee78c5e4669703845ecadf0f34114fbcdc1c827e30`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (BLOCKING: setfront failing with status 0; the SIGINT handler; Graphite; a missing switch frame; activate's result).
- **Disposition:** fixed in round 2's commit and the nit's.

---

1. **blocking** — `lsappinfo setfront` fails with exit status 0, so the script treats the restore as success. `scripts/measure-controls.mjs:152`

   `frontApp()` turns `lsappinfo front`'s `ASN:0x0-0xf00f:` into `ASN:0x0-0xf00f`. That trimmed form is a valid specifier (`lsappinfo info` resolves it to the front app, and `setfront` rejects a nonsense token differently). On this macOS 27.0 login session, `lsappinfo setfront` of that ASN, of the same ASN with the colon, and of `Finder` all print `ERROR #-54 permErr` on stdout, exit 0, and leave the front app unchanged. The same permErr happens under a pty. The `finally` only checks `status !== 0`, so it never prints `could not put … back in front`. SIGKILL skips `atexit`, which means this `finally` is the only restore, and it neither restores nor reports. A normal exit only gets the app's own handoff.

   Fix: treat the restore as failed when the status is nonzero or when stdout or stderr contains `ERROR`. For SIGKILL, use a restore that this OS actually allows, or fail the run in the open when `setfront` prints `permErr`.

2. **should-fix** — The SIGINT handler calls `exit`, so the AppKit restore runs inside the signal handler. `host/apple/Sources/ExactMac/main.swift:68`

   The handler is installed only when `EXACT_AGENT_ACTIVATE=1` (the default path does not replace SIGINT). It calls `exit(130)`, and `exit` runs `frontAgain` (`yieldActivation` and `activate`) before returning. That is not async-signal-safe, and the signal can be delivered to the stdin thread (`Agent.startStdio` starts `serve(fd: 0)` on a `Thread`), so those AppKit calls run off the main thread. Ctrl-C also kills `bun`, so `measure-controls`'s `finally` does not run. A throw or deadlock here leaves the previous app in the background. The file's SIGTERM path already avoids this: an empty `signal` handler and a `DispatchSource` on the main queue (`main.swift:632`).

   Fix: do the same for SIGINT under the flag. On the main queue, call `frontAgain()` and then `exit(130)`.

3. **should-fix** — The key-window check treats a graphite accent as an inactive window. `scripts/measure-controls.mjs:173`

   The guard passes only if some pixel of the on switch has a channel spread above 60. A chromatic system accent (blue, purple, pink, red, orange, yellow, green) still clears that in dark Aqua, and an inactive window's grey switch still fails it. Graphite is a neutral grey (spread near 0) on a key, active window, so the run throws `the capture is not a key window` and never records cells. A grayscale accessibility filter does the same.

   Fix: have the activated app report `NSApp.isActive` on the `ready` line the script already parses, and require that. Keep the pixel check only as a backstop for a chromatic accent.

4. **should-fix** — If the switch frame is missing, the key-window guard is skipped and the other cells are still judged. `scripts/measure-controls.mjs:173`

   The condition is `frames.switch && !pixels(...)`. With no `switch` frame, `capture` returns normally. The switch row becomes `unmeasured`; checkbox, radio, and the prominent button are still scored. An inactive window draws those grey, so they come back `none` and can be written into the table as measured.

   Fix: on macOS, throw when `frames.switch` is absent, before judging any row.

5. **nit** — `front.activate()`'s result is discarded. `host/apple/Sources/ExactMac/main.swift:63`

   That `Bool` is whether the activation request was sent. `yieldActivation` itself does not activate the other app. A `false` return is silent, and finding 1 means the script will not report it either.

   Fix: if `activate` returns `false`, log it on stderr before exiting.

Checked and holding:

- The `atexit` closure is legal. `{ frontAgain() }` captures nothing, same shape as `HatchBreadcrumb`'s `atexit`. ExactMac is Swift 5.9 with no strict-concurrency setting. On stdin EOF, `exit(0)` runs on the main thread (`DispatchQueue.main` and `StorageHold`'s main-run-loop timer), and this handler runs before Swift runtime shutdown.
- The default path is unchanged. With the flag unset, the app still uses `orderFrontRegardless`, installs no SIGINT handler, and registers no `atexit`.
- The trimmed ASN is not the `setfront` failure. With and without the colon, the tool returns the same `permErr`.
