# Five tests run in the async lane, not the blocking gate

**Status:** Closed
**Resolution:** Accepted as async-only under Charlie’s 2026-09-22 gate-budget decision; all five current ignored tests pass, including repaired TypeScript Web API fixtures. Their async lane markers remain.
**Systems:** kernel, contract, markdown, async lane
**Author:** Charlie Cheever
**Date:** 2026-09-22

Moved by Charlie's decision (2026-09-22) to keep the blocking test check under its 60 s share; they are slow, not flaky. Each is `#[ignore = "async lane: ..."]` and `bun scripts/async.mjs` runs them per commit (it greps that marker):

- `kernel/tests/it/content_region/split_facts.rs` `scalar_overflow_is_separate_from_192_source_admission` (~14 s warm)
- `kernel/src/layout.rs` `seeded_512_trees_match_fresh_frames_content_and_baselines` (~11 s warm)
- `contract/cli/tests/it/typescript.rs` `generated_signatures_check_real_sync_and_async_providers_and_the_dispatcher` and `caltrain_types_follow_its_real_plan_without_requiring_the_host_owned_source` (tsc through a subprocess, ~12 s together; the compiler lane agreed)

- `markdown/tests/markdown.rs` `adversarial_input_stays_linear` (2026-09-23): not slow but timed — a 2 s wall-clock bound that took 2.3–4.4 s at load 140–180. A ratio of timings did not hold either (the same input varied 5–30× between runs at that load). What would return it: a deterministic count of the parser's work, not time.

Run them by hand: `cargo test -p exact-kernel -p contract -p exact-markdown -- --ignored scalar_overflow_is_separate seeded_512_trees generated_signatures caltrain_types_follow adversarial_input_stays_linear`. Close this when each is fast enough to return (a cheaper fixture that keeps what it proves) or is accepted as async-only.
