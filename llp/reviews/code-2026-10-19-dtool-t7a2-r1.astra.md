# Code review: LLP 1122 T7a-2 (copies at loopback names of their own; a pinned copy's pin holds over the app's own), round 1, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `2a82f19e03`.
- **Method:** one brief (sha256 `e04570691ea030393fe8f5b427acc6941b1d49feeb6f610eb723e483e7c12e84`), theme2's rulings inline, shared by both reviewers, each blind to the other's review; commit 2a82f19e03. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (4 BLOCKER, 4 SHOULD-FIX, 1 NIT).
- **Disposition:** taken in the round's fix commit (named in round 2's records). The pin no longer re-asserts itself after the app's commit; the host holds the facts at the source:
  - On the wasm page, the runner keeps what the app asks of a held fact and applies none of it: `setMode` on a pinned axis, `setScheme` and `setRootFontSize` (its command taken out, so no host mirrors it). Holding the root size sets aside the app's own size for the pinned host size, in the runner as in CSS. Handed back, the runner applies the app's latest choice of each in one commit (runner `hold_facts`, ABI `exact_hold_facts`, tested in contract/cli tamber.rs) (1, 2).
  - On the JS page, the command sites (modes.js `command`, rt.js `setScheme`, commands.js `setRootFontSize`) keep the app's choice as asked, and apply it only when it isn't held (2).
  - What the app asked is recorded at every call, held or not and equal to the pin or not. So the latest choice is what's handed back, a launch-time choice made before the pin is known, and "system" and "medium" are choices too (3, 6).
  - Handing back puts back only the axes the last pin set (3).
  - Nothing retries, so a refused mode pin can't loop (4).
  - The copy names admit no port at port 80 (8), and `app`'s bound is in bytes (9).
  - The test pins a JS copy's scheme, mode and root size, presses the app's setMode, setScheme and setRootFontSize, checks what holds (including the page's own mode attribute) and what `app` says, then hands back and checks the app's latest of each. The wasm copy does the same for the mode (7).
- **Not taken:** a JS dev reload's checkpoint carrying what the app asked (5). The app's launch choices are asked again when it runs again; a choice made by a press before the reload is not handed back after it. This is recorded as a limit.

**This blocks on pin correctness.** Static review of `2a82f19e03`; nothing changed, and no builds or test suites run.

1. **BLOCKER — The WASM font pin overrides CSS but not the runner.** At [host/web/dev.js:544](/tmp/rv-dt-t7a4/host/web/dev.js:544), the root gets `!important`, but `exact_set_root_font_size` still sets the host size beneath the app’s choice. [runner/src/runner/root_font.rs:134](/tmp/rv-dt-t7a4/runner/src/runner/root_font.rs:134) retains app pixel sizes and multiplies app percentages. With a 16px pin and `setRootFontSize("125%")`, the runner resolves `rem` rows at 20px while status reports 16px. The WASM host emits those resolved sizes, so the content violates the pin.

2. **BLOCKER — Mode correction happens after the app can paint.** [host/web/dev.js:471](/tmp/rv-dt-t7a4/host/web/dev.js:471) and [host/web-js/dev.mjs:712](/tmp/rv-dt-t7a4/host/web-js/dev.mjs:712) defer `holdPin()` through `setTimeout`. An app calling `setMode(..., "dim")` during a frame changes the rendered mode before that timer restores pinned Black. Repeated frame actions can repeatedly paint Dim. Reading status after correction hides this failure; the pin needs to govern the effective mode during the commit.

3. **BLOCKER — Hand-back does not reliably restore the latest app mode.** [host/web/dev.js:456](/tmp/rv-dt-t7a4/host/web/dev.js:456) and [host/web-js/dev.mjs:708](/tmp/rv-dt-t7a4/host/web-js/dev.mjs:708) remember only values differing from the pin. Pin Black, request Dim, let correction finish, then request Black: the saved choice remains Dim, the disagreement stays stale, and hand-back restores Dim. Separately, [host/web/dev.js:551](/tmp/rv-dt-t7a4/host/web/dev.js:551) and [host/web-js/dev.mjs:714](/tmp/rv-dt-t7a4/host/web-js/dev.mjs:714) restore the whole original tuple: a scheme-only pin followed by an app mode change also loses that change on hand-back.

4. **BLOCKER — A refused mode pin creates an endless WASM retry loop.** At [host/web/dev.js:459](/tmp/rv-dt-t7a4/host/web/dev.js:459), `holdPin()` ignores the setter’s refusal. Pin an existing axis to an invalid value such as `dark-style=grey`: the initial handler reports unsupported but retains the pin. Every subsequent retry calls `wasmCall`, whose `x.send()` invokes the commit callback even for the error batch, scheduling another retry. Clear or disable refused mode enforcement.

5. **SHOULD-FIX — A JS rebuild loses the saved app mode.** The checkpoint at [host/web-js/dev.mjs:701](/tmp/rv-dt-t7a4/host/web-js/dev.mjs:701) saves `ch.before.mode`, omitting the newly introduced `ch.appMode`. Pin Black, click Dim, then make a structural edit: the reload carries Black, forgets Dim, and subsequently hands back Black. The latest app choice must survive the development reload alongside the replayed pin.

6. **SHOULD-FIX — `app` misses real disagreements and reports false ones.** [host/web/dev.js:441](/tmp/rv-dt-t7a4/host/web/dev.js:441) and [host/web-js/dev.mjs:708](/tmp/rv-dt-t7a4/host/web-js/dev.mjs:708) infer requests from current DOM state. A launch-time `setMode("dark-style", "dim")` before the Black pin produces no `app.mode`. Explicit `setScheme("system")` and `setRootFontSize("medium")` remove their DOM overrides, so their disagreements also disappear. Conversely, `setRootFontSize(24)` under a 24px pin reports an override despite agreeing. Track the app’s choices explicitly.

7. **SHOULD-FIX — The new tests establish only eventual mode correction.** [scripts/dev-copies.test.mjs:162](/tmp/rv-dt-t7a4/scripts/dev-copies.test.mjs:162) covers six registrations and one post-pin Dim → correction → hand-back sequence. It checks reported mode, without checking what paints. It exercises no app scheme/font commands, percentage fonts, launch timing, repeated choices, refused pins, partial pins, or rebuild while pinned. The `app-own` assertion checks storage keys, not D3 precedence. These omissions allow the blockers above to pass.

8. **SHOULD-FIX — Copies fail on port 80.** [scripts/install-page.mjs:297](/tmp/rv-dt-t7a4/scripts/install-page.mjs:297) requires the literal `:80`, but a browser normalizes `http://exact-copy-1.localhost:80/` and sends a Host without that suffix. The copy receives 421 although ordinary localhost works. The existing port-80 test covers only `127.0.0.1`.

9. **NIT — The documented 1 KiB bound counts UTF-16 units.** [scripts/tamber-builds.mjs:134](/tmp/rv-dt-t7a4/scripts/tamber-builds.mjs:134) uses `JSON.stringify(v).length`. Ten mode values containing 64 CJK characters each pass this check while exceeding 1 KiB in UTF-8. Use byte length if KiB is the intended wire bound.

Apart from the port-80 case, I found no hostname-gate bypass: the pattern is anchored, rejects other suffixes and mismatched ports, and never makes a copy `local`. Separate storage is documented. I found no new D3 regression in non-copy pages; native paths are unchanged.
