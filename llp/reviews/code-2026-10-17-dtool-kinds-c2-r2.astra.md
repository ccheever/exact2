# Code review: LLP 1122, stage 6 kinds in the tool, steps 3-4, round 2, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `056c17479`.
- **Method:** one brief (sha256 `60505da833a30f12c05271431589bec154647b939fdfe8381fc1873e58689f17`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 4 medium, 2 low).
- **Disposition:** all taken, in `3a3e08e27`:
  - the cut is keyed by the branch's value, and a field draft whose branch value moved is stale;
  - required parts are judged from the current texts (a blur before a written spread);
  - one CSS item per shadow word;
  - the fields show the draft only while it is this cell's token edit;
  - inlineWhy reads after its await;
  - a unit may be none;
  - `normal` is refused only before the size;
  - INSET shows as selected.

- **High — Refreshing the design separates fields from the retained draft.** [app.contract:594](/tmp/rv-dt-k3r2/apps/design-tool/app.contract:594). Change `12px` to `16px`, start a scrub, then Cancel scrub. Its mutation refreshes `design`, changing `branch.answer`; `partFor` stops matching, so fields show `12px` while `editValue` remains `16px`. Changing unit then produces `12rem`, losing the amount edit. Undo and refreshes during an outstanding draft have the same problem. **Fix:** preserve the cut and field text together with a retained draft across unchanged branch reads; invalidate the whole draft when its branch becomes incompatible.

- **High — Adding spread leaves blur incorrectly optional.** [parts.ts:115](/tmp/rv-dt-k3r2/apps/design-tool/parts.ts:115), [app.contract:743](/tmp/rv-dt-k3r2/apps/design-tool/app.contract:743). In the fixture’s first shadow, add spread `1px`, then clear blur. Because optionality was calculated from the original three lengths, this produces `0px 1px 1px #00000033`: Tamber accepts the displayed spread as blur. The second shadow stays intact, masking the mistake. **Fix:** determine required positional fields from the current field texts—blur is required whenever spread is populated. This round 1 finding remains partially unfixed.

- **Medium — Whitespace splitting still mislabels accepted shadow values.** [parts.ts:29](/tmp/rv-dt-k3r2/apps/design-tool/parts.ts:29). Tamber’s CSS reader accepts `0 1px 2px#000` as three lengths and a colour. The editor assigns `2px#000` to colour and offers an empty blur. Editing colour to `red` silently removes blur. **Fix:** recognize CSS token boundaries while retaining source spans, or refuse typed fields when a whitespace word contains multiple CSS items.

- **Medium — Replacing a token draft leaves its field state active.** [app.contract:598](/tmp/rv-dt-k3r2/apps/design-tool/app.contract:598), [app.contract:752](/tmp/rv-dt-k3r2/apps/design-tool/app.contract:752). Edit `space.edge` to `20px`, change a dial, then choose Only this mode on the still-selected cell. That action initializes `editValue` to committed `12px`, but the fields still show `20px`; another field edit resurrects the abandoned amount. **Fix:** clear `rawFor` and `partFor` whenever another operation replaces the token draft, including when Only this mode initializes a fresh draft.

- **Medium — `inlineWhy` captures the running design before its asynchronous read.** [dev.mjs:535](/tmp/rv-dt-k3r2/host/web-js/dev.mjs:535). Suppose pages run inline weight `400`, disk contains `500`, and its rebuild is underway. Propose reverting to `400`. If the rebuild finishes during `scrubDesign`, comparison still uses captured `400` and returns `live`, although pages now run `500` and require rebuilding. **Fix:** obtain `scrubbed ?? design` after the awaited read, as `scrubPreview` does.

- **Medium — The valid “none” unit is treated as a missing required field.** [parts.ts:65](/tmp/rv-dt-k3r2/apps/design-tool/parts.ts:65). For a length declared as `0`, choose `px`, then `none`. The latter sets `editValue` to `""` and blocks proposal instead of producing valid `0`. **Fix:** allow an empty unit; let Tamber reject nonzero unitless lengths.

- **Low — Valid `normal` line heights suppress every typed field.** [parts.ts:154](/tmp/rv-dt-k3r2/apps/design-tool/parts.ts:154). `16px / normal` returns no fields, whereas equivalent `16px/normal` works. `typeface.rs` explicitly accepts this leading. **Fix:** reject `normal` and `oblique` only in the style/weight prefix.

- **Low — Uppercase inset cannot appear selected.** [app.contract:1211](/tmp/rv-dt-k3r2/apps/design-tool/app.contract:1211). Accepted `INSET` is preserved as the select’s value, but its options contain only `""` and `"inset"`, leaving no matching option. **Fix:** normalize the displayed selection while preserving the original spelling until edited.

Round 1’s signed/exponent lengths, numeric weights, aliases, text-shadow-list refusal, dependency links, and consecutive edits against an unchanged cut are addressed. The synchronization and positional-field fixes remain incomplete.

`proposeHere` correctly strips unrequested texts, preserves held-proposal routing, handles rejected asynchronous hooks, and leaves the wasm path unchanged. Scrub proposals retain their own preview validation. The reader check avoids waiting for a reader build.

The added tests cover one-line writes for all five typed kinds and exact multi-shadow round trips, including inset and a token colour. They miss the transitions above; the protocol test mocks `inlineWhy`. I ran read-only, in-memory probes; all 16 supplied round-trip examples passed. Integration tests were not run because they write files.

**Verdict: Request changes — valid field-edit sequences can still silently change unintended values.**