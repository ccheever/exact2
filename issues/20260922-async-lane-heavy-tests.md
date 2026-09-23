# Four heavy tests run in the async lane, not the blocking gate

**Status:** Open
**Systems:** kernel, contract, async lane
**Author:** Charlie Cheever
**Date:** 2026-09-22

Moved by Charlie's decision (2026-09-22) to keep the blocking test check under its 60 s share; they are slow, not flaky. Each is `#[ignore = "async lane: ..."]` and `bun scripts/async.mjs` runs them per commit (it greps that marker):

- `kernel/tests/it/content_region/split_facts.rs` `scalar_overflow_is_separate_from_192_source_admission` (~14 s warm)
- `kernel/src/layout.rs` `seeded_512_trees_match_fresh_frames_content_and_baselines` (~11 s warm)
- `contract/cli/tests/typescript.rs` `generated_signatures_check_real_sync_and_async_providers_and_the_dispatcher` and `caltrain_types_follow_its_real_plan_without_requiring_the_host_owned_source` (tsc through a subprocess, ~12 s together; the compiler lane agreed)

Run them by hand: `cargo test -p exact-kernel -p contract -- --ignored scalar_overflow_is_separate seeded_512_trees generated_signatures caltrain_types_follow`. Close this when each is fast enough to return (a cheaper fixture that keeps what it proves) or is accepted as async-only.
