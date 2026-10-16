# Code review: LLP 1122 T1 commit 1 (Tamber server: history, revert, import, the tool's files), round 3, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `165174983` (the server files of `9479c6464..165174983`).
- **Method:** one brief (sha256 `517bc500333f181635a07a6521a631557cd3edb059c594a75e2a6c71b72e6c88`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer. The last round: its findings are fixed without another.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 3 medium, 1 low).
- **Disposition:** all taken: a revert's untouched files ride its propose and accept as `requires`, which the core checks on the snapshot it writes; an axis value's operation is its own selection item; a client's `review.revert` is refused; the `--via` body cap had landed in the skeleton's round 1; a second DTCG themes or resolver document is refused, named.

---

1. **High — Guard files remain outside the accepting transaction.** [scripts/tamber-server.mjs:527](/tmp/rv-dt-1r3/scripts/tamber-server.mjs:527)  
   `guardsMoved()` checks untouched files through a separate `hello`, then acceptance sends only `p.base`. If a commit changed A and B, with B already restored, another client can change B between that `hello` and the transaction. Acceptance succeeds on A without detecting B’s conflict. The core also checks digests only for operation targets. **Fix:** pass every planning precondition into the transaction and enforce them together, including missing files and the final pre-write check. The added test changes B before acceptance and misses this interleaving.

2. **Medium — Partial axis selections still regain rejected operations.** [scripts/tamber-server.mjs:811](/tmp/rv-dt-1r3/scripts/tamber-server.mjs:811)  
   `revertKey` gives every value operation on an axis the same identity. For a commit reducing `black | dim | navy` to `black`, select only the inverse that restores `dim`. After an unrelated edit makes that proposal stale, replanning includes both `dim` and `navy`. An in-memory probe confirmed this. **Fix:** preserve distinct operation identities, including the axis value, or expose each axis change as one indivisible selection. Test multiple operations on the same axis.

3. **Medium — Client-supplied review metadata can throw during acceptance.** [scripts/tamber-server.mjs:587](/tmp/rv-dt-1r3/scripts/tamber-server.mjs:587)  
   The review validator accepts `{revert: {commit: "<valid SHA>", only: {}}}`. When a stale acceptance replans it, `only.includes(...)` throws through the HTTP handler instead of returning an error; the proposal has already been removed. The probe confirmed both validation acceptance and the exception. **Fix:** keep executable revert metadata internal, validate its complete shape, and return a typed error for malformed requests.

4. **Medium — Forwarding bypasses the request-body limit.** [scripts/tamber-server.mjs:763](/tmp/rv-dt-1r3/scripts/tamber-server.mjs:763)  
   In `--via` mode, `forward()` buffers the entire body before contacting the upstream server, without checking its size or token. A local request can therefore consume arbitrary memory before the upstream 1 MiB limit applies. The probe forwarded 1,048,577 bytes unchanged. **Fix:** enforce the same bounded body reader at the forwarding entry point and return 413 before forwarding.

5. **Low — Additional DTCG theme or resolver roots are still silently ignored.** [tamber/src/import/dtcg.rs:409](/tmp/rv-dt-1r3/tamber/src/import/dtcg.rs:409)  
   Sorting sources fixes ordering, but `.find()` still selects only the first inline `$themes` document or resolver. Uploading two complete Tokens Studio exports imports the alphabetically first one; the second contributes neither tokens nor an unmapped explanation. This part of round 2 remains unresolved. **Fix:** reject ambiguous multiple roots or require an explicit root selection, with a regression test covering both documents.

Validation used source inspection and read-only in-memory probes. Cargo tests were not run; no files changed.

**Verdict: Request changes.**
