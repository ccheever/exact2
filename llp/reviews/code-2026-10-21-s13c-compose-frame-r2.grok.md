# Code review: @exact/actions ComposeFrame (stage 13 C piece 3), round 2, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `5648b6471b`.
- **Method:** one brief (sha256 `c78145f184f3883312f3152e0ec3032101a2ed6e8031948b73be0092fe22a518`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT
- **Disposition:** none needed

**VERDICT: RIGHT**

No findings.

The four round-1 fixes hold, and a root `children` in no part meets D2 and D4.

**Drafts.** `if drafts and not reply` (`packages/actions/compose-frame.contract:84`) leaves the button out of the tree. The bar walk records `display: none` as `fact::HIDDEN` (`host/apple/Sources/ExactKit/HeaderRule.swift:61`) and still places every pressable (`kernel/src/bars.rs:210–224`); the web walk does the same (`host/web/navigation.js:158–166`). Omitting the node is what keeps Drafts off the iOS bar. Mac already drops items that are not `shown` (`host/apple/Sources/ExactKit/Mac/HeaderMac.swift:134`), so this one `if` is the shape that is right on all three. `drafts` and `reply` are no longer read by a `?:`, so they are structure, not D5 variants. The gate covers them with `sample-cf-drafts` and `sample-cf-reply` (`apps/library-gate/app.contract:105–108`). The gallery line matches the two bools leaving the variant set: 20 cells, 0 refused (`scripts/library-gate.test.mjs:84`).

**Slot.** `children` is a view root between the header and the toolbar (`compose-frame.contract:98`). After inlining, one slot root is the header’s next sibling. There is no `body` part: a part is a node (D2), and this placement has no holder. D5 sets `slot: true` only on a part whose node holds `children`. The checker records an empty ancestor chain for a root `children` (`contract/lower/src/anatomy/slot.rs:221–224`, returned at `:282`): one placement, not inside `if` / `match` / `for`, not forwarded. The deny-list has no node to fail. The toolbar after the fill is D4’s stated sibling limit, not a failed check. `slottedIn` finds a slot from the `slot` declaration before `view` (`scripts/library-gate.test.mjs:21–32`), which is why a part-less root still counts (`:46–49`).

**Ring.** Both dash arrays divide by `max(limit, 1)` (`compose-frame.contract:186` and `:196`). The under-arc clamps with `min(max(count, 0), max(limit, 1))`. The over pie clamps the overshoot with `min(count - limit, max(limit, 1))`. `over` is `count > limit` (`:63`), so that numerator is positive on the branch that uses it.

**countInk.** The counter colour is `countInk`, grouped with `over`, `track`, and `arc` under `Counter` (`compose-frame.contract:28–31`) and used at `:176`. The token name stays `actions.compose-count`.

**D0–D9.** `ComposeFrame` is re-exported by the package entry (`packages/actions/index.contract:9`). `ToolGlyph` stays in the file. Lengths and colours are tokens or params; the stroke widths and font weights are the literals the length lint leaves alone. Parts are `bar`, `toolbar`, and `counter`, each on a node. No `exactDelivery`, `exactSurface`, or `exactViewport.displayMode` read. `role="button"` on the languages row is outside D6’s demoted-control list. Post stays the D9 pressable, composed in at `:95`.

**Hosts.** On iOS, scroll-under sets a target only when the header’s next in-flow sibling is a scroll view and is the route’s content scroller, and no projected item writes `appearance: none` (`host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:1034–1040`). With the app’s single slot root as that scroller, the inline title (the empty `aria-level=2` heading at `compose-frame.contract:81`) tracks it. The toolbar is still the last child, so the bottom inset is the same shape as before this round. On Mac the same header rule can take the bar: Cancel, Drafts, and Post are text buttons, and `showable` accepts a custom button whose face is text (`HeaderMac.swift:169–175`). Mac has no next-sibling scroller rule; the page toolbar stays in the page. On the web, a bar is made only when the header is the route’s first element child (`host/web/navigation.js:148–154`), which the multi-root splice keeps when `ComposeFrame` is the route’s content, and the only control the bar replaces is `id="back"`. Linux picks the shallowest vertical scroller, eight levels down (`host/linux/src/presenter/bars.rs:181–208`), and does not look at the header’s sibling.

**Counter.** With `limit > 0`: `count < 0` draws a zero-length arc and the label `limit - count` (`compose-frame.contract:171`), which stays above the limit. The clamp from round 1 is on the arc. `count == limit` draws the full accent ring (dash `78.54`) and the label `0`; the red pie starts once `count` exceeds `limit`. `count > 2 * limit` fills the pie (the ratio is 1) and the label is the negative remainder. `limit <= 0` divides by 1.

**Accessibility.** Cancel and Drafts expose their visible labels. Post’s name is “Publish post” or “Publish reply”. Gallery, camera, GIF, and add-post have labels; their glyphs are `aria-hidden`. The languages control is named “Post language selection” (`compose-frame.contract:152`), with `langLabel` as the visible value. The remaining count is text in the tree; the ring is `aria-hidden` (`:177`). The empty heading is the one heading the bar split needs, and an all-whitespace title is stored as empty (`NavigationBarIOS.swift:231`).
