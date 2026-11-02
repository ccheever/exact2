# Exact Live

A device-local creative production workspace for **Outside, together**: six imagined photographic
scenes, a crew transcript and a parsed runbook, from one `app.contract` on the web, macOS, iOS and
Linux. Select Go live, rearrange scenes by their grips, open a photograph, and write crew notes.

- **Scenes** reuse the interaction gallery's photo rows and viewer: a non-button grip reorders them
  (Pick up, Earlier, Later, Place and Cancel do the same by hand), and a photograph opens at Fit or 2×
  with pan.
- **Crew notes** are eight curated messages and a local draft. Go live streams 10,000 records in
  batches of 32; the workload drawer picks 100 to 100,000 records and batches of 1, 8 or 32.
- **The runbook** is `runbook.md`, through the Markdown parser and renderer; explicit controls load
  1 and 4 MiB stress documents.
- **The job strip** runs waves against the Completion Storm job service: 128 held requests whose
  progress arrives as server-sent events or over a WebSocket.

The workspace is device-local: there is no account and no cross-device sync. At 900px the work
surface sits beside a 360px transcript; narrower, one pane shows at a time.

## Run it

From the repository root:

```sh
bun apps/exact-live/fixture.mjs                       # the job service, on loopback 4339 and 4340
bun host/web/build.mjs exact-live-web                 # the web app, into target/web-dist
bun host/apple/build.mjs exact-live-apple --run       # macOS
bun host/apple/build.mjs exact-live-apple --ios --run # an iOS Simulator
cargo build --release -p exact-live-linux             # Linux
bun scripts/smoke.mjs web --app exact-live            # the stream smoke, with its own fixture
```

`job-origins.txt` names the job service's two origins, data and control, and the build compiles
both the request URLs and their `net.fetch` grants from it. To build against the local fixture, set
the same lines in `EXACT_LIVE_JOB_ORIGINS`:

```sh
EXACT_LIVE_JOB_ORIGINS=$'DATA=http://127.0.0.1:4339\nCONTROL=http://127.0.0.1:4340' \
  bun host/web/build.mjs exact-live-web
```

An unavailable service reports its error; no job completes from a timer, and the app never starts
the service.

## Tests

`data/tests/` holds the data source's tests (`cargo test -p exact-live-data`): full-source values
and row identity, the bounded local echo, manual order and reset, runbook continuation, responsive
composition, draft retention, request tickets across navigation, and the job origins' grants.
Synthetic HTTP responses occur only in tests.
