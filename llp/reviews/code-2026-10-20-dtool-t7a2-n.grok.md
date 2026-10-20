# Code review: LLP 1122 T7a-2, the narrow verification round of 71d19f4a83 (review2's 00:39Z ask), 2026-10-20 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `71d19f4a83`.
- **Method:** one brief (sha256 `bbe91e4d2f549ff46d41a8af14f3125e417e8ed349466e033cd3140c8f36ec0f`): round 3's findings verbatim, then the same question. Shared by both reviewers, each blind to the other's review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** a new BLOCKER (the retry loop, and a pending release handing a new pin back), 1 NIT; round 3's findings closed or partly closed.
- **Disposition:** small fixes, in the commit named in the sign-off note:
  - The quiet apply; a new pin clears a pending release.
  - The JS size check reads the live root size (NIT).
  - `app.mode` is bounded so the whole `app` stays within 1 KiB.
- **Not taken:** a mode-only refusal releasing the scheme and root the earlier pin held (A4, B2). The new pin doesn't pin them, so releasing them is what it asks; only its modes were refused. The page showing a refused hand-back's release is as Astra's.

I'll verify each round-3 finding against HEAD `71d19f4a83` only: read the commit and the code it touches, and leave the tree unchanged.HEAD is the post-round-3 commit. Next I’ll read its diff and the lines each finding names.The commit touches the hold path, both pages, and the copy test. I’ll read those sites against each finding.Reviewer A’s receipt blocker is closed. A refused hand-back still blocks: the page drops the pin while the runner keeps the hold, and the new retry spins and releases a later pin. Read-only; tests were not run.

**Reviewer A**

1. CLOSED — `host/web/src/host/mode.rs:72` puts `Ok(Some(receipt))` in the batch, and `host/web/dev.js:461` `x.send`s every hold except `?`, so a takeover, a partial release, and a dropped root size paint from that commit.
2. PARTLY — `host/web/dev.js:575` stores `channel.releasing` and `dev.js:478` retries `holdFacts("-")` after later commits, while `dev.js:549` still clears the pin sheet and font and `dev.js:575` nulls `channel.before` and `channel.held` and stores the revision, so the page reports the release and omits `app` for as long as `runner/src/runner/held.rs:117` keeps the hold.
3. CLOSED — `host/web-js/commands.js:49` uses `appRootFontSize`’s exponent grammar (`host/web/navigation.js:981`) and `Math.fround`, so `"125e0%"`, `"1e2%"`, and `"2E1%"` are recorded under the hold and `1e100` is left unrecorded.
4. PARTLY — `host/web/dev.js:543` re-holds the previous mode axes and `dev.js:564` skips `exact_set_mode`, so invalid Grey keeps Black held and leaves the snapshot unpainted; that second `holdFacts` still takes scheme and root flags from the refused pin, so a mode-only refusal releases a scheme and root that were held and the new receipt paints that release. `host/web-js/dev.mjs:714` is unchanged: a non-copy still writes the whole pre-pin snapshot, and a copy still applies every previously held fact the new pin omits.
5. CLOSED — `host/web/dev.js:450` and `host/web-js/dev.mjs:708` drop an axis or value longer than 64 characters, so a 65-character alternative stays out of `app.mode` and that status stays acceptable.
6. PARTLY — `scripts/dev-copies.test.mjs:193` asserts the wasm background after a mode release and the JS `data-x-modes` attribute, and `:186` asserts the JS root at 18 under the pin and 24 after hand-back. A pin equal to the host size, a wasm text node’s px, a refused hand-back, an exponent percentage, and a held command beside an unrelated command are unasserted; `contract/cli/tests/it/tamber_held.rs:73` still checks the runner’s `root_font_size` after the takeover.

**Reviewer B**

1. CLOSED — `host/web/src/host/mode.rs:72` and `host/web/dev.js:461` deliver the hold commit, which covers the 16px takeover, the scheme-only release of Black, and a pin that drops `rootFontSize`.
2. PARTLY — `host/web/dev.js:564` and `:569` leave a copy’s modes as they are when the pin’s modes are refused, and `dev.js:543` keeps the previous mode axes; with empty flags that second `holdFacts` still releases scheme and root, and the receipt now paints the release. `host/web-js/dev.mjs:714` still applies every previously held fact after `modes.set` fails.
3. PARTLY — `host/web/dev.js:478` asks the hand-back again after each commit and `dev.js:575` reports the refusal once; `dev.js:549` still clears the sheet, the font, `channel.before`, and `channel.held`, so the page paints the release and stops reporting `app` while `runner/src/runner/held.rs:117` keeps the hold.
4. CLOSED — `host/web-js/commands.js:49` accepts `/^[+-]?(\d+|\d*\.\d+)(e[+-]?\d+)?%$/i`, the same grammar as `host/web/navigation.js:981`, so `"1e2%"` and `"2E1%"` are recorded and held.
5. PARTLY — `scripts/dev-copies.test.mjs:194` requires `data-x-modes` to contain `dark-style=dim` after release on JS, and `:193` requires the wasm background to change. The painted px of a pin equal to the host size is unasserted, and `contract/cli/tests/it/tamber_held.rs:73` still stops at the runner.
6. PARTLY — `host/web/dev.js:450` and `host/web-js/dev.mjs:708` cap each axis and value at 64 and the mode JSON at 900 bytes. A mode of 890 bytes together with a 32-character scheme and a 64-character `rootFontSize` is 1025 bytes, past the 1024-byte `app` object at `scripts/tamber-builds.mjs:134`, so that status still returns 400.

**Introduced by `71d19f4a83`**

- BLOCKER — `host/web/dev.js:478` retries a refused hand-back from `committed`, and `holdFacts` `x.send`s the error batch (`dev.js:461`), whose apply calls `committed` again (`host/web/glue.js:900`) after `due` was cleared. A standing refusal retries on every turn. `dev.js:576` stores a later pin without clearing `channel.releasing`, so that retry hands the new pin back.
- NIT — `host/web-js/commands.js:49` judges a percentage as `Math.fround(parseFloat(v) / 100 * 16)` while `host/web/navigation.js:983` uses the live host. At a 24px host, `"1.5e39%"` is recorded and refused on replay.

A successful pin’s receipt, mode-release paint, exponent hold, and 65-character omission are in place. The hand-back refusal still blocks.
