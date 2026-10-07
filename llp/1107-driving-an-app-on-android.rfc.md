# LLP 1107: Driving an exact2 app on Android

**Type:** RFC
**Status:** Stage 1 built, 2026-10-07. Charlie approved an Android build and agent carrier (2026-10-07: "go ahead then yes"); Android is admitted (`rules/DEFERRED.md` §Surfaces, 2026-10-07).
**Systems:** `scripts/agent-android.mjs` (new), `scripts/agent.mjs` (the `android` carrier), the Linux host's agent (`host/linux/src/agent.rs`, unchanged), the authoring bench (`ccheever/authoring-bench`)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-07
**Related:** LLP 1012 and 1079 (the agent's operations); LLP 1015 §5 (the Linux host's agent over stdio); LLP 1076 §3.3–§3.4 (the Android Canvas host and the swapchain host); LLP 1087 (the authoring bench); `QUEUE.md` "Android is admitted: what that owes"

## 1. Summary

The agent can now drive an exact2 app on an Android emulator or phone: `bun scripts/agent.mjs android --app <app> …` and `--test <file>`, with the operations the Linux host answers (tree, layout, tap, type, clock, screenshot, state, logs, prefer, `fail fetch`). World save and restore are refused on Android, as the carrier refuses them on every host it has no transfer for, and native modules (`EXACT_NATIVE_LIBS`) are not built or pushed.

The carrier runs the app's **Linux host built for `aarch64-linux-android`**, headless, under `adb shell`:
- It runs the runner, kernel, layout, text and painter, compiled with the host's Android-gated code, against bionic and the phone's own fonts (`/system/fonts`).
- It does not run the Android lane's Canvas reader (LLP 1076 §3.3), which paints through a Kotlin view kept outside this repository.

It works today for apps whose data is Rust (Caltrain and the other in-repo apps). An app whose data is TypeScript (every app `exact new` makes, so every bench app) cannot run natively on Android yet, because the pinned Hermes release has no Android bundle (§4).

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
- **Per drive:** each drive deploys into a run directory of its own (`run-<pid>-<time>`, swept after an hour), so two drives of one app share neither a binary nor a screenshot.
- **Kept:** `HOME` is the app's and survives, so a named scratch store (`--storage`) persists between drives as on the desktop carriers.
- **Plans and URLs:** a local `--plan` is pushed and read on the device; a launch URL reaches the host as its argv.
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

1. **Hermes for Android (blocking for the bench).** `hermes-lean-sys` refuses `aarch64-linux-android` ("unsupported Hermes target"). The pinned Ibex release ships Apple, Linux and Windows bundles only. A TypeScript data module on Android needs an Ibex release with an Android bundle: Ibex work, outside this repository.
2. **A Linux crate in `exact new` apps.** The scaffold writes `web/` and `apple/` only. An app needs a `linux/` crate (the `duo-lab-linux` shape: `exact_js` over the baked bytecode, then `exact_linux::run`) to build its host for Linux or Android.
3. **The bench's Android cells**, once 1 and 2 exist:
   - **Where:** an adapter in the bench runner (`--platforms web,android`) and cells on the operator Mac, the only machine with an emulator. Or the mini, once an SDK and an AVD are installed there (a setup choice for Charlie).
   - **How:** the trial's builder gets `bun exact.mjs agent android …` and `test android` verbs. The scripted graders run the same scenarios through `agent.mjs android`.
4. **The reader carrier (D1's other half)**, when the Android lane moves its Kotlin reader into the repository.

## 5. Verification (2026-10-07, the operator Mac, emulator `Medium_Phone_API_36.1`, arm64)

- **Build:** `caltrain-linux` built for `aarch64-linux-android` (host-dev, 28.5 MB).
- **Screenshot:** a headless screenshot (`EXACT_SHOT`) of Caltrain's home screen on the emulator draws with Roboto from `/system/fonts`.
- **Drive:** `agent.mjs android --app caltrain "tap change-station" "type station-search Palo" "clock +500" "screenshot …" state` answered every operation (`carrier: "android"`). The screenshot shows the station search with "Palo Alto" matched and the train picture from the pushed assets.
- **Tests:** `agent.mjs android --app caltrain --test apps/caltrain/app.test.contract` passed 3 of 3.
- **The TypeScript blocker:** `agent-android.mjs build duo-lab` stops at `hermes-lean-sys`: "unsupported Hermes target aarch64-linux-android".
