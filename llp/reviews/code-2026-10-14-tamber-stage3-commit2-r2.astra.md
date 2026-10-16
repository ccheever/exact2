# Code review: LLP 1121 stage 3, commit 2, round 2, 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `c0c0579b23b00be20245cd7735d2b83769b5eccc1b7497b0926aa1c5326f0700`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: one high, six medium, one low; Grok: one high, four medium, one low.
- **Disposition:** fixed in round 2; named fields on the wire are commit 3's

---

Found **1 high, 6 medium, and 1 low** issue.

1. **High — `Restore` permits arbitrary checked source replacement.** [tamber/src/edit.rs:347](/tmp/rv-s3c2b/tamber/src/edit.rs:347)  
   Any caller can replace the entire document with another valid `.tamber` document, changing headers, declarations, exports, or role overrides without using their typed operations. Nothing proves these lines came from an inverse. The final checker constrains validity, not edit scope. **Fix:** exclude raw `Restore` from ordinary client operations. Accept it only through a verified server-issued inverse bound to the file, payload, and expected digest. A client-supplied “inverse” flag or digest alone is insufficient.

2. **Medium — `Restore` range arithmetic can panic on wasm32.** [tamber/src/edit.rs:354](/tmp/rv-s3c2b/tamber/src/edit.rs:354)  
   With `line=4294967295` and `remove=3`, `start + remove` overflows 32-bit `usize`. With overflow checks it panics immediately; otherwise it wraps to `1`, passes the end-bound check, and panics during `splice`. **Fix:** use checked arithmetic and explicitly validate both range endpoints before splicing.

3. **Medium — field validation still allows neighboring syntax to be changed.** [tamber/src/edit.rs:174](/tmp/rv-s3c2b/tamber/src/edit.rs:174), [edit.rs:230](/tmp/rv-s3c2b/tamber/src/edit.rs:230)  
   Setting a fixed token’s value to `light-dark(#000, #fff) //` comments out its preserved `scheme=fixed`, so the checker accepts a change it would otherwise refuse. Likewise, one metadata entry containing `label="x" scheme=fixed` passes validation but writes two entries; a trailing `//` can swallow subsequent entries. **Fix:** validate complete field grammar, rejecting top-level comments and metadata/range clauses in values. Require each metadata entry to be exactly one complete assignment, while allowing spaces inside quoted strings.

4. **Medium — rename still changes identifiers that are not token references.** [tamber/src/edit.rs:610](/tmp/rv-s3c2b/tamber/src/edit.rs:610)  
   Renaming token `r` to `b` changes `rgb(from #123456 r g b)` into `rgb(from #123456 b g b)`; both check successfully, but the color changes. Renaming `systemBlueColor` can also change the literal native selector in `-exact-platform-color(ios systemBlueColor, #fff)`. Renaming token `color` rewrites the kind in `ink: color = …`, causing refusal. **Fix:** rewrite declaration names and actual resolved reference positions, preserving kinds, channel keywords, color spaces, and platform selectors.

5. **Medium — a token named `when` is mistaken for a conditional line.** [tamber/src/edit.rs:591](/tmp/rv-s3c2b/tamber/src/edit.rs:591)  
   The parser permits `when = #000000`. Here, `starts_with("when ")` classifies it as a conditional; without a colon, the entire line is excluded from renaming. Renaming an unreferenced `when` token therefore succeeds without changing its name; references can instead make the operation fail. **Fix:** identify conditional lines through parsed `When` locations, or apply the parser’s declaration distinction.

6. **Medium — role-rename protection fails open during transactions.** [tamber/src/edit.rs:135](/tmp/rv-s3c2b/tamber/src/edit.rs:135)  
   Any checker error becomes “not a role.” Starting from a valid design, a transaction can set an unrelated token to an unresolved name, rename `-exact-label` to `brand`, then repair the unrelated token. Final validation succeeds and the role override disappears. **Fix:** resolve role identity directly from the declaration and vocabulary, independently of whole-design validity.

7. **Medium — edit names do not round-trip through the existing IR.** [tamber/src/edit.rs:715](/tmp/rv-s3c2b/tamber/src/edit.rs:715)  
   Lookup requires the authored spelling. The existing roles fixture declares `label`, while its IR reports `-exact-label`; using that reported name in `SetToken` returns “no token.” This affects clients using D13’s IR-based `describe`. **Fix:** use the checker’s role-name resolution for edit lookup while preserving the declaration’s authored spelling.

8. **Low — some refusals still point to 1:1 despite a known declaration.** [tamber/src/edit.rs:661](/tmp/rv-s3c2b/tamber/src/edit.rs:661)  
   Removing an absent value from an existing axis, naming an absent `before` value, and refusing a role rename still call `refuse()`. **Fix:** report the known axis/token span and add location assertions.

No further defect found in the bounded prefix/suffix calculation or `apply_all`’s inverse order. The raw snapshots address the listed byte-restoration failures. An independent in-memory model restored all **132,496** tested before/after line pairs, including repeated lines.

**Verification:** caps, changed-file Rust formatting, and diff whitespace checks passed. Cargo tests were not run: this read-only worktree has no compiled test binaries. JSON serialization is not implemented in this diff; commit 3 should test forward-operation and verified-inverse wire round trips. No files changed.
