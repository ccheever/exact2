# Agent drives: a native module's data folder ignores `--storage`, so its files do not survive a relaunch (macOS)

**Status:** Open
**Systems:** agent, host/apple, native modules
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/284

## Current scope

Use persistent named --storage module data/cache roots, reset by FRESH. Check write/relaunch/read and unnamed isolation. Explicitly select temporary-root lifetime rather than assuming durable tmp; check host parity.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

`--storage <name>` gives an agent drive a scratch store "kept between drives: a tree under the cache base on native" (`scripts/agent.mjs:870-874`). An app's storage sources (`app:/data`) follow it, so a drive can relaunch the app and check what it kept.

A native module's own data directory does not follow it. In agent mode, `NativeViews.roots` names the module's data, cache and temporary roots `$TMPDIR/exact-agent-<pid>-<runtime>` (`host/apple/Sources/ExactKit/NativeModule.swift:532-537`). That is per process, whatever `--storage` says. A module's files are gone at the next launch, so the agent cannot check a module's persistence across a relaunch: a draft kept on disk, a cache, settings a module writes.

### Current and expected behavior

- **Current (macOS, agent mode):** with the same `--storage` name, each launch gets a new module data folder (`…/exact-agent-69331-2/data`, then `…/exact-agent-70190-2/data`). A file the module wrote in the first drive is missing in the second.
- **Expected:** under `--storage <name>`, the module's data, cache and temporary roots live inside that named store, beside the app's storage, so they survive a relaunch of the drive. `EXACT_AGENT_STORAGE_FRESH` empties them with the rest. Without `--storage`, a per-process scratch tree is fine, as now.

### Reproduction and evidence

App made with `bun scripts/exact.mjs new <dir>`. Add `"modules": ["probe-view"]` to `app.json`, and `modules/apple/Probe.swift`:

```swift
import Foundation
#if os(macOS)
import AppKit
typealias ProbeView = NSView
#else
import UIKit
typealias ProbeView = UIView
#endif

final class ProbeBox: ExactNativeInstance {
    private let box = ProbeView(frame: .zero)
    override var view: ExactNativeView { box }
}

final class ProbeModule: ExactModule {
    override class var views: [String: ExactNativeFactory] {
        ["probe-view": ExactNativeFactory { _, events in ProbeBox(events: events) }]
    }
    private var file: URL { context.data.appendingPathComponent("probe.txt") }
    override func later(_ request: [String: Any], reply: ExactReply) {
        switch request["op"] as? String {
        case "write":
            do {
                try FileManager.default.createDirectory(at: context.data, withIntermediateDirectories: true)
                try "kept".write(to: file, atomically: true, encoding: .utf8)
                context.changed("read")
                reply.send(["ok": true])
            } catch { reply.fail(String(describing: error)) }
        case "read":
            reply.send(["text": (try? String(contentsOf: file, encoding: .utf8)) ?? "", "folder": context.data.path])
        case let op: reply.fail("no op \(op ?? "nil")")
        }
    }
}
let exactModule: ExactModule.Type = ProbeModule.self
```

`app.contract`:

```text
shape Probe
  text: string
  folder: string
shape Wrote
  ok: bool

component X50Module
  resource probe = readProbe() as shape Probe
  mutation wrote as shape Wrote refreshes probe
  action write
    send wrote = writeProbe()
  view
    main testId="root" padding=24
      column gap=12
        button press=write testId="write"
          text "Write the module's file"
        text `module file: "${probe.text}"` testId="status"
        text probe.folder testId="folder"
```

`app.ts`: `readProbe` returns `await native.later({ op: 'read' })` (after `native.watch('read')`), and `writeProbe` returns `await native.later({ op: 'write' })`.

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Write, then relaunch with the same store | `bun exact.mjs mac`; `bun exact.mjs agent macos --storage x50 "clock settle" "tap write" "clock settle" "tree status" "tree folder"`; then `bun exact.mjs agent macos --storage x50 "clock settle" "tree status" "tree folder"` | macOS 26.6.2, Apple Silicon | main `0365ad1a4` | first drive: `module file: "kept"`, folder `$TMPDIR/exact-agent-69331-2/data`; second drive: `module file: ""`, folder `$TMPDIR/exact-agent-70190-2/data` | the second drive reads `"kept"` from a folder inside the `x50` store | `tree` |

The relevant files are unchanged between `0365ad1a4` and main `e200397ec`.

### Acceptance criteria

- With `--storage x50`, the second drive reads `module file: "kept"`, and its `folder` is the same path as the first's, inside the named store.
- `EXACT_AGENT_STORAGE_FRESH=1` (a fresh store) starts the module's folders empty too.
- A drive without `--storage` keeps a per-process scratch tree and never touches the app's real Application Support folder.
- iOS and Linux agent hosts follow the same rule (not tested here).

### Constraints and related work

- Workaround: none for a module's own files in a relaunch check. A test can replay the module's file through a fixture instead of reading the real one.
- `state` reports `storage: {available: true, store: "x50"}` for the drive (`Agent.swift:267`), but says nothing about the module's roots.
- Not tested: iOS, Linux, and the web's page module.

## Discussion at transfer

### ccheever — 2026-10-08T08:07:22Z

**Decision: Use the named agent store for module roots.**

Keep open for a correctness fix.

--storage must include the native module's data/cache roots across launches; a per-process root defeats persistence verification.

Write in one launch and read in another with the same store; verify FRESH clears it and unnamed drives stay isolated. Define whether temporary roots survive a relaunch instead of implicitly promising durable tmp.
