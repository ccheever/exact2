// The LLP 0067 R4 intrinsic freeze (measured in LLP 0062 §3), run after the standard library is
// installed and before any module code.
//
// Three properties this walk must have, all of which the retired native
// __exactDeepFreeze also had: it reads property DESCRIPTORS so getters are not
// invoked, it is ITERATIVE so a deep graph cannot hit a native stack cap, and
// it tracks VISITED so cycles terminate.
//
// The global object itself is NOT frozen. Its existing bindings are locked —
// `Array` cannot be pointed at something else, and everything reachable from
// them is frozen — but the object stays extensible, so application code can
// add to it. That is the SES lockdown shape: shared intrinsics are immutable,
// the global object is a compartment's own. It is also what running Exact
// requires: its runtime anchors shared state on `globalThis` under 193
// distinct `__exact*` names, plus `process`, `window`, `self`, `global`, and
// `navigator`, and with a frozen global every one of those writes silently
// did nothing — the first symptom was a `TypeError` three modules later,
// reading a registry that had never been created. R1 is unaffected: nothing
// capability-bearing is on the global object to begin with, and a property
// an application adds is state, not authority.
(function () {
  "use strict";
  const seen = new Set();
  const queue = [];

  seen.add(globalThis);
  const globals = Object.getOwnPropertyNames(globalThis).concat(Object.getOwnPropertySymbols(globalThis));
  for (let i = 0; i < globals.length; i++) {
    let d;
    try { d = Object.getOwnPropertyDescriptor(globalThis, globals[i]); } catch (e) { continue; }
    if (!d) continue;
    if (d.configurable) {
      try {
        Object.defineProperty(
          globalThis,
          globals[i],
          "value" in d ? { writable: false, configurable: false } : { configurable: false }
        );
      } catch (e) {}
    }
    if ("value" in d) queue.push(d.value);
    else { queue.push(d.get); queue.push(d.set); }
  }
  try { queue.push(Object.getPrototypeOf(globalThis)); } catch (e) {}

  while (queue.length) {
    const obj = queue.pop();
    if (obj === null || (typeof obj !== "object" && typeof obj !== "function")) continue;
    if (seen.has(obj)) continue;
    seen.add(obj);
    try { Object.freeze(obj); } catch (e) {}
    // Names AND symbols: `Date.prototype[Symbol.toPrimitive]` and the RegExp
    // `Symbol.match`/`split`/... functions are reachable only by symbol, and a
    // walk by name left every one of them extensible.
    const keys = Object.getOwnPropertyNames(obj).concat(Object.getOwnPropertySymbols(obj));
    for (let i = 0; i < keys.length; i++) {
      let d;
      try { d = Object.getOwnPropertyDescriptor(obj, keys[i]); } catch (e) { continue; }
      if (!d) continue;
      if ("value" in d) queue.push(d.value);
      else { queue.push(d.get); queue.push(d.set); }
    }
    try { queue.push(Object.getPrototypeOf(obj)); } catch (e) {}
  }
})();
