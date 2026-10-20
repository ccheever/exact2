# Code review: LLP 1122 T1 commit 3 (the dials panel and the token grid), round 2, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `3ca4c6160` (commit `6b334829e` and its round-1 fixes, on commit 4).
- **Method:** one brief (sha256 `810b3af9f127bc88d53dbedc5e043d64db61e270d80396f8ad0b28d232c403cb`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (5 medium, 1 low).
- **Disposition:** all taken (commit 3 and 4's review fixes, in one commit): a select's option that is no literal of the dial's kind (a token's name) is left out and said in its help, its colours matched as the server folds them (hex and `rgb()`); a step's decimals count an exponent, and a length's `Npx` range is read; a design read is numbered as it starts and kept only if no newer read has started, so an overtaken read never evicts the shown one; a read is `describe`, each `resolve` and `hello` again, read again when the digests moved between or a tuple's modes do not match its column; the gate reads the drawn panel off the screen (each group's dials in order, More, control, label, value, a new range-and-step caption, options and help) against a fixture declared out of priority order, with a select and a scientific-notation range; refusals show each diagnostic's file, line, id and message.

---

The round-1 alpha correction, contrast dependency on `design.answer`, control-kind validation, explicit-step validation, priorities, column labels, role grouping, fallback badge, retry action, and refusal/empty-state distinction are correct by inspection. Controls are wired at HEAD. Tuple enumeration matches the IR and `modes_of`; dark-axis pruning and the alpha-preserving sRGB clips also look correct.

The step, select, cache, and test fixes still have gaps:

1. **Medium — Named select options remain unusable.** [app.ts:103](/tmp/rv-dt-3r2/apps/design-tool/app.ts:103), [app.contract:456](/tmp/rv-dt-3r2/apps/design-tool/app.contract:456)  
   Options are still passed through as strings. With `control=select options="brand.red|brand.blue"`, the fix inserts the current folded colour into the menu, but selecting `brand.blue` submits that name to `set-dial`. Tamber requires a dial’s value to be a literal and refuses it. Thus round-1’s named-option scenario remains broken. **Fix:** separate option labels from submitted values, resolve supported token choices to dial literals, and report or fall back from options that cannot be represented.

2. **Medium — Scientific notation defeats the default-step fix.** [app.ts:113](/tmp/rv-dt-3r2/apps/design-tool/app.ts:113)  
   Decimal precision is counted by splitting the spelling at `.`. Tamber accepts scientific notation and preserves range spellings in the IR. A valid dial such as `chroma: number = 0 in 0..5e-3`, used by an `oklch()` token, therefore receives `step=0.01` for a maximum of `0.005`. The slider cannot select a nonzero permitted step. **Fix:** account for exponents when deriving precision, or support an unrestricted step when metadata omits one. Add a fixture using scientific-notation bounds.

3. **Medium — Obsolete loads can evict the displayed snapshot.** [app.ts:256](/tmp/rv-dt-3r2/apps/design-tool/app.ts:256), [app.ts:334](/tmp/rv-dt-3r2/apps/design-tool/app.ts:334)  
   Snapshot numbers are assigned on completion, and eviction retains only the latest nine completions. Superseded asynchronous loads continue running even when the resource runtime discards their replies. If nine delayed, obsolete loads finish after the currently displayed load, they delete its snapshot. Changing either contrast picker then returns successful-but-empty ratios; cell lookup also becomes empty. **Fix:** make the resource own the data required by its dependents, or protect active snapshots and discard obsolete completions instead of evicting solely by completion count.

4. **Medium — One “snapshot” can combine different file revisions.** [app.ts:221](/tmp/rv-dt-3r2/apps/design-tool/app.ts:221), [tamber_serve.rs:638](/tmp/rv-dt-3r2/contract/cli/src/tamber_serve.rs:638)  
   `describe` and every `resolve` independently reread the files. The client discards the describe digests and associates resolve entries by array index without checking their `modes`. An edit between replies can combine foreground from one revision with background from another, producing a contrast ratio that existed in neither revision. Reordering axis values during that interval also puts swatches under incorrect column labels. **Fix:** obtain the design and resolves from one server snapshot, or return and validate revision digests on every resolve and retry inconsistent reads.

5. **Medium — The metadata gate still checks data rather than rendered controls.** [design-tool.test.mjs:134](/tmp/rv-dt-3r2/scripts/design-tool.test.mjs:134), [design-tool.test.mjs:150](/tmp/rv-dt-3r2/scripts/design-tool.test.mjs:150)  
   The expanded assertions check resource fields, while screen assertions still search for test IDs. Changing the Contract slider to `max=1 step=1` would leave this gate passing. The fixture’s declaration order already matches priority order, and it has no select. The real-server comparison meaningfully checks static canned replies, but the stub still cannot exercise changed values or overlapping successful file loads. **Fix:** inspect rendered types, bounds, steps, values, labels, options, and order—including “More”—and add deliberately reordered metadata plus delayed, changing replies.

6. **Low — Design refusals discard the actionable diagnostics.** [app.ts:55](/tmp/rv-dt-3r2/apps/design-tool/app.ts:55), [app.ts:222](/tmp/rv-dt-3r2/apps/design-tool/app.ts:222)  
   Tamber returns file, line, diagnostic ID, and explanation, but the panel receives only the top-level message, normally “the design is refused.” For an out-of-range dial or unknown token, the person sees Try Again without learning what needs correction. **Fix:** preserve and display the structured diagnostics alongside retry.

No files were changed, and no builds or tests were run.

**Verdict: Request changes — 5 Medium and 1 Low findings remain.**
