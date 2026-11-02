# A production Linux build serves the screen over VNC with no authentication

**Status:** Closed
**Systems:** Linux display (`host/linux/src/display.rs`, `host/linux/src/vnc.rs`, `host/linux/src/app.rs`)
**Severity:** P2
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** A production Linux binary started with `EXACT_VNC=1` listens on `0.0.0.0:5900` and serves the framebuffer. Any client that completes the handshake receives frames and injects pointer and keyboard input. The RFB challenge is not checked.

**Cause.** `Config::from_env_lent` stores `EXACT_VNC` after `ignore_agent_variables`, which removes only the agent and hatch variables. `display::run` calls `Vnc::start`. The address `"1"` is `0.0.0.0:5900`. For a 3.3 client, `serve` writes a security type of 2, a 16-byte challenge, reads 16 bytes into `_response`, and writes success 0. Later minors are offered security type None. `vnc` is compiled only for `target_os = "linux"`. The module comment describes this as a development tool and states that nobody is authenticated.

**Reproduction.** Launch a production bake with `EXACT_VNC=1`. From another machine, open a VNC client on port 5900 and send a pointer event. The app receives it. The log line includes `vnc on :5900`.

**Fix.** Drop `EXACT_VNC` in a production bake, with the other development switches. A development listen should bind a loopback address unless the variable names a different one, and should check the password it claims to ask for.

**Acceptance.** A production binary with `EXACT_VNC=1` opens no port. A development binary with `EXACT_VNC=1` is reachable only from the same machine, and a wrong password is disconnected.

**Resolution.** Production removes VNC configuration. Development defaults to loopback and requires a one-to-eight-byte password, checked with the RFB challenge-response protocol for 3.3, 3.7 and 3.8.

**Verification after the fix.** The Linux host suite passed 1713 tests, including production environment filtering, a known DES vector and real-socket handshakes accepting correct responses and rejecting wrong passwords. Linux display hardware was not used.
