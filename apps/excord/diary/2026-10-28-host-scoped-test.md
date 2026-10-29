surfaces: web
agent: Claude Code (Claude Opus 5.5)
task: mark a test that can only pass on one host

### Rough

- The test "a user action before the launch's restore lands wins: Back to the list cancels
  it" can only pass on the web: a web relaunch keeps the channel's address with the list
  beneath it, and a native relaunch starts at the list with nothing to go Back to. A
  comment said so, but a native run of the file would fail the test. The test language
  had no host scope; it has one now (LLP 1146 D7).

### Lean in

- `test "…" on web` makes the driver skip it elsewhere and say so
  (`test "…": skipped on macos (on web)`). Its host-neutral twin stays in
  `routing.test.contract`.

### Needed

- none

### Checkpoints

- `contract test apps/excord/app.test.contract` carries `"hosts":["web"]` for that test
  alone. The Excord suite was not run for this change.
