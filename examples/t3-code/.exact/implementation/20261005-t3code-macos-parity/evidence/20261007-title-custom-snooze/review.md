# title-custom-snooze: independent review

Reviewer: a separate agent (read-only), given the ticket, the staged diff against `07dcef1ab`, the
reference paths, the evidence and the earlier attempt's history; not told an expected verdict.

## Round 1 — the tree of runner attempt 1 (digest `76a010a3…`): FAIL

- B1 (blocking): "Unavailable actions" was claimed from builder unit tests only. `MenuRow` bound
  `hover` without regard to `entry.disabled`, the hover opens a submenu, and the macOS host and the
  runner deliver hover to a disabled node; the reference gives a disabled row no listeners and
  `pointer-events: none` (`contextMenuFallback.ts:336-348, 398`) and its `snoozeThread` checks
  `canSnooze` before sending (`useThreadActions.ts:900`). Before this change a Custom… under a
  disabled Snooze did nothing; with it, it would open the dialog and could send `thread.snooze`.
  (From reading the code; not reproduced live.)
- Non-blocking: the route, the confirm without navigation, the error path keeping `from`, and the
  focus return were judged correct and covered; `titleUi("close")` focused the title even after a
  right-click opened the menu (the reference restores what held the focus); a pending command drops
  the request silently, as the row does; no unit test covers `titleDialogClosed` or `expand` (drives
  do); the staged record overstated "each fails on the base sources" and the Outcome still said "No
  fix is included" (both since corrected); rules and checks met.

Repairs: `MenuRow` `point` (a disabled row hears no pointer), the `snooze()` guard with the
reference's message (test added), `titleMenuFromTitle` (Escape after a right-click leaves the focus
where it was), the record wording; the app rebuilt and re-driven (Retry section of
`agent-drives.txt`); runner attempt 2 (digest `53a6e426…`) passed.

## Round 2 — the tree of runner attempt 2 (digest `53a6e426…`): PASS

The reviewer checked that `hover=point` still receives the hover's `over` flag (a bare action takes the
one-boolean payload, `docs/contract-grammar.md:700`, as `chat.contract`'s `setHover` does; retry drive
1 shows an enabled Snooze still opens on hover), that a disabled row now opens nothing by pointer or
key, that the `snooze()` guard matches `snoozeThread` for every caller (row presets and Custom via
`park`, bulk via `snoozeMany`, the title confirm) and runs before any state change, and that the guard
test fails with the guard removed (scratch copy of the staged tree). Checks: 68 focused tests pass;
`checks2.log` five checks exit 0; `attempt2-report.json` passed; the drives and checks ran after the
last source edit.

Non-blocking, recorded: no live capture of a disabled Snooze (blocked; a queued turn start inside the
2-minute grace window also disables Snooze and might be reachable without a provider login — not
tried); after a right-click-opened menu, Custom…'s dialog still returns the focus to the title rather
than to what held it before (restoring an arbitrary earlier focus is X53's host modal); a row that
becomes disabled under the pointer keeps its highlight until the pointer moves (cosmetic); the title
menu's presets (`chat:snooze`) still bypass `snooze()` and its guard (already out of scope).
