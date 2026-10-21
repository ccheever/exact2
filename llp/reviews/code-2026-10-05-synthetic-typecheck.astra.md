# Code review: synthetic conformance plans skip the data app's type check (host/web-js/build.mjs), 2026-10-05 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort xhigh, read-only sandbox, a detached worktree at `35eb0700a`.
- **Method:** one brief (sha256 `0f68f932fb7fa34be71e4aaf2b1e57f5f70ce096d4981ad7571f6713c7e73f2d`), the same sent to both; one round, sized to a three-line change; blind to the other review. Requested by the coordinator. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND.
- **Disposition:** no findings to take.

---

LAND

1. [INFO] `host/web-js/build.mjs:144` — Diagnosis is correct. Skipping `typecheck()` prevents both checking RealWorld against incomplete synthetic declarations and overwriting its editor declarations. Smallest correct fix; checking its own plan again is unnecessary. No edit.
2. [INFO] `scripts/agent.mjs:130`, `scripts/agent-playwright.mjs:73` — Only other `--data` callers; both supply replacement plans with Rust artifacts, where `ts` is already false. Ordinary web, dev, deploy and native builds retain type checking. No edit.
3. [INFO] `host/web-js/build.mjs:142` — Comment accurately describes the intended use; it does not guarantee a prior check during standalone synthetic runs. No rules violation or required edit.

Static review only; no builds run.