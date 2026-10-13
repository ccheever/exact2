# Code review: a kept answer's identity read from a fallback (LLP 1027.005 D3) (63db2ec5e), 2026-10-10 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `42309550a8cf33cd1b93cc86552a0f970c71fd02d91599d0494476adecc84f28`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Both MAJORs taken in 75eae7437: only the identifying arguments are walked, never the `with` context; unknown means a store-reading resource on its compiled answer or placeholder, so kept, carried and checkpoint answers and non-store values stay known. The journal's shortened identity ends in an ellipsis.

---

- **MAJOR — [runner/src/runner/kept.rs:155](/Users/admin/x2p2/apps/exact2-kept/runner/src/runner/kept.rs:155): Context-only dependencies wrongly invalidate identity.** `deps.resource_args[i]` includes trailing `with` arguments. Consequently, `feed("alice") with policy` loses its matching kept answer when `policy` is an unseeded deferred resource, although `"alice"` is fully known. This contradicts D2/D3’s separation of identity and context. Scan only the identifying argument prefix, following its derives.

- **MAJOR — [runner/src/runner/kept.rs:160](/Users/admin/x2p2/apps/exact2-kept/runner/src/runner/kept.rs:160): `stale` does not identify only unresolved store fallbacks.** Settlement marks ordinary non-store compiled resources stale too. A reader keyed by a fixed, compiled `config()` therefore loses its valid seed: `config` is stale and cannot have a kept seed. Compatible carried readers can also be stale without being fallbacks. The predicate needs to distinguish these cases from the store-reading compiled answer or placeholder specified by the amendment; otherwise valid cached content disappears until a fresh answer succeeds.

All 25 kept-answer tests passed using the existing test binary. No rebuild or file changes; the added tests do not cover these cases.

NOT READY
