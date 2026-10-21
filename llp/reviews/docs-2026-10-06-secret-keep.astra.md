# Round 1
One inaccuracy in HEAD `a915a2410`:

- [docs/contract-for-agents.md:581](/Users/admin/projects/exact2-wt-sec/docs/contract-for-agents.md:581): “A drive keeps it only in a named `--storage` store” is too absolute. Apple agent mode explicitly permits `EXACT_STORE=real`, which uses the Keychain even without `--storage` ([store.rs:39](/Users/admin/projects/exact2-wt-sec/host/apple/src/store.rs:39), [LLP 1018 D6:261](/Users/admin/projects/exact2-wt-sec/llp/1018-durable-client-state.rfc.md:261)). Qualify this as the **default** behavior.

No other technical inaccuracies found. The grants, string/null and void return types, answer/awaited-parse usage, Apple/web backends, and ordinary Linux launch behavior match the code. Native background work has no answer’s secret store.

The read-only JS API probe passed. No files modified.
# Disposition

Fixed: a drive keeps it in a named --storage store by default; EXACT_STORE=real gives an Apple drive the Keychain.
