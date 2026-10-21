# Code review: stage 4 commit 2: the closure, round 3, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `d1cca8ee4562547760f0927f2c51491b9124d8042a88b7a070bf57c0c673de18`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (an unknown closure answered live; the cache key's resolution inputs; comments and the profile rule).
- **Disposition:** applied without re-review in the third-round commit

---

I'll review the last three commits read-only against the stage 4 brief, starting with the review instructions and the commit range.The review skill would write artifacts and review the whole branch. This request is a read-only pass over the last three commits, so I’ll inspect those commits and the surrounding code directly.The net diff is in. Next I’ll compare `folded` with the compiler’s synthetic-token path and the server’s cache.Auto-compacting conversation (80% full)...
Conversation compacted.
The review is ready to close. I'll re-check the load-bearing paths in the current tree so the verdict rests on the code, not the earlier notes.One finding.

**[P2] A compile that stops before the design is judged live with no synthetic tokens — `contract/cli/src/tamber_serve.rs:368`**

`synthetic()` clears the candidate list, then `compile_path_all` only fills it inside `declare_theme`, which runs after load, the native and picker checks, `strings::load`, and `declare_fonts` / `declare_sounds`. If any of those fail, the list stays empty. That empty list is not cached, so a later successful compile is retried, but it is still returned and passed to `plan`. A value edit of a token a row's mix reads is then `live` with `synthetic` empty. The last build that did lower still has that mix, so the live batch updates the token and leaves the mix on the old colour. An unknown closure has to be `rebuild`.

The closure rule holds for an app the compiler can lower. That is the path the brief's tests cover, and nothing else there is blocking.

1. **Fold parity.** `foldable` uses the same candidate list, the same `~` lines (`1 << 30`), and the same drop rule as `declare_theme`: a loose expression that does not fold is dropped and the rest are retried; a strict one, or any diagnostic that is not one of those, means that side does not fold. The compiler renumbers survivors; `plan` keeps the original index. Expressions name design tokens, not `~i`, so the folded colours match. The server's vocabulary is `theme::vocab()` (`tamber_vocab`). The compiler reads one design file (`design()`), and `plan` attaches the expressions only when the path is that file. A synthetic that names a token only declared in some other `.tamber` is not a candidate for either of them.

2. **Roles.** A changed token whose `Checked.role` is `Some` on either side is `rebuild` (`session.rs:127`). Alias chains are inlined in `Scope::named`, so `Canvas = mid` and `mid = brand` both change when `brand` changes, and `Canvas` carries the role. The serve test covers the one-step form (`Canvas = ink`). Becoming or ceasing to be a role changes the token's name, which `shape` already treats as rebuild. A live answer's `changed` list cannot contain a role: that token would have forced rebuild. A live mix may still read an unchanged role colour; that does not write the host role table. `set_token` refuses a non-empty plan role.

3. **Extraction and cache.** Candidates are recorded in `declare_theme` before the profile filter and before the drop loop, and `synthetic_candidates()` takes them, so a build that never gets there leaves none rather than a previous list. The old "`(` in a plan theme name" extraction is gone. The cache key is the contract source digests, skipping `*.tamber`, plus the design's token names in declaration order. Values, axes, and dials do not affect the candidate set; a value edit that changes which expressions fold is handled by `plan`, not by the cache. A name change or a contract edit misses the cache. Reordering tokens, or any contract byte change, recompiles without changing the answer. Every request still walks the source graph; the compile runs only on a miss. That cost is called out in the first-round message.

4. **Lost and gained.** If both sides fold and the kept index lists differ, the class is rebuild, in either direction. A strict expression that does not fold makes that side `None`, which is also rebuild. A lost expression can show up in a rebuild answer with an empty `values` list (`session.rs:137`), because the lookup is in the after fold. The class is rebuild, so it is not applied as a live batch.

5. **Tests.** The serve test checks a live `color-mix` against `check` with `tamber_vocab`, a direct role edit, a role reached through one alias, and an added token. The gain-then-live test checks that the next edit is live and names the mix; it does not check that mix's new colour. The strict and profile cases are in `tamber/tests/edit.rs`. The profile assertion allows `is_err()`. `valid()` accepts `color(--brand 0.1 0.2 0.3)`, and that value fails the crate's fold, so the assertion passes even if the substring rule were removed.

6. **Profile colours.** Either side's text containing `color(--`, case-insensitive, is rebuild (`session.rs:121`). A real `color(--…)` value also fails `color()` (`--` is not a colour space), so that side does not fold and is rebuild anyway. The substring is what catches a file the crate can still fold, including a comment. That over-approximates toward rebuild. Aliases of a profile token sit in the same file, so the literal is still there. `app.json`'s `"tamber"` is fixed when the server starts; the cache does not re-read it.

Tests were not executed.
