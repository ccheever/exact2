# Code review: LLP 1121 stage 3, commit 5, 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `d66f62b1feef0c8d662bdf5a9942b88acb9df12cc6714595164931d60f2edb9e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: two high, six medium, one low; Grok: one high, two medium, two low.
- **Disposition:** fixed in round 1 (bytes read once, the index before HEAD, the baseline before the first write, reply-order reads, proposals listed and announced); .contract files are watched when a server writes them

---

Found **2 high, 6 medium, and 1 low** severity issues.

1. **High — Foreign bytes can enter the commit after validation.** [scripts/tamber-commit.mjs:102](/tmp/rv-s3c5/scripts/tamber-commit.mjs:102)  
   The guard hashes the working file at line 86, but `hash-object` later reads it again. An editor save—or an in-flight server write—between those reads is committed without matching `f.last`. The synchronous Git calls also prevent watcher callbacks from intervening. **Fix:** retain the validated bytes and hash those through stdin; serialize commits with server writes and recheck before moving HEAD.

2. **High — An index failure leaves HEAD advanced despite an error response.** [scripts/tamber-commit.mjs:113](/tmp/rv-s3c5/scripts/tamber-commit.mjs:113)  
   `update-ref` succeeds before the real index is updated. An existing `index.lock` therefore produces an error after creating the commit, leaving old index entries staged against the new HEAD. Multiple files can leave a partially updated index. The retained session then refuses retries because HEAD moved. **Fix:** acquire the real-index lock and prepare all index changes before advancing HEAD; install them together and handle any subsequent failure as an already-created commit.

3. **Medium — A watcher reply can permanently classify the server’s own write as foreign.** [scripts/tamber-server.mjs:108](/tmp/rv-s3c5/scripts/tamber-server.mjs:108)  
   When an older watcher `hello` reply and a transaction reply arrive in one stdout chunk, the transaction’s synchronous callback publishes its new digests first. The watcher’s `await` continuation subsequently publishes the older snapshot, setting `foreign: true`. That clean session becomes uncommittable. **Fix:** process snapshot publication and write bookkeeping in response order, or discard superseded watcher snapshots.

4. **Medium — Conflicts can blame the previous client for a hand edit.** [scripts/tamber-server.mjs:342](/tmp/rv-s3c5/scripts/tamber-server.mjs:342)  
   After an agent writes, a hand edit followed immediately by undo returns the agent’s name until the watcher refreshes `lastBy`. The conflict already contains the changed file’s digest, but attribution ignores it. The new test waits for the watcher, masking this case. **Fix:** reconcile the conflict snapshot with the known digests before assigning `by`, while preserving response ordering.

5. **Medium — The Git baseline is captured after the first write.** [scripts/tamber-server.mjs:333](/tmp/rv-s3c5/scripts/tamber-server.mjs:333)  
   `session.wrote()` samples HEAD and the index only after Rust has changed the file. A concurrent commit affecting another file can become the recorded baseline; an initially staged entry cleared during the write can escape the staged-baseline guard. The returned `before` digest protects the content comparison, but not these Git facts. **Fix:** capture HEAD and index entries before dispatching the first write and retain that snapshot through completion.

6. **Medium — Mode-only dirtiness bypasses the guard.** [scripts/tamber-commit.mjs:54](/tmp/rv-s3c5/scripts/tamber-commit.mjs:54)  
   Working-tree cleanliness is determined solely by content hashes. A tracked file with an unstaged executable-bit change passes, and committing unconditionally selects HEAD’s mode. This does not enforce D9’s dirty-file refusal. **Fix:** capture and compare file type and Git mode before writing, alongside the content digest.

7. **Medium — Reviewers cannot identify partial-accept indices.** [scripts/tamber-server.mjs:361](/tmp/rv-s3c5/scripts/tamber-server.mjs:361)  
   The proposal event exposes an aggregate diff, sorted/deduplicated changed tokens, and an operation count. `accept.items`, however, indexes the original operations, which reviewers never receive. Reversing two independent operations produces the same review payload but changes what `items: [0]` accepts. **Fix:** publish ordered review items with stable indices and their corresponding operations.

8. **Medium — Held proposals cannot be recovered after missed events.** [scripts/tamber-server.mjs:360](/tmp/rv-s3c5/scripts/tamber-server.mjs:360)  
   Proposals remain in memory, but neither subscription snapshots nor missed-event responses include them, and there is no listing method. Once their announcements leave the 256-event buffer, a reconnecting tool cannot discover still-valid proposals. **Fix:** expose the current unexpired proposals through a snapshot/list operation and reconnect recovery.

9. **Low — Dropping a proposal loses attribution.** [scripts/tamber-server.mjs:314](/tmp/rv-s3c5/scripts/tamber-server.mjs:314)  
   `drop` requires no client and emits `proposal-dropped` without one, contrary to D10’s attribution requirement. **Fix:** require and publish the dropping client.

Per-request `client` is appropriate for the shared HTTP process. Allowing any token holder to accept/drop matches the stated authorization model. I found no shell-injection or push path; the `update-ref` old-value guard itself is present.

**Validation:** caps passed. In-memory runs of the actual JavaScript with mocked I/O reproduced findings 1–4 and checked proposal recovery, multi-file bases, and expiry. Integration tests were not run under the read-only constraint. The added tests also omit the staged-baseline T1 case. No files or Git state were changed.
