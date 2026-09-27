# LLP 1058: Long native calls, a page module on the web, and Swift modules

**Type:** RFC
**Status:** Implemented 2026-09-26
**Systems:** Runner (`Request::is_native`, `DataSource::native`, `Runner::native_work`); TypeScript executor (`NativeModule::later`, `native.later` in the prelude and the generated declarations); the native executor core (hand-off); Apple, Linux and render hosts (routing); Web glue, module realms, build, dev server, static serving (`host.web.native`); `js/native/ExactNative.swift`, `exact_js::swift_native_module!`, `exact_js_bake::swift_native`
**Author:** Claude (Opus 5.5) for Seth Webster
**Date:** 2026-09-26
**Related:** LLP 1027 D8/D10 (the native module seam); LLP 1027.002 D3/D4 (`Work::Later`: the I/O worker is never held while a module computes); LLP 1016 (requests, tickets, `fulfill`); LLP 1024 (native modules as a first-class seam, Draft); grnl's FRICTION.md F3, F8, M1–M3

## Summary

A TypeScript source can now `await native.later(request)`: long native
work that runs off the source's thread and outside its 100 ms budget, and
answers when the module's own code replies. It travels as a request, like
a `fetch`, to the URL `exact-native:`.

- **Apple and Linux:** the host hands the request to the source's native
  handler through the executor's hand-off. No I/O worker waits for it.
- **The web:** the glue hands the request to the app's *page module*, an
  ES module declared as `host.web.native`. It runs on the page, where
  `getUserMedia`, speech recognition and the browser's built-in model are.
  This is the web's first native seam.
- **Swift:** an app writes one class conforming to `ExactNativeModule`.
  Exact2 supplies the C seam, the Rust side (`swift_native_module!`) and
  the build (`swift_native`).

## Motivation

grnl is a voice journal. Its pipeline transcribes minutes of audio and makes
two passes of an on-device model, and every native call had to return
within 100 ms. So each step became start-a-job plus poll-the-job, driven by
an 80 ms Contract task (F3). The seam was synchronous JSON, with the Swift
build, the C entry points and the Rust shim written by hand (F8). On the
web, the data module runs in a Worker with no native seam at all, so it had
no microphone, no speech and no model (M1–M3).

## Design

### D1 — A long call is a request, not a new async mechanism

`native.later` is a `fetch` to `exact-native:` with the JSON request as its
body. Everything that already governs a request applies unchanged:

- tickets, and `fulfill` to answer one;
- forgetting when a newer answer supersedes it;
- `pending(x)` in the view;
- the answer waiting, as it does for HTTP;
- worker placement: the owner yields the request and does not block on it.

The reply is an HTTP-shaped response: 200 carries the JSON value, and any
other status carries a refusal message that rejects the promise.
`Request::is_native` names the URL, so no request type gains a field.

*Rejected:* a continuation token owned by the module. On a worker
placement the owner runs continuations inline, so a transcription would
hold the module's thread for minutes.

### D2 — The handler is taken at construction, filled at activation

`DataSource::native()` returns a `Native` handle, a shared slot, the way
`interrupt()` does.

- The TypeScript module fills the slot when it activates its native module
  and `NativeModule::later()` returns a handler, and empties it on unload.
- A worker-built instance shares its template's slot, so the proxy's handle
  reaches the instance on its owner.
- Every forwarding source forwards `native()`.
- `Runner::native_work` turns a native request into `Work::Later`: the
  handler, with the host's reply.

### D3 — Independent lane, handed off

A native request uses the independent lane with a 1 MiB response ceiling,
and `handoff` covers it. Ordered storage behind it completes first. The
worker returns at once, and the module's own thread sends the reply. A
module that takes no long calls answers `native.later` through `call`,
immediately.

### D4 — Whether `native` exists is the device's fact

Asking for `native` (op 6, `available`) counts as an external read. The
bake runs with no module, so an answer that branches on `native` is not
compiled, and the running host asks it again.

### D5 — The web's page module

`host.web.native` names an ES module in the app that exports
`later(request)`.

- **Build:** copies it into the dist as `native.js`, marks the page with
  `<meta name="exact-native">`, and serves it as a public file.
- **Dev server:** serves the current file, so a page reload picks up an edit.
- **Module realms:** they report `native` present; `call` throws, because
  nothing on the web answers at once.

### D6 — Swift without plumbing

`js/native/ExactNative.swift` exports four C symbols over an
`ExactNativeModule` protocol (`configure`, `call`, `later`). The app writes
`func exactNativeModule() -> ExactNativeModule`.

- **Rust:** `exact_js::swift_native_module!()` declares the symbols in the
  app's crate, so nothing references them unless it links Swift, and
  defines `native_module` for `Module::with_native`.
- **Build:** `exact_js_bake::swift_native(&[…])` compiles the bridge and
  the app's sources for Cargo's target at the deployment target the host
  build chose (LLP 1036.001 area; the `minimumOS` change). It rebuilds
  only what is stale and links the result.

## Not in this RFC

Streams of partial results, such as words as they are transcribed or
tokens as they are written. That is LLP 1016.000's question. A long call
answers once.
