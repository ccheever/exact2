# `t()` interpolation ignores the VM's string cap, and the strings base-table lookup is case-sensitive

**Status:** Fixed: interpolation checks expanded bytes before allocation and reports the VM string-limit trap; base locales resolve without case; plan and Contract regression tests pass.
**Systems:** Plan (`plan/src/strings.rs`), runner stdlib (`runner/src/stdlib.rs`, `runner/src/vm.rs`), Contract CLI (`contract/cli/src/strings.rs`)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1060

**Unbounded fill.** `T` fills its template with `push_str` and no bound (`plan/src/strings.rs:20`, reached from `runner/src/stdlib.rs:97-107`). The VM accepts the result unchecked (`runner/src/vm.rs:655`), whereas concatenation checks `MAX_STRING` (`vm.rs:607`).
- Two placeholders filled with a 40 MiB value give an 80 MiB string, past the 64 MiB cap.
- A translation with thousands of `{name}` repeats multiplies any permitted value.

**Case-sensitive base (P3).** The duplicate check compares locale names without case (`contract/cli/src/strings.rs:79-81`), but the base lookup `tables.get(&base)` (`:97`) is exact. So `strings.base = "pt-BR"` with a file `pt-br.json` fails `strings-base-missing`, although the runtime's `resolve_locale` matches without case (`plan/src/strings.rs:81-97`).

**Fix:**
- Compute the expanded length with checked arithmetic and refuse past `MAX_STRING` before allocating.
- Find the base table case-insensitively.

`plan/src/strings.rs` resolves borrowed pieces and sums their lengths with checked
arithmetic before allocating the output. `runner/src/stdlib.rs` passes
`MAX_STRING`; `runner/src/vm.rs` reports `StringTooLong` at the call. Tests cover
the exact boundary, UTF-8 bytes, literals, missing names, empty replacements and
1,025 repeats of a 64 KiB replacement. Before the fix the latter allocated the
oversized result and only the subsequent slot assignment refused it; now the VM
refuses interpolation itself.

`contract/cli/src/strings.rs` finds the base without case and retains the table's
actual spelling for typing and lowering. `contract/cli/tests/it/strings.rs`
reproduced `strings-base-missing` for `pt-BR` / `pt-br.json`, and now verifies
base-first baking, locale switching and missing-key fallback.

Required verification passed: root build, tests (1,726 passed; 0 failed; 8 ignored),
Clippy with warnings denied, formatting, staged caps, and boot.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Astra max, Opus 5.5 max. Verification: confirmed by reading.
