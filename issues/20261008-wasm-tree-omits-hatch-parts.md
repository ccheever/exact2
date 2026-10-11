# The wasm web host's agent tree omits a hatch's parts that the JS target joins

**Status:** Open
**Systems:** host/web (wasm agent tree), host/web-js/agent.js, access hatches
**Author:** Claude (Opus 5.5), for Charlie Cheever
**Date:** 2026-10-08
**Severity:** P2
**Related:** LLP 1075.003.000.001 §3.5; 2e4b89242 ("Access hatches, stage 3: the web's tree --ax joins a part by ownership"); apps/native-fixture/modules/web/index.js:95–102

Strict JS-target conformance (`host/web-js/conform.mjs native-fixture --strict`) fails at boot since b0d2f145..c248ad72: the JS target's tree has one more entry than the wasm oracle's (165 against 164). At tree #41 the JS target shows `7|View||||Verified|||`, the native-fixture web module's `seal` part (`e.parts = [{ id: 'seal', element: seal, role: 'button', label: 'Verified' }]`), and the wasm host's tree has no such entry; every later row is shifted by one.

2e4b89242 taught the web's tree (`tree --ax`) to join a part by ownership. The JS target's agent does; the wasm host's agent tree does not, so the two web hosts disagree for any app with a hatch that declares parts. One of them should change, as LLP 1075.003.000.001 §3.5 intends (likely the wasm host joining the part the same way). Found by the async lane on the mini (issue notes at 5d69f6b5f).
