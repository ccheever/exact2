# A listener can drop the first reply to a new connection

**Status:** Closed
**Severity:** P2
**Systems:** exact-listen, native listener executor
**Author:** Codex
**Date:** 2026-11-02

**Problem.** A mapper that sends a greeting on `connection` can lose it. The
listener publishes the event before inserting the connection into its outgoing
map. The app can consume that event on another thread and send while the map
still says the connection is gone. The executor silently drops that send.

**Cause.** `host/listen/src/lib.rs:485` calls the sink before the insertion at
line 491. `Listener::send` returns `Refused::Gone` for a missing map entry.
`host/apple/src/executor_listen.rs:441` treats this result as a dropped send,
without retrying. The same executor is used by the Linux host.

**Reproduction.** On revision `5b94c216a6fee8fca42026f5cb2e9a80063da03a`, loaded
the checkout's built `libexact_listen.dylib` through its C ABI. The event callback
queued `connection` and waited on a barrier before returning. Another thread
consumed the event and called `exact_listen_send`: it returned `1` (Gone).
After releasing the callback and receiving a line, the same send returned `0`
and the client received `welcome\n`. No product code was changed.

**Fix.** Install the connection's send state before exposing its event, then
remove that state if the sink refuses the connection. Add a synchronized test
that replies while the connection callback is still returning.

**Resolution.** The outgoing connection map is installed before publishing the connection callback; a refused callback removes the provisional entry.

**Verification after the fix.** The real-socket regression sends a reply before the connection callback returns and receives its bytes. All 19 listener tests passed.
