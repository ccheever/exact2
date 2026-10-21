# Code review: LLP 1120 §5's measurement counts and connection log, 2026-10-12 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `554ad2111f094f6c4d6108327c56d98a7d14e7b579a402119e26e320b6f57f91`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Every finding taken in the second commit; Grok's two NITs in the landing commit.

---

- **MAJOR — [metrics_log.rs:22](/Users/admin/x2p2/apps/exact2-measure/vendor/ibex/crates/ibex2/src/transport/metrics_log.rs:22): Invalid JSON escaping.** `{:?}` produces Rust literals, including escapes such as `\u{200d}` and `\0`, which JSON rejects. The async path retains the original URL, so an unescaped Unicode URL can produce an unreadable record. Use the existing `serde_json` dependency.

- **MINOR — [executor_outcome.rs:76](/Users/admin/x2p2/apps/exact2-measure/host/apple/src/executor_outcome.rs:76): Overflow can silently change the count.** With overflow checks disabled, as in `release` and `host-dev`, the pinned compiler evaluates `"18446744073709551623"` to `7`. I reproduced this during const evaluation. Use checked multiplication and addition with an explicit overflow failure. Empty, zero, and nondigit inputs are correctly rejected.

- **MINOR — [metrics_log.rs:39](/Users/admin/x2p2/apps/exact2-measure/vendor/ibex/crates/ibex2/src/transport/metrics_log.rs:39): Whole-record appends are not synchronized.** Append mode prevents competing file-offset updates, but a short write makes `write_all` issue multiple writes; another callback can append between them and corrupt the JSON lines. Serialize complete records with a shared lock. This limitation is explicit in [Rust’s append documentation](https://doc.rust-lang.org/std/fs/struct.OpenOptions.html#method.append).

- **MINOR — [darwin_http.mm:229](/Users/admin/x2p2/apps/exact2-measure/vendor/ibex/crates/ibex2/src/transport/darwin_http.mm:229): Disabled logging still prepares strings.** Endpoint formatting and the FFI wrapper’s owned string copies happen before checking the environment variable. Thus ordinary blocking requests incur allocations beyond the documented single lookup. Gate before constructing those strings.

Defaults remain 6 and 32. With logging unset, no file is opened and no URL is persisted. Nil metrics and addresses are handled safely; reading the callback’s `task` avoids dependence on the cleared `exchange.task`. Synchronous file I/O is reasonable for opt-in measurement, but stalls the delegate queue and contributes to measured latency.

Validation: Objective-C syntax check, const-evaluation probes, and three focused tests from the existing binary passed where applicable. No full rebuild or file-write stress test was performed; no files changed.

**NOT READY**
