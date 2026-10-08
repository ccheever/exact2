---
name: 20261008-x50-agent-module-data-storage
plan: 20261005-t3code-macos-parity
status: published
kind: framework-gap
blocks: [20261005-pr-conversation-and-refresh]
upstream_url: https://github.com/ccheever/exact2/issues/284
reproduced_on: 0365ad1a4 (main)
---

# X50: in agent mode a native module's data folder ignores `--storage`

Numbering note: the closed #260 branch (`feat(example)/t3-code-portable-app-download`) used X50
for "a distributed macOS build without the build machine's paths". That draft never reached this
branch. On this branch X50 is the agent's module data folder, as `EXACT2-GAPS.md` has it.

## Summary

`--storage <name>` keeps an agent drive's app storage (`app:/data`) between drives on native
hosts. A native module's own data, cache and temporary roots do not follow it. In agent mode
`NativeViews.roots` names them `$TMPDIR/exact-agent-<pid>-<runtime>`
(`host/apple/Sources/ExactKit/NativeModule.swift:532-537` on main `0365ad1a4`). That is per
process, so a module's files (the clone's `t3-code.json`) are gone at the next agent launch.

## Where the clone hits it

Agent relaunch checks of the pull request panel's kept detail (pr-conversation-and-refresh): the
kept file was replayed through the relaunch path offline. A normal launch keeps it in the app's
data directory, so users are not affected.

## Reproduction on main

Scratch app (`bun scripts/exact.mjs new`) with a Swift module whose `later` op `write` writes
`probe.txt` in `context.data` and whose `read` op returns it and the folder. Drive with
`--storage x50`: `"tap write"`, then relaunch with the same store. First drive: `module file:
"kept"` in `$TMPDIR/exact-agent-69331-2/data`. Second drive: `module file: ""` in
`$TMPDIR/exact-agent-70190-2/data`. Expected: the second drive reads `"kept"` from a folder
inside the named store. The issue body carries the full module, contract and commands.

## Upstream (filed 2026-10-08)

Upstream: https://github.com/ccheever/exact2/issues/284 (#284, [Bug] Agent drives: a native
module's data folder ignores `--storage`, so its files do not survive a relaunch (macOS)).
Reproduced on main `0365ad1a4` (relevant files unchanged on main `e200397ec`). Searched: native
module data, agent storage (issues and PRs): no duplicate.

Next: issue-close once #284 lands; then the pull request panel's relaunch check can read the
module's file instead of replaying it.

## Decided upstream (2026-10-08): waits for main fix of #284

[Charlie on #284](https://github.com/ccheever/exact2/issues/284#issuecomment-6055581761): "Use the named agent store for module roots."
- Waits for main fix of [#284](https://github.com/ccheever/exact2/issues/284), then an adoption round. Then the pull request panel's relaunch row and U6's relaunch row run in agent mode with `--storage`.
