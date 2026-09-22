// The two runtime helpers ESM lowering needs.
//
// Kept to two, and both are interop rather than semantics: everything the ES
// module spec actually says is expressed by the lowered code itself.
(function (global) {
  "use strict";

  // `import d from './x'` where './x' is CommonJS. A CJS module has no
  // `default`, and treating its module.exports as one is the interop every
  // bundler converged on. The __esModule marker is how a lowered ES module
  // says "my default really is .default".
  global.__ibex2_default = function (module) {
    if (module && (module.__esModule || typeof module.default !== "undefined")) {
      return module.default;
    }
    return module;
  };

  // `export * from './x'`. Live, like every other export, and `default` is
  // deliberately excluded — the spec does not re-export it.
  //
  // forEach, not `for (const name of ...)`: on the pinned Hermes a for-of
  // `const` does NOT create a per-iteration binding, so every getter closes
  // over the LAST name and `export * from` republishes one value under every
  // key. forEach gives a fresh invocation per element, so the capture is
  // correct regardless. See LLP 0062 §5.
  global.__ibex2_export_all = function (target, source) {
    if (source === null || typeof source !== "object") return;
    Object.getOwnPropertyNames(source).forEach(function (name) {
      if (name === "default" || name === "__esModule") return;
      if (Object.prototype.hasOwnProperty.call(target, name)) return;
      Object.defineProperty(target, name, {
        get: function () {
          return source[name];
        },
        enumerable: true,
        configurable: true,
      });
    });
  };
})(globalThis);

// Dynamic import(). The module is already compiled and local, so there is no
// I/O to wait for — but the contract is a promise, and callers rely on the
// continuation running in a microtask rather than synchronously.
//
// It takes the importing module's own `require`, so a relative specifier
// resolves against the right file and the imported module is looked up under
// its own resolved name for grants. A failure rejects rather than throwing
// synchronously, which is what `import()` promises.
(function (global) {
  "use strict";
  global.__ibex2_dynamic_import = function (req, specifier) {
    try {
      const exported = req(String(specifier));
      // The namespace of a CommonJS module has its module.exports as default,
      // matching the static-import interop above.
      if (exported && exported.__esModule) return Promise.resolve(exported);
      const namespace = Object.create(null);
      Object.getOwnPropertyNames(exported || {}).forEach(function (name) {
        Object.defineProperty(namespace, name, {
          get: function () {
            return exported[name];
          },
          enumerable: true,
        });
      });
      if (!("default" in namespace)) namespace.default = exported;
      return Promise.resolve(namespace);
    } catch (error) {
      return Promise.reject(error);
    }
  };
})(globalThis);
