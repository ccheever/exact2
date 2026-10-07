// The web's standard globals a data module reasonably expects and Hermes
// lacks, in JavaScript: `structuredClone`, `queueMicrotask`, and ES2023's
// copying array methods Hermes has not built (`toSorted`, and the typed
// arrays' `toReversed`/`toSorted`/`with`). Hermes only: a browser realm has
// its own. `AbortController` is Ibex's (vendor/ibex/crates/ibex2/src/bindings/abort.js),
// evaluated before this file. The list of what every executor has is in
// docs/reference.md ("What a data module can use"; pomodoro F4, calc F2).
// @ref LLP 1027 D10 — what the module can use
(function (global) {
  'use strict';
  var toString = Object.prototype.toString, defineProperty = Object.defineProperty, keysOf = Object.keys;
  var NativeDate = global.Date, NativeMap = global.Map, NativeSet = global.Set, NativeRegExp = global.RegExp;
  var TypedArray = Object.getPrototypeOf(Uint8Array.prototype).constructor;
  function method(target, name, fn) {
    if (typeof target[name] !== 'function') defineProperty(target, name, { value: fn, writable: true, configurable: true });
  }
  // The intrinsics, captured before the app runs, so an object's own
  // `constructor`, `valueOf`, `forEach` or getters never steer a copy: a
  // value is what its internal slots say it is, as the web reads it.
  function getter(proto, name) { return Object.getOwnPropertyDescriptor(proto, name).get; }
  var typedName = getter(TypedArray.prototype, Symbol.toStringTag);
  var typedBuffer = getter(TypedArray.prototype, 'buffer'), typedOffset = getter(TypedArray.prototype, 'byteOffset'), typedLength = getter(TypedArray.prototype, 'length');
  var viewBuffer = getter(DataView.prototype, 'buffer'), viewOffset = getter(DataView.prototype, 'byteOffset'), viewLength = getter(DataView.prototype, 'byteLength');
  var bufferLength = getter(ArrayBuffer.prototype, 'byteLength'), bufferSlice = ArrayBuffer.prototype.slice;
  var regexpSource = getter(NativeRegExp.prototype, 'source');
  var flagGetters = [['d', 'hasIndices'], ['g', 'global'], ['i', 'ignoreCase'], ['m', 'multiline'], ['s', 'dotAll'], ['u', 'unicode'], ['v', 'unicodeSets'], ['y', 'sticky']]
    .filter(function (f) { return Object.getOwnPropertyDescriptor(NativeRegExp.prototype, f[1]); })
    .map(function (f) { return [f[0], getter(NativeRegExp.prototype, f[1])]; });
  function regexpFlags(re) { return flagGetters.map(function (f) { return f[1].call(re) ? f[0] : ''; }).join(''); }
  var mapSize = getter(NativeMap.prototype, 'size'), setSize = getter(NativeSet.prototype, 'size');
  var mapEach = NativeMap.prototype.forEach, setEach = NativeSet.prototype.forEach, mapSet = NativeMap.prototype.set, setAdd = NativeSet.prototype.add;
  var getTime = NativeDate.prototype.getTime, arraySort = Array.prototype.sort, typedSort = TypedArray.prototype.sort, typedReverse = TypedArray.prototype.reverse;
  var typedConstructors = {};
  ['Int8Array', 'Uint8Array', 'Uint8ClampedArray', 'Int16Array', 'Uint16Array', 'Int32Array', 'Uint32Array',
    'Float32Array', 'Float64Array', 'BigInt64Array', 'BigUint64Array'].forEach(function (name) {
    if (typeof global[name] === 'function') typedConstructors[name] = global[name];
  });
  var boxes = [[Boolean, Boolean.prototype.valueOf], [Number, Number.prototype.valueOf], [String, String.prototype.valueOf]];
  if (typeof BigInt === 'function') boxes.push([BigInt, BigInt.prototype.valueOf]);
  function brand(fn, value) { try { fn.call(value); return true; } catch (e) { return false; } }
  function own(object, key, value) { defineProperty(object, key, { value: value, writable: true, enumerable: true, configurable: true }); }

  // HTML's structured clone, without transfer: what a browser copies, with
  // shared references and cycles kept, and DataCloneError for the rest.
  function cloneError(what) {
    return new global.DOMException(what + ' could not be cloned.', 'DataCloneError');
  }
  var errors = ['Error', 'EvalError', 'RangeError', 'ReferenceError', 'SyntaxError', 'TypeError', 'URIError'];
  function clone(value, memo) {
    if (typeof value === 'symbol') throw cloneError(String(value));
    if (typeof value === 'function') throw cloneError(value.name ? 'function ' + value.name : 'A function');
    if (value === null || typeof value !== 'object') return value;
    if (memo.has(value)) return memo.get(value);
    var out, typed = typedName.call(value);
    for (var b = 0; b < boxes.length && out === undefined; b++) {
      if (brand(boxes[b][1], value)) out = Object(boxes[b][1].call(value));
    }
    if (out !== undefined) { /* a boxed primitive */ }
    else if (brand(getTime, value)) out = new NativeDate(getTime.call(value));
    else if (brand(regexpSource, value) && value !== NativeRegExp.prototype) out = new NativeRegExp(regexpSource.call(value), regexpFlags(value));
    else if (brand(bufferLength, value)) out = bufferSlice.call(value, 0);
    else if (typed !== undefined) {
      out = new typedConstructors[typed](clone(typedBuffer.call(value), memo), typedOffset.call(value), typedLength.call(value));
    } else if (brand(viewLength, value)) {
      out = new DataView(clone(viewBuffer.call(value), memo), viewOffset.call(value), viewLength.call(value));
    } else if (brand(mapSize, value)) {
      out = new NativeMap();
      memo.set(value, out);
      mapEach.call(value, function (v, k) { mapSet.call(out, clone(k, memo), clone(v, memo)); });
      return out;
    } else if (brand(setSize, value)) {
      out = new NativeSet();
      memo.set(value, out);
      setEach.call(value, function (v) { setAdd.call(out, clone(v, memo)); });
      return out;
    } else if (toString.call(value) === '[object Error]') {
      var name = errors.indexOf(value.name) < 0 ? 'Error' : value.name;
      out = new global[name](value.message === undefined ? undefined : String(value.message));
      memo.set(value, out);
      if (Object.prototype.hasOwnProperty.call(value, 'cause')) own(out, 'cause', clone(value.cause, memo));
      return out;
    } else if (value instanceof Promise || /^\[object (Promise|WeakMap|WeakSet|WeakRef)\]$/.test(toString.call(value))) {
      throw cloneError(value instanceof Promise ? '#<Promise>' : toString.call(value).replace(/^\[object (\w+)\]$/, '#<$1>'));
    } else {
      // An array keeps its length; anything else is a plain object of its
      // own enumerable string keys, read through getters, as a browser does.
      // Each is an own data property: a key named `__proto__` stays a key.
      out = Array.isArray(value) ? new Array(value.length) : {};
      memo.set(value, out);
      var keys = keysOf(value);
      for (var i = 0; i < keys.length; i++) own(out, keys[i], clone(value[keys[i]], memo));
      return out;
    }
    memo.set(value, out);
    return out;
  }
  method(global, 'structuredClone', function structuredClone(value, options) {
    if (!arguments.length) throw new TypeError('structuredClone requires a value');
    if (options && options.transfer && Array.from(options.transfer).length)
      throw cloneError('A transfer list (a data source has no other realm to transfer to)');
    return clone(value, new NativeMap());
  });

  // A microtask: a callback's throw is reported, as the web reports it, and
  // never rejects anything the module awaits.
  method(global, 'queueMicrotask', function queueMicrotask(callback) {
    if (typeof callback !== 'function') throw new TypeError('queueMicrotask requires a function');
    Promise.resolve().then(function () {
      try { callback(); } catch (e) { global.console.error('Uncaught ' + (e && e.stack ? e.stack : String(e))); }
    });
  });

  // ES2023's copying methods, as the spec's own steps: read every index
  // into a fresh ordinary array (a hole reads as undefined) or a typed array
  // of the same intrinsic type, then the intrinsic in-place method on it.
  method(Array.prototype, 'toSorted', function toSorted(compare) {
    if (compare !== undefined && typeof compare !== 'function') throw new TypeError('toSorted: the comparator must be a function');
    var object = Object(this), length = Math.min(Math.max(Math.trunc(+object.length) || 0, 0), Number.MAX_SAFE_INTEGER);
    var copy = new Array(length);
    for (var i = 0; i < length; i++) copy[i] = object[i];
    return arraySort.call(copy, compare);
  });
  function sameType(array) {
    var name = typedName.call(array);
    if (name === undefined) throw new TypeError('not a typed array');
    return new typedConstructors[name](array);
  }
  method(TypedArray.prototype, 'toReversed', function toReversed() { return typedReverse.call(sameType(this)); });
  method(TypedArray.prototype, 'toSorted', function toSorted(compare) {
    if (compare !== undefined && typeof compare !== 'function') throw new TypeError('toSorted: the comparator must be a function');
    return typedSort.call(sameType(this), compare);
  });
  method(TypedArray.prototype, 'with', function (index, value) {
    var copy = sameType(this), length = typedLength.call(copy), at = Math.trunc(+index) || 0;
    if (at < 0) at += length;
    var number = typeof value === 'bigint' ? value : +value;
    if (at < 0 || at >= length) throw new RangeError('with: index out of range');
    copy[at] = number;
    return copy;
  });

  // Ibex's AbortSignal times out on a timer; a data source has none (time
  // is an argument, LLP 1027.000), so the one timer-backed member refuses.
  // Its internal hooks (`__ibex2_abort`) stay for the prelude's `fetch`,
  // which takes and deletes them before any module runs.
  if (global.AbortSignal) global.AbortSignal.timeout = function () {
    throw new Error('AbortSignal.timeout() is unavailable in data sources: there are no timers; pass time as an argument');
  };
})(globalThis);
