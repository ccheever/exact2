# Code review: the design checker's bound (rt1's PRIORITY finding), round 1 of 3, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `bbdda3f86a`.
- **Method:** one brief (sha256 `04b7242e627b089e6b63a37fb3b51dbc640786f62df8ab925d596c0e5520a086`), blind to the other reviewer; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Reviewed at bbdda3f86a, before the rebase onto exact47's main; that commit is a06ea011dc there.

HEAD `bbdda3f86a` still has blockers.

1. **BLOCKER — The residual cap runs after quadratic work.**  
   [measure.rs:378](tamber/src/measure.rs:378) checks only the completed function result. Meanwhile, [sum at line 485](tamber/src/measure.rs:485) repeatedly calls `plus`, whose [insertion searches the entire growing vector](tamber/src/residual.rs:87).

   A concrete construction: define 550 lengths, each containing 128 distinct `min(base.gap, Kpx)` terms, with disjoint `K` ranges. Each definition has 256 atoms and at most 3,203 source bytes. Then `calc(a0 + … + a549)` is 3,743 bytes but accumulates **140,800 atoms before checking**, requiring roughly **2.48 billion term comparisons**. These are construction counts, not measured timings. Bound intermediate sums and accumulation before searches/copies; checking the final function result is too late.

2. **BLOCKER — Reference traversal remains exponential on small cycles and recursive on long chains.**  
   [check.rs:821](tamber/src/check.rs:821) deliberately does not memoize kinds reached through cycles. Combined with [following every referenced operand](tamber/src/check.rs:888), this makes:
   ```
   t0 = calc(t1 + t1)
   …
   t29 = calc(t30 + t30)
   t30 = t0
   ```
   take exponential work before folding can report `tamber-cycle`. All 31 values satisfy both text caps. Changing `t30` from `0` to `t0` makes this reachable through one edit to a previously valid design.

   Separately, [value traversal](tamber/src/check.rs:990) still recursively follows arbitrarily long acyclic references, with a linear `stack.contains` at every level. All-px chains avoid the residual cap but can still exhaust the stack. This needs bounded-work graph traversal, not a token-count restriction.

3. **BLOCKER — Platform-colour parsing can recurse past 32 despite passing every pre-scan.**  
   [css.rs:53](tamber/src/css.rs:53) toggles quoting at every `"`, while platform arguments use escape-aware `lex::split_top`, then [recursively call a fresh `parse`](tamber/src/css.rs:304).

   Repeatedly wrap `red` using `-exact-platform-color("x\"y", {v} ")`. A scanner-model reproduction with 60 wrappers is **1,983 bytes**; every pre-scan reports depth at most **30**, yet parsing descends through **60 platform fallbacks** before reaching the terminal syntax error. Malformed input must also respect the recursion limit. Track depth through the reader and fallback calls rather than relying solely on this pre-scan.

4. **BLOCKER — Numeric dials and ranges lose limit diagnostics and echo oversized input.**  
   [check.rs:1353](tamber/src/check.rs:1353) converts `css::parse` errors to `None`. A number dial containing 4,097 zeros therefore produces `tamber-dial` and echoes the entire value at [line 570](tamber/src/check.rs:570), whereas `SetDial` returns the short `tamber-value-size` diagnostic. Range endpoints have the same problem at [line 608](tamber/src/check.rs:608).

   On-disk metadata also lacks the edit validator’s bound: `scheme=<oversized word>` is interpolated in full at [line 314](tamber/src/check.rs:314). Preserve limit errors and validate these fields through the shared path before formatting diagnostics.

5. **BLOCKER — Some design values never reach the guard, or are classified before it.**  
   [lib.rs:189](tamber/src/lib.rs:189) aliases and checks icons without a value preflight. [icon.rs:37](tamber/src/icon.rs:37) accepts `icon(sf "<5000 lowercase letters>")`; an invalid oversized symbol instead gets echoed at [line 101](tamber/src/icon.rs:101). Icon parameters enter this same path.

   The token path also calls [typeface inference before the guard](tamber/src/check.rs:830). An oversized untyped multi-shadow value can reach [shadow classification](tamber/src/check.rs:775), become refused as `tamber-kind`, and never receive its limit diagnostic. A shared design preflight must precede aliasing, inference and specialized readers.

6. **SHOULD — Reimport bypasses the new importer refusal contract.**  
   [tamber_import.rs:172](contract/cli/src/tamber_import.rs:172) routes recognized exports directly to `reimport_against`, which has no source-value preflight. Change an exported `fontFamily` value in both schemes to a 5,000-character string: the eventual edit rejection is converted into [a `Planned::Report`](tamber/src/export_batch.rs:275), and [reimport returns `Ok(Import)`](tamber/src/export.rs:1423). It should return the structured limit refusal consistently with ordinary DTCG import.

7. **SHOULD — A long CSS property name hides an oversized source value.**  
   [import/mod.rs:578](tamber/src/import/mod.rs:578) chooses the name-limit report when both name and value exceed their bounds, discarding the value. [source_read](tamber/src/import/mod.rs:830) only promotes reports marked `LONG_VALUE`. Thus a property with a name over 256 bytes and a value over 4,096 bytes remains an ordinary `Unmapped` report instead of refusing with `tamber-value-size`. Check the source value before choosing the reporting reason.

8. **SHOULD — The final importer guard mistakes report text for a source value.**  
   [Astryx combines different light/dark values](tamber/src/import/astryx.rs:170) into `"{light} | {dark}"`. [import::bounded](tamber/src/import/mod.rs:770) then checks that display string as one value. An unsupported variable with two individually legal 2,100-byte values now refuses the entire import because its report is 4,203 bytes. Validate actual source values and generated operations, not their combined presentation.

9. **SHOULD — The single-value residual test never executes its assertion.**  
   [limits.rs:59](tamber/tests/it/limits.rs:59) constructs twelve tripling levels: **7,174,448 bytes**. Consequently, the guard at line 63 is always false. The later chain subcase runs, but the claimed single-value case is untested.

   Add executable cases for intermediate sums, cyclic inference, escaped platform fallbacks, icons, numeric dials/ranges, and importer reporting. The [contract parity test](contract/cli/tests/it/tamber_limits.rs:33) proves equality for one residual refusal; the byte test does not compare build diagnostics.

The 51 tracked `.tamber` files show no byte/depth collision by static scan. Pencil’s existing 1,024-byte reporting rule remains in place below the new cap; HEAD contains no `import_pen.rs` test edits.

Rust tests were not run: the sandbox is read-only and this checkout has no build artifacts. Scanner and construction probes ran in memory. No files changed.
