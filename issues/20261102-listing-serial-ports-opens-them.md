# Listing serial ports opens every node read-write

**Status:** Closed
**Systems:** Linux serial (`packages/serial/linux/lib.rs`)
**Severity:** P2
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** `attachedPorts` and `getPorts` open every tty node read-write to decide `granted`. Opening a tty asserts DTR and RTS. Dropping the last descriptor clears them. A USB-serial adapter wired with DTR to reset restarts the board on every list.

**Cause.** `record` sets `granted` by calling `granted`, which `open`s the path with read and write, `O_NOCTTY` and `O_NONBLOCK`. Those flags avoid a controlling terminal and a carrier wait. They do not leave the control lines idle. `EACCES` is the only error reported as not granted. The same open-to-test is used for USB and hidraw nodes, which do not pulse DTR, and still opens every node on a list.

**Reproduction.** Call `attachedPorts` with a USB-serial device plugged in and not opened exclusively. Each call opens and closes the tty. A board that resets on DTR resets.

**Fix.** Test permission with `faccessat` (`R_OK` and `W_OK`), or by reading the node mode, and do not `open` the device until the app opens the port.

**Acceptance.** Listing ports does not change DTR on an idle adapter. A node the process cannot open still reports `granted: false`. `open` of a granted port still opens it read-write.

**Resolution.** Enumeration checks read/write permissions with access() instead of opening each tty. Only an explicit connection opens the device.

**Verification after the fix.** The actual permission helper passed a local POSIX probe against a pseudo-terminal, denied and allowed file permissions, and a NUL-containing path. No physical serial board or DTR transition was measured.
