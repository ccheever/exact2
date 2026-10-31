# Heavy list benchmarks (iOS)

Four apps draw the same chat-style feed of 10,000 rich messages from the same data: exact2
(`exact-heavylist/`, Contract), SwiftUI (`swiftui/`, the most ordinary `List`), UIKit (`uikit/`, a
hand-tuned `UICollectionView`) and Expo (`expo/`, `@expo/ui`). `SPEC.md` is the screen, the
row and the parity rules. A probe injected into each app (`probe/`) scrolls its list from
inside the process on a real device and records every frame, so the four are measured the
same way.

```sh
bench/heavy-list/prepare.sh                  # the data, once (gen.py, about a minute)
bench/heavy-list/probe/build.sh              # the probe dylibs
bench/heavy-list/exact-heavylist/build.sh    # exact2 for a device, signed with the probe
bench/heavy-list/series.sh                   # scroll series on BENCH_DEVICE (SERIES=<name>)
bench/heavy-list/cold-series.sh              # cold start series (linked probe)
python3 bench/heavy-list/probe/summarize.py target/bench/heavy-list/results/<series> swiftui,exact,expo,uikit
```

## What is measured

`series.sh` runs, per round and app: `fling` (constant-speed 2 s segments at ±1k–24k pt/s),
`ladder` (±3k–96k pt/s), each once for timing and once with blank sampling (`BENCH_RENDER=layer`),
`jump` (ten absolute jumps), `coldstart`, `fling` in live mode (`BENCH_LIVE=1`: an insert at the
top and a reaction bump every 250 ms) and `rest` (10 s without scrolling). Rounds alternate the
apps and rotate their order, so slow drift in the device lands on all of them.
`probe/summarize.py` prints the table below (medians over rounds); `probe/agg.py` prints per-speed
detail and `probe/analyze.py` one file.

| Column | Meaning |
|---|---|
| fps, live fps | frames presented per second over the whole fling run, without and with live mode |
| late/s | frames per second whose interval exceeded 1.5 display periods |
| worst ms | the longest frame interval |
| busy/f | main run-loop busy ms per frame |
| cpu ms/s, main ms/s | process and main-thread CPU per second of scrolling |
| peak MB, end MB | phys_footprint, peak and at the end of the fling run |
| blanks | blank-sampled fling frames with a blank band in the viewport |
| ladder <110 | first ladder speed under 110 fps (55 at 60 Hz) or with a blank, per round |
| jump p50 | median ms from a jump to a viewport with no blank band |
| cold ms | process start to the first frame with the list present and blank-free |
| rest cpu, rest main | CPU ms/s with nothing moving |

The `coldstart` scenario inside `series.sh` runs with the probe inserted by
`DYLD_INSERT_LIBRARIES`, which disables an app's prebuilt launch closure. `cold-series.sh`
measures cold start properly: the probe linked into a copy of each app by a load command
(`probe/makecold.sh`, bundle ids `…<app>cold`), launched without insertion, three interleaved
rounds. `coldsum.py` prints its table: median ms, each run, process start to the probe's
constructor (pre-main), and main-thread busy time to first ink.

Every result carries the device's thermal state and Low Power Mode; `summarize.py` prints the
worst state per app and scenario and says when the apps ran at different states. Do not compare
those rows.

## The apps

| App | Bundle id | Build | Output |
|---|---|---|---|
| exact2 | `dev.exact.heavybench.exact` | `exact-heavylist/build.sh` | `exact-heavylist/build/HeavyExact2.app`, signed, stamped `heavy@<commit>` |
| SwiftUI | `dev.exact.heavybench.swiftui` | `swiftui/build.sh` | `swiftui/build/HeavyBench.app` (device), `build-sim/` (simulator) |
| UIKit | `dev.exact.heavybench.uikit` | `uikit/build.sh` | `uikit/build/HeavyUIKit.app`, `build-sim/` |
| Expo | `dev.exact.heavybench.expo` | `expo/build.sh` | `expo/build/HeavyBenchExpo.app`, `build-sim/` |

A series names each app by its bundle id's last part (`exact`, `swiftui`, `uikit`, `expo`).
Each directory's README says how its app expresses every spec item, and what it cannot.
`exact-heavylist/` is its own Cargo workspace consumed by path from this checkout, built by
`host/apple/build.mjs` with `EXACT_APP_DIR` set (its README has the simulator commands).
The SwiftUI, UIKit and Expo device builds are unsigned; sign each with the probe, then install:

```sh
P=bench/heavy-list/probe
$P/resign.sh bench/heavy-list/swiftui/build/HeavyBench.app dev.exact.heavybench.swiftui
$P/resign.sh bench/heavy-list/uikit/build/HeavyUIKit.app dev.exact.heavybench.uikit
$P/resign.sh bench/heavy-list/expo/build/HeavyBenchExpo.app dev.exact.heavybench.expo
for a in bench/heavy-list/{exact-heavylist/build/HeavyExact2,swiftui/build/HeavyBench,uikit/build/HeavyUIKit,expo/build/HeavyBenchExpo}.app; do
  xcrun devicectl device install app --device $BENCH_DEVICE $a
done
```

`gen.py` writes `data/messages.json` and the 104 JPEGs deterministically; `prepare.sh` runs it,
checks the output against `data.sha256` and copies it into `exact-heavylist/` and `expo/`
(whose builds refuse or cannot reach files outside their directories). The JPEG bytes depend on Pillow's
encoder: 11.3.0 reproduces the checksums. None of the data is committed.

## Running

```sh
export BENCH_DEVICE=<udid> BENCH_SIGN_IDENTITY=<sha1> BENCH_PROFILE=<profile.mobileprovision>
SERIES=ipad-$(git rev-parse --short HEAD) bench/heavy-list/series.sh
# Expo is killed by jetsam on an iPad in every scroll scenario: give it round 1 only
SERIES=s APPS="swiftui exact expo uikit" ROUNDS=1 bench/heavy-list/series.sh
SERIES=s APPS="swiftui exact uikit" START_ROUND=2 ROUNDS=3 bench/heavy-list/series.sh
# cold start: linked-probe copies of the signed bundles, then the series
C=target/bench/heavy-list/cold
bench/heavy-list/probe/makecold.sh bench/heavy-list/exact-heavylist/build/HeavyExact2.app dev.exact.heavybench.exact $C
#   … the same for swiftui, uikit and expo
bench/heavy-list/cold-series.sh
python3 bench/heavy-list/coldsum.py target/bench/heavy-list/results/cold swiftui,exact,expo,uikit
```

A series takes about 10 minutes per app per round. Results go to
`target/bench/heavy-list/results/<series>/` (`BENCH_RESULTS` moves them) with a `provenance.txt`
and any bench crash logs (`crash/`); a run that printed `ok` may still have crashed, so read
them. On a simulator, `BENCH_SIM=<udid> probe/run.sh <bundle-id> <scenario> <sample> <out.json>`
runs one scenario with `probe-sim.dylib` inserted; a simulator is for checking, not for numbers.
`probe/tptrace.sh` records Time Profiler around one run; `tpagg.py`, `tpcallee.py` and
`tpcaller.py` read its export.

| Variable | Meaning |
|---|---|
| `BENCH_DEVICE` | the device's UDID (`xcrun devicectl list devices`); every device script needs it |
| `BENCH_SIM` | a simulator UDID, for `probe/run.sh` and `swiftui/shoot.sh` |
| `BENCH_SIGN_IDENTITY` | a codesigning identity's SHA-1 (`security find-identity -v -p codesigning`) |
| `BENCH_PROFILE` | a development profile covering `dev.exact.heavybench.*` and the device (a wildcard profile) |
| `BENCH_TEAM` | the team id; read from the profile when unset |
| `BENCH_LOCK` | optional command run as `<cmd> take <who>` / `<cmd> give <who>` around each device hold, for a device that other runs share; `LOCK_HOLDER_PID` is exported for it |
| `SERIES`, `APPS`, `ROUNDS`, `START_ROUND`, `ROTATE` | the series' name, apps (default `swiftui exact expo uikit`), rounds (3), first round, order rotation (1) |
| `BENCH_RESULTS`, `BENCH_COLD` | where results and cold copies go (under `target/bench/heavy-list/`) |
| `BENCH_ENV` | extra `KEY=VALUE,…` for the app's launch environment |

## Prerequisites

Xcode with an iOS 17+ SDK; a device in Developer Mode, paired, unlocked, with Auto-Lock off (a
locked iPhone refuses every launch until its passcode is entered); the signing identity and
profile above; Python 3 with Pillow for `gen.py`; GNU `timeout` (`brew install coreutils`);
and this checkout's toolchains for the exact2 app (Bun, Rust, `bun install --frozen-lockfile`).
The Expo app also needs Node, npm and CocoaPods. No TCC permission is needed: the probe scrolls
from inside the app, and devicectl only launches and copies files.

## The measurement Mac

Mac measurement series use the silver M5 Max at `ccheever@100.76.94.78`:
Mac17,6, 18 CPU cores, 40 GPU cores, 128 GiB, built-in 3456×2234 display,
macOS 27.0.1 (26A434), Xcode 27.2, Bun 1.4.2 and Rust 1.97.0.
Build only the benchmark arms in `~/projects/exact2-measure`, a detached
worktree made from fetched `origin/main`. Leave the main clone and the old
worktrees alone. Every Cargo invocation uses this worktree's `target/`,
including `--target-dir target` for the separate benchmark workspaces.
Nothing else may build there; no other work may run during a series.

Scoreboard CPU rows use `G1_GPU_HOOK=0` with `powermetrics` off; GPU rows
use `G1_GPU_HOOK=1` with power sampling off. Power is measured in its own
run and never joined to a CPU row. G5m's bar/caret CPU rows remain hook-on,
power-on (c), and its GPU rows remain power-on, until a hook-off static series
and a power-off static series replace them respectively.

Both driver and probe default to hook-off for every workload, including bar,
caret and cold-only. Set `G1_GPU_HOOK=1` explicitly to reproduce the old
hook-on instrument or to measure GPU work. Hook-on creates the system Metal
device and a queue before `main`, even in native, which otherwise creates
neither; its cold row cannot be compared to a hook-off launch. Empty and invalid
values are driver errors before launching. Rejections record requested hook
state; successful probes record observed state too. The drawable hook stays on.
`aggregate.mjs` prints the hook state, refuses mixed tables and prints GPU rows
as `—` when off. Missing hook metadata means on for pre-switch probes.

The retained G4m, G5m and hook comparison passed a load/compiler rule while
carrying 5–6 busy cores, a spinning PerfPowerServices and fleet/Claude work.
Their “quiet” label means that old rule only. The complete (a) CPU baseline was
taken under that background; re-take it when the machine is quiet. The owner
must stop the fleet/Claude session and its polling loops, simulators and
DeviceHub, and PerfPowerServices if it is spinning, before the next series.
The bounds are ≤1.0 non-benchmark busy cores, all load averages below 30 and
no compiler. Benchmark work is the arms, driver, wheel helper, probe helpers,
caffeinate and WindowServer. The aggregate is the sum of `ps` per-process CPU
shares outside that set; whole-machine CPU-tick deltas are retained separately.
M10 cut 1 amends the former 2.0 whole-machine-core limit: its launches alone
used 2.0–2.1 cores. M10 cut 1b applies the 0.25-core per-process bound only to user processes:
anything outside `/System`, `/usr/libexec`, `/sbin` and the XPC services inside
system frameworks. System daemons count only toward the 1.0-core total.
This replaces the transient-daemon allowance. Cut 1's crossings were
XprotectService (0.6 core, triggered by launches), mediaanalysisd, hybridsearchd,
searchpartyd, suggestd, duetexpertd, launchd and the system screen-sharing
agent (0.29 core). Retain the five largest non-benchmark processes and the
five largest user processes so system work cannot hide a user-process breach.
After signing, launch each arm once as an unrecorded throwaway before a cohort;
first-execution signature validation is not the scoreboard's cold launch.
`gpui/run-hook.py` retains CPU-tick deltas, the five largest other processes,
and power source/mode. Its `--single` mode retains an instrument sample even
when loaded and produces no scoreboard table. See
[the instrument comparison](gpui/RESULTS.md#g4m-the-instruments-share) and
[the amended G4q audit](gpui/RESULTS.md#g4q1-the-clock).

Every Mac CPU row carries its **effective GHz**: the phase's process-cycle
delta divided by its process-CPU-second delta and by 10⁹, calculated per
sample before aggregating, with instructions per cycle, and cycles and
instructions per second beside the milliseconds. The clock is reported, not
gated. Both Macs are the same M5 Max (6 **Super** and 12 **Performance**
cores; there is no efficiency cluster), and on an idle one the scheduler keeps
the Rust arms' main threads on the Performance level near 1.9 GHz while
native's user-interactive main thread runs on Super near 2.9, so milliseconds
per second compare arms at different clocks. Across arms, series or machines,
compare work (cycles and instructions per second) and energy; milliseconds per
second stand beside them with their GHz. A row without counters (G4 on the
black, G4m, the five-round hook comparison) has an unknown clock and compares
only within its own series.

**No restoring launch recipe is established.** G4q.1 tested the SSH/Aqua chain
below, LaunchServices and an interactive-QoS probe override. The Rust arms
remained below 2 GHz. LaunchServices requested interactive QoS and used an app
role, but did not restore G4's CPU ranges. The override is diagnostic only and
must never become a scoreboard setting. A Terminal child reached the driver's
preflight, which refused because Terminal lacks Accessibility permission.
The direct `osascript` route had timed out; opening a `.command` file in Terminal
worked without that automation route. A Terminal comparison still needs the
owner's Accessibility grant, then a new clock sample before any series.

To reproduce the tested LaunchServices route, enter the user's Aqua bootstrap
domain as below. For each signed bundle set that domain's
`DYLD_INSERT_LIBRARIES` to its probe, then invoke `open -n -W -a <bundle>` with
its probe environment and application arguments. Use the probe's recorded PID
for signals and accounting, not `open`'s PID. Clear the domain variable after
launch and on failure. G4q.1 retains the exact driver and wrapper under
`target/g0/clock-2035d50319/`; these are diagnostic launch instructions, not a
qualified CPU recipe. Keep power sampling separate and never run two lanes'
arms together. Check for M10's `measure`, arms and `target/g0/run-*.py` before
each launch; wait in bounded intervals up to one hour if they are active.

The G4 driver and its four signed bundles are prepared under `target/g0/`.
`build.sh`, `bundle.py` and the build logs retain the commands; the data must pass
`data.sha256` before any run. Keep all raw attempts, quiet observations,
power samples and provenance in that directory. Use a fresh series directory
for each later run; never replace evidence cited by `gpui/RESULTS.md`.
The native benchmark build uses `EXACT_IDENTITY=-` for ad-hoc signing over
SSH. Xcode's optional Metal Toolchain is required (`xcodebuild
-downloadComponent MetalToolchain`); without it, Ink can build with direct
Metal unavailable. It was installed and both Exact arms rebuilt on 2026-10-30.

GUI runs must join the signed-in user's Aqua session. `launchctl asuser`
needs privilege over SSH; it changes the bootstrap session, not the Unix
user. Enter that session with sudo, then drop back to `ccheever` before
running the driver. Every SSH command needs the PATH below, including the
Homebrew Rust shims. Start the one-arm qualification from the development Mac:

```sh
ssh ccheever@100.76.94.78 '
  export PATH="$HOME/.bun/bin:$HOME/.cargo/bin:/opt/homebrew/bin:/opt/homebrew/opt/rustup/bin:$PATH"
  cd ~/projects/exact2-measure
  sudo -n /bin/launchctl asuser $(id -u) /usr/bin/sudo -u ccheever \
    /usr/bin/env PATH="$PATH" /usr/bin/caffeinate -u -d -i \
    python3 target/g0/run-42d5066572/run-series.py qualify target/g0/run-42d5066572/qualification-repeat
'
```

This opens Ink with `G1_COLD_ONLY=1`, checks the probe's ready and focus
reports, captures its window with `screencapture -l`, then exits without
gestures. Inspect the new capture; the qualified original is
`target/g0/run-42d5066572/qualification/ink.png`.
The following retained wrapper reproduces the historical power-on instrument;
it does not implement the current CPU/GPU separation or busy-core bounds.
For historical reproduction, use a fresh result directory. In the same command,
replace `qualify target/g0/run-42d5066572/qualification-repeat` with
`series target/g0/run-42d5066572/g4m-repeat` to run
five rotating fling/rest rounds and five further cold-only rounds, with
five-second quiet observations and one-second power samples. The wrapper
requires a successful qualification first. Use `static` instead of `series`
with a fresh `g5m-repeat` directory for the three-round fling/bar/caret series.
For Stage 0b, the same launch chain runs `run-boot.py retained <fresh-directory>`
and then `run-boot.py miss <fresh-directory>`, from the refreshed directory.
G4m's first qualification passed activation, key-window, readiness and capture.

On this dedicated measurement Mac there is **no GO file** (Charlie,
2026-10-30): its session stays unlocked and awake. Dedicated use is an ownership
rule, not evidence that background processes have stopped.
The development Mac's display-ownership rule does not apply here. The
wrapper in `target/g0/run-42d5066572/run-series.py` runs without a GO wait.
Its `qualify`, `series` and `static` modes use the refreshed signed arms beside
it. Keep `launchctl asuser`, the user switch and `caffeinate -d -i` above.
Accessibility and Screen Recording must be granted
to `/usr/libexec/sshd-keygen-wrapper`, the responsible process for SSH-launched
programs. Power collection needs passwordless sudo for `/usr/bin/powermetrics`:
`--samplers cpu_power,gpu_power,tasks --show-process-energy --show-process-gpu
-i 1000`. Do not work around a missing TCC grant.

The rebuilt arms use `origin/main` `42d5066572`; G4m completed under the
background recorded above.
The wrapper reads Core Animation time for phase joins, since this Mac's Python
3.9 monotonic clock uses a different epoch. It stops its recorded sampler PID
through privileged `launchctl asuser`; an unprivileged SIGINT did not stop G4m's
sudo wrapper.

The Aqua-session preflight returned UID 501; Accessibility, event posting and
Screen Recording checks passed. Passwordless `powermetrics` sampled throughout
G4m. See [G4m](gpui/RESULTS.md#g4m-the-four-arms-on-the-measurement-mac) for
the measured source, qualification and retained evidence.

## Last standings

From the series of 2026-09-29 and 2026-09-30; the numbers come from `probe/summarize.py` and
`coldsum.py`, and the raw results are not committed.

**iPad Pro M1, 2026-09-30, exact2 origin/main 9fdf7e564** (three rounds; Expo not run):

| app | fps | live fps | late/s | worst ms | busy/f | cpu ms/s | main ms/s | peak MB | end MB | blanks | ladder <110 | jump p50 | cold ms | rest cpu | rest main |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| swiftui | 92.1 | 68.4 | 12.3 | 101 | 4.46 | 445 | 412 | 132 | 86 | 0 | 3000 | 79 | 316 | 23 | 23 |
| uikit | 119.6 | 96.1 | 0.3 | 17 | 2.04 | 297 | 245 | 157 | 82 | 0 | 96000 | 25 | 291 | 22 | 22 |
| exact | 119.9 | 120.0 | 0.0 | 8 | 1.74 | 435 | 186 | 123 | 109 | 0 | 96000 | 9 | 168 | 32 | 26 |

exact2 was ahead of both on late frames, worst frame, jump, cold start, main-thread CPU, live
fps and peak memory, and behind UIKit on process CPU (435 vs 297 ms/s), CPU at rest (32 vs 22)
and memory at the end (109 vs 82 MB). Its cold ms here is the inserted probe's.

**iPhone 13 Pro Max, 2026-09-29, exact2 origin/main dedbe6e8** (three rounds; Expo round 1 only):

| app | fps | live fps | late/s | worst ms | cpu ms/s | main ms/s | peak MB | blanks | jump p50 |
|---|---|---|---|---|---|---|---|---|---|
| swiftui | 106.4 | 0.6 | 8.7 | 42 | 342 | 308 | 87 | 0 | 32 |
| exact | 116.9 | 117.0 | 2.9 | 25 | 411 | 275 | 110 | 0 | 17 |
| expo | 66.3 | 3.6 | 9.4 | 371 | 813 | 555 | 451 | 22 | 322 |
| uikit | 117.3 | 91.5 | 2.3 | 33 | 263 | 203 | 109 | 0 | 15 |

On the iPad of that day, Expo was killed by jetsam at about 5.1 GB in every scroll scenario.

**Cold start, linked probe, 2026-09-29 evening, exact2 origin/main dedbe6e8**:

| app | iPhone 13 Pro Max ms (3 rounds) | pre-main | busy to ink | iPad Pro M1 ms (2 rounds) | pre-main | busy to ink |
|---|---|---|---|---|---|---|
| swiftui | 330 | 19 | 306 | 328 | 31 | 300 |
| exact | 156 | 20 | 139 | 210 | 51 | 161 |
| expo | 632 | 55 | 456 | 795 | 156 | 572 |
| uikit | 311 | 19 | 277 | 320 | 40 | 285 |
