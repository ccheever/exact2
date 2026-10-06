# Late resize after terminal ID reuse

Production transport/session loopback regression holds an old 100x30 resize response, queues old 120x40, drops and rebinds the same environment/thread/term-5, requests replacement 80x24, then releases the old response. Before the repair, a third RPC sent the old queued 120x40 to the replacement (resize-before.log: 54 tests, 1 failure).

Resize entry and completion now require exact session ownership. After repair, the queued old resize is discarded; resize-after.log reports 54 tests, 0 failures. Existing disconnected resize and latest-wins/retry tests still pass. git diff --check passed for both owned files. Source hashes: resize-source-sha256.json.

No app launch, GUI interaction, or physical keyboard input was performed.
