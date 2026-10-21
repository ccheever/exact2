# Code review: tamber scale_css held as ratios, not wall clocks (the lead's 22:29Z load flake), and the walks they found quadratic, round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `fb8a3dc7b9`.
- **Method:** one brief (sha256 `1c0fb5dbfe1e77c5f79c936b7638d93f0df0aa213ead42018bc2b5f95f995d6b`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocking or should-fix findings
- **Disposition:** none needed

No blocking or should-fix findings.

**(a) The memo matches the old per-token walk.** `kind_of` / `inferred_kind` / `named_kind` / `measures` read only the token text, `names`, `dials`, and `named_length`. They never read `refused` or `kinds`. The only path-dependent branch is the `on[i]` re-entry at `tamber/src/check.rs:768`, which returns `Color` and increments `cycles`. A result is stored only when `cycles` did not change (`check.rs:775`), so a memoized kind did not observe `on`. Ancestors of a cycle are not stored, and a later top-level walk recomputes them. `on[i]` is cleared before `known` is set, and a memo hit returns without setting `on`, so `on` is all false after every top-level `kind_of` and after each `shadow::looks` callback. Sharing `walk` at `check.rs:732` is safe.

**(b) The shadcn change is behavior-preserving.** `kind` is passed into `other_kind` only for `Fate::NotColour` (`tamber/src/import/shadcn.rs:288`). The `if let` at line 293 requires that fate, and the only match arms that read `kind` are the `NotColour` arms at lines 317–346. Other fates now get `None`, and that value is not used.

**(c) The ratio test is sound for a quadratic that dominates on this thread.** `T(4n)/T(n)` is 16 for a pure square and the cap is 8 once the small side is over 50 ms (`scale_css.rs:30`). Below that, the 400 ms cap still rejects a pure square whose small side is above about 25 ms. The 40-vs-160 chain stays under the 256-link cut and is what exposed the checker. These paths do not spawn threads, so `CLOCK_THREAD_CPUTIME_ID` counts the walk and does not hide it behind other cores; time spent blocked (an allocator lock) is invisible, which is the point of the clock. The `not(unix)` fallback (`scale_css.rs:55`) is only `OnceLock` and `Instant::elapsed`, so it typechecks on wasm32 and Windows. It is wall time, so the load flake remains there.

**(d) Every previous output assertion is still at full size** (200_000 depth, 20_000 wide and copied, 6_000 labels, 80×250 aliases, 300 and 5_000 chains, 5_000 and 2_000 Tailwind, 22_001 keys, 20_000 lengths, 3_000 marks, 256/257 shadows, the 4 KiB value, 30_000 colours). The new depth sample is timing-only; the 300-link and 5_000-link asserts still cover resolution. The `seen` comment at `tamber/src/import/engine.rs:207` matches the cut: only the walk’s start is stored as past `LINKS`.
DONE
