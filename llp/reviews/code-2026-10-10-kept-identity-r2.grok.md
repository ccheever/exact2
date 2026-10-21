# Code review: a kept answer's identity read from a fallback (LLP 1027.005 D3) round 2 (75eae7437), 2026-10-10 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `7acae572e2d1d8c20e8ecd716337d741b6590da4ca3775ddb68fc75d64c6a062`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** The shared finding is taken in 019b88ead: a refused kept seed equal to the compiled fallback no longer swaps in for it, so the fallback keeps its compiled mark; a test, direct and through a derive, fails without the fix. Grok's NIT (deps.rs's misplaced doc comment) is taken.

---

The round-1 fixes hold for the case they target. One hole remains, so this is not ready.

## BLOCKER

`runner/src/runner/settlement.rs:717` — a refused kept value that equals the compiled fallback is published without the compiled mark, so a dependent settled later in the pass treats that fallback as a known identity and admits its kept answer.

`identity_unknown` (`runner/src/runner/kept.rs:172`) decides from `placeholder || value.is_compiled()`. The compiled-fallback arm builds a compiled `Held`, then this pre-existing reuse replaces it with the previous object when the two are equivalent:

```716:718:runner/src/runner/settlement.rs
                        match &states[i] {
                            Some(s) if Held::equivalent(&s.value, &value) => s.value.clone(),
                            _ => value,
```

`kept_seed` is already cleared, and the compiled branch leaves `placeholder` false. The published state is neither a kept seed, nor a placeholder, nor compiled. A later resource whose identifying argument reads it (directly, through a derive, or via `pending`/`failed`, which share this flag) does not see a fallback.

That is the empty-id collision. A store-reading account whose kept `""` is not admitted — its own identity reads a fallback, or its identifying arguments no longer match — still shows `""`, and `feeds` kept for `""` by a signed-out launch is admitted and reused. The no-row case is fine: `states[account]` is `None`, nothing is swapped, and the compiled mark stays. The new tests use that case (`"nobody"` versus `"feeds of somebody else"`), so they do not catch this.

Keep the compiled `Held` when the replacement is compiled and the previous object is not. Add a test where the dependency's kept value equals its compiled fallback and the seed is refused (directly and through a derive).

## NIT

`runner/src/instance/deps.rs:269` — the sentence "The inputs `reads` names, in bit order" was the docs for `inputs` and now sits on `reads_of`.

## What holds

The other round-1 checks match the code and LLP 1027.005 D3 (2026-10-10):

- Only the identifying prefix is scanned, per argument, then through derives. A `with`-only read does not refuse. The derive walk is transitive and cycle-safe.
- Unknown means `store_readers[r]` and the dependency's settled state is a placeholder or a compiled value. An admitted kept seed, a carried answer, a checkpoint answer, and a non-store value stay known, including when they are settled earlier in the pass. Argument evaluation waits until a real read has settled, so the check sees that published state.
- `pending` / `failed` are in the match, and the scan also marks the resource itself, so a status read of a real fallback refuses. They use the same flags, so they miss the same swapped `Held`.
- Refusal does not publish the tentative `kept_seed` clear. A later gate refusal restores `resources` from the commit checkpoint. Before readiness, an unknown seed takes the fallback instead of reuse. `data_ready` then asks. A seed that was actually admitted still stands across an async answer, with the mark consumed.
- The journal line is the kept seed's identifying arguments, truncated to 200 characters with `…`, and it is written only after a successful boot settle.

NOT READY.
