# Code review: stage 12 D2, the slot check on writes, round 3 re-check 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `40daf9bd0d`.
- **Method:** one brief (sha256 `4aca5cc011fd51d2994cf73eb3a60b7f3e9b75a8f11533fe22c6ef5e02989aac`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- [contract/cli/src/tamber_walk.rs:355](contract/cli/src/tamber_walk.rs:355): `trim_end_matches(['.', '-'])` removes legal trailing hyphens ([lexer:196](tamber/src/lex.rs:196)). `calc(lib.w- * 2)` indexes `lib.w` instead of `lib.w-`. Changing referenced `lib.w-` to `0px` can bypass the slot check; editing unrelated `lib.w` can select the chain and wrongly reject the edit. Preserve trailing hyphens.

**Should-fix:** None additional.  
**Nits:** None.

The permitted test could not run: the read-only sandbox denied Cargo’s target-directory write.
