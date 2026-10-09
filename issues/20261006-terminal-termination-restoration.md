# Restore terminal state on SIGTERM and SIGHUP

**Status:** Open
**Systems:** terminal host, coding harness
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P1
**Related:** LLP 1101 D7; host/terminal/src/term.rs

`term::run` puts stdin into raw mode and restores it only after `run_raw` returns or through a panic hook (`host/terminal/src/term.rs:573–592`). Ordinary termination signals bypass both paths. The parent shell inherits a raw, noncanonical TTY, and the fullscreen/cursor modes may remain active.

Reproduced safely on a private pseudo-terminal: recorded its termios, launched `target/debug/exact-terminal` with a minimal entry, waited until “Ready” was displayed and raw mode was entered, sent SIGTERM to that recorded child PID, then inspected the slave. The process exited with signal 15; the original termios were not restored and ICANON remained clear. The review harness restored its own private PTY afterward.

Arrange for catchable termination signals to wake the host loop and exit through its normal restoration path. Keep the signal handler minimal and use appropriate signal-safe notification. Cover SIGHUP and the relevant termination signals; SIGKILL cannot be restored by the child.

Acceptance: PTY tests terminate both inline and fullscreen hosts while idle and while data work is pending. Catchable termination restores the original termios and emits the exit sequences before the process ends. Normal exit and panic restoration continue to work.
