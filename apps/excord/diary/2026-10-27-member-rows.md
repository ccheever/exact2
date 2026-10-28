surfaces: all
agent: Claude Code (Claude Opus 5.5)
task: the member sheet opening on rows after a list's first update empties it

### Rough

- While no member list shows, `members` was read once its list's revision passed 0
  (`min(gw.members, 1)`), so a server whose first op 37 update was an `INVALIDATE` was read
  empty and not again when the `SYNC` filled it: the sheet opened on no rows for a data
  turn (Astra's review of the sheet's gate). Each list now keeps the revision at which its
  rows last changed (`rows`: any op but a presence's `UPDATE`, a new list, or a role, which names groups
  and colours names), and the
  closed sheet's key follows that. Whether the list holds rows was not enough: `INVALIDATE`
  leaves holes rather than shortening it, and a new list replaces the old without emptying
  it first (Astra, Grok).

### Checkpoints

- `bun test` in `apps/excord`: the rows revision moves with every op but `UPDATE`, with a
  new list and with a role; `members.test.contract` 4 of 4 on the web.
