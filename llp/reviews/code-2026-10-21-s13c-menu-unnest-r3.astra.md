# Code review: exact2's menu rule (a popover a menu row opens, opened from the menu's own invoker), round 3, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `36fc7c2792`.
- **Method:** one brief (sha256 `2779f42e0ed774b7d364c815f488e1ea506b284b44cebf93d6140225054471bd`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** both blockers and the should-fix taken in f57839b257, the nits taken or declined (a popover written inside a menu stays nested, documented); escalated to theme2 (02:15)

VERDICT: **WRONG**

1. **blocker — [host/web/menu-rows.js:55](/tmp/iosfix-rv-mu3/host/web/menu-rows.js:55): Dialog commands are swallowed.** Every menu-row `commandfor` click is canceled, but `invoked()` only accepts popover commands. A `command="show-modal"` row therefore does nothing. This breaks the existing [“Clear count…” row](/tmp/iosfix-rv-mu3/apps/native-fixture/app.contract:452) on both web targets. Canceling the click suppresses native activation; returning from `act()` cannot restore it. Preserve dialog commands when intercepting activation. [DOM dispatch rules](https://dom.spec.whatwg.org/#concept-event-dispatch)

2. **blocker — [MenusMac.swift:400](/tmp/iosfix-rv-mu3/host/apple/Sources/ExactKit/Mac/MenusMac.swift:400): Native context menus still lose sibling-submenu openers.** `menu(of:)` builds the submenus and appends them to `menuOpener.pops`, then this assignment replaces that list with only the root. After tracking closes, choosing a row from a submenu authored beside the root cannot recover the context source. `show()` falls back to the hidden row and rejects opening the picker. Initialize the opener record before building the menu. The new tests call `menu(of:from:)` directly and miss this context-menu path.

3. **should-fix — [MenusIOS.swift:849](/tmp/iosfix-rv-mu3/host/apple/Sources/ExactKit/IOS/MenusIOS.swift:849), [MenusMac.swift:304](/tmp/iosfix-rv-mu3/host/apple/Sources/ExactKit/Mac/MenusMac.swift:304): The opener walk stops after eight menus.** With nine open painted menus, the picker’s source remains inside the outermost menu. The picker consequently stays nested and closes with that menu. This contradicts the outermost-invoker ruling and [the existing unlimited-depth contract](/tmp/iosfix-rv-mu3/llp/1021-menus.rfc.md:1063). Use cycle detection rather than a fixed hop limit.

4. **nit — [contract/lower/src/menu_rows.rs:86](/tmp/iosfix-rv-mu3/contract/lower/src/menu_rows.rs:86): The compiler note still misses the empty-`commandfor` fallback.** A literal `commandfor="" popovertarget="picker"` opens the picker at runtime but earns no note: `opens()` treats attribute presence as precedence. This statically known case should follow the runtime’s corrected fallback.

Read-only review; no builds or tests run. The added browser suite covers Chrome only, so Firefox/WebKit behavior remains unverified. Line caps pass by inspection: `MenusIOS.swift` 1,496; `glue.js` and `warnings.rs` 1,499 each.

These remaining findings go to theme2 under the round-three rule.
