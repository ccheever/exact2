# Code review: stage 13 B, the pen importer, round 3's final budget check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `74fc5dfc38`.
- **Method:** one brief (sha256 `22a34dbeb4ea8f6f41bf5b509a7ec061c1f160d84675889f00423c152fc900c6`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- [import/kinds.rs:301](tamber/src/import/kinds.rs:301): **Charging probe text does not bound the checker.** Pixel-bound numeric strings accept CSS. Nineteen variables, each bound to `gap`, starting at `space.md` and repeatedly applying `clamp(previous, previous, previous)`, fit in roughly 2.3 KB on the wire but expand to `3^18` owned residual leaves at [residual.rs:133](tamber/src/residual.rs:133), uncharged. Independently, [css.rs:225](tamber/src/css.rs:225) retains enclosing function substrings: an approximately 809 KB nested-`calc` payload requests over 1.2 GB.

- [landing.rs:244](tamber/src/import/landing.rs:244): **Budget exhaustion does not stop folding.** The failed charge is discarded; every diagnostic’s probes continue until the sentinel at line 473. The successful-fold return at line 350 bypasses that sentinel entirely. Also, charging token count misses the checker’s dial-by-token refolding at [check.rs:1119](tamber/src/check.rs:1119).

- [landing.rs:415](tamber/src/import/landing.rs:415): **Refusal strings amplify without charging.** A valid existing calculation summing 10,000 imported numbers and dividing by the first becomes invalid when that first number becomes zero. A 440,080-byte wire request then copies the approximately 120 KB diagnostic into 10,000 candidate reasons: **over 1.2 GB**, before subsequent report copies.

- [landing.rs:395](tamber/src/import/landing.rs:395): **Attribution remains quadratic.** Each reached name scans `tokens`, then `candidates`; the 32-candidate cutoff happens afterward. The example above performs about 50 million token comparisons alone. Further uncharged design-by-import scans remain at [landing.rs:458](tamber/src/import/landing.rs:458) and [import/mod.rs:150](tamber/src/import/mod.rs:150). The table’s reads/ops bounds do not cover these.

- [pen.rs:633](tamber/src/import/dtcg/pen.rs:633): **Alias-type comparison multiplies unbounded strings.** Two identical 256 KiB unknown types and 25,000 `$b` entries fit in a 999,436-byte request. They cause approximately **6.55 GB of uncharged comparisons**, despite already being marked refused.

- [export_dials.rs:347](tamber/src/export_dials.rs:347): **Marks are not bounded by the existing design alone.** Every prior reference rescans and lowercases the imported value through [names:464](tamber/src/export_dials.rs:464). A long accepted RGB literal therefore multiplies attacker-controlled bytes by existing references, after the import budget has ended at [tamber_import.rs:205](contract/cli/src/tamber_import.rs:205).

- [landing.rs:412](tamber/src/import/landing.rs:412): **The cutoff changes decisions.** With 33 imported numbers in a sum divided by the first, changing only the denominator to zero makes all 33 candidates refused. Previous narrowing rejects the denominator update and retains the other 32 harmless updates. This changes both mapped tokens and reports while ample budget remains.

**Should-fix**

- [import_pen.rs:959](tamber/tests/it/import_pen.rs:959): The long-string fixture has no `fontFamily` reader, so it never exercises other-kind resolution/probes. At [line 1028](tamber/tests/it/import_pen.rs:1028), results are discarded; early refusal can satisfy the test without reaching its intended pass. Add assertions that each targeted phase is reached, including CLI marks.

**Nits:** None. No additional wrong-kind route confirmed.

Findings are from source inspection and byte/operation accounting. Both permitted test commands failed before execution because Cargo could not create temporary files in the read-only sandbox. No files edited.
