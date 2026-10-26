# Code review: exact2's menu rule (a popover a menu row opens, opened from the menu's own invoker), round 2, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `unknown`.
- **Method:** one brief (sha256 `2fd74a160be639d238d5a0fed8ba2657069f24865447f31ab6d514dd3c3d73ca`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** all findings taken in 2498455129 and 36fc7c2792

VERDICT: **WRONG**

1. **blocker — [MenusIOS.swift:1455](/tmp/iosfix-rv-mu2/host/apple/Sources/ExactKit/IOS/MenusIOS.swift:1455): opening an already-open target creates recursive dismissal.** The new native action calls `agentShow` without checking whether the picker is open. This happens when its press already calls `showPopover`, or when it targets an open manual popover. `agentShow` then selects the picker itself as its ancestor: UIKit’s [`isDescendant(of:)` includes identity](https://developer.apple.com/documentation/uikit/uiview/isdescendant%28of%3A%29). It stores `ancestor == name`, and `drop(name)` recursively calls itself before removing that entry. Preserve show/toggle semantics and prevent duplicate lifts; test dismissal after these cases.

2. **blocker — [MenusMac.swift:301](/tmp/iosfix-rv-mu2/host/apple/Sources/ExactKit/Mac/MenusMac.swift:301), [MenusMac.swift:691](/tmp/iosfix-rv-mu2/host/apple/Sources/ExactKit/Mac/MenusMac.swift:691): the native-menu fallback still misses valid selections.** A row in a submenu authored beside its parent is not a descendant of `menuOpener.pop`, which records only the root menu. Separately, a menu opened by `showPopover(menu, source=message)` passes `nil` to `menu(of:from:)` when `message` does not declaratively target it, so its opener is never recorded. After native tracking closes, both paths retain the hidden row as source and fail `show`’s visibility guard. Carry the actual presentation source through the selected item’s submenu path.

3. **should-fix — [menu-rows.js:27](/tmp/iosfix-rv-mu2/host/web/menu-rows.js:27): post-press resolution only runs for clicks accepted before the press.** If the row’s press mounts its picker, or changes its target from a submenu to a picker, this early return skips interception. The browser subsequently resolves the new target and opens it nested in the menu. [HTML activation follows the event listeners](https://dom.spec.whatwg.org/#concept-event-dispatch); the rule must cover targets made available by those listeners too.

4. **should-fix — [menu-rows.js:34](/tmp/iosfix-rv-mu2/host/web/menu-rows.js:34): intercepted toggles cannot close their target.** With a manual picker already open, choosing its menu row prevents native activation and then returns because the picker is open. Default `popovertarget` and `command="toggle-popover"` therefore leave it open instead of closing it. A press that changes the action to hide is likewise swallowed. Retain and execute the resolved action, changing the source only for an opening. [Popover activation rules](https://html.spec.whatwg.org/multipage/popover.html#popover-target-attribute-activation-behavior).

5. **should-fix — [menu-rows.js:15](/tmp/iosfix-rv-mu2/host/web/menu-rows.js:15): `commandfor` presence is not HTML’s precedence rule.** For `commandfor=""` or a missing target ID alongside `popovertarget="picker"`, HTML falls back to `popovertarget`; this resolver returns no target. The browser consequently opens the picker without the menu rule. Resolve the associated element before choosing the command path, as the existing native `Invocation` does. [Button activation rules](https://html.spec.whatwg.org/multipage/form-elements.html#the-button-element).

6. **should-fix — [menu_rows.rs:60](/tmp/iosfix-rv-mu2/contract/lower/src/menu_rows.rs:60): conditional targets still produce definite false notes.** Both arms of `if`/`match` enter one map, and the last occurrence of an ID wins. If one arm declares `id="picker" role="menu"` and another declares the same ID without a role, the latter makes every reference receive the non-menu note—even when the menu arm is active. Conflicting possibilities should become `Unknown`; add a conditional-target test.

Read-only review; no tests executed. Browser coverage added here is Chromium-only. The stated caps hold: **1492 / 1499 / 1499** lines.
