# A precise timer during a fling, in compiled Messages

This measures whether Messages' `task network mount` / `every(250, poll)` runs while its transcript is flung, on an iOS simulator, at commit `b0ce559698ebafc8a2b91c47e6efcc83979670a0`. The simulator was an iPhone Duo on the iOS 27.1 runtime under Xcode 27.1, created for the run and deleted after it. It presented frames at 60 Hz (period 16.67 ms). The Mac's load average was between 150 and 700 during the runs, so frame lateness here is indicative only (LLP 1144 D1); the commit and per-site counts are what hold.

## Under the agent, the timer does not run

`scripts/fling.mjs` launches the app on the agent's socket. Under the agent (`EXACT_AGENT=1`), even with platform timing, the driver owns the runner's clock: `SessionClock.scheduleClock` arms no wake, and `fireClock`, `advanceIfDue` and the display link's timer advance all return under `ExactEnv.agentMode`. `agent-probe.mjs` shows it: the runner's clock stayed at 12,719 ms, and the `network` task's next deadline at 12,959.69 ms, across 5.4 s of wall time that included a real fling of the transcript (`results/agent-probe.json`, `states`). Two `fling.mjs` runs of six flings each (`results/fling-agent-2.json`, `results/fling-agent-3.txt`) report no rows created, no handler dispatches and no work, and still 21 late frames each, with 187 to 216 ms waiting on the runtime and 708 to 923 ms unattributed: that lateness has no timer in it. A third run (`results/fling-agent-1-failed.txt`) stopped when a real touch landed on another node than its aim. So the fling report cannot show a timer's batches today.

## Without the agent, every beat lands in a frame of the fling

`fling-free.mjs` launches the development build with `simctl launch` and no `EXACT_AGENT`, opens the thread "demo", plays the same flings through the XCTest touch runner at the points `agent-probe.mjs` recorded, and saves a trace by `SIGUSR1` before the flings and 1.5 s after each one. The runner's accessibility snapshot after each fling shows the transcript moving about 47 messages each way (`fling-free-log.json`, `positions`; a simulator screenshot of a Duo is black). `analyze-free.mjs` reads the frames in the 1.5 s after each touch and the per-site work between traces; `analyze-late.py` relates every late frame in a run to the frame before it.

Three runs gave 22 flings and 1,970 sampled frames in those windows (`free-1/`, `free-2/`, `free-3/`, each with `analysis.txt` and `analysis.json`). Every frame applied one batch, because Messages' `every(frame, animate)` commits once a frame. Every 250 ms, on every 15th frame, that batch held two commits: the `poll` beat and the frame task's. That is six beats per fling, 132 in all, and all 132 came in the display link's batch (one batch, two commits), not as a batch of their own from `SessionClockTimer`'s run-loop timer.

Each beat re-evaluated the `MessageRow` site at `bubble.contract:76` once for every message in the thread (153 per beat in `free-1`, 453 in `free-2` and `free-3`) and the sites at `app.contract:309`, `316` and `317` once, `app.contract:354` three times and `bubble.contract:66` and `87` twice. Almost none of it changed anything: of 134,249 evaluations between the traces, 2 changed a value (`bubble.contract:87`, once each in the first fling of `free-1` and `free-2`), and every frame's `apply` was 0 ms.

Five frames in the windows were late. Four of them were the frame right after a beat's frame, where about 7 % of frames follow one, and they waited 14.6, 28.6, 49.4 and 86.7 ms on the runtime. Over the whole runs, set-up and rest included and trace saves (intervals over 300 ms) left out, 30 of 46 late frames followed a beat's frame, with runtime waits from 3.4 to 137.7 ms (`results/late-frames.txt`). The beat's own frame was on time; the wait came in the next frame's synchronous runner call, which is consistent with the beat's follow-on work (its `heartbeat` send and the `data` refresh it causes) still running on the runtime thread, though nothing here traces that.

## Running it

```sh
export DEVELOPER_DIR=/Applications/Xcode-27.1.app/Contents/Developer EXACT_SIM=<udid>
bun host/apple/build.mjs --ios messages-apple --sim $EXACT_SIM
bun experiments/timer-wait-points/compiled/agent-probe.mjs experiments/timer-wait-points/compiled/results/agent-probe.json
bun scripts/fling.mjs ios messages --target transcript --to "tap conversation-demo" --to "wait 1500" --to "tap add" --to "wait 800" --to "tap add-fifty" --to "wait 1500" --flings 6
bun experiments/timer-wait-points/compiled/fling-free.mjs --sim $EXACT_SIM --aims experiments/timer-wait-points/compiled/results/agent-probe.json --out experiments/timer-wait-points/compiled/results/free-N --flings 6 --positions
bun experiments/timer-wait-points/compiled/analyze-free.mjs experiments/timer-wait-points/compiled/results/free-N
python3 -I experiments/timer-wait-points/compiled/analyze-late.py experiments/timer-wait-points/compiled/results/free-N
```

The `fling.mjs` runs added the Add panel's 50 messages three times; `fling-free.mjs` does too unless given `--fifty 0`. The app keeps its messages between launches, which is why the thread grew from 153 to 453 messages between runs.
