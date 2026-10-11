---
name: 20261008-x50-agent-module-data-storage
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261005-pr-conversation-and-refresh]
upstream_url: https://github.com/ccheever/exact2/issues/284
reproduced_on: 0365ad1a4 (main)
---

# X50: in agent mode a native module's data folder ignores `--storage`

Moved to main `issues/20261009-agent-native-module-storage-roots.md` (2026-10-09); tracked there.

Numbering note: the closed #260 branch (`feat(example)/t3-code-portable-app-download`) used X50
for "a distributed macOS build without the build machine's paths". That draft never reached this
branch. On this branch X50 is the agent's module data folder, as `EXACT2-GAPS.md` has it.

## Summary

`--storage <name>` keeps an agent drive's app storage (`app:/data`) between drives on native
hosts. A native module's own data, cache and temporary roots do not follow it: in agent mode they
are per process (`$TMPDIR/exact-agent-<pid>-<runtime>`), so a module's files (the clone's
`t3-code.json`) are gone at the next agent launch.

## Why it arose

Agent relaunch checks of the pull request panel's kept detail
([pr-conversation-and-refresh](../../tasks/closed/20261005-pr-conversation-and-refresh.md)) and U6's relaunch row
needed the module's file to survive an agent relaunch. A normal launch keeps it in the app's data directory, so
users are not affected.

## Clone workaround

Nonblocking: the kept file was replayed through the relaunch path offline, and `T3Storage.dataRoot(agent:)`
(`T3Protocol.swift`) re-roots the module's data under the launcher's `TMPDIR` in agent mode. Once modules get the
named store's roots, check whether that re-rooting still applies, and run the kept pull request detail and U6
relaunch rows in agent mode with `--storage`, reading the module's file instead of replaying it.

## Evidence and history

- Reproduced on main `0365ad1a4` with a scratch app (`bun scripts/exact.mjs new`) whose Swift module writes
  `probe.txt` in `context.data`, driven with `--storage x50` and relaunched with the same store: first drive
  `module file: "kept"` in `$TMPDIR/exact-agent-69331-2/data`; second drive `module file: ""` in
  `$TMPDIR/exact-agent-70190-2/data`.
- Filed as [#284](https://github.com/ccheever/exact2/issues/284) on 2026-10-08 (relevant files unchanged on main
  `e200397ec`); no duplicate found.
