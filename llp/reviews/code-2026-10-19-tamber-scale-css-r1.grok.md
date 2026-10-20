# Code review: tamber scale_css held as ratios, not wall clocks (the lead's 22:29Z load flake), and the walks they found quadratic, round 1, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `63b3ba9351`.
- **Method:** one brief (sha256 `50b55d243082414cacc49e1fe0902743f44bf710b477eedd5efcea809a855eb9`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocking; three should-fix (the 10x gate misses the pre-fix ~7x curve; copied's prelude swamps n; alias chains fixed at 250 links), one nit (seen's doc)
- **Disposition:** all taken in fb8a3dc7b9

Coverage of the old checks holds, and the colour-alias memo is correct. The ratio gate does not reliably fail the quadratic cases it is there to catch.

**Coverage.** Every previous output check is still at the same full size: 200_000 closed and open, 20_000 wide, 20_000 copied, 6_000 labels, 40 doubling selectors, 80×250 aliases, the 300-chain (`mapped == 257`), 5_000 type keys, 2_000 doubling `var()`s, theme `source == 22_001`, 20_000 lengths, 3_000 marks, the 5_000-chain (`mapped >= 257`), kind 256 vs 257, the long-value reasons, and 30_000 colours. Braces (250_000), unmatched braces (100_000), empty rules (20_000), and resets (18_000 + 12_000) never asserted a result; they still run at those sizes inside `linear` (`4 × n`).

**Memo (`engine.rs:244`).** A start is stored as `(None, usize::MAX)` only after this walk has already pushed 257 referring nodes, so that chain is at least 257 links. A later walk that hits that entry (`engine.rs:215`) is longer still, and the back-walk only copies `usize::MAX` onto variables that reach it. A chain of 256 links reaches the literal without taking the cut (`links == 256`, and `256 > LINKS` is false); the cut does not write the nodes it passed, so a shorter suffix stays unmarked. The 300-chain check (`mapped == 257`) and `kind(256)` still lock that boundary. The key is `(at, var)` for the block `seen` was called with (`engine.rs:215`, `engine.rs:247`); a `.dark` result is not reused for `:root`.

**Ratio.** A pure quadratic is 16× at 4× size. `b <= a * 10 + 50ms` (`scale_css.rs:29`) therefore passes whenever the small sample is about 8 ms or less. The 50 ms term only loosens the bound, so it does not create false failures. Best-of-three plus a 2.5× gap over linear (4× vs 10×) also makes a noise failure unlikely: load that multiplies both sides cancels. Fixed work does cause false passes.

### Should-fix

**`tamber/tests/it/scale_css.rs:29` — the 10× gate misses the regression these commits measure.** A power-law fit of the commit’s own before-fix release times (158 ms at 1,250 names, 2.8 s at 10,000, about 18× on an 8× input) is about 7× at this test’s 1,250→5,000 span (`scale_css.rs:348`), under `10× + 50 ms`. `cargo test` compiles `exact-tamber` at `opt-level = 3` (`Cargo.toml:129`), so that release curve is what the test runs. Removing the new memo and keeping the 256-step cut stays near that curve and stays green. Quadratic also passes while the small sample is under about 8 ms (`doubling selectors` at `n = 10`, `scale_css.rs:163`, and any shadcn run dominated by `vocab()` re-parsing `schema.json` inside `shadcn_of`, `scale_css.rs:40`).

Fix: count steps for the long chain (a 5,000-chain that memoizes should stay on the order of `n + 256` visits; the pre-fix walk is `n × 256`). Keep the ratio for shapes whose broken form is actually ~16×, and apply the 50 ms pad only when `a` is under it, for example `b <= a.saturating_mul(8)` once `a > 50 ms`.

**`tamber/tests/it/scale_css.rs:103` — `copied`’s fixed prelude swamps `n`.** Sixty-three preludes of 4,000 `x`s are ~252 KB. At `n = 5_000` the rules are ~80 KB, so the sheet grows about 1.8×, and a byte-quadratic scan is about 3×, which is under 10× even with a zero floor. Both sizes also hit the 8 MiB copy cap after the same ~30 rules, so the expensive part does not scale.

Fix: keep `copied(20_000)` as the functional assert. Drive the ratio with a prelude whose size scales with `n` (or with rules and no fixed 252 KB wrapper) so `S(4n) / S(n)` stays near 4.

**`tamber/tests/it/scale_css.rs:173` — alias chains scale the count, with link depth fixed at 250.** The regression named in the comment (each token rewalks its chain: `chains × 250²`) is linear in `chains`, so 20 vs 80 stays about 4× and passes. The long-chain ratio does not cover it either: both ends are past the 256 cut.

Fix: add a second ratio over depth under the cut, one chain (or a few) at 40 links vs 160. An `O(L)` walk is about 4×; an `O(L²)` rewalk is 16×.

### Nit

**`tamber/src/import/engine.rs:206` — `seen`’s doc says each variable on the chain is remembered and a long chain costs its length.** The cut remembers only the start (`engine.rs:244`). A later read of a variable the cut walked past still repeats up to 256 steps until some walk reaches a stored deep start.

Fix: say that a walk which resolves stores every variable on it, and a walk cut at 256 stores only its start.
