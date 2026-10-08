# The device scoreboard (iOS)

The runner behind the exact2-vs-UIKit-vs-SwiftUI scoreboard: it builds exact2's apps for three benchmarks at one
commit, runs each benchmark's series on each device in interleaved rounds under a shared device lock (optionally
gated on the device's thermal state), and writes `SCOREBOARD.md`: medians per benchmark per device, exact2 against
the better of UIKit and SwiftUI, metric by metric, with a stale-build check.

| benchmark | series (per device) | apps (bundle id suffix) | its harness |
|---|---|---|---|
| Extra Heavy feed, 19 kinds | `<tag>-19-<dev>` | `swiftui`, `uikit`, `exact2` (`dev.exact.xheavy.*`) | `../extra-heavy` |
| crypto list | `<dev>-<tag>` | `swiftui`, `uikit`, `svgi` (`dev.exact.cryptobench.*`) | `../crypto-list` |
| plain heavy list | `<dev>-<tag>` | `swiftui`, `uikit`, `exact` (`dev.exact.heavybench.*`) | `../heavy-list` |

Every run goes through `../heavy-list/probe` (`devrun.sh`, `devclean.sh`, `crashcheck.sh`, `resign.sh`); results
land where each benchmark's own series writes them, `target/bench/<benchmark>/results/<series>/`, so each
benchmark's `summarize.py` reads them too.

| file | what |
|---|---|
| `builds.sh <tag> [commit]` | exact2's three apps at this checkout's HEAD, one at a time, through each benchmark's own builder; leaves `built-<what>-<tag>` / `failed-<what>-<tag>` in the state dir |
| `chain.sh <iphone\|ipad> <tag> [rev]` | the board on one device: xheavy, then crypto, then heavy, each waiting for its build |
| `list-series.sh <xheavy\|crypto\|heavy> <dev>` | one benchmark's rounds on one device (what `chain.sh` runs) |
| `lib.sh` | sourced by `list-series.sh`: the lock take/give, devclean at every take, installs, the run retries, the thermal gate, crash logs |
| `devlock.sh <device> take\|give\|who\|queue <who>` | the queued device lock |
| `board.py` | writes `SCOREBOARD.md` from the results on disk |
| `wait.sh [s]` | waits for the next event in a chain's log, then prints each device's lock holder, queue, last steps and thermal readings |
| `resign-all.sh <app.app>…` | re-signs bundles with a rebuilt probe, keeping their ids and stamps |

## Before the first run

Each benchmark's README has its prerequisites and one-time steps; the board needs them all done: the data
(`bench/extra-heavy/prepare.sh`, `bench/crypto-list/prepare.sh`, `bench/heavy-list/prepare.sh`), the probe
(`bench/heavy-list/probe/build.sh`), and the SwiftUI and UIKit apps built and signed with the probe at the paths
`chain.sh` installs from:

- `bench/extra-heavy/device/XHeavy.app`, `bench/extra-heavy/device/XHeavyUIKit.app` (`../extra-heavy/README.md`, "iOS")
- `bench/crypto-list/swiftui/build/CryptoBench.app`, `bench/crypto-list/uikit/build/CryptoUIKit.app`
- `bench/heavy-list/swiftui/build/HeavyBench.app`, `bench/heavy-list/uikit/build/HeavyUIKit.app` (also the thermal
  gate's keeper, below)

Competitor bundles are not rebuilt per board: they are the same builds from board to board, which is what lets
`board.py --prior` tell a device change from a code change. `XHEAVY_MAP`, `CRYPTO_MAP`, `HEAVY_MAP`
(`"<app>=<bundle.app> …"`) install others.

| variable | meaning |
|---|---|
| `BENCH_IPHONE`, `BENCH_IPAD` | the devices' UDIDs (`xcrun devicectl list devices`); `BENCH_DEVICE` for a single device of either name |
| `BENCH_SIGN_IDENTITY`, `BENCH_PROFILE`, `BENCH_TEAM` | signing for `builds.sh` and `resign-all.sh` (`../heavy-list/README.md`); the profile must cover `dev.exact.xheavy.*`, `dev.exact.cryptobench.*` and `dev.exact.heavybench.*` and the devices |
| `BENCH_LOCK` | the lock command (default `bench/board/devlock.sh <dev>`); see "The device lock" |
| `BENCH_LOCK_DIR` | where `devlock.sh` keeps its locks and queues (default `/tmp/exact-bench-lock`, shared by every process on the Mac) |
| `BENCH_STATE` | build markers, bundles the board copies, priority files, thermal log and readings (default `target/bench/board`) |
| `BENCH_TARGET` | results root (default `target/bench`; results in `<root>/<benchmark>/results/`) |
| `GATE`, `COOL` | thermal gating (default on for the iPhone) and the cool-down between holds (300 s) |
| `ROUNDS`, `FROM`, `ROTATE`, `PASSES`, `RUN_CAP` | rounds (3), first round, app-order rotation (1), passes per series (4), seconds one run may take (420; 600 on the iPhone) |
| `BENCHES`, `TIP_TAG`, `LOCK_PRIO` | the steps a chain runs, a second exact2 xheavy build in the rotation, the ticket priority (5) |

## Running

Use a checkout dedicated to the board (a `git worktree add --detach` at the commit you want measured): `builds.sh`
refuses a dirty checkout and stamps the commit into every bundle, and the probe writes the stamp into every result.

```sh
export BENCH_IPHONE=<udid> BENCH_IPAD=<udid> BENCH_SIGN_IDENTITY=<sha1> BENCH_PROFILE=<profile.mobileprovision>
TAG=m$(git rev-parse --short=4 HEAD); REV=$(git rev-parse --short=9 HEAD); S=target/bench/board; mkdir -p $S
bench/board/builds.sh $TAG > $S/builds-$TAG.log 2>&1 &              # the Mac builds while the devices wait

# one device
bench/board/chain.sh ipad $TAG > $S/chain-ipad.log 2>&1 &
# both devices: two chains at once, one per device lock; the builds are shared
bench/board/chain.sh iphone $TAG > $S/chain-iphone.log 2>&1 &

bench/board/wait.sh                                                 # poll until something happens
python3 bench/board/board.py --tag $TAG --rev $REV                 # -> target/bench/board/SCOREBOARD.md
```

A chain is three series of three rounds of three apps; per device that is four to five hours (about ten minutes per
app per round), more when it shares the device, and on a gated iPhone a five-minute cool-down per take on top. Rerunning a chain resumes it: every
series runs with `SKIP_DONE`, skipping results already on disk; a series is passed over up to `PASSES` times; a
series that cannot launch anything twice in a row (exit 3) stops the chain, lock given back.

Per take (one app's scenarios, under ten minutes): the lock taken; `devclean.sh` terminates leftover bench apps
(about a hundred suspended ones once hung every launch on an iPad); the app about to run is installed; its
scenarios run, each capped and retried (a launch failure three times, a partial or timed-out run once, its partial
result kept as `<out>.partial` and never summarized); the bench apps' crash logs since the series began are
copied into the series' `crash/` (a run that printed `ok` may have crashed and relaunched: read them); the lock
given back. The app order rotates per round (round 2 starts from the second app): an app run from a fixed position
measured ~19 ms/s more CPU on an iPad. Each results directory gets `provenance.txt`: start time, device, commit,
probe sha1, and each bundle's stamp and executable UUID.

A second exact2 build in the same rotation (the day's landings against the board's commit, interleaved in time
rather than hours apart): build it from another checkout, then name it in the chain and the board.

```sh
EXACT2=<checkout at the tip> ONLY=xheavy bench/board/builds.sh tip
TIP_TAG=tip bench/board/chain.sh iphone $TAG > $S/chain-iphone.log 2>&1 &
python3 bench/board/board.py --tag $TAG --rev $REV --tip-tag tip --tip-rev <its commit>
```

### The thermal gate

An iPhone throttles under ten minutes of scrolling and is then indistinguishable from a regression. With `GATE=1`
(the default on the iPhone), every take starts with a reading: a ~20 s `rest` run of the static UIKit heavy list
(`BENCH_PING_APP`, `BENCH_PING_ID`), which then stays in front as the keeper so the phone does not auto-lock. The
take proceeds at state 0 (nominal) or 1 (fair) and at least `COOL` seconds after the previous give; at 2, 3 or no
reading the lock goes back and the phone cools for `COOL` seconds. Every reading and every run's worst state are
logged to `$BENCH_STATE/thermal-<dev>.log`, and every result carries the probe's own thermal fields, so the board
can refuse to rank a comparison across states. Nothing is set aside for heat; the rotation gives each app the same
states. The probe records thermal state since sha1 `6b10a15abb37`; `resign-all.sh` re-signs older bundles with a
rebuilt one.

## The device lock

`devlock.sh <device> take|give|who|queue <who>` is a lock per device name, with a queue. A taker files a ticket
`<prio>-<ms>-<pid>-<who>` and takes the lock only when its ticket is first in line, so the fastest poller does not
win; tickets order by priority (`LOCK_PRIO`, default 5; quick A/Bs 3, landing soaks 0), then filing time. A ticket
whose waiting process has died is dropped; a holder that set `LOCK_HOLDER_PID` (its script's PID, which every
series exports) and died is reclaimed by the next waiter. `take` blocks until held; `give <who>` gives only a lock
`<who>` holds; `who` prints the holder (`<who> <time> <pid>`) or `free`; `queue` lists live tickets. A digit in
`$BENCH_STATE/prio-<dev>` sets the board's priority at every take, so a running chain can be moved up or down the
queue. Hold the lock only while running on the device, never while building, and in takes of at most ten minutes,
so other runs on the same device take turns.

Every harness in `bench/` brackets its device holds with an optional `BENCH_LOCK` command, run unquoted as
`$BENCH_LOCK take <who>` and `$BENCH_LOCK give <who>`, with `LOCK_HOLDER_PID` exported. The device name goes in the
command, so pointing a harness at this lock is:

```sh
BENCH_LOCK="$PWD/bench/board/devlock.sh iphone" BENCH_DEVICE=$BENCH_IPHONE SERIES=s bench/heavy-list/series.sh
BENCH_LOCK="$PWD/bench/board/devlock.sh ipad" BENCH_DEVICE=$BENCH_IPAD SERIES=s bench/extra-heavy/ios/series.sh
bench/board/devlock.sh ipad who; bench/board/devlock.sh ipad queue
```

Use an absolute path (the harnesses run from anywhere) without spaces (the command is word-split). The board's own
scripts use `bench/board/devlock.sh <dev>` unless `BENCH_LOCK` is set; any command with the same `take`/`give`
interface can stand in for it.

## Reading SCOREBOARD.md

`board.py --tag <tag> --rev <commit>` reads `<xheavy>/<tag>-19-<dev>`, `<crypto>/<dev>-<tag>` and
`<heavy>/<dev>-<tag>` (roots `--xheavy`, `--crypto`, `--heavy`, default `target/bench/<benchmark>/results`) and
writes `target/bench/board/SCOREBOARD.md` (`--out`, `--stdout`). `--devices "iphone=iPhone 13 Pro Max,ipad=iPad
Pro M1"` names the devices; `--head` and `--notes` put a hand-written account (what was built, what happened) above
and after the ranked losses; `--prior <tag>` compares the Extra Heavy tables with an earlier series. `--help` has
the rest. Top to bottom:

- **Problems found while building this board**: any results directory whose stamps are not the expected build
  (exact2's are `xheavy@<rev>`, `crypto-svgi@<rev>`, `heavy@<rev>`; `--expect bench.app=STAMP` adds SwiftUI's and
  UIKit's, `{rev}` replaced), and unreadable results. A board with problems compares the wrong builds.
- **Losses ranked by ratio**, per device: every metric where exact2 is worse than the better of UIKit and SwiftUI
  by 3% or more, the largest ratio first, tagged with its benchmark.
- **Per benchmark per device**: the results path, whether every scenario has three rounds for every app, and the
  order the rounds actually ran (from the result files' times). Then the table, medians over the rounds: the columns
  are `../heavy-list/README.md`'s (fps, live fps, late/s, worst ms, busy/f, cpu and main ms/s, peak and end MB,
  blanks, ladder <110, jump p50, cold ms, rest cpu and rest main, the last two followed by each round's value
  because rest swings run to run). Peak MB is the probe's own peak through the fling, never a segment-end sample.
  Then fling fps by speed, and the thermal table: the worst state each app reached in each scenario (`—` = the probe
  recorded none, `n/m` = rounds that recorded it, `LP` = Low Power Mode).
- **Extra Heavy only**: the inner lists' fps by speed (filmstrip, inbox), their CPU and blanks, and innerkeep.
  innerkeep's primary column is `kept (same item, same place ±1 pt)` (the probe's `keptAnchor`); `raw offset kept`
  is shown but never ranked (exact2's offset space changes as rows above the anchor are measured).
- **Verdicts** under each table: **exact2 worse than the better of UIKit and SwiftUI** (the ratio is how many times
  worse; *noise* marks a gap under 3%), **exact2 better than both** (the margin over the better one), **equal**
  (within 0.5%), and **not ranked** where the compared apps ran at different thermal states in that metric's
  scenario (a throttled phone is not a slower app; a state nobody recorded is unknown, not different).
- With `--tip-tag`, the second exact2 build is a fourth row; the verdicts are against it (the newest code), and a
  delta table compares the two exact2 builds metric by metric (±3% is noise; ↑ marks higher-is-better).
- With `--prior`, **Against <prior>**: every metric where any app moved more than 10%, with a reading — the
  SwiftUI and UIKit bundles are the same builds in both series, so when they moved too it was the device, and when
  only exact2 moved it was the code.

## What is not here

The board as run on 2026-09-30 also had a fourth benchmark, features F1–F5 (exact2 vs SwiftUI, with a first-ink
series of one baked app per feature). Its apps are not in this repository, so its series, its frozen-shot checks and
its tables in `board.py` stayed out; so did the scripts that only steered that day's run (restarts by recorded PIDs,
one-off reruns, the tip rerun now done by `TIP_TAG`), and the iPhone thermal reader that needed a lane's diagnostic
build (the gate above reads the probe instead).
