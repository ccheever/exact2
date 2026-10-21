# Code review: host/web in the gate (Cargo.toml default-members, AGENTS.md), 2026-10-05 (grok)

- **Family:** xAI — `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless, a detached worktree at `be1669d21`.
- **Method:** the shared brief (sha256 `748a084bd94fc65bc1c7f8146cc24a5e3c84a8a94936970fc13cb5ca8d5c22b5`); one round; blind. The author (Claude) is not a reviewer.
- **Transcription:** stdout, unedited; opening sentences are progress narration.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** 1. Taken: the unlinked-capability test calls `link(Linked::CORE)` first. Not reproduced: with `--test-threads=1` the old test passed three times (libtest gives each test a fresh thread today); kept as a guard, since the gate must not depend on that.

---

I'll review only commit `be1669d21` against its parent, read-only, and check whether exact-web stays inside the deterministic gate.The diff is small. Next I’ll check exact-web’s remaining tests, how the gate and async lane treat `default-members`, and whether the docs match that.**LAND WITH FIXES**

1. **P1** — `host/web/src/link.rs:339` (now blocking via `Cargo.toml:9`). `a_plan_that_uses_an_unlinked_capability_is_refused_by_name` boots only if this thread’s `LINKED` cell is still `Linked::CORE`, but libtest reuses worker threads and earlier tests in the same binary (`host::height_drag`, `host::reorder_drag`, `host::transform_drag`) call `link_for_tests()`, which sets `motion`, `drag`, and `format` for the rest of that thread’s life. The markdown/segments/grid checks still pass; the `motion, drag` and `format` checks then fail. Isolated runs stay green. Edit: start the test with `link(Linked::CORE)` (all of those fields are `None`, so this only stores the cell).

Otherwise the new gate set holds. Non-ignored `exact-web` tests stay in-process: the Bun/Chrome tests are `#[ignore]`, and only `static_serving_falls_back_to_an_interrupted_previous_build` matches `scripts/async.mjs`’s `async lane:` grep. `dev.rs` no longer sleeps or asserts compile/bake under 100 ms; scratch dirs are `exact-dev-{pid}-{name}` with distinct names, so parallel test binaries do not share them. `exact-web` / `exact-web-capabilities` / `exact-web-geometry` turn on no Cargo features (`resolver = "2"`, no `[features]`), and `caltrain`, `contract`, and `exact-bake` were already default members. Nothing but Cargo reads `default-members` (`scripts/kept.mjs` deletes the key). The 60 s budget was already missed (~90 s); the measured suite delta sits inside the ±15 s noise, and the ~5 s note matches the 4.9–5.7 s test timing.
