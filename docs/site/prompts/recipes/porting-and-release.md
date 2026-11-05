```prompt Port my app to Exact
Port an existing app to Exact in this folder, and make the build that ships. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/porting-and-release.md as the source for each step. The exact2 checkout has the same page at docs/recipes/porting-and-release.md.

1. Find the exact2 checkout (the folder with scripts/exact.mjs); if you cannot find it, ask me where it is. Ask me which app to port and where its code is, which platforms the port runs on, its name and bundle id, and whether it replaces the original on a device (then it keeps the original's name and bundle id).
2. Make the app with its name and id before the first build: `bun <exact2>/scripts/exact.mjs new <path> --name "<Name>" --bundle-id <id>`, then `cd <path>`. A bundle id changed later makes the next build cold and leaves the old app installed.
3. If the original is a React Native app, read its pitfalls first: `bun exact.mjs docs porting a react native app`.
4. Port the screens to app.contract, the data to app.ts, and a test in app.test.contract for each thing a person does. Sound, MIDI and a transport are Exact's: never port an audio or MIDI engine into a native module. Use a native module only for what Exact lacks, and tell me first.
5. To stop shipping a platform, run `bun exact.mjs update --without <platform>`; ask me before you do.
6. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json until they pass. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 9) and report it; do not keep trying.
7. Compare the port's screens with the original's on iOS: `bun exact.mjs agent ios --touch platform` makes each tap a real touch.
8. For the build that ships on iOS, start it early with `bun exact.mjs ios --prebuild --release`; later, `bun exact.mjs ios --release` waits for it. It ignores `EXACT_AGENT`, so drive the development build, not the release.
9. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
10. Tell me what you ported and what is left, what the tests and each verify reported, how the port compares with the original, and the launch command for each target platform.
```
