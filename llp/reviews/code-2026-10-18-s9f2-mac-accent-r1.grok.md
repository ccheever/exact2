# Code review: LLP 1121 stage 9 F2, the Mac's accent cells in a key window, round 1, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the F2 commit.
- **Method:** one brief (sha256 `f47c9700b76b6e82312d372593c9e0215adb866477c23594856a5511f6dfa46a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (the restore never ran on the driver's quit; open -b launches and reopens; provenance in an author-facing why).
- **Disposition:** fixed in rounds 1, 1b and 1c's commits.

---

The default path is unchanged, and the measurement script does put focus back on the exits the driver actually takes. The in-app restore does not run on those exits, and `open -b` does more than activate.

1. **should-fix** — The in-app restore never runs on the quit the driver uses. `host/apple/Sources/ExactMac/main.swift:570`, `host/apple/Sources/ExactKit/Agent.swift:143`, `scripts/agent.mjs:585`.

`applicationWillTerminate` activates `frontBefore`, and the SIGTERM handler does reach `app.terminate`. The measurement does not quit that way. `agent.mjs` closes stdin and, if the process is still up after 2 seconds, sends SIGKILL (`scripts/agent.mjs:585`). Stdin EOF calls `exit(0)` from `exitAfterStorage` (`Agent.swift:143`), including the `quit` op (`Agent.swift:153`). `exit(0)` and SIGKILL both skip `applicationWillTerminate`, so the previous app is not activated by the Mac app on a normal measure run, a hung run, or a crash. `docs/reference.md:716` says it is put back when the app quits. Even on a real terminate, `front.activate()` is `activate(options: [])` while this process is still frontmost, so AppKit can refuse it; the call that steals focus is `activate(options: [.activateIgnoringOtherApps])`. The script's `finally` is what actually restores on these paths.

Fix: call one restore helper, with `.activateIgnoringOtherApps`, from `applicationWillTerminate` and immediately before every `exit(0)` in `exitAfterStorage` (an ExactMac callback, not `terminate`, which `beforeunload` can cancel and which the driver's 2-second SIGKILL can beat). Keep the script `finally` for SIGKILL and crashes.

2. **should-fix** — `open -b` launches and reopens; it does not only activate. `scripts/measure-controls.mjs:154` (bundle id from `scripts/measure-controls.mjs:121`).

`frontApp()` keeps any bundle id `lsappinfo` prints, and the `finally` runs `open -b` with the status ignored. `open -b` with no file asks Launch Services to open that bundle. If that app quit during the run, it is launched again. If it is still running, several apps treat that as reopen: Terminal and iTerm open another window, and this script calls it twice (authored, then plain). `lsappinfo info` also prints `type=` (`Foreground`, `UIElement`, `Background`); a menu-bar agent that was front is opened the same way. A failed `open` is silent, and finding 1 means the app will not have restored either.

Fix: activate only a still-running foreground process (read `type=` and the pid from `lsappinfo`, then System Events `frontmost` or the same `activate(options:)` call). Do not call `open -b`. If that fails, print it.

3. **nit** — The new compiler text is right about the drawing, and it carries a measurement note. `kernel/tables/schema.json:5961` and `kernel/tables/schema.json:5963`.

A disabled checked checkbox or radio under an accent (`scripts/fixtures/control-roles/app.contract:31` and `:33`, `checked=on` and `disabled=true`, accent on the parent column) is now `none`, so `tamber-unreachable` fires. The host sets `contentTintColor` (`host/apple/Sources/ExactKit/Mac/ControlsMac.swift:171`) and then `isEnabled`. The message is `accent-color on macOS disabled checkboxes: AppKit draws a disabled checkbox grey, whatever its contentTintColor (measured in a key window)`. That matches the key-window result (0 pixels, ΔE00 33.5) and the enabled cells staying `exact`, which do not warn. "(measured in a key window)" is provenance in an author-facing diagnostic; best-effort rows stay silent, so this phrase only shows up on these two new `none`s.

Fix: drop the parenthetical. Leave the grey / `contentTintColor` sentence.

Nothing is blocking. With the flag off, `agentMode && !agentActivates` is the old `agentMode` branch: `front()` and the launch path still call `orderFrontRegardless()` and do not activate (`main.swift:338`, `main.swift:724`). `AgentMac`'s `type` path still only `makeKey()`s (`host/apple/Sources/ExactKit/Mac/AgentMac.swift:1024`), which does not activate; with the flag, launch has already activated, and the measure script never `type`s. Documents, extra windows, reopen, and a cancelled quit all go through `front()`, which honors the flag. Relaunch is refused while `EXACT_AGENT` is set. `agent.mjs` copies `process.env` into the Mac process (`scripts/agent.mjs:556`) and does not overwrite `EXACT_AGENT_ACTIVATE`, and a non-production binary keeps `EXACT_AGENT_*` (`ExactEnv.environment`). `lsappinfo info -only bundleid` on this machine prints `bundleID="…"`, which the regex matches. The Mac accent cells in `docs/control-roles-measured.md` match `schema.json` (checkbox, radio, and prominent button `exact`; disabled checkbox and radio `none`; disabled prominent `best-effort`), and `SCHEMA_DIGEST` is repinned. `measure-controls.test.mjs` only checks that fidelity and provenance strings agree. It does not cover restore, `open -b`, or the flag.
