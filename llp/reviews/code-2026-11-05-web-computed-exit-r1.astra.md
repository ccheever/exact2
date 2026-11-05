# Code review: computed web exit animations, round 1, adversarial (astra)

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `f1d8155daa` on `ea352093a8`.
- **Method:** one brief (sha256 `8cdd8ed81401c4fc921d7cf8e5fd22001c19c45edbd6052568cddf0f98565e82`), one reviewer, pointed at the commit, with four passes: adversarial correctness, simplification, writing and verification. The author is not a reviewer.
- **Transcription:** the final message, unedited; its links name the review worktree’s paths.

One introduced finding remains.

- **P2 — The change exceeds the source-file cap**, at [rows.rs:490](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-exit-review-uu3cdocc/checkout-r1/host/web-js/src/rows.rs:490). Adding the exit-animation arm takes this file from **1,498 to 1,502 lines** under the caps checker’s counting rule, exceeding the binding 1,500-line limit. I independently reproduced those counts; `/tmp/exact-exit-caps.log` reports the same new violation. This adds a required-check failure beyond the two unchanged violations elsewhere. **Settlement:** consolidate the shared presence-binding code or move a small helper into `presence.rs`, bringing the checker’s count within the limit.

All four passes are complete. I found no additional correctness, simplification or writing findings, including in the commit message. The finite mapping, kernel parsing, initial-`none` loading, paint policy and press-scale handling are consistent with the requested scope. There are no prior settlements on this topic.

I independently ran the existing compiler test binary’s five `presence_tests::` tests; all passed. I also ran the line-count comparison and `git diff --check`. I did not rebuild the compiler or launch browsers from this read-only worktree.

The inspected author logs report 179 compiler tests and seven Chrome/WebKit tests passing, plus 13/13 Chrome conformance steps matching wasm. The negative control fails all four new cases at compilation when the exit-binding arm is removed. Completed check records report docs, build, compiler/root Clippy, formatting, boot and workspace success; the full Rust test run failed on Apple worker completion. These are inspected author results, not my own reruns.

Firefox was not run. The worktree remains unchanged.