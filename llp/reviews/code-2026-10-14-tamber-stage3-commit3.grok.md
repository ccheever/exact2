# Code review: LLP 1121 stage 3, commit 3, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `da1d6887f9caac15f81e94ac87fca8795f260d511fa8d26dc410c81e1c352baf`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: five high, ten medium, three low; Grok: three high, ten medium, six low.
- **Disposition:** fixed in round 1 (confined paths, a durable journal with digests, one snapshot per request, run-qualified inverse ids, strict requests); D13's closure through the compiler named as stage 4/6

---

The session core commits a transaction as a set and refuses a second server, a stale digest, and a client-sent restore. The journal and inverse-id handling do not hold the crash and restart guarantees the commit claims.

## High

**High — Journal recovery renames whatever paths the journal names.** `contract/cli/src/tamber_serve.rs:149`

`recover` runs at startup, before any request. For each pair it does `self.dir.join(tmp)` and `self.dir.join(target)`, then `rename` if the source exists, then it deletes the journal. `Path::join` keeps `..` components, and an absolute path replaces the app directory (`/app` joined with `/etc/passwd` is `/etc/passwd`; `strip_prefix` of `/app/foo/../../.bashrc` is `foo/../../.bashrc`). A `.exact/tamber.journal` in the project — `.exact/` is not gitignored — or a symlink to a JSON file is enough. Starting `contract tamber serve` then moves that source onto that target: a home-directory file, a key, another project. A client `file` of `../…` is refused, because `plan` only accepts paths already in the server’s file list (`tamber/src/session.rs:96`). The journal read path has no such check. `read_to_string` also follows a symlink, so the JSON need not even sit in `.exact`.

Fix: lexically confine both sides (temp must be a direct child of `.exact`, target must be one of `self.files`), reject `..` and absolute paths, open with `O_NOFOLLOW`, and if any pair fails that check keep the journal and refuse to start.

**High — A rename error leaves a half-applied transaction, and the next commit destroys the journal.** `contract/cli/src/tamber_serve.rs:420`

Temps are written, the journal is created with `File::create` (truncate), then renames run in order. If a later `rename` fails, the function returns and the process keeps serving. The earlier files are already replaced; the journal still lists every rename. The next successful `commit` truncates that journal and writes a new one, so a restart can no longer finish the first transaction. The lock is still held, so a second process cannot recover it either. The killed-between-renames test only covers `exit` while the temp is still there.

Fix: if a journal exists at the start of `commit`, run recovery before creating another. Write the journal to a new file and rename it into place. On a rename error, finish the recorded renames or stop accepting writes until that has happened.

**High — A missing temp is treated as success and the journal is deleted.** `contract/cli/src/tamber_serve.rs:154`

After a crash between renames, a temp that was already renamed is gone, and skipping it is right. A temp that was lost without the rename (deleted under `.exact`, or not durable — see the directory `fsync` gap below) is indistinguishable: `exists` is false, that file is left on the old bytes, and `remove_file` drops the only record. The transaction is then permanently half-applied. `exists` is also false when the directory is unsearchable, so a permission error takes the same path.

Fix: record each target’s post-image digest in the journal. If the temp is gone, keep the journal and refuse to start unless the target’s bytes already have that digest.

## Medium

**Medium — The journal and the renames are never forced durable in the directory.** `contract/cli/src/tamber_serve.rs:412`

Temp contents and the journal file are `sync_all`’d. The directory entries are not. `File::create` also truncates the journal in place, so a crash while writing it leaves a partial file and startup refuses (`the journal: …`) until someone deletes it. After a power loss the journal inode and the renames can hit disk independently, which is the half-apply case above. Process kill, which is what the test uses, does not show this.

Fix: write the journal to a temp, `fsync` it, rename it over `tamber.journal`, `fsync` `.exact`, rename the design files, `fsync` the app directory, then unlink the journal and `fsync` `.exact` again. Do the same after recovery.

**Medium — `File::create` follows symlinks on fixed and predictable names.** `contract/cli/src/tamber_serve.rs:145` and `:411`

A dangling `.exact/tamber.journal` symlink fails `read_to_string`, so `recover` returns and leaves it. The next commit’s `File::create` follows it and creates the target. A symlink at `.exact/tamber-{pid}-{i}.tmp` is truncated and filled with the new design text. Rename of the design file replaces a symlink and does not write through it; the create path does.

Fix: `O_NOFOLLOW` and `O_EXCL`, and a random temp name. At startup, refuse a symlink journal instead of ignoring an unreadable one.

**Medium — Undo ids are reused after restart, so a stale undo can apply a different inverse.** `contract/cli/src/tamber_serve.rs:391`

`issued` lives only in the process, and `next` starts at 0, so the first commit after every start is `inv-1`. The id is removed only when that undo succeeds. A client that still holds `inv-1` from before the crash (the shape commit 4’s long-lived stdio driver will have) sends it after a new commit has reused `inv-1`. The digest check compares against the new inverse’s post-state, which is the current files, so the undo succeeds and restores the new transaction’s preimage.

Fix: use an id that cannot recur (random 128-bit, or a boot id plus counter). A stale id must miss. Persisting the map is a separate choice; uniqueness has to hold either way.

**Medium — Every issued inverse is kept until that one id is undone.** `contract/cli/src/tamber_serve.rs:85`

The map stores each file’s full restore lines and the post-digests. Nothing evicts old entries, and a new undo inserts another inverse. A long serve grows without a bound.

Fix: cap the count or the bytes and drop the oldest. Say so in the error when a dropped id is used.

**Medium — Request lines have no cap.** `contract/cli/src/tamber_serve.rs:51`

`BufRead::lines` reads until newline into one `String`. One line with no newline grows until the process is killed. There is also no cap on `ops`.

Fix: a maximum line length (and a maximum op count) that returns an error line and discards the rest of the line.

**Medium — `explain` only reads the token’s default value.** `contract/cli/src/tamber_serve.rs:306`

`names` is a word scan of `declared.value` only. A token named from a `when` line is omitted. In `tamber/tests/corpus/axes.tamber`, `accent.fill`’s dim line is `light-dark(accent, color-mix(...))` while the default is `accent`; a when that names some other token would not appear. `token.dials` is the checker’s dial closure and does include when-lines. `names` is a separate text scan, so it also counts a token word in the source that the folder did not resolve.

Fix: take names from the default and every `when` value, using the same name resolution as the checker, and return them per branch if commit 4 needs the chain by mode.

**Medium — `resolve` drops the residual’s fallback.** `contract/cli/src/tamber_serve.rs:284`

D13’s resolve is the stored value per tuple and scheme, and a residual with its fallback labelled as a fallback. `Folded::Host::Platform` already carries the fallback pair (`tamber/src/fold.rs:368`); the arm discards it and sets `residual: true`. A role’s light/dark fallback is computed in `fallback()` (`tamber/src/lib.rs:124`) and is not on the `Folded` value; `Folding.fallbacks` keeps only tuple 0, so a `when` that selects a different role has no per-tuple fallback on this path. Concrete colours do return `light` and `dark`, and tuple 0’s modes match the checker (`first axis slowest`), which the clone test covers.

Fix: for a platform colour, return `fallback: [light, dark]`. For a role, resolve the vocab fallback the same way `fallback()` does, per tuple.

**Medium — The digest is checked on a different read from the bytes that are written.** `contract/cli/src/tamber_serve.rs:323` and `:371`

`digests()` reads the files, then `run` reads them again and plans that second text. A hand edit or `tamber fmt` in between passes the client’s digest and is overwritten. Nothing re-reads immediately before the rename. The flock stops a second server; it does not stop another writer. `digests` also maps any read error to `""` (`tamber_serve.rs:172`), which is not the SHA-256 of an empty file.

Fix: hash the same bytes you parse. Before renaming, read again and abort the whole commit if an edited file changed, without replacing the journal.

**Medium — An inverse is invalid if any design file changed, including files it does not restore.** `contract/cli/src/tamber_serve.rs:353` and `:390`

The stored precondition is `self.digests()`, every `.tamber` file. Undo then requires all of them to match. D13’s precondition is the digest of each file the transaction edited. A later edit to `brand.tamber` blocks undo of an `app.tamber` write whose bytes are still exactly what that inverse expects.

Fix: store post-digests only for the files in the inverse, and check those.

**Medium — The design path is not confined to the app directory.** `contract/cli/src/tamber_serve.rs:101`

`design_path` honors `app.json`’s `"tamber"`. `strip_prefix` is lexical, so `"tamber": "foo/../../outside.tamber"` stays `foo/../../outside.tamber` and is the default file for every op. An absolute `"tamber"` fails `strip_prefix` and is stored as an absolute path, which `join` then uses as-is. The write still has to pass the checker, so a non-design file is refused; a design that lives outside the tree is renamed into place, and that outside path is what the journal records.

Fix: canonicalize and require the design file, and every other `.tamber` file, to sit inside the app directory. Refuse to start otherwise.

## Low

**Low — `class` does not apply D13’s build-substitution rule.** `tamber/src/session.rs:149`

Live versus rebuild is shape equality: axis names and values, dial names and kinds, token names, kinds, roles, and `scheme=fixed`. Values are excluded, and `changed_tokens` is the folded value closure, so dependents of a dial move with it. For a design the checker accepts today that matches D13: only colour tokens exist, and a duration or easing token is refused with `tamber-stage` (`tamber/src/check.rs:484`). A value edit of a kind that is substituted at build, or of a dial that folds into one, would still be `live` once stage 6 allows those kinds.

Fix: when those kinds exist, force `rebuild` if the closure of `changed` includes a duration or easing token or a dial or param folded into one.

**Low — `changed.dedup()` only drops adjacent copies.** `tamber/src/session.rs:111`

Two files can both change `accent` with another name between the two entries. The vector keeps both. Sort, then dedup.

**Low — `add-token` meta that is not a string is discarded.** `contract/cli/src/tamber_serve.rs:524`

`describe` returns metadata as `[[key, value]]`. `add-token` keeps only string elements (`scheme=fixed`) and `filter_map` drops the rest, including a pair copied from `describe`. The token is added without `scheme=fixed`.

Fix: reject a meta element that is not a string, and document the two shapes (or accept both).

**Low — Attribution falls back to the last `hello`, and a `hello` without `client` clears it.** `contract/cli/src/tamber_serve.rs:192` and `:221`

A write’s event uses that request’s `client`, otherwise the `hello` client, otherwise JSON `null`. Commit 4’s stdio process will multiplex clients. One `hello` then a transaction that omits `client` is attributed to the hello client. A later `hello` without `client` sets `self.client` to `None`. Commit 5 wants the event to name the writer; a conflict still does not say who won, and the last writer is not stored.

Fix: require `client` on `transaction` and `undo`, and do not clear a known client when `hello` omits it. Remember the last writer per file so a conflict can name them when commit 5 needs it.

**Low — Temps from a crash before the journal is linked are never removed.** `contract/cli/src/tamber_serve.rs:409`

The names include the pid, so the next process does not overwrite them. On startup, delete `.exact/tamber-*.tmp` that the journal does not name.

**Low — Invalid UTF-8 on stdin ends the server with status 0.** `contract/cli/src/tamber_serve.rs:52`

`lines()` yields an error and the loop `break`s. The client gets EOF and no error object.

Fix: on a non-UTF-8 line, write `{"id": null, "error": …}` and keep reading, or exit non-zero.

## Checked and sound

There is no separate lock defect. The flock is held on the open `.exact/tamber.lock` for the process lifetime; a second server exits with “another tamber server”; if `.exact` cannot be created or the lock file cannot be opened, the process exits and does not serve. The lock is advisory, so it does not cover `tamber fmt` or an editor; that is the digest race above. Undo across a restart is not durable, which is the id finding, not a stuck lock.

`propose` and `transaction` share `run` → `plan`, so the diff, diagnostics, `changed` set, and `live`/`rebuild` answer match. `propose` skips the digest check; the serve test calls it without `files`. A client restore is rejected in `parse_ops` (`no operation restore`) and again in `edit::apply_all`. `undo` only runs `Restore`s the server stored, and `edit::undo` writes the preimage without requiring it to check.

JSON ids are echoed on success and on a handled error. A line that is not JSON is `{"id": null, "error": …}`. A committed write emits `changed` only after the commit, with that request’s client when it sent one. `hello` returns `tamber/1.0`, the IR format, and `"builds": []`.

For commits 4 and 5 the wire shape is usable as an additive protocol: one JSON line in, a reply with `id`, then an event line; `propose` already returns `class`. What those commits cannot build on safely is an inverse id that dies and gets reused on restart, a conflict that cannot name the writer, and `explain`/`resolve` payloads that omit when-line names and residual fallbacks. Held proposals can store the base digests in the server when `hold` arrives; `propose` does not need to return them for that.

The tests cover a one-line write, its undo, a stale digest, a forged restore, the second-server lock, and a kill between two renames. They do not cover a journal whose paths leave the app directory, a temp that is gone or is a symlink, a rename that returns an error and a later commit, undo after restart, a when-line `explain`, or a platform-colour `resolve`.
