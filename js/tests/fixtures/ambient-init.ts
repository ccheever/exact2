import "./inputs";
// A module that does not catch the diagnostic must fail to load, before
// either a bake or a runtime answer can observe its exports.
(globalThis as any).initClock = Date.now();
