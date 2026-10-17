// The LLP 0067 R4 intrinsic freeze (measured in LLP 0062 §3), run after the standard library is
// installed and before any module code.
//
// Three properties this walk must have, all of which the retired native
// __exactDeepFreeze also had: it reads property DESCRIPTORS (an accessor's
// getter and setter, never its value) so getters are not invoked, it is
// ITERATIVE so a deep graph cannot hit a native stack cap, and it tracks
// VISITED so cycles terminate.
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
  // Everything the walk calls is captured here, and every descriptor it builds
  // or reads is classified by OWN fields only. Bootstrap code may pollute
  // Object.prototype (a planted `value` would make an accessor look like data,
  // and would leak into an attributes literal as a value to write) or delete
  // globals such as Function and Reflect, as Snapback 2 effects do.
  const O = Object;
  const getOwn = O.getOwnPropertyDescriptor;
  const names = O.getOwnPropertyNames;
  const symbols = O.getOwnPropertySymbols;
  const define = O.defineProperty;
  const freeze = O.freeze;
  const protoOf = O.getPrototypeOf;
  // Descriptor records inherit from Object.prototype. Only when bootstrap has
  // planted a `value` there does an `in` test lie; decide that once, since no
  // other code runs during the walk, and pay for the own-field lookup only then.
  const isData = ("value" in {})
    ? function (d) { return getOwn(d, "value") !== undefined; }
    : function (d) { return "value" in d; };
  const DATA_LOCK = { __proto__: null, writable: false, configurable: false };
  const ACCESSOR_LOCK = { __proto__: null, configurable: false };
  const keysOf = function (o) {
    const out = names(o);
    const syms = symbols(o);
    for (let i = 0; i < syms.length; i++) out[out.length] = syms[i];
    return out;
  };
  const seen = new Set();
  const queue = [];
  const reachByDescriptor = function (obj, key) {
    const d = getOwn(obj, key);
    if (!d) return;
    if (isData(d)) queue[queue.length] = d.value;
    else { queue[queue.length] = d.get; queue[queue.length] = d.set; }
  };
  // Past the global object, the walk reads each property's getter and setter
  // from its descriptor without building a descriptor record: for an own
  // property, __lookupGetter__ and __lookupSetter__ (ECMA-262 B.2.2.4, B.2.2.5)
  // return the descriptor's [[Get]] and [[Set]], and undefined for a data
  // property. A record for every property walked was most of what hardening
  // allocated, all of it garbage. Reflect.ownKeys gives names and symbols in
  // one array. Where bootstrap has removed any of these, the walk reads
  // descriptor records as before.
  const call = getOwn.call;
  const lookupGetter = O.prototype.__lookupGetter__;
  const lookupSetter = O.prototype.__lookupSetter__;
  const R = typeof Reflect === "object" && Reflect !== null ? Reflect : undefined;
  const lean = typeof call === "function" && typeof call.bind === "function" &&
    typeof lookupGetter === "function" && typeof lookupSetter === "function";
  const ownKeys = lean && R !== undefined && typeof R.ownKeys === "function" ? R.ownKeys : keysOf;
  const reach = lean
    ? (function () {
      const getterOf = call.bind(lookupGetter);
      const setterOf = call.bind(lookupSetter);
      return function (obj, key) {
        const get = getterOf(obj, key);
        const set = setterOf(obj, key);
        if (get !== undefined || set !== undefined) {
          queue[queue.length] = get;
          queue[queue.length] = set;
        } else {
          // A data property, or an accessor with neither function: [[Get]]
          // stops at the property __lookupGetter__ stopped at, so it runs no
          // getter.
          queue[queue.length] = obj[key];
        }
      };
    })()
    : reachByDescriptor;

  seen.add(globalThis);
  const globals = keysOf(globalThis);
  for (let i = 0; i < globals.length; i++) {
    let d;
    try { d = getOwn(globalThis, globals[i]); } catch (e) { continue; }
    if (!d) continue;
    const data = isData(d);
    if (d.configurable) {
      try { define(globalThis, globals[i], data ? DATA_LOCK : ACCESSOR_LOCK); } catch (e) {}
    }
    if (data) queue[queue.length] = d.value;
    else { queue[queue.length] = d.get; queue[queue.length] = d.set; }
  }
  try { queue[queue.length] = protoOf(globalThis); } catch (e) {}

  while (queue.length) {
    const obj = queue.pop();
    if (obj === null || (typeof obj !== "object" && typeof obj !== "function")) continue;
    if (seen.has(obj)) continue;
    seen.add(obj);
    try { freeze(obj); } catch (e) {}
    // Names AND symbols: `Date.prototype[Symbol.toPrimitive]` and the RegExp
    // `Symbol.match`/`split`/... functions are reachable only by symbol, and a
    // walk by name left every one of them extensible.
    let keys;
    try { keys = ownKeys(obj); } catch (e) { continue; }
    for (let i = 0; i < keys.length; i++) {
      try { reach(obj, keys[i]); } catch (e) {}
    }
    try { queue[queue.length] = protoOf(obj); } catch (e) {}
  }
})();
