# Code review: a disabled pressable its author styles is not dimmed (LLP 1115.000 D6), 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `4b77f6bf525fd774b62d42595b1ad2a278ac52f15f3538057e95331b54d3f2c5`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Both reviewers' MAJOR (bound names counted as reads) and Grok's (the border shorthands) taken in round 2's code.

---

**MAJOR — Shadowed identifiers incorrectly suppress the disabled dim.** [contract/lower/src/disabled_look.rs:51](/Users/admin/x2p2/apps/exact2-comp/contract/lower/src/disabled_look.rs:51)

With `state busy = true`, this valid custom button receives `disabledStyled`:

```contract
button appearance="none" disabled=busy color=join(map(["red"], busy => busy), "")
```

Its colour never reads the outer `busy`. However, `idents` includes the callback’s bound identifier, so the comparison falsely finds shared state. Renaming the callback parameter to `ink` removes the marker; compiler dumps confirmed the extra binding. Consequently, an unrelated local-variable spelling determines whether a disabled button retains its disabled cue.

Collect free references, respecting arrow, `let`, and `match` binders, and add a regression covering parameter renaming.

No additional findings in host coverage, selector specificity, Mac opacity ordering, or schema propagation. Five targeted tests passed using existing binaries. Browser/macOS suites were not rerun. Working tree unchanged.

NOT READY
