# Code review: LLP 1121 stage 9 F2, the Mac's accent cells in a key window, round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max (round 3: high), sandbox read-only, a throwaway worktree.
- **Scope:** the change and round 1.
- **Method:** one brief (sha256 `1e6aeda52da10daf8523a0ee78c5e4669703845ecadf0f34114fbcdc1c827e30`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (BLOCKING: exit inside the SIGINT handler; silent restore failures; Graphite taken for inactive; the second picture unchecked).
- **Disposition:** fixed in round 2's commit and the nit's.

---

**One blocking finding.**

1. **blocking — [main.swift:68](/tmp/rv-f2-2/host/apple/Sources/ExactMac/main.swift:68): unsafe cleanup inside SIGINT handler.** Ctrl-C calls `exit(130)` synchronously inside the signal handler, which runs `atexit` and AppKit restoration there. This can interrupt code holding runtime/AppKit locks or execute off the main thread, causing a hang or crash. The parent processes also receive Ctrl-C, so their cleanup cannot reliably rescue it. **Fix:** under the flag, ignore SIGINT’s default action and retain a `DispatchSourceSignal` targeting `.main`; call `exit(130)` from its event handler. [Apple’s signal-source guidance](https://developer.apple.com/library/archive/documentation/General/Conceptual/ConcurrencyProgrammingGuide/GCDWorkQueues/GCDWorkQueues.html).

2. **should-fix — [measure-controls.mjs:152](/tmp/rv-f2-2/scripts/measure-controls.mjs:152): restoration failures can remain silent.** The installed `lsappinfo` prints `setfront` failures as `ERROR #…` on stdout without making its exit status nonzero; executable inspection confirms this. If the saved app exits or restoration is rejected, this condition misses the failure and discards the diagnostic. **Fix:** validate the saved ASN, inspect command diagnostics, and verify the resulting front application with a bounded check.

3. **should-fix — [measure-controls.mjs:173](/tmp/rv-f2-2/scripts/measure-controls.mjs:173): Graphite falsely fails activation verification.** An active window’s on-switch can legitimately use the gray system accent. Its pixels fail the channel-spread threshold, causing a successful activation to be reported as unsuccessful. **Fix:** verify `NSApp.isActive` and the capture window’s `isKeyWindow` directly instead of inferring activation from color.

4. **should-fix — [measure-controls.mjs:173](/tmp/rv-f2-2/scripts/measure-controls.mjs:173): the second screenshot’s activation is unchecked.** If focus changes between the first screenshot and `.again.png`, the first passes this guard. The later stability filter removes the changed accent pixels, potentially leaving enough neutral pixels to classify an otherwise exact control as `none`. **Fix:** validate activation at both captures and reject the pair if either is inactive.

The noncapturing `atexit` closure itself is valid; EOF already reaches `exit` on the main queue. Default activation and signal behavior remain gated. The colon-trimmed ASN is accepted by the installed parser. Dark desktop appearance is overridden by the agent’s Aqua default; a missing switch frame ultimately fails the run. SIGKILL restoration depends on the measurement script surviving.

No builds or edits. Both measurement tests pass, and the recomputed schema digest matches the pin.
