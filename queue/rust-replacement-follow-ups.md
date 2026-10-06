**Rust replacement follow-ups** — **LLP 1029.000**: native/browser/Wasmi replacement
and dev/prod controls are implemented. Next: physical-phone size/latency and
20-edit/50-replacement measurements, normalized native-baseline reuse, and
custom out-of-tree host composition; independent multi-module routing stays later.
Parent **LLP 1029** (Draft RFC, 2026-09-03), mixed by default and optional
executors, builds on LLP 1028 (the measurements: wasmtime runtime +0.49 MB / +Pulley
+0.58 / wasmi +1.0 / Cranelift +5.6, never shipped). Charlie's three rulings of
2026-09-03 — wasmtime optional, never mandatory even for Rust logic, the default app
mixed TS + Rust — are its Summary; §7 stages it (phone numbers first), §8 asks
Pulley-or-wasmi and four more. Awaits Charlie; the 1026/1027 amendment notes land at
acceptance. **LLP 1032** (Research, 2026-09-04) measured Charlie's "does wasm make
Hermes unnecessary?": QuickJS inside a wasm module on the same twin and seam is
1.5–3× the lean Hermes VM with precompiled desktop code, 40–134× under Pulley and
17–64× under wasmi (an interpreter inside an interpreter), and 1.5–2.1 MB as a
Pulley artifact against Hermes's 1.8 MB; 20/20 bytes on every engine. Feeds 1029 §8
and stage 0's phone afternoon (the iOS Pulley host builds; the phone was not
connected).

*Filed under “Next, in order (2026-08-29)”.*
