# @exact/background-transfer: drives by hand

What the agent cannot drive: iOS relaunching a terminated app for its
transfers' events, a person's force-quit, and a phone before its first
unlock (LLP 1133.017 D10). Record the date, the OS version and what
happened beside each step when you run it.

Build the fixture as `test/apple.mjs` makes it, for the phone
(`bun host/apple/build.mjs --ios --device --run bgtfixture-apple` with
`EXACT_APP_DIR` set to the made app), against a copy of that suite's server
the phone can reach. The host's lines are in the system's log, subsystem
`dev.exact`, category `wakes`.

1. **In front.** Tap Upload u1. The server receives `hello u1` with the
   `Authorization` header; the app shows `u1 success success  201 kept u1`
   and the ids line empties.
2. **Terminated by the system.** Make the server hold `/upload`'s answer
   for 60 s. Tap Upload u2, go Home, and kill the process from Xcode's
   debugger before the answer. See the server's `/heard` for u2 within
   seconds of the answer, with no launch by hand: iOS relaunched the app,
   the host booted the session with no view, and the handler ran.
3. **A person's force-quit.** As 2, but swipe the app away in the app
   switcher. Nothing is heard while it stays closed. Open it: the app
   shows `u2 abort failure aborted`.
4. **Before the first unlock.** As 2, then restart the phone before the
   answer and leave it locked. Record whether anything is heard before the
   first unlock (D10's staging is not built, so it should not be), and what
   the app shows when opened after it.
5. **A large download.** Serve a file of several hundred megabytes at
   `/file`, tap Download d1 and lock the phone. Record how long it took and
   whether `saveTo` holds the whole file.
