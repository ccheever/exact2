# Narrow re-check: kept iOS module roster (50ac0e4456), Grok

**RIGHT**

No findings.

1. **(a)** A `--sim` or `EXACT_SIM` value is kept only when it is an iOS simulator's name or UDID (`SimRuntime.iOS`). Anything else, including an unknown name or an Apple TV, is dropped. The route then uses a booted iOS simulator, or `simulator(null)` if none is booted — the same fallback as before `1bf3022d2d`. `null` does not re-read `EXACT_SIM`. A matched iOS simulator is passed by UDID to `simulator()`, booted if it is down, and shut down only when `bootedHere` is set.

2. **(b)** D8.4 is accurate. `ExactFirstFrameModules` in the device Info.plist is the first-frame list from that Mac, Catalyst, or simulator read. The running host uses the plist, not a fresh `views` evaluation, so an `#available` split by OS was already outside what a device build could express. The arm key is the probe arguments (SDK and target included), source text, compiler, link flags, and each library and archive header by path, size, and mtime.

3. **(c)** The queue entry matches the probe. `moduleArgs` puts `-I` and each package `.o` under the fresh `webBuildDir`, those paths are in the probe key, and the `.o` bytes are not. A package app (`appleImplementations`, LLP 1133.001) misses the kept roster every build. On an iOS simulator build the Catalyst link of those `.o` files fails, so the simulator route runs again and boots one when none is booted.

4. **(d)** `host/apple/build.mjs` is 1499 lines.
