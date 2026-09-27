# `t()` interpolation ignores the VM's string cap, and the strings base-table lookup is case-sensitive

**Status:** Open
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

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Astra max, Opus 5.5 max. Verification: confirmed by reading.
