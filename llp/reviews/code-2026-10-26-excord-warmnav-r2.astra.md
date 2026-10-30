# Code review: Excord warm address-following, round 2, 2026-10-26 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, `-s read-only`, a throwaway worktree at `86e0d85457`.
- **Method:** one brief (sha256 `96af19cad19259441167a39c7c3e43c2eaaa374ef792dbe632d8209d19ff94cf`), blind to the other reviewer. The author (Claude, x2-perf4) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

- **MAJOR — The duplicate-transcript fix leaves UIKit’s Back destination blank.** [app.contract:1217](apps/excord/app.contract:1217) removes the beneath-visit’s content. Open A, follow B, then hold an edge swipe back toward A: UIKit reveals that empty controller. Contract Back runs only after transition completion ([NavigationIOS.swift:1212](host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:1212), [NavigationIOS.swift:1333](host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:1333)). The duplicate IDs are eliminated, but the native presentation is broken.

- **MAJOR — Following the already-open channel cancels B but leaves B’s address selected.** [app.contract:982](apps/excord/app.contract:982) calls `supersede()` and optionally jumps; neither restores the address nor rejoins the live view. Scenario: A is open, navigate to `/c/B`, then follow A before B opens. The final screen shows A under `/c/B`; reload opens B. Because `askRoute(B)` called `leaving()`, live-view bookkeeping also remains empty. The new test checks `open`, `chan.name`, and `pendingOpen`, but misses both problems ([routing.e2e.test.ts:219](apps/excord/fixture/routing.e2e.test.ts:219)).

- **MAJOR — Wide navigation to `/` still discards composer state although the channel should stay open.** [routing.contract:70](apps/excord/routing.contract:70) unconditionally calls `leaving()` for the list. At 1100×760, edit a message in A and navigate to `/`: `leaving()` clears the unsaved edit ([app.contract:549](apps/excord/app.contract:549)), then `resume` reopens A. That reopen also clears replies and attachments ([app.contract:458](apps/excord/app.contract:458)). The new wide test checks only the final channel, masking this close/reopen cycle.

- **MAJOR — Sign-out leaves a cancelled request’s server persisted.** [app.contract:530](apps/excord/app.contract:530) invalidates the intent without restoring `wantServerWas`. Scenario: A belongs to guild G; a pending DM request selects `@me`; sign out before it opens. The persisted `server` remains `@me`, and signing back in wide lets `resume` open a DM from that cancelled selection. Rollback exists only in `ask` and `supersede` ([routing.contract:40](apps/excord/routing.contract:40)). Explicit server choices are correctly protected by `pickServer` invalidating the old intent.

- **MINOR — Decoding happens after destructive same-channel handling.** [routing.contract:64](apps/excord/routing.contract:64) compares the raw segment with `open`. While editing channel `119…`, navigate to `/c/%3119…`: the comparison fails and `leaving()` discards the edit. [routeDecoded:96](apps/excord/routing.contract:96) eventually resolves the correct channel, but cannot undo those effects.

- **MINOR — Two race tests still establish only the final result.** The Forward-then-link case ([routing.e2e.test.ts:241](apps/excord/fixture/routing.e2e.test.ts:241)) and browser-Back cancellation case ([routing.e2e.test.ts:262](apps/excord/fixture/routing.e2e.test.ts:262)) still use `data()`/`settle()` followed by end-state assertions. A transient cancelled open or persistence write would escape them. The new millisecond observations improve three other cases.

The remaining delta claims check out structurally: `follow` no longer writes `server`; checked `selectWant` records its previous value; `askFollow` no longer calls `leaving`; same-open address/Back sends one rejoin; and the routing comment now cites the host implementations. I found no additional rollback error for an explicit server choice or a tap in the newly selected server’s list, and no duplicate `viewChannel` send within those actions.

Approval conditions:

| Condition | Assessment |
|---|---|
| **(1)** Address feedback | The host prevents feedback without a timed flag: [navigation.js:254](host/web/navigation.js:254), [NavigationRules.swift:49](host/apple/Sources/ExactKit/NavigationRules.swift:49). This delegates suppression to the host rather than implementing the specified app address/sequence marker. |
| **(2)** Resolved persistence | `want` is transient; `onScreen` comes from `open` ([routing.contract:128](apps/excord/routing.contract:128)). The separate persisted-server leak remains above. |
| **(3)** No extra history | Channel resolution uses `replace`, preserving the visit ([app.contract:495](apps/excord/app.contract:495)). |
| **(4)** Draft survives | Per-channel draft storage and the A→B→A test remain intact ([app.contract:693](apps/excord/app.contract:693), [app.test.contract:472](apps/excord/app.test.contract:472)). |
| **(5)** Full gate | **Unverified at HEAD.** Relaunch and scroll tests remain present, but the available gate logs predate HEAD. |

Read-only verification: two navigation utility tests passed; all eight routing e2e tests skipped because this checkout lacks the web build. Native findings are source-traced, not simulator reproductions. Worktree unchanged.

**REQUEST CHANGES**
