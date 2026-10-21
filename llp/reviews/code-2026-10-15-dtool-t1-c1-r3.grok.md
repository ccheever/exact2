# Code review: LLP 1122 T1 commit 1 (Tamber server: history, revert, import, the tool's files), round 3, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `165174983` (the server files of `9479c6464..165174983`).
- **Method:** one brief (sha256 `517bc500333f181635a07a6521a631557cd3edb059c594a75e2a6c71b72e6c88`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer. The last round: its findings are fixed without another.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 medium, 1 low).
- **Disposition:** all taken (with Astra's): `requires` checked on the write's own snapshot; axis-value keys; the `--via` body cap.

---

The round-1 and round-2 findings that were taken are fixed in this tree, including axis order, dial identity, per-file compare, path handling, the DTCG name cut, and the nested design path. Three problems remain.

## Medium — A revert's untouched-file digests are checked on a separate hello, and the write ignores them

`scripts/tamber-server.mjs:518`, `scripts/tamber-server.mjs:534`, `contract/cli/src/tamber_serve.rs:703`

`accept` of a held revert calls `guardsMoved`, which sends `hello` and compares the digests of files the plan did not edit. The `transaction` that follows sends only `p.base`, the files the operations name. In the session core, a file is stale only when some operation names it:

```703:711:contract/cli/src/tamber_serve.rs
        let stale: Vec<&String> = snap
            .iter()
            .filter(|(p, _, d)| {
                ops.iter().any(|(f, _)| f == p)
                    && match seen.get(p) {
                        Some(s) => s != d,
                        None => write,
                    }
            })
```

A digest for a path with no operation is never compared to the snapshot the write reads. The server reads stdin in order and handles other clients while `hello` is in flight (D12: several local clients, serialized by digests).

Scenario: a commit changed `app.tamber` and `b.tamber`. `b.tamber` is already back to the parent, so the held revert's only operation is on `app.tamber` and `b.tamber`'s digest is a guard. Accept's `hello` observes `b.tamber` unchanged. Before that reply returns, another client's transaction changes `b.tamber`. That write sits on stdin after the `hello` and before accept's `transaction`, so the core applies it in between. The `transaction` sees `app.tamber`'s digest match, reverts `app.tamber`, and leaves `b.tamber` at the third value. The tree is no longer that commit's inverse, and the accept returns success. The test at `scripts/tamber-server.test.mjs:658` changes `b.tamber` before `accept` is called, which the `hello` still catches.

Fix: on the same snapshot the transaction writes, treat every digest in the request's `files` as a precondition, including paths no operation touches. Pass `p.base` together with `p.guards` in that one request.

## Medium — Every axis operation on one axis shares one selection key, so a replan restores a rejected value

`scripts/tamber-server.mjs:811`, filter at `scripts/tamber-server.mjs:587`

`revertKey` classifies any operation whose name matches `/axis/` as `what: 'axis'` with an empty `when`, and the name is the axis. `add-axis-value` and `remove-axis-value` do not include the value, so "remove navy" and "remove olive" on `scheme` are both `app.tamber\0axis\0scheme\0[]`. `planRevert` keeps every operation whose key is in `only`. The first partial accept holds the chosen operation objects, so the selection survives until the next replan. A replan (`guardsMoved`, or a digest conflict on the edited file) calls `planRevert` with that shared key, and both value operations come back. The replacement is held (`recomputed: true`) for a second accept. Accepting it applies the value the person rejected. D10 says a partial accept is recomputed and shown again; the stored `only` is what that recompute is supposed to keep.

Scenario: a commit appends `navy` and `olive` to `scheme`. The person accepts only the `remove-axis-value` for `navy`. The held proposal has that one operation, and `review.revert.only` stores the shared key. `edge` then changes. Accept replans, and the replacement proposal contains both removals. Accepting it deletes `olive` too. The same expansion happens for two `add-axis-value` operations when the person restores only one removed value. The test at `scripts/tamber-server.test.mjs:667` selects the token `ink` against `edge`, and those keys differ by name, so it stays green.

Fix: key an axis-value operation by the value and by add versus remove. Keep the axis name alone for `add-axis` and for an axis-level conflict. Filter the replanned operations with that key.

## Low — `tamber tool --via` buffers the request body with no 1 MiB cap

`scripts/tamber-server.mjs:764`

`handle`'s `body` stops at `BODY_LIMIT` (`LINE_LIMIT - 64`, about 1 MiB minus the id) and answers 413. `forward` concatenates every chunk, then posts that buffer to the dev loop. A local client can open `http://127.0.0.1:7740/__tamber` while `tamber tool --via` is running and send a body far past 1 MiB. The proxy holds the whole thing.

Fix: apply the same `BODY_LIMIT` in `forward`, destroy the request past it, and answer 413.

Verdict: request changes — the untouched-file guard still misses a write that lands during accept, and a partial revert of two axis values loses the rejected value on replan.
