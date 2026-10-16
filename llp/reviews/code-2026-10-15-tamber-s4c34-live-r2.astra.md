# Code review: stage 4 commits 3 and 4: the live table, round 2, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Method:** one brief (sha256 `f9830548c200cfb10201b032f80a7b2772dfffe301ec9eea0897a7e0bdb63080`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (an older snapshot over a newer generation; accent transitions; themes with no axes; deleted assets beside a design file; clashing guard digests; Apple platform colours; the Rust-data loop).
- **Disposition:** fixed in the second-round commit.

---

Blocking issues remain: one P1 and six P2 findings.

1. **[P1] An older snapshot can overwrite a newer generation.** [host/web/dev.js:277](/tmp/rv-s4-4/host/web/dev.js:277)  
   The design queue serializes snapshots against batches, but generation application runs independently. If a snapshot fetch is delayed while a new plan with the same structure boots, the old snapshot applies to that new plan. For example, a token edit plus a Contract comment edit forces publication without changing the structure digest. The subsequent snapshot request returns 204, leaving the old values installed indefinitely. I reproduced this ordering with the actual `dev.js`: generation D2 finished on design D1. Snapshot application must check which generation requested it.

2. **[P2] Live edits cannot create newly needed accent companion rules.** [host/web-js/modes.js:203](/tmp/rv-s4-4/host/web-js/modes.js:203)  
   The round-1 fix updates only companions emitted by the original build. Consider a token whose default tuple is `AccentColor` and another tuple is black. Changing the default to red preserves the plan structure and slice count, but that default tuple has no companion rule to update. A boundary using the token as its accent therefore leaves descendants on the inherited accent. The batch nevertheless advances the design digest, so a snapshot cannot repair it. Create missing companions and update the associated metadata, or require rebuilding for these transitions.

3. **[P2] Themes without axes lose local accent resolution in development builds.** [host/web-js/src/modes.rs:391](/tmp/rv-s4-4/host/web-js/src/modes.rs:391)  
   Development now emits every token as a custom property, including rules for `[data-x-modes=""]`. However, `modes.js` assigns tuple attributes only when `Axes.length` is nonzero. With no axes, a token such as `ink = AccentColor` resolves its custom property at the root and inherits that resolved value. Text explicitly using `ink` inside a locally tinted boundary therefore gets the root tint. Release output still resolves the inline expression locally. The runtime must assign the empty tuple to these boundaries too.

4. **[P2] Deleted assets beside a design file still qualify as temporary files.** [host/web-js/dev.mjs:116](/tmp/rv-s4-4/host/web-js/dev.mjs:116)  
   With the design configured as `assets/theme.tamber`, changing a value while deleting `assets/data.json` satisfies `designOnly`: the deleted file shares the design’s directory. Asset contents are absent from the logic comparison, and deleting an asset need not change the plan structure. Consequently, the loop discards the rebuilt output and continues serving the deleted asset. I confirmed that the classifier accepts this case. Directory proximity cannot distinguish a temporary file from an existing asset.

5. **[P2] Merging guards can silently replace an explicit `requires` digest.** [scripts/tamber-server.mjs:469](/tmp/rv-s4-4/scripts/tamber-server.mjs:469)  
   Untouched entries from `files` overwrite matching entries from `requires`. A client requiring `brand.tamber` at digest A, while supplying a newer files map containing digest B, gets B forwarded to the compiler and retained for acceptance. Its explicit precondition is lost. I confirmed the replacement through the actual server handler with a mocked compiler. Conflicting guards must be rejected; they cannot silently become the newer digest. The partial-accept fix preserves guards, but preserves these already-overwritten values.

6. **[P2] Apple does not resolve platform colours introduced by a live edit.** [host/apple/Sources/ExactKit/LiveDesign.swift:14](/tmp/rv-s4-4/host/apple/Sources/ExactKit/LiveDesign.swift:14)  
   A token may change to a previously unseen `-exact-platform-color(...)` without changing structure. Applying it interns a new platform-colour reference, but this path only applies the resulting batch; it never requests a fresh colour report. Kernel-resolved gradient stops, SVG paint and filters therefore use the fallback until an unrelated appearance/tint report occurs. Normal boot explicitly interns references before reporting them. Live application needs equivalent handling for newly introduced references.

7. **[P2] The standalone portable-Rust dev path still publishes value-only edits as generations.** [host/web/dev.mjs:692](/tmp/rv-s4-4/host/web/dev.mjs:692)  
   With `portableRust` enabled and no TypeScript module, `startCompiler()` returns before installing the resident compiler’s design handling. `.tamber` changes instead reach the Rust producer, and `readRustGeneration()` unconditionally publishes the new plan and restarts clients. This affects standalone `--wasm`, beyond the explicitly deferred `--serve-as` relay. That producer needs the structure/module comparison and batch path before advertising live support.

I checked every fix listed in HEAD. The snapshot-ordering, accent-companion and deleted-file fixes remain incomplete as described above.

Validation: both `modes.test.mjs` tests passed; targeted in-memory checks confirmed findings 1–5. Swift syntax parsing and `git diff --check` passed. Rust and full Swift builds were not run under the read-only restriction. No files changed.
