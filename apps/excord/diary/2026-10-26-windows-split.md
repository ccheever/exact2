surfaces: windows, linux
agent: Claude Code (Claude Opus 5.5), Codex (GPT-6 Astra)
task: show the channel list beside the open channel on a wide Windows window

### Rough

- On Windows at 1100×760 Excord pushed the channel over the list, as on a phone. The Mac and
  the web already split; the painted presenter that draws Windows and Linux had no split at
  all, so the fix was in exact2 (`host/linux/src/presenter/split.rs`, LLP 1115.004 D16), not
  in the app.
- The list route keeps the window as its viewport and is narrowed by a host cover, unlike the
  Mac, where the page is the list's column. A cover computed from the route's laid-out width
  grew on every sync in Shelf; it is derived from the containing block instead.
- Driving it on the NucBox: another session's fixture held port 4790 and the app kept
  connecting to it ("Reconnecting…"). Building with `EXACT_BACKEND_ORIGIN` on a free port and
  passing the same `--backend` to the agent fixed it. Screenshot paths with spaces fail there.

### Lean in

- Excord needed no change to split: `navigationSplit="placeholder"` on `no-channel` and the
  action rows that push a channel were enough, as on the Mac. Selection follows the row that
  pushed the channel, and Alt+Left returns to "Choose a channel".

### Needed

- Android does not split yet (LLP 1115.004 D16 proposes 840 dp).
- After a pointer tap on a row while the composer had focus, the row showed a focus ring.

### Checkpoints

- Windows build and drive (`bun host/windows/build.mjs excord`, `scripts/agent.mjs windows`):
  smooth once the fixture port was free.
- Linux headless drive of Shelf for the presenter's tests: smooth.
