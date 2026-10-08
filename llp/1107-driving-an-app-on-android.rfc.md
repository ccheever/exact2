# LLP 1107: Driving an exact2 app on Android

**Type:** RFC
**Status:** Stage 1 built, 2026-10-07; §4.1 (Hermes for Android) built the same day with a locally built bundle, pending an Ibex release. Charlie approved an Android build and agent carrier (2026-10-07: "go ahead then yes"); Android is admitted (`rules/DEFERRED.md` §Surfaces, 2026-10-07).
**Systems:** `scripts/agent-android.mjs` (new), `scripts/agent.mjs` (the `android` carrier), the Linux host's agent (`host/linux/src/agent.rs`, unchanged), the authoring bench (`ccheever/authoring-bench`)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-07
**Related:** LLP 1012 and 1079 (the agent's operations); LLP 1015 §5 (the Linux host's agent over stdio); LLP 1076 §3.3–§3.4 (the Android Canvas host and the swapchain host); LLP 1087 (the authoring bench); `QUEUE.md` "Android is admitted: what that owes"

## 1. Summary

The agent can now drive an exact2 app on an Android emulator or phone: `bun scripts/agent.mjs android --app <app> …` and `--test <file>`, with the operations the Linux host answers (tree, layout, tap, type, clock, screenshot, state, logs, prefer, `fail fetch`). World save and restore are refused on Android, as the carrier refuses them on every host it has no transfer for, and native modules (`EXACT_NATIVE_LIBS`) are not built or pushed.

The carrier runs the app's **Linux host built for `aarch64-linux-android`**, headless, under `adb shell`:
- It runs the runner, kernel, layout, text and painter, compiled with the host's Android-gated code, against bionic and the phone's own fonts (`/system/fonts`).
- It does not run the Android lane's Canvas reader (LLP 1076 §3.3), which paints through a Kotlin view kept outside this repository.

It works for apps whose data is Rust (Caltrain and the other in-repo apps), and, with a locally built Hermes bundle (§4.1, `scripts/hermes-android.mjs`), for apps whose data is TypeScript (duo-lab's four sources answer on the emulator). What `exact new` writes still lacks the `linux/` crate an Android build needs (§4.2).

## 2. What was found (2026-10-07)

- **The Android host** is two hosts inside `host/linux`, both under `cfg(target_os = "android")`:
  - the Canvas host (`canvas.rs`, `canvas/jni.rs`: an op stream a Kotlin view replays; JNI for `dev.exact.bench.exactcanvas.Native`);
  - the swapchain host (`android.rs`: vello into an `ANativeWindow`).
  - Neither carries the agent.
  - The Kotlin reader and the app libraries that call `canvas_jni!` live in the Android lane's own project, not in this repository; no app here builds for Android.
- **Tooling:** only the operator Mac has an Android SDK:
  - NDK 27.0 and 27.1, an arm64 API 36.1 emulator image and an AVD;
  - build-tools 35–36.1.
  - The mini and the Studio have none. No phone was attached. The Hetzner builders have `adb` but no `/dev/kvm` access, so no emulator.
- **The Linux host's headless path has no OS gate.** On every target other than Linux it is the only path, so the agent mode (`EXACT_AGENT=1`, JSON lines over stdio) runs on Android unchanged.

## 3. Decisions

### D1 — Drive the Linux host headless on Android, not the Canvas reader

The carrier pushes the app's Android-built Linux host and its `assets/` to `/data/local/tmp/exact/<app id>` and starts it with `adb shell -T` and only the `EXACT_*` environment. The device supplies:
- **`HOME`:** inside that directory, for the update store and scratch stores;
- **`EXACT_FONTS`:** `/system/fonts`, so the phone's faces draw (the desktop carriers' pinned DejaVu is dropped);
- **`EXACT_NATIVE_LIBS`:** where native modules sit.

A screenshot is written on the device and pulled to the path asked for.
- **Per drive:** each drive deploys into a run directory of its own (`run-<pid>-<time>`, removed when its process ends; one a killed `adb` orphaned goes after a day), so two drives of one app share neither a binary nor a screenshot. An authored test run's scratch stores carry the run's own tag, as on the desktop.
- **Kept:** `HOME` is the app's and survives, so a named scratch store (`--storage`) persists between drives as on the desktop carriers.
- **Plans and URLs:** a local `--plan` is pushed and read on the device, with the replacement Rust module beside it (`rust/`) when there is one; a launch URL reaches the host as its argv. Fonts are `/system/fonts` unless the drive names `EXACT_FONTS` or `EXACT_FONT`.
- **A known limit:** a drive whose local `adb` client is killed can leave its process running on the device until that session ends. It cannot touch another drive's files, but it shares the app's `HOME`. No APK, Gradle or Kotlin is involved, and the host's own agent answers, so the carrier's contract is the Linux one.

The trade-off: this proves the app's logic, layout and paint on Android's ABI, libc and fonts, but not the Canvas reader's drawing, not touch through Android's input system, and not the soft keyboard. When the Android lane brings the reader into the repository, a `--reader` carrier can drive the APK instead; the operations stay the same.

### D2 — The CPU painter by default

The GPU painter (vello on wgpu Vulkan) crashes under the emulator's SwiftShader Vulkan (a segfault before the first frame). The carrier sets `EXACT_PAINTER=cpu` unless the environment names a painter; a phone can take `EXACT_PAINTER=gpu`.

### D3 — Build with the NDK's clang, from the SDK the machine has

`bun scripts/agent-android.mjs build <app>` runs the app's Linux build with `--target aarch64-linux-android`. It sets the NDK's `aarch64-linux-android30-clang` as linker and C compiler:
- The NDK is `ANDROID_NDK_HOME`, else the newest under the SDK's `ndk/`.
- The SDK is `ANDROID_HOME`, else `~/Library/Android/sdk` or `~/Android/Sdk`.
- `ANDROID_SERIAL` picks the device; otherwise the first ready one.

The Rust target is installed once by hand (`rustup target add aarch64-linux-android`, in the app's directory so its pinned toolchain gets it); the build names that command when it is missing. This lives beside `agent.mjs`, not in `scripts/app.mjs`, which is at the line cap.

## 4. What stage 2 needs (not built)

1. **Hermes for Android: built locally, an Ibex release owed.** The pinned Ibex release ships Apple, Linux and Windows bundles only. Charlie: "give it a shot" (2026-10-07). As built:
   - **The bundle:** Ibex's own `scripts/build-hermes-vanilla-release.sh` gains an `aarch64-linux-android` target (Ibex branch `android-hermes-bundle`, local and not pushed; the patch is for Charlie to send). The pinned Hermes commit, unmodified (an empty patch set, as the receipt requires), cross-built with the NDK at API 30: MinSizeRel, `ANDROID_STL=c++_static`, Unicode Lite and no Intl (Hermes's own `HERMES_IS_ANDROID` build needs fbjni and a JVM for Unicode and Intl, which a native process has neither of). `hermes.cpp` includes `<fbjni/fbjni.h>` under `__ANDROID__` only to attach its finalizer thread to the JVM; a no-op stand-in header (recorded in the receipt by digest) keeps the source unmodified, since exact's JavaScript holds no JNI references. The canonical receipt (schema 2, HBC 99, the same bytecode as the host's `hermesc`) binds the four archives. 63 MB compressed; the lean VM archive is 103 MB before linking.
   - **Installing it:** `bun scripts/hermes-android.mjs build` runs that script from an Ibex checkout with the target (`IBEX_DIR`) and unpacks the result to `~/.cache/exact/hermes-android/<sha256>/` (`current` names it). `agent-android.mjs build` passes it as `HERMES_LEAN_SYS_DIR_aarch64_linux_android`.
   - **`hermes-lean-sys`** (vendored, EXACT-PATCHES.md §8, the same change on the Ibex branch): a per-target install override, `HERMES_LEAN_SYS_DIR_<target>`, read before `HERMES_LEAN_SYS_DIR`, so the host's instance (exact-js's build-dependency) keeps its pinned bundle; and Android's link line (`c++_static`, `c++abi`, `log`, `dl`, `m`).
   - **exact2:** `js/engine_os.rs` lists `android` (a target left out builds, then refuses every TypeScript app at run time); `js/src/shim.cc` leaves Ibex's Intl group out on Android (which defines `__linux__` too, so the Linux check asked for an Intl the bundle lacks and creating the runtime threw); the NDK build links one static C++ runtime (`CXXSTDLIB_aarch64_linux_android=c++_static`, as the bundle does).
   - **No Intl on Android yet:** `Intl.*` is absent in a TypeScript source there. Apple and Linux keep theirs.
2. **A Linux crate in `exact new` apps. Built 2026-10-07** (LLP 1086): the scaffold writes `linux/` in the `duo-lab-linux` shape, with `bun exact.mjs linux` and `bun exact.mjs android` build verbs. `--update` adds the crate to an older app. Building it found a break that had stopped every TypeScript app's Linux crate from compiling since LLP 1047.001 D7 (c043bc7f4). With no Rust module the bake's entry was the bare embedded source, which has no `Default`, and the Linux and Windows hosts make their source with `Default`. `js/bake` now emits a replacement-free `Swappable::off` there, which links no executor.
3. **The bench's Android cells**, once 1 and 2 exist:
   - **Where:** an adapter in the bench runner (`--platforms web,android`), with cells on a machine whose emulator runs (§4.5).
   - **How:** the trial's builder gets `bun exact.mjs agent android …` and `test android` verbs. The scripted graders run the same scenarios through `agent.mjs android`.
4. **The reader carrier (D1's other half)**, when the Android lane moves its Kotlin reader into the repository.
5. **The mini's toolchain (installed 2026-10-07; the emulator does not run yet).**
   - **Installed under `~/android`, no sudo:**
     - a JDK (Temurin 21, 336 MB);
     - the SDK (6.4 GB): command-line tools 23.0, platform-tools 37.0.1, emulator 37.2.12, android-36, the `android-36.1` Google Play arm64 image and NDK 27.1.12297006;
     - emulator 37.3.3 beside it in `~/android/emu373`.
   - **`. ~/exact2-verify/android-env.sh`** sets `JAVA_HOME`, `ANDROID_HOME`, `ANDROID_NDK_HOME` and `PATH`.
   - **`~/exact2-verify/android-emulator.sh start|stop`** creates the AVD (`pixel_6` with the Mac AVD's 1080×2400 at 420 dpi; the mini's device list has no `medium_phone`), boots it headless and stops it by its recorded PID.
   - **The build works there.** The emulator does not:
     - Both 37.2.12 and 37.3.3 report Hypervisor.framework as working (`-accel-check`), then print "hvf is not enabled on this aarch64 host" when the VM is made, and the guest's CPU threads hang. 37.3.3 got as far as an offline device.
     - The operator Mac's emulator (36.4.10) runs. The repository no longer offers 36.x.
     - Another session runs colima/lima VMs (Virtualization.framework) on the mini, a likely contender for the hypervisor, which this lane did not stop.
   - **Until it runs:** Android cells go on the operator Mac, or the mini once those VMs are stopped or the emulator fixed.

## 5. Verification (2026-10-07, the operator Mac, emulator `Medium_Phone_API_36.1`, arm64)

- **Build:** `caltrain-linux` built for `aarch64-linux-android` (host-dev, 28.5 MB).
- **Screenshot:** a headless screenshot (`EXACT_SHOT`) of Caltrain's home screen on the emulator draws with Roboto from `/system/fonts`.
- **Drive:** `agent.mjs android --app caltrain "tap change-station" "type station-search Palo" "clock +500" "screenshot …" state` answered every operation (`carrier: "android"`). The screenshot shows the station search with "Palo Alto" matched and the train picture from the pushed assets.
- **Tests:** `agent.mjs android --app caltrain --test apps/caltrain/app.test.contract` passed 3 of 3.
- **The TypeScript blocker, first:** `agent-android.mjs build duo-lab` stopped at `hermes-lean-sys`: "unsupported Hermes target aarch64-linux-android".
- **With the Android bundle (§4.1):** `agent-android.mjs build duo-lab` links (31 MB, host-dev); `agent.mjs android --app duo-lab "clock data" logs screenshot` journals `query items`, `query rows`, `query doc`, `query feed` and each `answered`, then `data_ready`, and the screenshot shows the Fold page's list and document. The same app on the Linux host (macOS, headless) answers the same four.

## 6. Revisions

- r1, 2026-10-07: stage 1 built. Astra and Grok reviewed it blind, two rounds:
  - **Round 1** (both said LAND WITH FIXES): a drive's own run directory; `HOME` kept for `--storage`; `--plan` pushed; launch URLs as argv; the executable quoted; the build in the resolved workspace and target; source maps from the Android binary's bake; the missing-target hint.
  - **Round 2** (Astra said LAND WITH FIXES): test runs' own store names; run directories removed at process end instead of swept by age; a plan's `rust/` module pushed with it; a session's fonts kept.
