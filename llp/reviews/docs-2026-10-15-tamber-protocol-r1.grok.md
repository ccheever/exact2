# Docs review: docs/tamber-protocol.md (the Tamber protocol's reference), round 1, 2026-10-15 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, edits and writes denied, a throwaway worktree at `80a292c52`.
- **Method:** one brief (sha256 `50452d554d32302ff0c7584730032af302e5d2e72a259a74160bec7edb7ceb3c`), shared by both reviewers, blind to the other review. The author (Claude, theme2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 blocker, 22 should-fix or unclear, 10 nits).
- **Disposition:** the page rewritten against the code. Taken: the error and status table (bare 404s, 421 from the gate, 400/409/500/504 cases); a 504 does not cancel a write; no client-version negotiation; the 1 MiB less 64 body limit; extra design files only in the app directory; edits splice touched lines (not whole-file fmt); the IR has no `when`s; the shapes of `resolve` and `explain`; `changed` includes dependents; `undo` returns a new inverse and ids are single-use; `accept` with no items applies role offers; a held propose ignores `requires` (files not edited become guards; queued as a wire fix); a recomputed proposal is not written; `history` compares parsed declarations, first-parent; revert refusals by edit (an added axis, not a removed one); import shapes and defaults; subscribe replays nothing, `epoch` is checked only when sent; the journal kept when it cannot be judged. Not taken: the `tamber tool` command and the design-tool README, which landed on main (`9bdf8cf62`) after the reviewed commit; LLP 1122 §10 now exists (same push).

---

I'll review the Tamber wire reference against the server and session code, read-only, and check each claim against what those files actually do.The commit adds the protocol page and two short pointers. Next I’ll read the server and session code those claims are supposed to match.The wire, the operation table, the commit guard, and the event names match the code. The problems below are places a client would follow the page and get a different result. Fields you flagged as not on this branch (`hello.app` / `hello.epoch`, `resolve.srgb` / `resolve.files`, `explain.files`, `wait: false`, long-poll 503) are not treated as missing.

## Wrong or misleading

1. **Blocker.** A full `accept` applies role-override operations. `docs/tamber-protocol.md:243-244` says each offer's items are "off until accepted." `importReview` appends every offer's ops after the mapped ops (`scripts/tamber-server.mjs:703-710`), and `accept` with no `items` (or with every index) writes `p.ops` whole (`scripts/tamber-server.mjs:509-522`). Leaving them off means the client must pass `items` of `0 .. review.tokens - 1`. A full accept does the opposite.

2. **Should-fix.** `docs/tamber-protocol.md:220-222` says history edits come from comparing the two versions' IR, and that a version that "doesn't check" has `edits: null`. Comparison is of the parsed declarations, and a version the checker refuses still produces edits (`tamber/src/history.rs:1-8`, `61-68`). `edits: null` and a string `refused` happen when a side does not parse, a name or `when` is declared twice, or the compare line is at least 1 MiB (`scripts/tamber-server.mjs:546-550`). `what` is `token`, `dial`, `axis`, or `other`; `before` and `after` are strings or null. The "`radius.md` 8 → 12px" sentence is not a field.

3. **Should-fix.** `docs/tamber-protocol.md:229-232` has the axis case backwards. A commit that removed an axis is invertible with `add-axis` while that axis is still absent. A commit that added an axis is refused: "no operation removes an axis" (`tamber/src/history.rs:291-300`). Also refused, and not listed: an axis's `under` changed, or the design's name or exports changed. "Added or removed a dial" and "reordered an axis" are right. "Everything is undone already" is a 409 whose body is `already` (array of `{file, edit}`), not `refused` (`scripts/tamber-server.mjs:584-588`). A commit that touched no design file is 400, not 409.

4. **Should-fix.** `docs/tamber-protocol.md:88-91` puts `when` lines in `describe`'s `ir`. The IR is the folded table (`format`, `digest`, `name`, `tuples`, axes, dials, tokens with `values` / `entries` / `dials` / `fallback`, `exports`) and has no `when`s (`tamber/src/ir.rs:54-155`). Source `when`s are only on `explain`.

5. **Should-fix.** `docs/tamber-protocol.md:26-27` says a second server answers every request with 503. The design process exits and `POST /__tamber` and `POST /__tamber/subscribe` do return 503 with the lock error (`scripts/tamber-server.test.mjs:224-233`). `GET /__tamber/token` and `GET /__tamber/tool/…` do not look at `down`. `POST /__tamber/events` currently answers 200 (`scripts/tamber-server.mjs:327`), which is the behaviour the *1.1* note at line 277 says will change.

6. **Should-fix.** `docs/tamber-protocol.md:47-49` and `:63` describe every failure as `{"error": {"message"}}`, and a POST without the token as 404. A missing token, a wrong token, and any non-POST on those paths are 404 `{"message": "Not found."}` (`scripts/tamber-server.mjs:85`, `301`). There is no 401. The development gate rejects an unknown host or a cross-site POST with 421 and an empty body before the token is checked (`scripts/install-page.mjs:286-292`, `scripts/tamber-server.mjs:642-644`).

7. **Should-fix.** `docs/tamber-protocol.md:132` says every written file is formatted with `tamber fmt`. Only the lines the operation rewrites are laid out with the fmt helpers; the rest of the file is left as it was (`tamber/src/edit.rs:547-570`). A value edit is one line because of that, including on a file that was not formatted. `set-token`'s `comment` is kept only when a new `when` line is inserted; an update of an existing line keeps that line's comment and ignores the field (`tamber/src/edit.rs:547-577`).

8. **Should-fix.** `docs/tamber-protocol.md:155-157` says a startup that finds a journal completes or withdraws it. That is true when every rename can be judged. A partial landing, a temporary file whose digest does not match, or a journal that cannot be read is kept and the process refuses to start (`contract/cli/src/tamber_serve.rs:342-394`). The wire then reports that as down.

9. **Should-fix.** `docs/tamber-protocol.md:284-287` says `bun exact.mjs tamber tool` serves `http://127.0.0.1:7740`, `--via` forwards `/__tamber`, and `apps/design-tool/README.md` exists. `scripts/exact.mjs:893-896` only dispatches `serve`, `import`, and `op`. There is no `apps/design-tool/`. `GET /__tamber/tool/…` itself is implemented (`scripts/tamber-server.mjs:266-291`). This was not in the list of *1.1* fields to treat as intended.

10. **Nit.** `docs/tamber-protocol.md:13-14`: no request carries a client version. `hello` reports `"tamber/1.1"`; another major is not refused.
11. **Nit.** `docs/tamber-protocol.md:46`: the HTTP body limit is `1 MiB - 64` bytes (`scripts/tamber-server.mjs:71-72`), so a body of exactly 1 MiB is 413.
12. **Nit.** `docs/tamber-protocol.md:91-92` lists `range` among editor metadata. On a dial, range is the `in low..high` clause (and the IR's `range`), not a `meta` pair. `meta` is whatever `name=value` words were written; D4's names are `step`, `control`, `label`, `help`, `group`, `priority`.
13. **Nit.** `docs/tamber-protocol.md:153`: `design-tool:`, `agent:`, and `import:` are convention. Any non-empty string is accepted.
14. **Nit.** `docs/tamber-protocol.md:299` puts `resolve-on` on LLP 1122 T2b. LLP 1121 stage 4's gate is `uses` and `resolve-on` (`llp/1121-tamber.rfc.md:1386`). T2b is inspect.
15. **Nit.** `llp/1121-tamber.rfc.md:2248-2249`: neither RFC defines a "stage 4 commit 5." The build channel is LLP 1122 D4 / T2, with Tamber stage 4.

The new recipe line (`docs/recipes/tamber.md:66`) is right. The operation names and fields match `parse_ops`. The commit guard matches `scripts/tamber-commit.mjs:146-177` and LLP 1122 D9. Event names and `proposal-dropped` reasons match the wire.

## Omitted, and a client needs it

16. **Should-fix.** A 504 does not cancel the write. `ask` answers "the design server did not answer" at 120s and still applies the reply when it arrives (`scripts/tamber-server.mjs:176-184`, `392-427`): the change is announced, and a held proposal can still be marked accepted, restored, or `stale`. The same is true of `undo` and `accept`.

17. **Should-fix.** `undo`'s reply is the transaction reply, including a new inverse id. The id just used is deleted (`contract/cli/src/tamber_serve.rs:757-760`). An unknown or dropped id is 400, not 409; the message names the 256 cap. The inverse is a byte `restore` of the lines that write replaced, so an unformatted file comes back as it was. `docs/tamber-protocol.md:173-179` does not say the reply shape, that the id is single-use, or that undo is itself undoable.

18. **Should-fix.** A successful `accept` returns that same transaction reply. `client` is the proposer. `acceptedBy` and `proposal` are on the `changed` event only (`scripts/tamber-server.mjs:522`). `docs/tamber-protocol.md:205-210` only shows the recomputed reply. `drop` returns `{dropped: id}`.

19. **Should-fix.** `changed` is every token whose folded value differs, including dependents of a dial, plus tokens added or removed (`tamber/src/session.rs:136-164`), not only the name in the op. `files` on a write is every design file after the write, not only `written`.

20. **Should-fix.** On `propose` with `hold: true`, the client's `requires` is ignored. The wire turns `files` entries that no op names into `requires` (`scripts/tamber-server.mjs:368-373`, `446-450`). A plain `propose` does forward `requires`, and its 409 has no `by`. `by` is added only on `transaction` and `undo`.

21. **Should-fix.** Commit refusals are 400 with `refused`, except a write in flight, which is 409 without `conflict: true` (`scripts/tamber-server.mjs:363`, `tamber-commit.mjs:163-164`). "Nothing to commit", "not a git repository", and a git failure are also 400 (`commit: …`). Git reads that throw, in `history`, `revert`, or `prepare`, are 500. None of these codes are in the table at `docs/tamber-protocol.md:47-53`.

22. **Should-fix.** `subscribe` does not replay the kept events. It sends `subscribed` at the current `seq` and then only later events (`scripts/tamber-server.mjs:305-310`). Catch-up is the long poll. `epoch` is checked only when the client sends it (`scripts/tamber-server.mjs:323`). Omitting it, a new process whose `seq` still covers `after` is not `missed`. An `after` greater than the current `seq` is `missed` too.

23. **Should-fix.** Import defaults and shapes `docs/tamber-protocol.md:238-245` leaves out:
    - Omitted `prefix` becomes `shadcn`, `tw`, or `dtcg` (`contract/cli/src/tamber_import.rs:114-117`). That is the token prefix.
    - `name` is the filename. Figma modes come from `<Collection>.<Mode>.json` (`tamber/src/import/dtcg.rs:463-478`). `sources` is 1 to 256 `{name, text}`, then sorted by name.
    - The first `review.tokens` operations are the importer's own ops, tokens and any `add-axis`, not only tokens.
    - `counted` is `{source, mapped, reported}`. `unmapped` is `[{block, name, value, why, token}]`, and `token` is null when there is none. `offers[].note` is null or a string when that offer has no ops.
    - A source or design that does not parse uses string `diagnostics`, not `{file, line, col, id, message}`. "Nothing new" is 200 `{proposal: null, review, message}`, not an error.

24. **Should-fix.** History is first-parent only. `parent` is null on a root commit. Renames are a deletion plus an addition (`scripts/tamber-history.mjs:8-9`, `60-75`). `date` is the author date. `moved` on a held revert is paths whose bytes differ from the commit's result even when the proposal is held; refusal is the separate 409.

25. **Nit.** A removed path is listed in `edited` and omitted from `files`. `docs/tamber-protocol.md:259` only says the digest moved. Replacing the client's file map from `files` still sees the deletion.
26. **Nit.** Other design files are `*.tamber` directly in the app directory, plus the file `app.json` names even if that path is nested (`contract/cli/src/tamber_serve.rs:463-477`). Not every `.tamber` beside a nested design.
27. **Nit.** `expires` is an ISO-8601 UTC timestamp. `proposal-dropped.client` is null for `expired`, `stale`, and `too many held`.
28. **Nit.** If `update-ref` fails, the index entries for those paths are put back (`scripts/tamber-commit.mjs:186-196`). The page only says HEAD moves from the checked HEAD.

## Unclear

29. **Should-fix.** `docs/tamber-protocol.md:207-209`: "The old operations are never replayed onto new bases." For an ordinary proposal, a conflict or a partial `accept` re-proposes those same op objects on the current files and returns a new proposal; it does not `transaction` them onto the new bytes. A revert is planned again from the commit (`scripts/tamber-server.mjs:483-507`). The sentence matches LLP 1122 D10 for the revert path and is easy to read as applying to every proposal.

30. **Should-fix.** `docs/tamber-protocol.md:98-104`: the non-residual tuple is `{modes, value, light, dark}`. `value` is `Folded::css()` (`#rrggbbaa`, `light-dark(…)`, or a wide serialization). `light` and `dark` are each side's `Half::css()`, and for a single colour they are equal (`contract/cli/src/tamber_serve.rs:652-657`). They are not the 8-bit clip. The *1.1* `srgb` description matches `Half::clip` (`#rrggbbaa`). A residual has no top-level `light` or `dark`; the next bullet says that, and it does not say whether a residual also gets `srgb`. `fallback` is null when the role is not in the vocabulary. `explain`'s `value` and `whens[].value` are source text, not `resolve`'s CSS (`contract/cli/src/tamber_serve.rs:681-689`). `dials` and `names` are string arrays.

31. **Unclear, *1.1*.** `docs/tamber-protocol.md:277-278` says the long poll returns the `down` event if it has not been seen, then 503, and never an empty answer. The status list at line 52 says 503 is `{"error": {"message"}}`, which cannot also be `{events, …}` carrying `down`. Line 27 says every request is already 503, while this 503 is marked *1.1*. Those two cannot both be the current contract. `hello.epoch` and the event `epoch` are both "this run," which is consistent if they are the same value. `app` does not say whether it is the canonical absolute directory.

32. **Nit.** `docs/tamber-protocol.md:4-5` says what was built is in each RFC's §10. LLP 1121 has §10. LLP 1122 stops at §9.
33. **Nit.** `docs/tamber-protocol.md:142-144`: `class` is only a label in `tamber/1.1`. Nothing applies a live table yet; that is stage 4, which the last section does say.
