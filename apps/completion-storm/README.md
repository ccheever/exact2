# Completion Storm

A Contract UI and binary-bound Rust `DataSource` on the existing web, Apple,
and Linux hosts. Each active lane returns its own `Answer::Later(Request)`.
The host performs real loopback HTTP; replies return through `Runner::fulfill`.
No app JavaScript, external traffic, account, or simulated synchronous work.

From the repository root (Bun and the repository's Rust/wasm toolchain installed):

```sh
export PATH="$HOME/.cargo/bin:$PATH"
EXACT_WEB_DIST="$PWD/apps/completion-storm/target/dist" bun host/web/build.mjs completion-storm-web
bun apps/completion-storm/fixture.mjs
```

Open **http://127.0.0.1:4320** on this machine. The server serves that app's
built dist and control API on 4320; held data requests use 4319. Both bind
loopback only. The Rust grants deliberately admit only these two origins.
This is a desktop-local example; a public web URL would still point the
browser at its own loopback. The Apple wrapper targets the existing macOS
presenter; the Linux wrapper can run headlessly, including on a Mac.

Build the native adapters with the same fixture running:

```sh
bun host/apple/build.mjs completion-storm-apple --bundle
EXACT_UPDATE_TRUST=development cargo build --profile host-dev -p completion-storm-linux

# Each command launches and drives the app, then closes it.
bun apps/completion-storm/smoke.mjs macos
bun apps/completion-storm/smoke.mjs linux
```

For native-only runs, no browser build or filesystem-helper Cargo invocation
is needed: start `bun apps/completion-storm/fixture.mjs --api-only` instead.
It binds the same ports and returns 404 for static routes. With read-only source
mounts, pass `--out /tmp/completion-storm` to the smoke command to put screenshots
and its JSON report in a writable directory.

The Apple builder prints the app-specific `.app` path; open it for manual use,
or add `--run` to the build command. The native wrappers disable update stores
and replacement logic; they use the same Rust source and Contract as web.
Building/running `completion-storm-linux` on macOS verifies the Linux-host
code path on macOS, **not actual Linux**. The smoke report names both host and OS.

1. Set 1–128 concurrent lanes (6, 32, and 128 presets) and an error percentage.
   Settings affect the **next** wave. Invalid input falls back to 32/0;
   integers above the limits clamp to 128/100.
2. **Start / replace / remount wave** asks the fixture for a unique wave ID,
   then issues one independent request/ticket per lane. Requests remain held.
3. **Release current** opens that wave's gate. Type in the interaction field
   and click its counter while completions land. Start and release again for
   repeated waves. The input is `stress-input`; its exact value is rendered
   as `stress-echo` for the shared metrics sampler.
4. For stale replies: start a held wave, **Navigate away**, start a new wave,
   then **Release all old + current**. Old replies must neither populate the
   remounted lanes nor decrement their pending count. Replacing a held wave
   without leaving exercises the same newest-ticket rule.

On **native**, held data explicitly opts into two independent HTTP workers.
Wave opening, release and inspection remain ordered on a separate transport, so
**Release current** and **Release all old + current** work while data is held.
This is transport concurrency; native module continuations and storage still
share the ordered worker. HTTP methods and origins do not infer independence.

Navigation forgets the old tickets, and every host lets go of their work:
held reads are aborted, queued ones are never sent and undrained results are
dropped, so another full cohort is admitted at once. Work that can't be dropped
(an ordered write) still runs and keeps its reservation until it ends. At the
128-request admission limit a new lane is visibly refused; there is no overflow
queue. The smoke drive exercises this recovery.

The compiler currently permits resources only at the root. There are 128
explicit root resources, one per possible lane; inactive lanes answer idle
without HTTP. Navigation removes the lane views and **explicitly** disables
the root resource arguments, forgetting their tickets. This does not claim
child-owned resource effects or automatic cancellation on child unmount.

Error injection is deterministic: `(lane * 37) % 100 < percentage` selects
failures, then `lane % 4` selects HTTP 503, malformed JSON, wrong wave ID, or
a truncated HTTP body. Percentage is a selection threshold, not a promise
of an exact fraction for small waves. These all settle as failed result data;
network failure is not a runner exception. Every success validates the wave,
lane and exact `wave N lane M` payload before it can count as valid.

## Counters and limits

- **Requests emitted**: lifetime data requests handed to the host, not sockets
  opened or server receipts. **Current-ticket replies parsed**: replies the
  runner accepted far enough to invoke this source; includes failures and
  excludes stale tickets dropped before parsing. These refresh every 500ms.
- **Pending lanes** sums actual `pending(resource)` flags. **Valid successes**
  and **failed/invalid replies** count only results for the current wave.
  Control API mutations are excluded from all three lane counts.
- **Fixture snapshot** is manual: server requests received, responses issued,
  and held responses. Issuing a response is not proof the browser delivered
  it or the runner accepted it. It may include old/unmounted waves.
- At most 128 lanes per wave, 8 live fixture waves, 512 held responses, a
  4KiB decoded reply limit, and a 30-second lifetime per admitted wave. Open
  overload returns 429, bad bounds 400, duplicate lanes 409, unknown/expired
  waves 410. Held replies expire with 408; queued arrivals then get 410.
  Ctrl-C resolves outstanding held replies and closes both listeners.
- Browser HTTP/1 connection pools usually admit only a handful of simultaneous
  sockets per origin. Logical pending lanes can exceed fixture-held requests.
  The separate control origin keeps release reachable; opening a gate also
  releases later browser-queued arrivals. Thus “together” means the fixture
  releases its currently held responses in one turn, **not** 128 simultaneous
  sockets or 128 commits in one frame. This is not an HTTP/2 benchmark.
- Native limits: 128 admitted independent requests and 32 MiB of reserved
  payload/result capacity, including queued, running and undrained results;
  two concurrent data transports. The ordered lane separately admits 16
  requests within 512 MiB: a waiting call is charged its request buffers, the
  one running its 64 MiB response ceiling and a completed one what it retains.
  Request-owned buffers are capped at 4 MiB. Independent replies here are capped
  at 4 KiB while received, not just after JSON decode. No effect runs on refusal.
- One outcome settles per native pump; ready lanes and admission refusals get
  alternating turns. Ordered refusals wait for earlier accepted results and
  prevent later ordered admission until settled or forgotten, so failure parsing
  cannot reorder Store writes. Wakes coalesce. Retirement aborts HTTP and drops
  queued work on its executor owner, without joining active native closures.
  Each native host implementation caps live/retiring executor workers at 48
  until they actually exit. Arbitrary Rust closure captures are count-bounded
  only; their heap size and transient source allocations are outside transport
  byte accounting. This is no absolute memory bound for native continuations.

The input remains bound to Contract state and its echo is the full value. There
is no app-side frame sampler; use the shared metrics integration to measure
responsiveness. Web completion work still runs on the browser's main thread. This
example exposes overload and exercises the first bounded native transport/pump slice.
It does not implement module-worker placement or prove a physical frame rate.

## Drive and test

```sh
# An external release is useful while an agent samples typing latency.
curl -X POST 'http://127.0.0.1:4320/api/release?wave=0'
curl 'http://127.0.0.1:4320/api/stats'

EXACT_UPDATE_TRUST=development cargo test -p completion-storm-data -p completion-storm-web -p completion-storm-apple -p completion-storm-linux
bun test apps/completion-storm/fixture.test.mjs

# With the fixture running and CHROME set to a Chrome executable:
bun apps/completion-storm/smoke.mjs web
# With an API-only fixture and a separately served build:
EXACT_STORM_WEB_URL=http://127.0.0.1:4322 bun apps/completion-storm/smoke.mjs web

# Native defaults to 6 lanes; optionally select 6, 32, or 128.
bun apps/completion-storm/smoke.mjs macos 32
bun apps/completion-storm/smoke.mjs linux 32
```

Agent action IDs: `count-6`, `count-32`, `count-128`, `errors-0`, `errors-50`,
`start-wave`, `release-wave`, `release-all`, `inspect-fixture`, `navigate-away`,
`interact`. The normal agent `clock settle` waits for in-flight HTTP: release
held requests first, or deliberately wait for the bounded fixture timeout.

Smoke timing is agent command → matching echo and Contract state acknowledgement,
including protocol roundtrips. It is not hardware input latency, physical-display
presentation, or a 120fps result. Releases in the smoke come from app controls;
external release is used only in cleanup. The report includes the number of
responses the fixture held before release. Run one smoke at a time:
its cleanup releases remaining waves from this local fixture.

Integration members: `data`, `web`, `apple`, and `linux` under this app,
named `completion-storm-<member>`. Web output and smoke screenshots stay in
the ignored app-local `target/` tree. Apple artifacts use the shared builder's
app-specific `target/clients/…/com.exact.completionstorm/` directory; Linux
uses the workspace target directory. No new workspace dependency is needed.
