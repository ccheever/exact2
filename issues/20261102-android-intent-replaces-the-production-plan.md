# An Android intent replaces a production app's plan

**Status:** Closed
**Systems:** Android activity (`host/android/canvas/src/main/java/dev/exact/canvas/ExactActivity.kt`, `host/android/app/src/main/AndroidManifest.xml`), host boot (`host/linux/src/app.rs`, `host/linux/src/android.rs`, `host/linux/src/fetch.rs`)
**Severity:** P0
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** Any other app on the device can cold-start an Exact Android app and make that process boot an unsigned plan. The app's own data module stays in place, so the replacement plan runs with the app's storage, grants and sources. A production bake does this. The same extras also set `EXACT_DEV_PLAN`, which keeps polling after first pixel.

**Cause.** `MainActivity` is exported. On the process's first activity, `ExactActivity.environment` copies every intent extra whose name starts with `x_` into `EXACT_` plus the rest of the name, uppercased, with overwrite. `host/linux/src/android.rs` then boots with `Config::from_env_static`. A production bake drops `EXACT_AGENT`, `EXACT_HATCHES` and `EXACT_AGENT_*` (`ignore_agent_variables`) and then still reads `EXACT_PLAN` and `EXACT_DEV_PLAN`. A file path is `std::fs::read`. A URL is `fetch_changed`, which checks that the envelope's sha256 matches the bytes that same response names, and checks no signature. An envelope with no Rust section leaves `module` empty, so boot keeps `D::default()` and runs the fetched plan. `runner/src/delivery.rs` describes a production bake as taking no development override.

**Reproduction.** With the target app not running, another app starts it by explicit component name and sets the extra `x_plan` to an `https` URL whose envelope and plan hash match each other and declare no Rust module. The process logs `plan ←` that URL and presents the fetched plan. `x_dev_plan` set to a URL that is not ready yet still marks the locator explicit and the poller keeps requesting it.

**Fix.** On a production bake, ignore `EXACT_PLAN`, `EXACT_DEV_PLAN` and the other development switches, as `EXACT_AGENT` is ignored. Stop copying arbitrary `x_*` extras into the environment; name the switches a benchmark build is allowed to set. A plan that did not come from the embedded bytes or a signed update must not boot.

**Acceptance.** A production APK cold-started with `x_plan` or `x_dev_plan` presents the baked plan and does not fetch or open the extra. A development build can still point `EXACT_PLAN` at a local file from a shell the developer controls.

**Resolution.** Production launches discard unsigned plan and development-server overrides before configuration reads them. Android imports a fixed set of development extras only when the baked APK metadata enables development; exported intents cannot set arbitrary Exact environment variables.

**Verification after the fix.** The isolated production/development environment regression passed in the Linux host suite. Android canvas sources compiled against the installed SDK. No Android device was available.
