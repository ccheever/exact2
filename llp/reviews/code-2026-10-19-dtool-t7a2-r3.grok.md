# Code review: LLP 1122 T7a-2 (copies at loopback names of their own; a pinned copy's pin holds over the app's own), round 3, 2026-10-19 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `9f5ec71884`.
- **Method:** one brief (sha256 `c32903843a2684cd430dd4ebc1c680371a65ef148219b0805f16f102c5e73c15`), theme2's rulings inline, shared by both reviewers, each blind to the other's review; commits 2a82f19e03, 55cefa3b2d and 9f5ec71884, with the records of rounds 1 and 2. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 BLOCKER, 3 SHOULD-FIX, 2 NIT).
- **Disposition:** all taken post-round, in the commit named in the sign-off request:
  - A hold's commit reaches the page (1).
  - A refused mode pin keeps the earlier holds and paints nothing new (2).
  - A refused hand-back is said and asked again (3).
  - The JS size check takes exponent percentages, as `appRootFontSize` does (4).
  - The test checks what paints (5).
  - `app.mode` is cut to the bound (6).

I'll review the last round of LLP 1122 T7a-2 read-only: the gate, pin hold, hand-back, status `app`, and the runner/JS hold path, against the round 1 and 2 records.This is a read-only review of the T7a-2 hold and host-gate change, including the round 1 and 2 fixes. I'll start from the review records and the commits on top of main.Round 1 and 2 records are in. Next I’ll read the hold, gate, and hand-back code at HEAD and check each recorded fix against the source.One blocker remains. On the wasm page a hold that changes the tree commits in the runner, and the page throws that commit away, so the painted text and modes stay on the old values while status reports the new ones.

## 1. BLOCKER — A wasm hold commit never reaches the page

`host/web/src/host/mode.rs:71`, `host/web/dev.js:539`, `host/web/dev.js:556`, `host/web/dev.js:559`, `runner/src/runner/held.rs:109`

`hold_facts` does commit a takeover or a partial release (`held.rs:115`). For any pin other than hand-back (`"-"`), `Host::hold_facts` keeps the receipt out of the batch (`Ok(_) => (vec![], None)`). `holdFacts` then `x.send`s that batch (`dev.js:460`). The batch has the released `setScheme` / `setRootFontSize` commands and no node ops. The wasm document writes absolute `px` (`host/web/src/document.rs:623` via `css_text`, relative off), so those ops are the only way the text and the mode colors move.

The page then does not fill the gap. `exact_set_root_font_size(hostSize())` reads the pin's own `!important` size. After the takeover the kernel is already at the host px, so a pin of that same px returns `Ok(None)` (`runner/src/runner/root_font.rs:155`, `kernel/src/kernel.rs:808`). An axis the app has asked for is left out of `exact_set_mode` (`dev.js:559`).

Scenario A. The browser root is 16px and the app has called `setRootFontSize(24)` or `"200%"` (laid out at 32). The tool pins `{rootFontSize: 16}` and leaves scheme and mode alone. The runner lays out at 16 and the receipt is dropped. The follow-up setter sees 16 already set and commits nothing. Status says 16, because `facts` reads the computed root (`dev.js:434`). The text stays at 24 or 32. A pin of 32 does paint, because 32 differs from the 16 the takeover just committed, and that second commit is a normal `exact_set_root_font_size` batch.

Scenario B. The copy is held at `dark-style=black`. The app asks for `dim`. The next pin is scheme-only. The runner sets the window to `dim` and commits. The receipt is dropped, and the page sends no `exact_set_mode` because `asked.mode['dark-style']` is set. Status, which reads the runner via `exact_set_mode("?")`, says `dim`. `data-x-modes` and the colors stay on black.

Scenario C. A root pin of 18, then the app asks for 24, then a pin that drops `rootFontSize`. The runner lays out at 24. The node ops are dropped. The follow-up `set_root_font_size` sees an absolute app size and returns without a commit (`root_font.rs:150`). The CSS rule from the command makes status say 24. The text stays at 18.

Hand-back (`"-"`) does put the receipt in the batch (`mode.rs:46`), so a full hand-back paints. The JS page paints from CSS and from `asked[fact].apply()`, so it does not take this path.

## 2. SHOULD-FIX — A refused mode pin releases the hold that was already up, then paints the pre-pin snapshot

`host/web/dev.js:539`, `host/web/dev.js:563`, `host/web-js/dev.mjs:714`

`hold_facts` refuses an unknown value before it changes anything (`held.rs:50`). The page then calls `holdFacts` again with the same flags and no modes. A pin that is only `{mode: {dark-style: "grey"}}` has empty flags, so that second call is a full release: the previous scheme, mode, and root holds take the app's choices (and, on wasm, that commit is the one finding 1 drops). The page still runs the refused `exact_set_mode`, and the refusal path writes `before.mode`, the snapshot from the first pin.

Scenario. The copy is held at dark / black / 32, and the app has asked for dim / 24. The tool sends `dark-style=grey`. The runner's first answer changes nothing. The fallback releases the whole hold. The mode refusal then sets the window to the pre-pin snapshot and that commit does paint. The copy shows the snapshot, the runner is not holding, and `channel.held` is the grey frame, so `app` describes grey. The JS page does the same split: `modes.set` fails, the snapshot is written, the mode keys are deleted from `exact.held`, and every previously held fact is applied.

## 3. SHOULD-FIX — A refused hand-back keeps the hold in the runner and clears it on the page

`runner/src/runner/held.rs:117`, `host/web/dev.js:569`

The runner's rollback on `Err` puts back the hold, the window, the root size, and the commands. The page never reads that error. It has already cleared the pin sheet and the font override, and it still sets `channel.held` to null and `channel.before` to null.

Scenario. A gated task refuses every commit. Hand-back is one of those commits. The runner stays on the pin and keeps swallowing `setMode`, `setScheme`, and `setRootFontSize`. The page paints the handed-back scheme and font in CSS, stops reporting `app`, and will not ask again. A later hand-back is not sent, because the page believes nothing is held.

## 4. SHOULD-FIX — On the JS page an exponent percentage slips past the font hold

`host/web-js/commands.js:49`, `host/web-js/navigation.js:981`

The hold records a percentage only when it matches `/^\+?(\d+|\d*\.\d+)%$/`. `appRootFontSize` accepts CSS's exponent form, `/^[+-]?(\d+|\d*\.\d+)(e[+-]?\d+)?%$/i`, which is the same grammar as the runner's `css_percent` (`num/src/lib.rs:82`). A value the regex misses is applied immediately, hold or not.

Scenario. A copy is pinned at 18px. The app calls `setRootFontSize("1e2%")` or `setRootFontSize("2E1%")`. `exact.held.rootFontSize` is set, and the call still writes the `#exact-root-font-size` rule. The pin's `!important` can lose to that rule. `"200%"`, `"+200%"`, `".5%"`, and `"medium"` stay held. The wasm runner holds `"1e2%"`, so the two pages disagree.

## 5. NIT — The new tests read the runner and the status, so finding 1 passes them

`contract/cli/tests/it/tamber_held.rs:70`, `scripts/dev-copies.test.mjs:185`

The Rust test asserts `root_font_size()` after the takeover commit, which is the kernel's size, and the hand-back path, which does return a receipt. The browser test pins a font only on the JS copy, and on wasm it checks `status.mode` after a mode release. That status comes from `exact_set_mode("?")`, the runner window. `data-x-modes` is checked only while the pin holds, and a missing attribute becomes `'none'`, which also satisfies `not.toContain('dim')`. Nothing asserts the painted px after a pin equal to the host size, or the attribute after a mode is released.

## 6. NIT — `app.mode` is not cut to the bound that refuses the status

`host/web/dev.js:449`, `scripts/tamber-builds.mjs:134`

`app.scheme` is sliced to 32 characters and `app.rootFontSize` to 64. Mode axis names and values are sent whole. The server allows 64 characters each and 1 KiB of UTF-8 for the whole `app` object (`Buffer.byteLength`). One longer design value makes every later status 400 while that disagreement lasts. `app` is otherwise omitted when nothing differs, and the server stores `r.app ?? null` (`tamber-builds.mjs:427`), so a later status clears a stale one.

---

The gate holds. `developmentGate` admits only `exact-copy-<n>.localhost` with `n` from 1 to 99, at this port (`scripts/install-page.mjs:298`). `0`, `100`, a leading zero, the wrong port, `evil.localhost`, and a name that merely ends in the suffix fail. Port 80 also admits the `Host` with the port omitted, and `:8080` does not match the optional `:80`. A copy is never `local`: that still requires the host to be `127.0.0.1` or `localhost` (`install-page.mjs:300`). A rebinding `Host` fails the pattern. `.localhost` is loopback in the browser. Separate storage is stated (`docs/tamber-protocol.md:1116`). The port-80 test checks admission and does not check `local: false`.

Rounds 1 and 2, against the records. Choosing the pinned value, a `setMode` before the pin, `"system"` and `"medium"`, and a pin with no mode are recorded at the source and handed back as that latest choice. The `"?"` question applies no batch, so a pin does not reschedule itself. An invalid font no longer replaces a valid ask, except the JS exponent form in finding 4. A refused hold commit in the runner rolls the hold, window, root size, and commands back together. Two recorded limits still stand: a dev reload checkpoints `ch.before.mode` (`host/web-js/dev.mjs:701`), and a resident wasm reload starts from an empty `asked`, so a press from before the reload is not what hand-back applies. The launch choices are asked again when the app runs again.

What also holds. While a fact is held, a later `setMode`, `setScheme`, or `setRootFontSize` is recorded and not applied, including a run of them and a choice equal to the pin. In one commit, `take_set_modes` runs first and `take_held` then removes a held `setScheme` or `setRootFontSize`; other commands stay. A gate refusal before those two calls records nothing. Hand-back when the app asked nothing of the held facts commits nothing in the runner, and the page puts back `before` for scheme, font, and the axes the last pin set. The `asked` object is escaped with `quote` / `untyped_json` and spliced in front of a batch that starts with `{` (`host/web/src/host/mode.rs:78`, `host/web/src/batch.rs:900`).

A non-copy (the single canvas, a tab, a frame named `app-own`) does not take the hold. `holdFacts` returns immediately, the sheet has no `!important`, and `app` is omitted. An active pin on those pages still merges the pre-pin mode snapshot. Hand-back restores only axes the last pin named, so an unpinned axis the app changed during the pin stays. That is the rule the comment states, and it is D3 for that axis. Native builds do not run these scripts.

With nothing held, every valid `setMode` still inserts one map entry, and `take_held` clones the first argument of every command in the commit before it looks at the name (`held.rs:150`). That is the cost round 2 left in place: a handful of allocations on the app's own appearance commands, in production too, so a pin that arrives after launch can see the launch choice.
