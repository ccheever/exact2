# Code review: stage 12 D2, the slot check on writes, round 3 re-check 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `81a77d1958`.
- **Method:** one brief (sha256 `f78dce245612229ea9756afa4c7814b3c65d02f9551ef5fda9132bd0377c7a84`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- [contract/cli/src/tamber_walk.rs:304](contract/cli/src/tamber_walk.rs:304): Valid identifiers may start with `_` (`tamber/src/lex.rs:189`). `width="calc(_lib.w * 2)"` therefore loses `_lib.w` from the index. Changing it to `0px` can bypass the slot check. Include underscore-leading names.

- [contract/cli/src/tamber_walk.rs:300](contract/cli/src/tamber_walk.rs:300): Over-indexing can cause wrong refusals. `calc` is a legal token name, but `calc(lib.w * 2)` does not reference that token. Editing unrelated `calc` selects this chain; an existing `lib.op = 0` then rejects the edit, violating the established unrelated-edit behavior. Extract actual references, excluding function names and quoted text.

**Should-fix:** None additional.  
**Nits:** None.

Other punctuation/non-ASCII names are rejected by the declaration grammar; `var()` is explicitly refused. Quoted words and keywords can produce extra matches; reserved-word checks do not eliminate the demonstrated `calc` collision. Over-indexing therefore costs work **and** affects correctness.

The allowed test could not run: the read-only sandbox denied Cargo’s target-directory write.
