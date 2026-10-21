# Code review: One retained-shape check for propose and the dev loops (theme2's ruling, Tamber stage 5), round 1, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the change.
- **Method:** one brief (sha256 `ee0ddb7e0972cdef24db01ad32800a124dd9a5629448caf587cbbbce1d72e4ce`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** changes requested (the cached `bindable` went stale: P1; P3s on a stricter shape and a failed fold caching every token).
- **Disposition:** in the first-round commit.

---

I'll review HEAD read-only against the retained-shape ruling: what lowering keeps, how `propose` classifies live versus rebuild, and whether the design server's cached retention can go stale.HEAD is one commit that moves token retention onto `Retention::keeps` and classifies `propose` from the plan's retained shape. I'll compare that keep-set with the old filter and with `Plan::structure`.The keep-set moved from filtered `said.names` to unfiltered words plus `Retention::keeps`. Next I’ll line that up with `Plan::structure` and the server’s cache key.The compiler still keeps the same tokens, and a fresh retention classifies a design edit the way `Plan::structure` moves. One cache hole produces a false `live`.

Lowering matches the old predicate. `Retention::keeps` is still a role override, `computed`, a literal word, or `bindable` bounded by `exports`, and a synthetic row is still kept beside that (`contract/lower/src/theme.rs:308`). Words are no longer filtered by `is_token` (`contract/lower/src/theme/scan.rs:157`), but every theme entry's name is a declared token, so it was already in the old set whenever it occurred as a word. `contains` on that name does not change. With that retention, `shape` (`tamber/src/session.rs:337`) drops only tokens the plan drops, and `varies` flips exactly when the plan's slice list goes from empty to one entry per tuple. An unkept add or first `when` stays out of both.

### P1 — cached `bindable` goes stale

`contract/cli/src/tamber_serve.rs:347` (hit at `:377`)

The cache key is contract sources, consulted manifests, and the design's token names. `bindable` also comes from `code_beside` (`contract/cli/src/sources.rs:225`): `app.ts`, `data/Cargo.toml`, or a non-empty `modules` / `hatches` in `app.json`. None of those are in the key, and a hit returns the old `Retention` (`tamber_serve.rs:948`).

Failure: the server has already compiled a plain app whose view names `ink` only. The author adds `data/Cargo.toml`. Token names are unchanged, so the cached retention still has `bindable: false`. `propose` of a first `when` on `spare` is `live`. The next compile keeps every token, the plan gains `spare`'s slices, and `Plan::structure` changes. Scrub is allowed because the class is `live` (`scripts/tamber-scrub.mjs:68`); the dev loop then rejects that preview (`host/web/src/dev.rs:321`). A client that applies the live batch anyway writes values for a shape the running plan does not have. The new tests start the server only after the crate is already there (`propose_classes_by_the_plans_retained_shape`), so a warm cache never sees `code_beside` flip.

### P3 — `shape` is stricter than the plan

`tamber/src/session.rs:328` and `:343`

`under_dark`, dial kind, token kind, and `scheme=fixed` are in `shape`. A plan axis is a name plus values, and a theme row is name, role, value, and slices (`plan/src/builder.rs:332`, `:353`). Toggling `scheme=fixed` or adding a dial is `rebuild` while `Plan::structure` stays put. The loops still batch. Slower only.

### P3 — a failed design fold caches "keep every token"

`contract/cli/src/tamber_serve.rs:388`

Candidates are recorded before the fold, and `RETENTION` is set only after it succeeds (`contract/lower/src/theme.rs:167`, `:307`). A design that fails the fold still has exprs, so the cache stores `retention: None`, and `shape` then keeps every token. The key ignores values, so fixing the value does not recompile. A later first `when` on a token the views do not name is `rebuild` once the app compiles and keeps only named tokens. Slower only. A compile that never reaches the design leaves the cache unset, and the wire class is forced `rebuild` (`tamber_serve.rs:1000`).

The keep-set and a fresh retention do not produce a false `live`. The blocking case is the stale `bindable` cache.
