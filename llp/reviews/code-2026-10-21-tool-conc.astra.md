# Code review: design-tool.test beside another run, round 1 (quick) of 637d00a336: two runs at once (claimPort, the --via hello wait, the harness) (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort xhigh, read-only sandbox, a throwaway worktree.
- **Method:** round 1 (quick) of 637d00a336: two runs at once (claimPort, the --via hello wait, the harness); HEAD `637d00a336`. The author (Claude) is not a reviewer.
- **Verdict:** one BLOCKER (T2b and the checkout's theme-fixture lock), answered in 5e5435b866.

1. **BLOCKER — [scripts/design-tool.test.mjs:1116](/tmp/rv-dt-conc-astra/scripts/design-tool.test.mjs:1116):** T2b still starts both runs against the same `apps/theme-fixture`. Separate ports and dist directories do not isolate `.exact/tamber.lock`: [tamber_serve.rs:353](/tmp/rv-dt-conc-astra/contract/cli/src/tamber_serve.rs:353) immediately refuses the second writer. If T2b overlaps, one run’s design server fails and its `--via` cannot connect successfully. Give this test a private fixture or serialize access to that fixture.

Otherwise, the exclusive turn listener protects competing claims, cleanup releases turns, and inherited `EXACT_PORT_TURNS` correctly bypasses child waits. The contract stub remains coordinated through `takePort`; tool stages, stub databases, PID-based storage, temporary directories and explicit dev dist directories appear isolated.

A loop that answers its token then stalls can delay `--via` for 120 seconds, but not indefinitely; refused connections still fail immediately.

Read-only review; no builds or tests run.
