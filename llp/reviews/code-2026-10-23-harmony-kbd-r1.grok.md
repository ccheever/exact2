# Code review: Harmony transcript keyboard (on-drag removed), quick review, 2026-10-23 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit, Write and Bash denied, a throwaway worktree at `2ecf80136d`.
- **Method:** one brief (sha256 `c4c4a8da22a12440b3f798452ebfffa487275642c036e87802d55849fbda46af`), blind to the other reviewer; one quick round (the lead's 00:58Z: a one-line Harmony change). The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix and two lines of opening narration before it.
- **Note:** landed as f7e0fce73f (the same change rebased); both NONE

**NONE.** The transcript list still lowers, and with `keyboardDismissMode` absent iOS gives that list `.interactive`. Web, Android, and Mac do not read the prop, so dropping it does not change them.

### 1. Contract — NONE
`apps/harmony/app/app.contract:742-744`

```742:744:apps/harmony/app/app.contract
        list id=`transcript-${routeKey}` appearance="none" virtualized=true scroll-start="end" scrollFollowEnd=true flex=1 min-height=0
          padding-bottom=8
          for m in thread.messages key=m.id
```

`padding-bottom=8` sits two spaces under `list`, at the same depth as `for`. That is a continued attribute line: the parser takes `name=` lines before children (`contract/syntax/src/parser/view.rs:193-209`). `padding-bottom` lexes as one identifier (`contract/syntax/src/lexer.rs:247-264`), and a numeric inset is legal on a virtualized list (`contract/lower/src/collection.rs:316-318`). `scroll-start="end"` stays on `list virtualized=true` (`collection.rs:191-194`).

The compiler was not executed; this is the grammar and the source as they stand.

### 2. iOS dismissal — NONE
`host/apple/Sources/ExactKit/IOS/KeyboardAvoidanceIOS.swift:257-263`

With the prop absent, the default arm is `scrollsVertically ? .interactive : .none`. This list scrolls vertically:

- `scrollsVertically` is `overflow_y` of `scroll` or `auto` (`NodeViewIOS.swift:284`).
- A `list` scrolls on its block axis unless an overflow row says otherwise (`kernel/src/node.rs:161-162`). Nothing here sets overflow, so the style sent to the host is `overflow_y: "auto"` (`host/apple/src/style.rs:718-732`, written at `1072-1084`).
- `scroll-start="end"` only opens at the last row (`runner/src/instance/collection/mod.rs:348-349`). The axis stays vertical because `display` is not `flex` (`mod.rs:328-334`).

The virtualized path uses that node's plain `ScrollView` (`CollectionIOS.swift:43`, `node.scroll`), not a grouped collection. `appearance="none"` is not a grouped list (`contract/lower/src/grouped.rs:73-85`). `syncScroll` builds that scroll view and then calls `updateKeyboardDismissal()` (`NodeViewIOS.swift:1285-1328`), and `scrollView` is that same view when there is no native collection (`NodeViewIOS.swift:283`).

So the mode written on this list is `.interactive`, not UIKit's `.none`.

### 3. Web, Android, Mac — NONE
`keyboardDismissMode` is only applied in the iOS host. The web host has no reader. Android still does not implement it (`host/android/README.md:305`). Mac has no `keyboardDismissMode` assignment. Absence leaves each of those hosts on the behavior they already had.
