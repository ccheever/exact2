# Code review: context menus with a preview and a commit (LLP 1021 §5.1), round 2, 2026-10-05 (grok)

- **Family:** xAI. `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless, `--cwd /private/tmp/x2-peek-review`; read-only by instruction.
- **Method:** one brief (sha256 `9207e12c127f73a1a01365a25739bb99025f64ac629a705ec47f080b55ec5c4d`), shared with astra. Round 2, blind to astra's round-2 review. Reviewed the uncommitted diff in a detached worktree at `a51d6ecab`. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY WITH CHANGES.
- **Disposition:** the origin-zero frame taken (astra's 4). `unanimated` cleared in `NavigationHost.reset`, taken.

---

[P2] Record a same-size frame at the origin as the preview's home — host/apple/Sources/ExactKit/IOS/ContextMenusIOS.swift:107

`place` keeps `home` only when `preview.frame` differs from `placed`. `placed` is the rect `place` itself last wrote: origin zero and the current size (`ContextMenusIOS.swift:108`). A kernel frame of `(0, 0, currentWidth, currentHeight)` — the row moved onto the origin without changing size — compares equal, so `home` stays at the previous box and `restore` writes that box back (`ContextMenusIOS.swift:205`). A resize at the origin still updates `home`, because the size differs. The lifetime test only moves a row whose origin stays `(8, 8)` (`ContextMenuIOSTests.swift:193`). §5.1 says any frame the batch gives the row, including one at the origin, is the new home.

[P3] Clear `unanimated` when the navigation host resets — host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:780

A preview commit sets `unanimated` and leaves it set when `sync` returns early behind a transition (`ContextMenusIOS.swift:242`, `NavigationIOS.swift:326`). The flag is cleared only when a later `sync` reaches `NavigationIOS.swift:352`. `reset`, which `unmounted()` and `PresenterIOS.reset()` both call, clears `changing` and `pendingSync` and leaves `unanimated` true. The next `setViewControllers` on that host (`NavigationIOS.swift:347`) then runs with its animation off. A commit that lands during a transition, followed by the session view leaving or a presenter restart before `didShow` applies the deferred sync, suppresses the next stack animation.

## Assessment

**READY WITH CHANGES.** The round-1 fixes hold on the paths they cover, and the two holes above are what is left.

Taken and fixed: a configuration is reported only after a provider has run, and `willEndFor` matches that configuration object. A source that is gone or stops naming the popover dismisses the menu and restores the row immediately, except while a commit animation is holding it (`ContextMenusIOS.swift:74`). Restore targets the popover's container as it is then when the saved container is the one the row left. The commit walks the popover's authored ancestry, and an inert popover refuses the press. The dismissal target is dropped only when `activeKey` changed. `unanimated` stays set across a `pendingSync` defer and is cleared when that sync applies the push. A children op that moves the row records the new parent. Both web listeners (`rt.js` `cp`, `glue.js`) read `contextpopover` before `preventDefault` and resolve the popover inside the timeout, after the node's own action.

Deferred, and the deferral stands: the node pool still refuses these rows because of the interaction. §5.1 and `QUEUE.md` already say so.

The tests call the delegate directly. They do not drive UIKit's provider closures, `setViewControllers`'s animated flag, a drop with no new parent, a material container swap, or macOS `MenuHost.context` / `rightMouseDown`. Those gaps are already named in §5.1's evidence.
