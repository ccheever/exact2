# Code review: LLP 1121 stage 3, commit 6, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `2f01c58bedd43c294fb1eca660c17b70bc71d18885be1de057b0d6d3f407904e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: two high, eight medium, three low; Grok: two high, six medium, three low.
- **Disposition:** fixed in round 1 (a CSS reader that reads strings and nesting, dark media, selector lists, colour references, collisions, when lines taken back, the review across a recompute); the accent-color offer stays a hand Contract edit until the server writes Contract

---

Found **2 high, 8 medium, and 3 low severity issues**.

1. **High — CSS declarations can disappear before counting.** [tamber/src/import/mod.rs:228](/tmp/rv-s3c6/tamber/src/import/mod.rs:228)  
   Brace matching, comment removal, and semicolon splitting ignore strings. For example, `:root { --label: "{"; --background: #fff; }` produces no blocks, so the count passes as zero values. A block containing nested rules also loses its direct declarations. **Fix:** tokenize CSS with quote, escape, comment, and nesting awareness; account for every declaration before mapping.

2. **High — Reconciliation leaves stale conditional values.** [tamber/src/import/mod.rs:146](/tmp/rv-s3c6/tamber/src/import/mod.rs:146)  
   Only incoming `when` lines are compared. If an existing blue override becomes equal to the default, shadcn omits that incoming `when`, and reconciliation retains the old override—even reporting “nothing to propose.” Removing a source theme has the same problem. **Fix:** compare existing and desired condition sets and propose `UnsetTokenWhen` for obsolete overrides.

3. **Medium — Nested media conditions become unconditional values.** [tamber/src/import/mod.rs:245](/tmp/rv-s3c6/tamber/src/import/mod.rs:245)  
   Recursion discards at-rule ancestry. A white `:root` followed by a black `:root` inside `@media (prefers-color-scheme: dark)` imports black for both schemes and reports white as superseded. **Fix:** retain enclosing conditions; recognize dark media queries or report unsupported conditional declarations.

4. **Medium — Selector lists are interpreted inconsistently or lose themes.** [tamber/src/import/shadcn.rs:37](/tmp/rv-s3c6/tamber/src/import/shadcn.rs:37)  
   `:root, .light` works, but equivalent `:root,.light` does not. Worse, `.theme-blue, .theme-red` retains only the last theme while counting the declaration as mapped. **Fix:** parse comma-separated selector alternatives, map every supported alternative, and count each original declaration once.

5. **Medium — Valid CSS theme names produce invalid axes.** [tamber/src/import/shadcn.rs:278](/tmp/rv-s3c6/tamber/src/import/shadcn.rs:278)  
   `.theme-default` produces `default | default`, rejected by `AddAxis`. `.theme-123` and `[data-theme="blue.dark"]` similarly produce invalid Tamber identifiers after their values have been counted as mapped. **Fix:** allocate valid, unique axis values, including a collision-free default sentinel, or report unrepresentable themes before proposing.

6. **Medium — Token-name collisions defeat accounting and idempotence.** [tamber/src/import/mod.rs:75](/tmp/rv-s3c6/tamber/src/import/mod.rs:75)  
   `--chart-1` and `--chart-s1` both become `shadcn.chart.s1`. An empty design receives duplicate additions; an existing design can receive conflicting setters, lose one mapped value, and alternate values on successive imports. **Fix:** detect destination-name collisions before reconciliation and report them explicitly.

7. **Medium — References to unmapped variables are counted as mapped colors.** [tamber/src/import/shadcn.rs:89](/tmp/rv-s3c6/tamber/src/import/shadcn.rs:89)  
   `names` includes non-color declarations. Thus `--radius: 1rem; --copy: var(--radius)` reports radius as pending but creates a color token referencing nonexistent `shadcn.radius`, causing the whole proposal to fail. Alias chains ending in unknown references have the same problem. **Fix:** resolve reference dependencies and kinds before marking aliases mapped; report unresolved, cyclic, or pending dependencies.

8. **Medium — Existing role overrides are missed under supported spellings.** [tamber/src/import/shadcn.rs:317](/tmp/rv-s3c6/tamber/src/import/shadcn.rs:317)  
   Lookup requires the exact `-exact-*` spelling. A valid existing `background = …` or differently cased spelling causes another `-exact-background` declaration to be proposed. The checker rejects the duplicate, preventing even the token-only review from being held because all offers are initially planned together. **Fix:** use the checker’s canonical role matching and update the existing declaration.

9. **Medium — Recomputed proposals lose their review information.** [scripts/tamber-server.mjs:416](/tmp/rv-s3c6/scripts/tamber-server.mjs:416)  
   `accept` calls `hold` without `review` when selecting a subset or recovering from a conflict. The replacement loses unmapped values, counts, and optional-role identification. I reproduced this through the actual server handler in memory. **Fix:** preserve review metadata and remap item indices when operations are filtered.

10. **Medium — The accent-color offer cannot be accepted.** [tamber/src/import/shadcn.rs:348](/tmp/rv-s3c6/tamber/src/import/shadcn.rs:348)  
    It has no operations, so its wire offer has `items: []` and requires a manual Contract edit. D14’s accent mapping therefore cannot participate in T1a’s selectable choices and single accepted transaction. **Fix:** provide a separately selectable root `accent-color` edit, including detection of an already-correct declaration.

11. **Low — Valid bare HSL forms are rejected.** [tamber/src/import/shadcn.rs:102](/tmp/rv-s3c6/tamber/src/import/shadcn.rs:102)  
    Slash alpha works, but `222.2deg 84% 4.9%` and `222.2, 84%, 4.9%` are reported as non-colors. The underlying color parser supports these forms when wrapped in `hsl()`. **Fix:** validate the wrapped channels with that parser instead of the narrower numeric heuristic.

12. **Low — Adding the first axis relocates header comments.** [tamber/src/edit.rs:878](/tmp/rv-s3c6/tamber/src/edit.rs:878)  
    Inserting immediately after the header puts an existing indented header comment beneath the new axis, making it read as an axes comment. **Fix:** insert after the header’s associated comment group and preserve block spacing; extend the placement test accordingly.

13. **Low — Cargo startup failures mask the actual error.** [scripts/tamber-server.mjs:531](/tmp/rv-s3c6/scripts/tamber-server.mjs:531)  
    When spawning cargo fails, `stderr` can be null. `process.stderr.write(r.stderr)` then throws a `TypeError` instead of reporting the spawn error and returning failure. Reproduced with a mocked spawn failure. **Fix:** handle `r.error` explicitly and guard nullable output.

Caps and formatting checks passed. An in-memory invocation probe confirmed that source and app paths containing spaces remain single arguments. The existing tests miss the cases above. Rust tests were not run: this read-only worktree has no build artifacts. No files were changed.
