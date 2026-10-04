// The web's standard globals a data module reasonably expects and Hermes
// lacks, in JavaScript: `structuredClone`, `queueMicrotask`, and ES2023's
// copying array methods Hermes has not built (`toSorted`, and the typed
// arrays' `toReversed`/`toSorted`/`with`). Hermes only: a browser realm has
// its own. `AbortController` is Ibex's (vendor/ibex2/src/bindings/abort.js),
// evaluated before this file. The list of what every executor has is in
// docs/reference.md ("What a data module can use"; pomodoro F4, calc F2).
// @ref LLP 1027 D10 — what the module can use
(function (global) {
  'use strict';
  var toString = Object.prototype.toString;
  var NativeDate = global.Date, NativeMap = global.Map, NativeSet = global.Set;
  var TypedArray = Object.getPrototypeOf(Uint8Array.prototype).constructor;
  function method(target, name, fn) {
    if (typeof target[name] !== 'function') Object.defineProperty(target, name, { value: fn, writable: true, configurable: true });
  }

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
    var tag = toString.call(value).slice(8, -1), out;
    if (tag === 'Boolean' || tag === 'Number' || tag === 'String' || tag === 'BigInt') out = Object(value.valueOf());
    else if (tag === 'Date') out = new NativeDate(NativeDate.prototype.getTime.call(value));
    else if (tag === 'RegExp') out = new RegExp(value.source, value.flags);
    else if (tag === 'ArrayBuffer') out = value.slice(0);
    else if (ArrayBuffer.isView(value)) {
      var buffer = clone(value.buffer, memo);
      out = tag === 'DataView' ? new DataView(buffer, value.byteOffset, value.byteLength)
        : new value.constructor(buffer, value.byteOffset, value.length);
    } else if (value instanceof NativeMap) {
      out = new NativeMap();
      memo.set(value, out);
      value.forEach(function (v, k) { out.set(clone(k, memo), clone(v, memo)); });
      return out;
    } else if (value instanceof NativeSet) {
      out = new NativeSet();
      memo.set(value, out);
      value.forEach(function (v) { out.add(clone(v, memo)); });
      return out;
    } else if (tag === 'Error') {
      var name = errors.indexOf(value.name) < 0 ? 'Error' : value.name;
      out = new global[name](value.message === undefined ? undefined : String(value.message));
      memo.set(value, out);
      if (Object.prototype.hasOwnProperty.call(value, 'cause')) out.cause = clone(value.cause, memo);
      return out;
    } else if (value instanceof Promise || tag === 'Promise' || tag === 'WeakMap' || tag === 'WeakSet' || tag === 'WeakRef') {
      throw cloneError('#<' + (value instanceof Promise ? 'Promise' : tag) + '>');
    } else {
      // An array keeps its length; anything else is a plain object of its
      // own enumerable string keys, read through getters, as a browser does.
      out = Array.isArray(value) ? new Array(value.length) : {};
      memo.set(value, out);
      var keys = Object.keys(value);
      for (var i = 0; i < keys.length; i++) out[keys[i]] = clone(value[keys[i]], memo);
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

  // ES2023's copying methods, as the spec's own steps: copy, then the
  // in-place method on the copy.
  method(Array.prototype, 'toSorted', function toSorted(compare) {
    if (compare !== undefined && typeof compare !== 'function') throw new TypeError('toSorted: the comparator must be a function');
    return Array.prototype.slice.call(Object(this)).sort(compare);
  });
  function typed(array) {
    if (!ArrayBuffer.isView(array) || array instanceof DataView) throw new TypeError('not a typed array');
    return new array.constructor(array);
  }
  method(TypedArray.prototype, 'toReversed', function toReversed() { return typed(this).reverse(); });
  method(TypedArray.prototype, 'toSorted', function toSorted(compare) {
    if (compare !== undefined && typeof compare !== 'function') throw new TypeError('toSorted: the comparator must be a function');
    return typed(this).sort(compare);
  });
  method(TypedArray.prototype, 'with', function (index, value) {
    var copy = typed(this), length = copy.length, at = Math.trunc(+index) || 0;
    if (at < 0) at += length;
    var number = typeof value === 'bigint' ? value : +value;
    if (at < 0 || at >= length) throw new RangeError('with: index out of range');
    copy[at] = number;
    return copy;
  });

  // Ibex's AbortSignal times out on a timer; a data source has none (time
  // is an argument, LLP 1027.000), so the one timer-backed member refuses.
  delete global.__ibex2_abort;
  if (global.AbortSignal) global.AbortSignal.timeout = function () {
    throw new Error('AbortSignal.timeout() is unavailable in data sources: there are no timers; pass time as an argument');
  };
})(globalThis);
