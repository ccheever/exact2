# Code review: native abort, landing: the rebase onto main and door 23, round landing re-check 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `ea22acccaf`.
- **Method:** one brief (sha256 `79d2811cca7987e7a12d123a3be0802eb04fcadb0cb70db9227e715f7eb01484`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [runner/src/runner/admission.rs:200](runner/src/runner/admission.rs:200): **Aborting an adopted POST leaves module work permanently pending.** Main’s `keep_sent` retains superseded writes in `module_work.tickets`. This pending-only check skips their synthetic aborted outcome but still requests executor cancellation, which drops the eventual completion. Consequently, `module_work_landed` never removes the ticket and `has_pending()` stays true. Return an aborted outcome for retained module-work tickets too, settling them through the existing module-work path.

2. [runner/src/runner/admission.rs:168](runner/src/runner/admission.rs:168): **The retirement cap now loses cancellation on ordinary main-thread requests.** Main’s adoption preserves a superseded POST’s fetch and abort subscription. After 256 unrelated retirements, its mapping can be evicted while that POST still runs; `match_aborts` searches only ordinary pending requests and retired mappings, so a subsequent abort never cancels it. This requires no worker-retirement race. Preserve adopted fetch mappings with `module_work` until settlement and consult them when matching aborts.

**Non-blocking**

none.

Op 23 has no competing door assignment across native/render and both web realms. The runtime 5 compatibility gate, renumbered tests, and `finish`/`complete` split look consistent. Review was static; no files changed, network, tests, or builds.
