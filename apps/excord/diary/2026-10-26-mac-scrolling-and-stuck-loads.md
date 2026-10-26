surfaces: macos
agent: Claude Code (Claude Opus 5.5)
task: Mac scrolling against Discord's app, a gateway stuck on "Connecting…", a channel that never loaded

### Rough

- On a busy Mac the first gateway answer after a megabyte READY took 868 ms, over the data
  module's 100 ms turn. The host refused it, and nothing asked the gateway again, so the app
  sat on "Connecting…" until relaunch. It took a trace (`kill -USR1`) to see it: the refusal
  is in the journal, not on screen. Fixed in Excord (`regateway`, and large frames are applied
  in `digest`), over three review rounds.
- One server's #general never loaded. Its window answer held a lone UTF-16 surrogate (a reply
  preview cut with `.slice(0, 100)` through an emoji), which the Apple host's reader refuses
  and the browser accepts, so the web build hid it. The refused window also left the page
  unanswered, and it was asked of Discord again every few hundred milliseconds. The cuts are now
  by code point (`clip`), and a page asked three times stops with Retry.
- The contract tests could not show either: `request` sends an escape as text, so the fixture
  cannot hold a lone surrogate, and the web host reads one without complaint.
- Three test lanes were broken on main for reasons outside this work (QUEUE.md): `bun test`
  (the Translate package's import), the web contract run (the fixture's port moved), and four
  Mac contract tests.

### Lean in

- Turning LLP 1072's asynchronous fills on for macOS was one flag, and an A/B of two signed
  copies (one with `EXACT_FILL_SYNC=1` in its `LSEnvironment`) let the person compare them side
  by side. In the traces, none of the new build's last 600 frames was late; the old build's p99
  was 143 ms.
  It is not landed: three review rounds closed two ways a newer layout could land before a
  resize waiting to apply, and found a third (a focus handler's nested apply; QUEUE.md).

### Needed

- scrolling: partial; the main thread still waits on the owner during a scroll's collection
  flush (about a fifth of it), where the kernel lays out again. The jitter at the bottom is not
  in the trace's frames; a screen recording is asked for.

### Checkpoints

- editing app.contract / live.ts: smooth.
- macOS build and drive: smooth; a second instance on the same storage wedged at launch.
- web and macOS contract runs: rough; see above.
