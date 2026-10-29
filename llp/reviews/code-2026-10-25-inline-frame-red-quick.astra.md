# Code review: Linux inline frame main-red, quick (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `4c63e4ecdc`.
- **Method:** one brief (sha256 `bc2b5a43bc52a55437eb804337a32455ee6ea346106656d22aa7a8a1fe079338`). The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

None. The diagnosis is correct: [reload builds a fresh plan-scoped text engine](host/linux/src/presenter.rs:566) and installs it for measurement and painting. Preserving the test’s injected font catalog would be wrong product behavior.

**Non-blocking**

1. **The intended assertions survive, with narrower coverage.** The [unavailable assertion precedes the helper](host/linux/src/presenter/inline_frame_tests.rs:243), so it still catches stale geometry after reload. The shifted-box assertion then checks newly painted geometry. However, `fixture_fonts` rebuilds layout and calls `replaced()` again, so it can mask a future reload-specific layout or invalidation regression. I found no current product bug being concealed. Declaring fixture fonts in both plans would preserve the direct `reload → frame` path.

2. **Another test retains the same font switch:** [a_field_not_painted_is_unavailable_and_a_reload_reads_only_its_own](host/linux/src/presenter/range_frame_tests.rs:306). It boots with DejaVu, reloads with host fonts, then compares against the pre-reload position. Its y-only assertion can pass despite differing font metrics, so this is another fixture-consistency issue, not a confirmed additional failure.

Source review only; tests were not run under the read-only constraint.
