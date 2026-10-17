// Appended to the prelude for `bootstrap_tests` (js/build.rs): bootstrap hides
// a payload behind a Proxy whose `get` trap answers undefined, publishes one
// under a symbol after making Reflect.ownKeys list only names, and plants a
// property on Object.prototype, which hardening then freezes. The probe runs
// after hardening, with readers it took before any of that, and reports what
// the freeze left open.
(function () {
  "use strict";
  var getOwn = Object.getOwnPropertyDescriptor, ownKeys = Reflect.ownKeys;
  var protoOf = Object.getPrototypeOf, isFrozen = Object.isFrozen;
  var payload = { inner: {} }, symbolic = { inner: {} };
  Object.prototype.en = "planted";
  globalThis.__exact_hidden_box = new Proxy({ payload: payload }, {
    get: function () { return undefined; },
  });
  Reflect.ownKeys = Object.getOwnPropertyNames;
  var box = {};
  box[Symbol.for("payload")] = symbolic;
  globalThis.__exact_symbol_box = box;
  globalThis.__exact_probe = function () {
    // Every object reachable by descriptor from the bindings hardening locked
    // (a binding still configurable was added after it), as harden.js walks.
    var seen = new Set(), queue = [], objects = 0, open = 0;
    var reach = function (d) {
      if (!d) return;
      if (getOwn(d, "value")) queue.push(d.value);
      else queue.push(d.get, d.set);
    };
    seen.add(globalThis);
    ownKeys(globalThis).forEach(function (key) {
      var d = getOwn(globalThis, key);
      if (d && !d.configurable) reach(d);
    });
    queue.push(protoOf(globalThis));
    while (queue.length) {
      var object = queue.pop();
      if (object === null || (typeof object !== "object" && typeof object !== "function")) continue;
      if (seen.has(object)) continue;
      seen.add(object);
      objects++;
      if (!isFrozen(object)) open++;
      ownKeys(object).forEach(function (key) { reach(getOwn(object, key)); });
      queue.push(protoOf(object));
    }
    return JSON.stringify({
      objects: objects,
      open: open,
      payload: isFrozen(payload) && isFrozen(payload.inner),
      symbolic: isFrozen(symbolic) && isFrozen(symbolic.inner),
      firstDay: new Intl.Locale("en").getWeekInfo().firstDay,
    });
  };
})();
