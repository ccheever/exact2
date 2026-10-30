# Code review: Excord's windowOf row reuse, round 1 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `b202ad0ca0`.
- **Method:** one brief (sha256 `e3ccb18812775dc032614210af81c7e3b25fc334114d0d53f9dbe48711fac86e`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

The window-row cache key is complete. A cached row is reused only when the renderer returns the same row object and both `since` and the previous message id are unchanged, and nothing else this function puts on a row can go stale behind that key.

`windowOf` still calls `render` on every row. The WeakMap only decides whether to reuse the previous window row or build a new one. On a hit, that row is the earlier result of the same rendered row, the same `since`, and the same `previous?.id`. Those are exactly the inputs of `isNew`:

```428:431:apps/excord/render.ts
    const isNew = since !== '' && m.id !== null && cmpId(m.id, since) > 0
      && (!previous || previous.id === null || cmpId(previous.id, since) <= 0);
    const out = isNew ? { ...row, isNew } : row;
    cache?.set(row, { since, previousId, row: out });
```

Checked against the other inputs you listed:

- **Reader, directory, edits, reactions.** Author, roles, reply target, command user, system-line user, and guild name are part of `createRenderer`'s key (`render.ts:392-394`). The store replaces the message object on edit, reaction, and poll updates (`store.ts:344`, `630`, `656`), so the renderer returns a new row and this cache misses. `dirs.me()` and mentioned-user names are not in the renderer key. That hole is older than this commit: the uncached path returns that same rendered row. This cache does not make it staler.
- **Locale and time zone.** The row stores `at` and `prevAt` as absolute times. The day divider is computed in the view from `offset` (`transcript.contract:61`). A time-zone change does not need a new row object.
- **Channel, unread, missed anchors, jump highlight.** `channel`, `anchor`, and the unread count live on the window object, which is built fresh on every call. Missed anchors are rewritten in `loadWindow` before `windowOf` (`app.ts:286`). The jump highlight is `lit=(m.id == highlight)` in the view (`app.contract:1357`), not a field of the row.

**Memory.** `windowRows` is a `WeakMap` keyed by the renderer row. `createRenderer` holds at most 1,200 of those (`render.ts:398`). An `isNew` copy is retained only as the map value for that key, so it dies with the renderer entry. Evicting and rebuilding a row produces a new object, which is a cache miss, not a stale hit.

**Mutation.** No caller writes a returned row. For a non-new row the object is the renderer row, which was already shared before this commit. For a new row the cached value is a shallow copy, and a later `since` or neighbour change replaces that entry without writing into the old copy. The host JSON-serializes the resource result in the same turn (`js/src/prelude.js` `ok()`), and the transcript list is keyed by `m.key`, so row identity does not change what the view renders.

**Tests.** Same object across two windows, including the New copy; a `since` move renews only the two rows whose divider changes; an edit renews that row and its successor; a delete renews only the successor. The 300-step sequence appends, edits, deletes, moves `since`, and changes the tail width (older rows enter and leave the window) and deep-equals a fresh renderer with the cache forced off. Grouping and `prevAt` move with the previous message object, which is what makes the renderer emit a new row. That is covered by the edit, delete, and equality checks.

VERDICT: APPROVE

No findings.
