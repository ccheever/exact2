// The runtime's own clock, timers, commits and journal, for a chunk loaded lazily (motion's drags, a TypeScript
// stream) to use without importing rt.js: a chunk that imports the module that loads it is shared code a single-file
// bundle can't make (the paint harness's, fa737ea8ca), and dynamic-chunks.test.js guards it. rt.js fills it at load.
export const rt = {};
