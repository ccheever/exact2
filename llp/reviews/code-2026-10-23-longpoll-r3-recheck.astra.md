# Code review: long-poll streams (LLP 1016.000 D8), round 3's re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `1bdeb192fb`.
- **Method:** one brief (sha256 `feffed37b58c1c866c2eda820c220c8e90c37279830af26539aa7eb7f37ddc37`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- [background.rs:55](runner/src/runner/background.rs:55): Background requests bypass `enqueue`. Returning `Request::continuation(BACKGROUND).long_poll(...)` from `background()` or `background_landed()` still executes continuation work on Apple/Linux and the renderer before validation. This path needs the same refusal check.
- [commit.rs:820](runner/src/runner/commit.rs:820), [abi.rs:263](host/apple/src/abi.rs:263): Runner-generated refusals do not wake Apple’s pump. With no dispatched request or other completion, an idle app can leave the refusal pending indefinitely.
- [admission.rs:127](runner/src/runner/admission.rs:127), [commit.rs:1116](runner/src/runner/commit.rs:1116): The new refusal passes `ordered=false`, which also leaves `pending.refused=false`. If parsing returns a valid fallback that traps in a derive, rollback restores the ticket after its refusal was consumed; cleanup is skipped and the resource remains pending permanently.

**Should-fix:** None additional.  
**Nits:** None.

`take_admission_refusal` leaves auth answers untouched. No new rejection of requests without `long_poll` found.

Both permitted test commands stopped before assertions because the read-only sandbox denied Cargo writes. No files changed.
