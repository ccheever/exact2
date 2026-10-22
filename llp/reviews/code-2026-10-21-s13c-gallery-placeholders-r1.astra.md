# Code review: contract gallery: a symbol's prop gets a role and a handler's action prop an inert action of its payload (stage 13 C), round 1, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `92c9a0fbbe`.
- **Method:** one brief (sha256 `294583b2d57c42e9e60e312708387c86e342c431ed61e95e9cc9c6b9565f5be1`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken: a role only for a prop that is the whole remainder after symbol:; for and match bindings shadow the prop; tests widened

VERDICT: WRONG

1. **should-fix — [contract/cli/src/gallery/inert.rs:103](/tmp/iosfix-rv-gal1/contract/cli/src/gallery/inert.rs:103): The symbol match accepts a suffix.** The trailing `..` matches `symbol:${chevron}-left`. Previously, the placeholder `"chevron"` produced `chevron-left`, a valid schema alias; now it becomes `"back"`, producing invalid `back-left` and refusing the cell. Require the interpolation to occupy the entire remainder; otherwise retain the ordinary string placeholder.

2. **should-fix — [contract/cli/src/gallery/inert.rs:130](/tmp/iosfix-rv-gal1/contract/cli/src/gallery/inert.rs:130): The walk ignores lexical shadowing.** Both helpers compare identifier spelling without accounting for `for` item/index bindings or a `match` binding. For example, a component can display its `symbol: string` prop as text while a loop binds a different `symbol` used by an image. The new code changes the outer text placeholder from `"symbol"` to `"back"` despite that prop never naming an image. Track shadowing when entering these bodies; restore the outer scope for siblings and the `none` arm.

3. **nit — [contract/cli/tests/it/gallery.rs:694](/tmp/iosfix-rv-gal1/contract/cli/tests/it/gallery.rs:694): Coverage establishes the motivating cases, but misses the boundaries.** Role fallback is covered. Own-name role recognition, generated-name collisions, repeated-signature deduplication, explicit action arguments, and forwarding through another component are not. Each tested signature occurs only once, so the test does not establish “one per signature.” Add the suffix and shadowing regressions alongside their fixes.

The other requested checks look sound, with these qualifications:

- First-wins is sufficient for incompatible **mandatory** payloads: either choice, including `noop`, must be refused. Different parameter names with identical types need no conflict fallback. `navigate` is an optional-payload exception: `noop` can satisfy both `navigate` and `press`, whereas the selected string action cannot. However, gallery placement already rejects nested navigation roots, so this is not an additional placement regression today.
- Every nonempty payload-table entry names a compiler handler. `aria-label` returns none; an identifier-valued `resize` is classified as a handler by the compiler too. `Use` arguments are skipped; their slot children are correctly visited.
- Payload ordering and types match the compiler, including scroll’s two numbers and transformgeometry’s four. `reorderdrop` correctly reports `string, option<string>`, but the primitive filter deliberately leaves it using `noop`. Current parameter names are legal, distinct within each signature, and local to their actions.
- `symbol_role(prop)` correctly recognizes canonical roles and accepted aliases. Declaration ordering is deterministic. Empty declarations preserve the original Gallery header bytes, and component/shape names are allocated before inert-action names, so those allocations cannot be renamed by this change. The extraction matches the surrounding code style.

Static review of `92c9a0f` only. No files edited, Cargo invoked, or 46-app byte comparison performed. The diff leaves existing compilation and Tamber-check paths unchanged.
