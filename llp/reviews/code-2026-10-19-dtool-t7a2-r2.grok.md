# Code review: LLP 1122 T7a-2 (copies at loopback names of their own; a pinned copy's pin holds over the app's own), round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `55cefa3b2d`.
- **Method:** one brief (sha256 `cde5b4fcb74f0d69bdfbba58c1bb3095a5c6dcaf620a7a4b44abbdbd03d613e6`), theme2's rulings inline, shared by both reviewers, each blind to the other's review; commits 2a82f19e03 and 55cefa3b2d, with round 1's records. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 BLOCKER, 3 SHOULD-FIX, 1 NIT).
- **Disposition:** all taken in the round's fix commit (named in round 3's records):
  - Taking over the root size commits at the pinned size (1).
  - A hold refused for its modes holds the rest; on the JS page a refused mode pin holds no axis (2).
  - A root size the app may not set is no choice (3).
  - A refused hold commit rolls back whole (4).
  - Tests for the takeover and a smaller pin (5).

I'll review round 2 read-only: the hold moved to the source, against the round-1 records and the T7a base. First I'll read the review notes and the diff, then check the gate, pin, hand-back, and runner hooks.This is a read-only review of the round-2 delta, not a GitHub review. I'll pull the records and the HEAD~1..HEAD diff, then check the gate, pin, and runner hooks against the rulings.The round-1 records are in. Next I’ll read the pin, hold, and hand-back paths in full, including the cases the records said were fixed.Auto-compacting conversation (81% full)...
Conversation compacted.
The review is still open. I'll confirm the hold, hand-back, and gate paths against the source before writing findings.One thing blocks. On the wasm page, a font pin equal to the host size never reaches the laid-out tree.

## 1. BLOCKER — A wasm font pin equal to the host size never lays out

`runner/src/runner/root_font.rs:228`, `runner/src/runner/held.rs:58`, `runner/src/runner/root_font.rs:154`, `kernel/src/kernel.rs:808`, `host/web/dev.js:527`

`hold_facts(Some)` calls `drop_app_root` and returns without a commit. `drop_app_root` stages the host px on `root_font_size_next` and discards the app size. The kernel treats "already equal to `root_font_size()`" as no change, and `root_font_size()` prefers that pending next over the size the tree was laid out at (`kernel/src/arena.rs:234`). `resolve` copies the next into the document only inside a commit (`kernel/src/txn/relative.rs:19`). The wasm document then writes those laid-out px (`host/web/src/document.rs:623` via `css_text`, `relative = false`).

The page asks the runner to hold first, then sets `font-size: <pin>px !important` and calls `exact_set_root_font_size(hostSize())` (`host/web/dev.js:539`). `hostSize` reads the computed style, which the inline `!important` has already made the pin. When that number is the host px just staged, `set_root_font_size` returns `Ok(None)` and commits nothing.

Scenario: the browser root is 16px and the app has called `setRootFontSize(24)` (or `"200%"`, which laid out at 32). The tool pins `{rootFontSize: 16}` and leaves scheme and mode alone. Status reports 16, because `facts` reads the computed root (`host/web/dev.js:434`). The wasm text stays at 24 until some later commit happens to apply the pending next. An idle copy never does. A pin of 32 does commit, because 32 differs from the staged 16. A pin that also changes a mode commits from `exact_set_mode`, and a pin whose scheme bit actually changes commits from the `resize` listener's `exact_resize` (`host/web/glue.js:342`); both of those flush the pending 16 as a side effect. A font-only pin of the host size, or a scheme pin that repeats the scheme the browser already has, does not.

The JS page is unaffected: its layout follows CSS, and the inline `!important` wins. The new Rust test never reaches this path. It pins before the app sets a size (`contract/cli/tests/it/tamber.rs:812`) and asserts `root_font_size()`, which returns the pending next, so it would pass while the document was still at the app's px.

## 2. SHOULD-FIX — A refused pin still becomes the page's hold

`host/web/dev.js:527`, `host/web/dev.js:556`, `host/web-js/dev.mjs:714`, `runner/src/runner/held.rs:48`

The runner checks the axes before it changes the hold, and an unknown axis holds nothing (`held.rs:48` returns before the assignment). The wasm page reports `pin.hold` and then goes on: it installs the new sheet and font, calls `exact_set_mode`, and stores the refused frame in `channel.held`. The JS page sets `exact.held` from the message before `modes.set`, and a refusal there restores the window but leaves `exact.held` in place. The server only checks that mode values are strings without `,` or `=` (`scripts/tamber-builds.mjs:101`), so an unknown axis is delivered.

Scenario: a copy is held at `{scheme: "dark", mode: {dark-style: "black"}, rootFontSize: 32}`. The next pin is `{mode: {dark-style: "grey"}}`. The runner keeps the old hold and returns an error. The wasm page clears the scheme sheet and the font override, puts the mode back to the pre-pin snapshot, and now describes the grey pin in `app`. The runner goes on suppressing scheme, `dark-style`, and the root size from the hold it kept. On the JS page the grey pin never paints, but later `setMode("dark-style", …)` is swallowed because `exact.held["mode:dark-style"]` is set.

## 3. SHOULD-FIX — A refused `setRootFontSize` replaces the choice hand-back will apply

`runner/src/runner/held.rs:138`, `runner/src/runner/commit.rs:609`, `host/web-js/commands.js:8`

`take_held` stores the command's first argument as what the app asked, then deletes the command when the font is held. Validation runs only for a `setRootFontSize` that remains (`commit.rs:612`), and a refusal there changes the live size nothing (`root_font.rs:254`) while leaving the stored ask overwritten. `setMode` does not do this: an unknown axis is rejected before `held_mode` records it (`mode.rs:109`). The JS command site records the value before `appRootFontSize` can refuse (`commands.js:48`).

Scenario: the app has called `setRootFontSize(24)` and a font pin is up. It then calls `setRootFontSize(0)` or `setRootFontSize("nope")`. While the pin holds, nothing is journaled and the canvas stays at the pin. Hand-back runs `app_root_font_size` on `0`, gets `false`, and leaves the host size. The 24 is already gone, because `drop_app_root` took it when the pin began. The same overwrite happens when the font is not held: the refused call is journaled and the live size stays 24, but `asked.root` is now `0`, so a later pin hands back the host size. A call with no argument stores `None` and wipes a previous ask the same way. On the JS page the old `#exact-root-font-size` rule happens to remain, because a refusal does not remove it (`navigation.js:987`); `app.rootFontSize` still says `"0"`.

## 4. SHOULD-FIX — A refused hand-back commit has already dropped the hold

`runner/src/runner/held.rs:63`, `runner/src/runner/admission.rs:39`, `runner/src/runner/commit.rs:187`, `host/web/dev.js:555`

`hold_facts(None)` clears the hold flags, writes the asked window and root size, and pushes `setScheme` / `setRootFontSize` before `commit_again`. The checkpoint is taken inside `commit_again`, so it captures those new values, and `Held` is not in it. `conclude` therefore keeps the cleared hold and the asked window. `set_modes` and `set_root_font_size` roll their own mutation back when that commit fails (`mode.rs:163`, `root_font.rs:163`); this path does not. `batch_for` still drains commands on an error batch (`host/web/src/host.rs:1007`). The page ignores an error from `holdFacts("-")` and clears `channel.held` and the pin CSS anyway.

Scenario: the app has a gated task whose key is not a key, so `gate_step` refuses every commit (`runner/src/runner/gates.rs:35`). Hand-back is one of those commits. The runner is no longer holding, the page paints the handed-back scheme and font, and the wasm tree stays on the pin because `update` never ran. A second hand-back is a no-op (`nothing held`). A healthy app's hand-back commits and is fine.

## 5. NIT — The tests prove the happy path and miss the hole above

`contract/cli/tests/it/tamber.rs:794`, `scripts/dev-copies.test.mjs:178`

The Rust test refuses `grey`, holds all three facts before the app asks, checks that dim / dark / 24 are stored and not applied, and checks that hand-back applies them and that a second hand-back commits nothing. The browser test pins a font only on the JS copy (18, then the app's `"24"`), and the wasm pin is scheme plus mode. `data-x-modes` missing becomes `'none'`, which also satisfies `not.toContain('dim')`. Nothing covers a pin that arrives after the app set a size, a pin px equal to the host px, a percentage or `"medium"` or `"system"`, a `setMode` before the pin, the app choosing the pinned value, a pin with no mode, a refused pin, a refused hand-back, or a non-copy page. The port-80 gate test checks that `exact-copy-2.localhost` is admitted (`scripts/install-page.test.mjs:520`) and does not check `local: false`.

Round 1, against the records. The hand-back that forgot a later choice of the pinned value is fixed: `held_mode` records every valid `setMode`, equal to the pin or not. A launch `setMode` is recorded before any pin, and `app` says it once a pin names that axis. A pin with no mode leaves `setMode` applied, and hand-back restores only axes the last pin named. `"system"` and `"medium"` are real asks. The font hold is fixed when the pinned px differs from the host px. Astra's finding 5 is recorded as not taken: a JS dev reload still checkpoints `ch.before.mode` (`host/web-js/dev.mjs:701`), so a press from before the reload is not what hand-back applies. That limit stands.

What holds:

- The gate admits only `exact-copy-<n>.localhost` with `n` from 1 to 99, at this port, and port 80 also admits the `Host` with the port omitted (`scripts/install-page.mjs:298`). `exact-copy-0`, `100`, a leading zero, the wrong port, `evil.localhost`, and a name that merely ends in the suffix are refused. A copy is never `local`: `local` still requires the host to be `127.0.0.1` or `localhost` (`install-page.mjs:300`). A rebinding `Host` fails the pattern. `*.localhost` is loopback in the browser, not an attacker zone.
- A non-copy (the single canvas, a tab, a frame named `app-own`) does not take the hold. `holdFacts` returns immediately, the sheet has no `!important`, and `app` is omitted. The app's own `setScheme`, `setMode`, and `setRootFontSize` win, which is D3. Native builds do not run these scripts. A frame whose name is `exact-copy-N` is treated as a copy even on the main origin; that is the page's name check (`host/web/dev.js:367`), and `app-own` does not match it.
- While a fact is held, a later `setMode`, `setScheme`, or `setRootFontSize` is recorded and not applied, including a run of them, a choice equal to the pin, and a choice made before the pin arrived. A pin with no mode does not hold `setMode`. Valid percentage forms match `css_percent` and, while held, are removed before they can scale the kernel.
- Hand-back applies the latest recorded choice of each held fact. When the app asked nothing of them, the runner commits nothing and the page puts back `before` for scheme, font, and the axes the last pin set.
- In one commit, `take_set_modes` runs first and `take_held` then removes a held `setScheme` or `setRootFontSize`; other commands stay and are applied once. A gate refusal before those two calls records nothing. The `asked` object is spliced in front of a batch that always starts with `{`, with `quote` / `untyped_json` (`host/web/src/host/mode.rs:78`).
- `app` is omitted when nothing differs from the pin, and the server stores `r.app ?? null`, so a later status clears it (`scripts/tamber-builds.mjs:427`). The bound is 1 KiB of UTF-8 plus the per-field string limits (`tamber-builds.mjs:134`). It is refreshed from the runner when a copy's commit is reported (`dev.js:467`).
- With nothing held, the new hooks are a second `window_with` plus a map insert on a valid `setMode`, and a scan of that commit's new commands. The JS runtime writes `exact.asked` per command. That cost is per command, and it is what lets a pin that arrives after launch know the launch choice.
