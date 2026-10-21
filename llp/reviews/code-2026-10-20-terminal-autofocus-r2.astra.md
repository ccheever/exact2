# Code review: the terminal's autofocus taken once (QUEUE), round 2 of 2, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `eb75ec69c6`.
- **Method:** one brief (sha256 `6491d2e4a92efdb4628eb48eedec8da4cac360b8207ab21d42285c5d34003166`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (a small host fix, the lead's 21:00Z task). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (no findings).
- **Disposition:** none needed.

**Verdict: APPROVE. No BLOCKING or SHOULD-FIX findings.**

Both round 1 items are addressed: [host.rs:198](host/terminal/src/host.rs:198) documents deferred eligibility, and [screen.rs:1021](host/terminal/tests/it/screen.rs:1021) checks that arrival while a button holds focus is spent before a subsequent blur.

Static review found no introduced regression in boot autofocus, dialog/popover opening and reopening, saved-focus restoration, newly mounted routes, or keyed rerenders. Hidden/disabled deferral follows Linux’s existing approach; this remains an approximation of [HTML’s candidate processing](https://html.spec.whatwg.org/multipage/interaction.html#the-autofocus-attribute).

At [host.rs:786](host/terminal/src/host.rs:786), selection follows document order, independently of HashSet iteration. Stable keys preserve consumption across rerenders; generation-checked pruning removes destroyed allocations. Entry count stays bounded by live nodes, although capacity retains its historical peak.

The test’s typing assertions exercise the claimed behavior. Rendering and frame counters are synchronous, with no timing-dependent waits.

`cargo test -p exact-terminal --test it` could not start: the read-only sandbox denied access to `target/debug/.cargo-build-lock`. Working tree unchanged.
