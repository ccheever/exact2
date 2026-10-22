# Code review: exact2's menu rule (a popover a menu row opens, opened from the menu's own invoker), round 1, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `unknown`.
- **Method:** one brief (sha256 `9a35f02fdd0cd203144f5014764e3da02e29209928e177bd0853c71d2a49096e`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** all findings taken in bb2bb7835b

VERDICT: **WRONG**

1. **blocker — [MenusMac.swift:300](/tmp/iosfix-rv-mu1/host/apple/Sources/ExactKit/Mac/MenusMac.swift:300): Native menu selection loses the opener before this lookup.**  
   `track` closes the menu before running `afterTracking`; `pick` then calls `presenter.press`, which reaches `show`. By then, `entries` no longer contains the menu, so `source` remains the row. The hidden-ancestor guard at line 307 rejects it. A native context-menu picker therefore does not open. Toolbar menus likewise lack an entry for this lookup. Carry the presentation’s opener through selection. The painted-only test misses this failure.

2. **should-fix — [menu-rows.js:17](/tmp/iosfix-rv-mu1/host/web/menu-rows.js:17): The target is resolved before the row’s press.**  
   If `press` changes `popovertarget` from A to B, the timeout still opens A. If `press` mounts the target, the interceptor initially returns and HTML subsequently opens it nested. Removing or disabling the row also does not cancel the scheduled opening. Defer target resolution and revalidation, not merely `showPopover`.

3. **should-fix — [menu-rows.js:15](/tmp/iosfix-rv-mu1/host/web/menu-rows.js:15): `commandfor` needs the same rule and its existing precedence.**  
   A row with `commandfor="picker" command="show-popover"` bypasses this handler and stays nested. Worse, a row containing both attributes gets its authoritative `commandfor` activation cancelled and its `popovertarget` opened instead. Apple’s `Invocation` already resolves this distinction. The web handler and compiler note should use equivalent resolution.

4. **should-fix — [menu-rows.js:21](/tmp/iosfix-rv-mu1/host/web/menu-rows.js:21): `exactOpener` records attempted invocations, not actual openings.**  
   A menu opened through `showPopover(menu, source=message)` never records that source; its picker opens unanchored or from a stale source. Conversely, a menu row targeting its already-open menu with action `show` overwrites the opener despite performing no opening. The subsequent picker then nests in that menu again. Record the source on actual opening; the existing `beforetoggle` listener in `navigation.js` already observes `e.source`.

5. **should-fix — [menu-rows.js:26](/tmp/iosfix-rv-mu1/host/web/menu-rows.js:26): Changing the source does not remove authored popover ancestry.**  
   If the picker is declared inside the menu, `showPopover({ source: menuInvoker })` still nests it through DOM ancestry. The menu remains open, and hiding it takes the picker along. Apple’s painted paths retain the same relationship through `pop.isDescendant(of:)` and `contains(entry.popover, pop)`. This case needs handling or an explicit restriction in the documented guarantee. [HTML’s ancestor algorithm](https://html.spec.whatwg.org/multipage/popover.html#topmost-popover-ancestor) considers both ancestry and source.

6. **should-fix — [glue.js:21](/tmp/iosfix-rv-mu1/host/web/glue.js:21): This load is not deferred until paint.**  
   `loadAfterPaint` immediately appends a module script; calling it during module initialization can execute `menu-rows.js` before the baked frame. Move installation into the actual activation path and ensure it completes before accepting invocations. The JS target’s static import is part of its existing bundle; the two-file wasm boot checker does not directly govern that bundle.

7. **should-fix — [menu_rows.rs:37](/tmp/iosfix-rv-mu1/contract/lower/src/menu_rows.rs:37): Unknown values produce definite, potentially false notes.**  
   `role=someState` becomes `false` in the map, even when its runtime value is `"menu"`. A dynamic `popovertargetaction`, or literal `"HIDE"`, also passes the “not hide” filter. Distinguish absent, literal and unknown values, and apply keyword case rules. Components themselves are covered because this walks the expanded root. The documentation should say “statically resolvable rows.”

8. **nit — [menu-rows.test.mjs:56](/tmp/iosfix-rv-mu1/host/web/tests/menu-rows.test.mjs:56), [MenuRowPopoverIOSTests.swift:59](/tmp/iosfix-rv-mu1/host/apple/tests/ExactKitTests/MenuRowPopoverIOSTests.swift:59): Replace fixed-duration waits with completion conditions.**  
   The tests wait 50 ms and 200 ms despite the testing guide’s event-based requirement. Coverage also omits Firefox/WebKit, Enter/Space, actual context-menu opening, and a picker opened from a submenu.

Click cancellation itself is sound in [Blink](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/dom/events/event_dispatcher.cc), [Gecko](https://raw.githubusercontent.com/mozilla/gecko-dev/master/dom/events/EventDispatcher.cpp), and [WebKit](https://raw.githubusercontent.com/WebKit/WebKit/main/Source/WebCore/dom/EventDispatcher.cpp). Normal Enter/Space activation reaches `click`; the timeout follows synchronous press dispatch and pointer release.

The submenu role requirement follows the ruling. Line caps are met: **1,493 / 1,500 / 1,500** under `caps.mjs` counting. This was static review; no files were edited and no builds or tests were run.
