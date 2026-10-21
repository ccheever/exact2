# Code review: LLP 1121 stage 2, commit 3 (app axes, the slice and mode boundaries), 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a throwaway worktree at the reviewed commit.
- **Method:** one brief (sha256 `126302baf4bfd50bc436c42a0bf3506f602055c16c5282ccb837297c11861b14`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix (both).
- **Disposition:** fixed in round 1's commit (416b54839); the virtualized-list rekey cost (Grok P2) noted, not changed

---

**Fix before landing.** These are the main findings from code inspection:

- **MAJOR — Retired views can make `set_token` fail.** [runner/src/runner/theme.rs:32](/tmp/rv-s2c3/runner/src/runner/theme.rs:32) selects indexed rows without checking whether their views still exist. Index retirement only happens during periodic `Ids::retain` cleanup. Remove one themed subtree, then edit its token: the generated `SetStyle` targets destroyed views and the kernel rejects the batch with `UnknownView`. Remove entries on retirement or filter live views before applying.

- **MAJOR — Inherited-text changes do not invalidate descendants.** [runner/src/instance/theme.rs:177](/tmp/rv-s2c3/runner/src/instance/theme.rs:177) forces traversal only when the tuple changes. Changing an ancestor’s colour from token A to token B—or to a literal—leaves otherwise clean descendants and regions skipped. Their boundary rows retain A instead of updating or clearing. Skipped collections also retain the old inherited context for later mounts.

- **MAJOR — An unset binding incorrectly terminates inherited-token tracking.** [runner/src/instance/theme.rs:169](/tmp/rv-s2c3/runner/src/instance/theme.rs:169) treats any non-themed binding as a specified literal. An intermediate `color` binding that currently inherits, is absent, or fails parsing therefore loses the ancestor’s token context. A pin below it cannot reselect that token. [Boundary handling also skips every declared binding](/tmp/rv-s2c3/runner/src/instance/theme.rs:232), including an absent one. Track whether a row actually supplied a value separately from whether it read a token.

- **MAJOR — Nested pins leave the kernel’s mode row stale.** [runner/src/instance.rs:973](/tmp/rv-s2c3/runner/src/instance.rs:973) forces only token-reading bindings. With axes A and B, changing an outer A pin while an inner B pin stays textually unchanged recomputes the inner instance’s tuple, but does not rewrite its numeric `Mode` row. Colours and the tuple exposed to kernel readers then disagree.

- **MAJOR — Token edits leak between plans and disappear on unrelated boots.** [runner/src/bridge.rs:343](/tmp/rv-s2c3/runner/src/bridge.rs:343) applies the thread-wide `EDITS` list to whichever plan builds a table. Two live runners sharing a token name can consume each other’s edits. Conversely, `set_role_overrides` clears all edits when another runner boots; even a subsequently refused boot loses them because rollback restores only role overrides. Scope edits to the owning runner.

- **MAJOR — Role edits update explicit rows but leave defaults unchanged.** [runner/src/runner/theme.rs:19](/tmp/rv-s2c3/runner/src/runner/theme.rs:19) changes the parse table without updating the role-override table. Editing `CanvasText` can change explicit token rows while unsaid text retains the old override. `AccentColor` has no indexed tint rows to refresh. Either implement role refresh or explicitly refuse these edits until supported.

- **MAJOR — The document-fold refusal misses scheme-only boundaries.** [runner/src/instance/document.rs:401](/tmp/rv-s2c3/runner/src/instance/document.rs:401) refuses only plans with app axes. An inline theme without axes can still require implied colour rows at a `color-scheme` boundary. The detached path skips token tracking, and the document fold emits no implied rows, so its output differs from the kernel-backed render.

- **MINOR — Terminal mode pins are rejected.** [kernel/tables/schema.json:3958](/tmp/rv-s2c3/kernel/tables/schema.json:3958) lacks a `terminal` admission. The terminal compiler consequently refuses `-exact-mode`, despite the runner implementing its selection.

- **MINOR — Computed duplicate pins bypass the compiler’s rule.** [runner/src/bridge.rs:246](/tmp/rv-s2c3/runner/src/bridge.rs:246) accepts repeated axes with the last value winning. The compiler rejects the same literal with `tamber-mode`. Runtime validation should enforce the same rule.

- **MINOR — The inherited-colour list omits an existing colour row.** [runner/src/instance/theme.rs:13](/tmp/rv-s2c3/runner/src/instance/theme.rs:13) excludes `TextStrokeColor`, which the schema marks inherited and which already accepts theme colours. Its token remains in the ancestor’s slice across a pin.

The mixed-radix ordering matches Tamber’s ordering, and role-spelling bloom matching and `written` preservation look consistent. The digest changes use the shared generators. I treated runner-side selection and the deferred JS boundary implementation as the stated staging decisions. Additional RFC differences needing documentation are `axis=value` versus D5’s `axis:value`, and the role-axis refusal.

Performance remains unverified: the allocation test could not run. The no-theme path still performs tuple TLS operations and named-bit resets. The new tests also lack retirement/edit, nested multi-axis, inherited-text clearing, later virtualized mounts, reuse, and cross-runner edit cases.

Validation: Cargo was blocked by the read-only build lock. Using the existing test binary, **7 Tamber tests passed; 11 were blocked creating temporary fixtures**. All **9 existing theme tests passed**. Formatting and diff whitespace checks passed. No files were edited.

VERDICT: fix (MAJOR — runner/src/runner/theme.rs:32)
