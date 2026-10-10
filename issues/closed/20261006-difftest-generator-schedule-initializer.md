# Update the semantics generator test for Size.schedule so workspace tests compile

**Status:** Closed
**Resolution:** Fixed by 0d7fa9bf7; the generator fixtures now compile and pass.
**Systems:** Contract semantics, Differential generator, Async verification
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** semantics/difftest/tests/gen.rs:20, semantics/difftest/src/gen/mod.rs:48

The generator's `Size` gained the `schedule: bool` field, but `larger_programs_compile` still constructs it with only states, derives, actions, depth and events.

Verified by running:
```sh
cargo test --workspace --lib --bins --tests --no-fail-fast
```
Compilation fails with E0063 at `semantics/difftest/tests/gen.rs:20`: missing field `schedule` in initializer of `Size`. No workspace test execution follows that compilation failure. The root default-member gate does not expose this test target.

Choose the intended scheduling coverage explicitly in this fixture or use a default update where that is appropriate. Preserve a separate unscheduled case for the component-level semantics if needed; the field's documentation says that semantics does not implement scheduling.

Acceptance: the full workspace test command compiles all its test targets, the generator tests run, and both intended scheduling domains have correctly configured fixtures. Keep this in the existing async verification rather than adding a blocking check.
