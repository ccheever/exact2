# Code review: during needs a hold under the touch runner (LLP 1080.000 §11), 2026-10-05 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `e6ad31304` (round 1) and `aeaf71532` (round 2).
- **Method:** one brief per round, the same sent to both (round 1 sha256 `6ca942ae239db8d93b19b7f424670f186ed3935c4a3476597e4e9b49b49a512d`; round 2 `e3e54e755c43bfc4bf6f43604c1c759e374ebcbd5b9019051621cce2243560e0`, the whole change); blind to the other review. Requested by the coordinator after smoke-touch's still press failed on main. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message (`--output-last-message`), unedited.
- **Verdict:** round 1 LAND WITH FIXES; round 2 LAND.
- **Disposition (round 1), taken in `aeaf71532`:**
  1. Taken: the hold is required only under the touch runner; the docs say the carriers run the ops before a hold that may be 0, and only the runner uses the hold as their window.
  2. Taken: **Elsewhere** lists `down`, `hold press`, `move`, the ops, `hold`, `up`.
  3. Taken: both remaining messages say "lengthen hold".

---

## Round 1

LAND WITH FIXES

1. **Should-fix — [docs/contract-for-agents.md:753](/tmp/rv-still/docs/contract-for-agents.md:753)**: “inside the hold” overstates parity. Non-runner carriers execute reads, then a separate full hold phase; `hold` does not bound those reads. State that `hold > 0` is required everywhere, but only the runner uses it as the reads’ time window. Correct the same wording in `agent-drag.mjs:29` and LLP §11:915.

2. **Should-fix — [llp/1080.000-real-touches-on-ios.rfc.md:953](/tmp/rv-still/llp/1080.000-real-touches-on-ios.rfc.md:953)**: “Elsewhere” still documents the old order. Change it to `down → hold press → move → during → hold → up`.

3. **Nit — [host/apple/touches.mjs:307](/tmp/rv-still/host/apple/touches.mjs:307)**: Two diagnostics still recommend “lengthen press or hold” (also line 310). Extra press is consumed by the pre-read wait; change both to “lengthen hold”.

The diagnosis and smoke correction are sound. Uniform refusal is a reasonable shared API constraint. No other executable callers needing migration found. Caps and diff checks pass; no builds or simulators run.
## Round 2

LAND

No unresolved findings. The runner-only refusal, smoke correction, docs, and messages match the phase ordering. No other affected callers or cap violations found.

Read-only review; no builds or simulators run.