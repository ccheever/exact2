# Code review: The wasm host preloads its presence module where the plan names a presence row, round 1 of a dropped first design (hold the gaining batch), 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `38e50e7ab3`.
- **Method:** one brief (sha256 `bdbf0f13ac6737cdaa7408302ced8661128e95787149ba49194960db4f0e32d7`), blind to the other reviewer; 2 rounds (the lead's 23:50Z), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Its four findings (agent waits, event commands, restart, reentrancy) led to the preload design reviewed below; Grok's run on it was stopped.

Four P2 findings in the newly held style path:

1. **P2 — Agent operations do not await held batches.** [navigation.js:568](host/web/navigation.js:568)  
   The presence load is absent from `pieces.pending()` and the clock’s waits. With the load unresolved, I reproduced `clock data` and `clock settle` returning `settled:true` with nothing applied, and a jump to 100 returning clock 0. Fix: expose a drain promise and await it at agent/clock boundaries, including loads started during an operation.

2. **P2 — Holding the batch loses synchronous event commands.** [navigation.js:569](host/web/navigation.js:569)  
   A key handler that gains the row and calls `preventDefault()` or `stopPropagation()` now queues those commands. The handler clears `keyEvent` before replay, so both become ineffective ([glue.js:769](host/web/glue.js:769)). I reproduced this with the production batch functions. Fix: execute event-bound commands synchronously, exactly once, before holding visual work.

3. **P2 — Held batches survive a restart into the wrong incarnation.** [navigation.js:556](host/web/navigation.js:556)  
   `bootNow()` clears views and increments `incarnation` without clearing this queue. Replay consequently executes old operations against the new runner. I reproduced an old queued refusal being stamped with the new incarnation and answering its reused ticket. Fix: discard retired batches at successful restart acceptance, before submitting the new boot batch; retain the loaded module separately.

4. **P2 — Reentrant replay breaks batch order.** [navigation.js:557](host/web/navigation.js:557)  
   `release()` drains a detached array after setting `live`. If batch A focuses a field, its focus handler can synchronously produce C while older batch B still awaits replay. C bypasses the queue, producing A → C → B. I reproduced widths 10 → 30 → 20, leaving the DOM behind the runner’s latest state. Fix: keep a drain guard and queue reentrant batches behind existing work.

All three loader tests pass, but their stubbed application callback misses these integrations. The create-only boot and rejected-load fallback look correct; the conservative wait for a restyle without movement is not a separate finding. No files changed.
