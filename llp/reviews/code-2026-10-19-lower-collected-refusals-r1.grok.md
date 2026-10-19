# Code review: Lowering keeps collected refusals beside a fatal one, round 1, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `17d73adb17`.
- **Method:** one brief (sha256 `7b98f23c855367add657e47b0e02a69b9820f34420189432acb956644b9277ec`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; one nit (the test never names the stopper or the single-error API)
- **Disposition:** taken in cc7055f272 (Astra's nit too)

Nothing blocking.

The collected-error returns are unchanged, the success path builds the same plan, and no caller depends on the stopping refusal being the first diagnostic.

**Collected paths and the success path.** Both `errors.is_empty()` checks still return only what was collected. They now return `Err(Vec::new())`, and `lower_with_sites` does `mem::take` of `l.errors`, appends nothing, and truncates to `MAX_REFUSALS` (20). That is the old list in the old order. A `?` or `return Err(err_one(...))` used to replace that list; it is now appended after it, deduped with the same key as `theme::push` (`id`, `span`, `message`), then truncated. `lower()` and `lower_all` still run `shown_errors` after that, so param names are rewritten without reordering. On `Ok`, `finish` still only interns theme strings already stored on the builder and validates cross-references. Icons are still registered before `finish`. `l.errors` is empty on that path (both checks sit above it), so a `lower-invalid-plan` from `finish` is still that one error.

**Guards.** `_profiles` and `_theme` are the only guards in the body. They now drop at the end of `lower_body`, before `finish`. `ThemeScope` and `DeclarationScope` only restore the thread-local theme table and profile declarations. Color parsing and profile interning happen earlier, while those guards are alive (`nodes`, decorations, `declare_theme`). `PlanBuilder::finish` does not read them. Callers that re-fold (`synthetic_candidates`, `retention`) already ran after those guards dropped, because the old function dropped them on return.

**Cascades.** Nothing new should be suppressed. `share` (`lower-share-picture`) is independent of the design refusal, and it runs in the action body before the view, so it does not pull in later attribute errors. The "design declares no axes" cascade is already avoided: axes are stored before the fold, and `setMode` pushes `tamber-mode` instead of returning. `lower-row-slot` still cannot appear beside collected errors; the first empty-check returns before child state. A `decoration` that names a token can still `?` out of `declare_decorations` as `lower-attr-value`, because a refused fold binds an empty theme and `token_error` only rewrites when `theme::bound()` is true. That line is a follow-on, and the design refusal stays first. Dropping it would hide a real stopper.

**First diagnostic.** `lower` / `compile_path` take `swap_remove(0)`, so the single error is now the first collected refusal. Nothing branches on the old fatal id:

- `contract build` prints the whole list (`compile_path_all_warned`).
- `contract types` prints only the first error's text. It does not match an id.
- `tamber serve`'s re-fold uses `synthetic_candidates()` and `retention()`, which `declare_theme` sets before any later `?`. It does not use error order. `compile_path_mapped` only puts the first message in "the app does not build".
- `admission` reports design-file diagnostics whenever it has any. `other.first()` is "stopped before the design" only when the design contributed nothing, which is still a pre-theme fatal (`declare_fonts` / `declare_sounds`). Those run before anything is collected.
- The editor matcher records every `[id]` on its own line. There is no quick-fix table that requires the fatal id to be first.

**Order.** Collected, then the stopper, then dedupe, then truncate. A font refusal cannot hide a design refusal: fonts are checked before `declare_theme`.

**Tests.** The new case is a real regression test for the reported bug. The action body `?`s on `share` before the view, the under-light edit is one `tamber-under-light`, and `len == 2` fails if the stopper replaces the design error or sorts first. I did not run it.

1. **nit** — The new case never names the stopper or the single-error API. `contract/cli/tests/it/tamber.rs:821`. `ids[0] == "tamber-under-light"` and `ids.len() == 2` pass for any second diagnostic. The duplicate-axis case above asserts `compile_path(...).id == "tamber-duplicate"`; this case does not assert `lower-share-picture` or `compile_path(...).id == "tamber-under-light"`.

If `l.errors` already holds 20, truncate drops the stopper. That matches the cap (collected first, at most 20). It takes a design that already filled the list before the action body.
