# Code review: apps/expose clean build (main-red), quick (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `af650c8776`.
- **Method:** one brief (sha256 `ab050ac09d42b49f120f1a015447b8f26adafb26329c1854a3f964148b631956`). The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **[P2] Creating `local.ts` after a fallback bake can leave native builds stale.** Removing the empty-file branch in [apple/build.rs:17](apps/expose/apple/build.rs:17) (also Linux/web) means a clean bake no longer registers `local.ts` as an input: [exact-js-bake watches only existing captured files](js/bake/src/lib.rs:547). If you subsequently create `local.ts` manually—or another platform’s build creates it—while leaving this build’s environment unchanged, an incremental native build can retain the empty-key output. [The receipt check hashes only previously watched paths](scripts/cargo-dep-info.mjs:67), so it misses this addition too. Track the override’s presence to invalidate that build. This finding is from source inspection; full bakes were not rerun.

**Non-blocking**

none. Resolver precedence, the ignored key destination, and invalidation when deleting an already captured `local.ts` look correct. No other script directly requires that file to exist.

## Re-check (worktree at `03aade762b`, brief sha256 `8706dabce440963325f6d88e9dab2086183e446320cdc6187cc88d39653600e7`)

**Blocking**

None. Keeping the empty-file branch resolves the earlier blocker: all three native build scripts create `local.ts` before capture, and [the bake watches captured sources](js/bake/src/lib.rs:549). Deletion invalidates those inputs; the dev loop and web freshness checks also detect deletion. No broken reader or new stale-cache/receipt path found.

The resolver paths consistently prefer `local.ts`, falling back to `local/index.ts`. Generated keys go only into ignored `local.ts`; normal staging excludes it. Force-adding it or manually putting a key in the tracked fallback remains possible.

**Non-blocking**

1. The absent-case verification claim needs one correction: [`build.mjs --check`](host/web-js/build.mjs:226) invokes Expose’s `web/build.rs` **before** staged TypeScript checking. That check therefore normally sees recreated `local.ts`. The early ambient analysis, standalone `tsc`, and bare bundle can exercise the absent case. This qualifies the stated coverage, not the fix.

Read-only review; confirmed the module walk’s absent → present → absent behavior in memory. Full builds were not rerun.
