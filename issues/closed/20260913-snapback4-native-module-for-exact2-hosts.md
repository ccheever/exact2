# Snapback 4's device as a Rust module for Exact2 hosts

**Status:** Closed
**Resolution:** Added exact-snapback4 with Ibex storage authority and one shared SQLite, and converted Messages on native/web to durable records, offline writes and sync.
**Systems:** Apple host, Linux host, data seam, apps/messages
**Severity:** P2
**Author:** Claude (Snapback 4 lead session), at Charlie Cheever's direction: "ship a native Rust module for both Expo and Exact2"
**Date:** 2026-09-13
**Related:** Snapback LLP 3000 §7 (2026-09-13: one interpreter hosted two ways); LLP 2000.000 §9

Snapback 4 now ships its device as a Rust crate: `snapback4-device` in
`~/projects/snapback-sb4/snapback4/crates/snapback4-device` (landed on
Snapback's `main` at `e301d0045` and after). It owns the viewer's partition in
its own SQLite (the server's two-table layout, the server's own code),
applies the sync stream a page at a time, keeps the outbox in submission
order, and answers queries and predictions with the same interpreter the
server runs. One JSON call: `snapback4_device::ffi::call(&mut device,
&json!({"op": ..}))` — ops `state`, `adopt`, `apply`, `query`, `predict`,
`withdraw`, `enqueue`, `queued`, `next_submission`, `dequeue`, `meta`,
`set_meta`, `clear_rows` — or the C ABI in `include/snapback4.h`. It costs
an iOS app 1.7 MB linked, SQLite included. The web is not this module: the
same interpreter as wasm measured 194 KB gzip, so a browser host keeps
Snapback's TypeScript twin (`snapback4/local`, 17 KB) over
`storage.sqlite` or IndexedDB.

## What an Exact2 host would do

A `DataSource` for an app on Snapback (`runner/src/runner/source.rs`):

- `configure_storage(data, ..)` records the path; the device opens after
  first pixel (the crate's `Device::open(path, backend)`; `backend` is the
  server's `GET /schema` JSON, or `None` to open on the one the device
  kept, which is how a cold start offline opens).
- `answer(store, source, args)`: a source named after a Snapback query
  maps to `ffi::call({"op": "query", "name", "viewer", "args", "now"})`
  and returns `Answer::Now(value)`; a source named after a mutation maps
  to `predict` + `enqueue` and returns the predicted result now.
- The transport is the host's I/O: `Answer::Later(Request)` for `GET
  /sync` (paged; each page to `apply`), `GET /changes` (the long poll), and
  `POST /m/<op>` for each queued entry in order (`dequeue` on an answer,
  `withdraw` on a refusal). This is exactly what Snapback's own JavaScript
  tier does (`packages/snapback4/src/native.ts`, ~330 lines) and can be
  transcribed.
- `grants`: `sqlite.open app:/data/<store>` and the network.

`apps/messages` ("local-only … relaunch resets all messages") is the
natural first app: its conversations become a Snapback schema (the chat
reference app's `snapback/schema.q` is 100 lines), and the module gives it
persistence, sync and offline writes with no server-side code.

## Why a ticket and not a landing

Exact2's `main` is mid-edit by another session as this is written, its
rules ask a human's word before an agent adds a crate or a check, and a
host crate here needs an implementer and a date. The crate is ready to
depend on by path; publishing it to crates.io is a Snapback decision to
make when this ticket is picked up.
