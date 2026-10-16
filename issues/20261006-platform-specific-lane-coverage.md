# Qualify platform-specific carrier code in the async verification lane

**Status:** Open
**Systems:** Windows carrier, Linux KMS/input, async verification
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** rules/DEFERRED.md scoped Windows consumers; scripts/async.mjs

The repository's asynchronous workspace build runs on macOS. It compiles shared native code, but excludes the Windows window module and Linux-only display/input code. Tier 2 adds tvOS only. There is no repository-owned Windows/Linux qualification step in the current lane.

`host/windows/src/lib.rs` compiles `window` only under cfg(windows); other platforms compile a small “requires a Windows target” stub. Linux KMS/DRM and device input are also selected by target OS. A green root/workspace build on this machine therefore cannot detect syntax/API failures in those branches, let alone qualify a native window or device.

The Windows work remains scoped to the admitted Skirmish/Windows Desk consumers, and general Windows/Android work is deferred. This issue asks to verify code already shipped for those consumers, not broaden product scope.

Add target-native verification through the existing asynchronous fleet/lane: build the admitted Windows consumers and exercise their carrier; build/test Linux-qualified code on Linux, with headless checks plus a bounded device smoke where needed. If such evidence is already produced outside this repo, link and integrate its failures into the existing lane instead of duplicating infrastructure.

Acceptance: changing a cfg(windows) or cfg(target_os = "linux") carrier branch produces a corresponding platform build result. A deliberate compile failure is detected and attributed. Publish which runtime/device behavior remains unqualified; do not treat the macOS stub build as platform proof.

## 2026-10-11 lane result

Tier 2 now cross-checks `exact-linux` with `caltrain-linux` and `exact-windows`
with `windows-desk` on their real targets. Both checks passed on this Mac with
Zig supplying C and the Rust target libraries installed. This proves those
source branches compile. Window presentation, KMS, and input devices still need
native-machine runs.

**Remains open:** select and qualify the other Linux app crates. The all-app
cross-check reached `ibex2`'s target C++ binding after the pinned Linux Hermes
bundle and frozen Bun dependencies were installed, then failed because the Mac
has no `x86_64-linux-gnu-g++`. A target `CXX` setting or a stub-engine compile
may resolve it, but neither was verified within the three-round fix limit.

**2026-10-14:** #532 fixes the cross-check half: tier 2's Linux step sets
`CXX_x86_64_unknown_linux_gnu=zig c++ -target x86_64-linux-gnu` and checks
`-p '*-linux'` (the host and all 23 app crates; ibex2's and exact-js's C++
compiles for the target), green on this Mac. Remaining: native window, KMS and
input-device runs on Linux, and the Windows carrier on Windows.

## 2026-10-16 native runs

At 07fdd0ff4, on the fleet. Both machines ran the pinned Rust (1.97.0) and Bun
(1.4.2).

**Linux** (`expo-build-450b`, Ubuntu 24.04, kernel 6.8, x86_64, no GPU):

- `cargo test -p exact-linux --lib --tests --no-fail-fast`: 1,072 pass with
  the fixes below. The run includes the Linux-only `display` (24 tests on a
  fake card), `input` (3) and `vnc` (3) modules.
  `text::ink_tests::actual_color_bitmap_placement_matches_full_paint` needs a
  colour emoji face installed; this box has none, so it fails there. It passes
  with `fonts-noto-color-emoji` named through a private `FONTCONFIG_FILE`. The
  Linux app crates have no tests (`test = false`).
- `cargo build --profile host-dev -p caltrain-linux`, then
  `bun scripts/smoke.mjs linux`: ok in 52 s (boot 32 ms, canvas readback
  0.00% off its reference, motion, deck, precedence, 3/3 Contract tests).
- `scripts/agent.mjs linux`: `tap change-station`, `type station-search Palo`,
  `clock settle`, `state`, `screenshot` reached the app (`query: "Palo"`, one
  match, focused field).
- `cargo test -p exact-windows` on Linux: its stub, 0 tests, compiles.

**Windows** (NucBox EVO-X2, Windows 11, MSVC):

- `cargo test -p exact-windows --lib --tests`: 1/1 pass.
- `cargo build -p windows-desk`, then `windows-desk --smoke-exit`: exit 0,
  `PASS: native EDIT -> Contract state -> HWND; Unicode; checkbox/topmost;
  save bytes; sample selection; font; resize`. In the logged-on console session
  (a scheduled task) `--capture` drew the whole window: menus, listbox, EDIT
  controls, combobox, checkbox, status bar. From the SSH session (session 0) it
  draws black.
- Skirmish (a local clone of its 144e508; `exact.mjs update` and `lock` against
  this exact2; its Contract moved to today's grammar: `T[]`, `if`, `for`):
  `host/windows/build.mjs` builds it. `agent.mjs windows` drives it (`tap play`,
  `clock +2000`, `screenshot` of the battle at 1280×720). The native winit
  window with `EXACT_AGENT_WINDOW_FRAMES=600` and `EXACT_AGENT_WINDOW_TAP=play`
  presented 600 frames: interval p50/p95 17.05/17.69 ms, paint and readback
  10.36/15.97 ms, upload and present 0.62/0.95 ms, exit 0.
- Grey box (`game/games/greybox`) builds and runs the same way: 240 window
  frames, interval p50 17.07 ms. Its proof is not pinned for Windows
  (`pins.json` hosts: linux, web). On Windows it fails 3 checks: the entity
  tap's delivery is `presenter`, a branch only Linux skips; the continuation
  save differs; the world journal lacks `loop wind on`.
- `cargo test -p exact-linux --lib --tests` on Windows: the shared presenter's
  1,032 tests pass when `TEMP` has no space. Under this box's
  `C:\Users\Charlie Cheever\…` the agent's two picker tests fail: a one-line
  pick answer is split on whitespace (`runner/src/runner/picker.rs`
  `answer_paths`; the web, Apple and driver parsers do the same). That is a
  real bug for any absolute path that contains a space. In 1 of 3 full runs,
  `executor::core::idle_tests::an_idle_pool_is_not_woken` counted 87 worker
  wakes while idle. Its "every worker back in its wait" is a fixed 100 ms sleep.

Fixed here: `a_request_runs_on_the_executor_and_its_reply_commits` read one
packet of the request and then closed. The reset lost the reply in 6 of 240
loaded runs on Linux and in both full runs on Windows. Its server now reads
the whole request: 0 of 240 on Linux, and it passes on Windows. Two tests that make their fixture with `chmod` or
`mkfifo` are `cfg(unix)`; they stopped `exact-linux`'s tests from compiling on
Windows.

A deliberate type error in `host/windows/src/window.rs` and in
`host/linux/src/input.rs` is caught by tier 2's two cross-checks, each naming
its file and line, while the macOS check of the same crates passes.

**GMKtec M8** (Ubuntu 24.04, kernel 7.0, amdgpu Radeon, a USB keyboard; the
user is in `video`, `input` and `render`). It has no C toolchain, so a private
Zig 0.17 served as `cc` and `c++`; Bun 1.4.2 was installed privately.

- At origin/main (a15f44bdc): `caltrain-linux` builds in 2 min 22 s.
- KMS cannot take the card. `caltrain-linux` opens `/dev/dri/card0` and reads
  its resources, then says `exact: no display: no connected connector with a
  mode`: all ten connectors (DP-1 to DP-8, HDMI-A-1, Writeback-1) are
  disconnected, so no monitor is attached. gdm's greeter (gnome-shell on tty1,
  seat0, idle ten days) also holds DRM master: `DRM_IOCTL_SET_MASTER` from
  this user returns `EACCES`. The SSH sessions have no seat, so switching to a
  free VT needs root. `EXACT_VNC=1` stops at the same message: the VNC server
  starts only after the display opens.
- evdev: the host's `exact_linux::input::Input::open()` (a scratch example,
  not committed) opened 2 devices, the keyboard's two interfaces. It decoded
  `KEY_LEFTSHIFT` down and up as `Key { code: 42, shift: true, down: true }`
  and `{ shift: false, down: false }`; the probe wrote those events to
  `/dev/input/event2`, which evdev feeds to every reader of that device.
  Nobody was at the keyboard, so no physical keypress was observed. Without a
  display, the app does not open its input.
- `cargo test -p exact-linux --lib --tests` at 07fdd0ff4: two tests fail on
  this machine's fonts, which make sans-serif Noto Sans (521 faces) where
  450b had DejaVu only: `shelfs_library_is_read_bar_then_page_then_navigation_bar`
  and the neutral look's `gray tabs have no fill`. Both pass with a
  `FONTCONFIG_FILE` that names DejaVu only. `smoke.mjs linux` passes (39.8 s).
- At a15f44bdc two more tests fail, on this machine and on the Mac alike:
  `timer_demand_empty_receipts_do_not_repaint_but_explicit_clock_does` and
  `a_held_press_multiplies_scale_about_the_origin_even_under_reduced_motion`.
  Bisection names 793907f4f ("Present Windows game canvases directly on the
  GPU"). The smoke fails its motion checks there (`at 250 ms the linear box
  is 75 wide, not 100`), probably for the same cause.

**Remains unqualified:**

- Linux KMS/DRM presentation, page flips at the display's refresh, the VNC
  server, and evdev input reaching a running app. Neither fleet Linux box can
  run them: 450b's SSH user lacks `video` and `input`; the M8 has the groups
  but no monitor, and gdm holds master. A run needs a monitor (or an HDMI/DP
  dummy plug) on the M8 and gdm stopped, or the run started from the M8's VT
  as a local login, both of which need someone at the machine or root.
- Windows: input from a person's real devices (the drives inject through the
  presenter or the agent), DPI changes across monitors, and the Grey box proof
  failures above.
- Linux app crates beyond Caltrain natively: only tier 2's cross-check
  compiles them. Natively they need the pinned Linux Hermes bundle, which
  450b has not installed.
