# Code review: the checker sweep (after the checker bound), round 2 of 3, 2026-10-24 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `b410b6ef1c`.
- **Method:** one brief (sha256 `b4a8d109875339a536fa1b9aeef8e9dd90fd7ddec24ddb209d18d3788fce20cf`), blind to the other reviewer; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

## BLOCKER

**`tamber/src/edit/batch.rs:88`** — a run of `add-token` with no `before` is not the text stepping would write, and the checker then accepts that text.

`added` steps only the first token (`Doc::apply`, which inserts after the last token). Every token after it is spliced at a common suffix of the original lines and the lines after that first insert:

```88:95:tamber/src/edit/batch.rs
    let suffix = before
        .iter()
        .rev()
        .zip(doc.lines.iter().rev())
        .take_while(|(x, y)| x == y)
        .count()
        .min(before.len());
    let mut at = doc.lines.len() - suffix;
```

A when-line does not carry the token name (`when tone=b: #222222`, from `when_code` at `tamber/src/edit.rs:1208`). When the first new token's trailing when-line equals the line just before the insertion point — the previous token's last line, in a theme that does not end with an indented closing comment — that line is part of the suffix. `at` lands on it. Later tokens are inserted between the first new token's name and its when-line, and the parser attaches that when to the last token in the run.

Stepping this:

```
theme
  ink = #111111
      when tone=b: #222222
```

with `t0` (`when tone=b: #222222`) and `t1` (`when tone=c: #333333`), both `before: None`, yields `t0` keeping its when and `t1` keeping only `tone=c`. The batch writes `t0` with no when, and `t1` with `tone=c` and `tone=b`. Conditions differ, so `check` at `tamber/src/edit.rs:529` passes and the write lands. If `t1` already has `tone=b`, the batch is refused and stepping would have been accepted.

`edit::run` (`tamber/src/edit.rs:511`) treats that string as the run. Import reconcile emits exactly this shape: axes first, then a consecutive run of `AddToken { before: None }` (`tamber/src/import/mod.rs:159`). Accept of that hold, and a wire transaction of two or more such adds, both go through `run`. A DTCG re-import into a design whose last token already ends with the same when-line is enough; the when values only have to match, which shared dark values do.

The one combined restore (`tamber/src/edit.rs:516`, `restore` at `tamber/src/edit.rs:260`) is a real inverse of the text `added` returned, so undo puts the file back. It does not make that text equal stepping.

`new_tokens_with_one_parse_land_where_stepping_puts_them` (`tamber/src/edit/batch.rs:394`) does not hit this. The only fixture with a when ends in an indented `// the end`, which the inserter steps past, and the generated when value is fixed at `#000000` against the fixture's `#222222` (`tamber/src/edit/batch.rs:423`). This was read, not executed.

## SHOULD

**`scripts/tamber-server.mjs:546`** — the local token's 16 MiB body is every method. A body over 1 MiB is supposed to reach only the import planner.

`who.local && !access.remote` selects `LOCAL_BODY_LIMIT` with no later check that the method is `import`. `ask` still refuses a checker line at 1 MiB (`scripts/tamber-server.mjs:377`), so `propose`, `transaction`, and `export` do not reach the design process. These do:

- `commit` writes `request.message` into the git message with no other cap (`scripts/tamber-commit.mjs:193`). A local commit body of about 15 MiB is stored.
- `write` calls `session.prepare` (`scripts/tamber-server.mjs:896`) before `ask` returns 413.
- `pin` (`857`), `outline` (`821`; `at` is capped at 4096 locations, the file strings are not), `scrub*` (`872`), and `hold` / `navigate` (`844`) run on the parsed body.

Paired, remote, and relay clients do not get this limit. `access.remote` or `access.tunnel` clears `local` before auth (`scripts/tamber-server.mjs:514`); a remote door is read with the 1 MiB body first (`538`); the tunnel and the relay are `remote: true` (`scripts/tamber-remote.mjs:336`). The length is judged from `content-length` before the read, and the byte count still caps a missing or unusable length (`scripts/tamber-request.mjs:31` and `:38`).

## NIT

- **`scripts/tamber-ops-limit.mjs:25`** — `reholds` still says a target is "its kind and its token's or param's name". `target` (`:45`) is the whole operation except the values a re-hold may edit. The comment describes the round-1 key.
- **`scripts/tamber-import-review.mjs:71`** — `--app` is refused only when `hello.ok.app` is present. A hello without it skips the check and the import is still posted.

## Round 1, re-checked closed

- Re-hold key. `target` (`scripts/tamber-ops-limit.mjs:45`) keeps every field except `value`, and an add-token's when values. `file`, `when` conditions, `before`, `kind`, `meta`, and `comment` stay in the key. The design tool's re-hold only writes those values (`apps/design-tool/app.ts:1346`). Length cannot grow, ids are not reused, a paired client is overwritten before the door, and accept applies the stored ops.
- Contrast is a name map, first name wins (`tamber/src/lib.rs:364`). An edit's conditions are a set (`tamber/src/edit.rs:336`). Retention expands each lowercased bucket once and keeps every spelling (`contract/lower/src/theme.rs:350`). The scheme walk keys aliases the same way `same_name` does (`contract/lower/src/theme/scheme.rs:41`).
- `same_conds` is set equality past 8 (`tamber/src/syntax.rs:251`). On a when that names each axis once, that matches the old two-way check. History refuses a version that names an axis twice before it keys when-sets (`tamber/src/history.rs:170`). `landing.rs:29` still has the old one-way `same`; it disagrees only when a when names an axis twice, which `valid` and the checker refuse, so a design that folds is unchanged.
- An import past the axes cap keeps `tamber-axes` (`tamber/src/import/landing.rs:277`). The whens message covers params (`tamber/src/bound.rs:130`).
- CLI sources for anything but Astryx go through `import`. Astryx stays local. The narrowest-when search stops at 4096 candidates (`tamber/src/export.rs:1235`). Reference folding stops at the depth bound before the next call (`tamber/src/check.rs:983`). No new super-linear pass on the wire checker. Exports membership and history's when-line / axis-value scans were left as answered.

Not run: the suite. The batch finding is from the placement above.
