---
name: 20261006-native-module-termination
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-bug (reproduced)
blocks: [20261005-ssh-password-and-remote-open]
upstream_url: null
reproduced_on: 6d41ae81194b707a66d01049f411351d900c2dbc
---

# macOS termination can skip native module teardown

Closing the last window with ⌘W left three `/usr/bin/ssh` tunnel processes alive,
reparented to PID 1, during PR #157's real SSH acceptance on 2026-10-06.
The app had an SSH password prompt open and existing connected environments.
This leaves loopback listeners and authenticated remote connections after the UI exits.

## Reproduction

1. Build and launch T3 Code with an isolated app identity and test storage.
2. Connect to a password-accepting OpenSSH server, then open another password prompt.
3. Record the app PID and its SSH child PIDs using `ps -axo pid,ppid,comm`.
4. Press ⌘W to close the last window. Query those recorded PIDs again.

Observed: app PID 40404 exited; SSH PIDs 41077, 42082 and 43277 remained with PPID 1.
Evidence: `target/ssh-live/close-processes-before.json` and
`close-processes-after.json` (local, ignored verification output).

`ExactMac/main.swift` defers `session.destroy()` from `windowWillClose` to the next
main-queue turn. Its delegate immediately permits termination after the last window
closes and has no `applicationWillTerminate` cleanup. In a diagnostic build, calling
`destroy()` on the remaining sessions from that notification left no recorded app or
SSH process alive (`close-fixed-before.json`, `close-fixed-after.json`). That diagnostic
framework patch is **not included** in this app PR. Session.destroy is idempotent.

## App workaround and remaining upstream work

T3Ssh observes `NSApplication.willTerminateNotification` and calls its own idempotent
cleanup. It cancels pending password requests, drops cached secrets, removes askpass
helpers and terminates managed tunnels. An AppKit test posts the notification and
checks that subsequent password requests fail as window-closed; a normal app close
with two live tunnels verified process cleanup separately: app PID 53997 and SSH
PIDs 54313/55132 all exited (`app-cleanup-before.json`, `app-cleanup-after.json`).

This workaround protects SSH only. Every module should receive its ordinary destroy
callback on normal application termination. Upstream needs a lifecycle regression test
covering both last-window close and quit with open windows. This draft has not been
published. A repository issue search for `SSH teardown termination` found no match.
