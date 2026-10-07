// Abort signals hold JavaScript reasons; fetch carries only cancellation to Rust.
(function (global) {
  "use strict";
  var brand = global.__ibex2_brand || function (value) { return value; };
  var signals = new WeakMap(), controllers = new WeakMap(), views = new WeakMap();
  var objectCreate = Object.create;
  var objectDefineProperties = Object.defineProperties;
  var objectFreeze = Object.freeze;
  var useEvents = typeof global.EventTarget === "function" && typeof global.Event === "function";
  var EventCtor = global.Event;
  var fireTrustedEvent = global.__ibex2_fire_trusted_event;
  delete global.__ibex2_fire_trusted_event;
  var setEventAbortHooks = global.__ibex2_set_event_abort_hooks;
  delete global.__ibex2_set_event_abort_hooks;
  var consoleError = global.console && typeof global.console.error === "function"
    ? global.console.error
    : null;
  var reportException = typeof global.reportError === "function"
    ? global.reportError
    : consoleError
      ? function (error) { consoleError("Uncaught " + String(error)); }
      : function () {};
  function own(signal) {
    var state = signals.get(signal);
    if (!state) throw new TypeError("not an AbortSignal");
    return state;
  }
  function inspect(signal) {
    var state = own(signal), view = views.get(signal);
    if (view) return view;
    view = objectCreate(null);
    objectDefineProperties(view, {
      aborted: { get: function () { return state.aborted; }, enumerable: true },
      reason: { get: function () { return state.reason; }, enumerable: true }
    });
    objectFreeze(view);
    views.set(signal, view);
    return view;
  }
  function create() {
    var signal;
    if (useEvents) {
      signal = new global.EventTarget();
      Object.setPrototypeOf(signal, AbortSignal.prototype);
    } else {
      signal = Object.create(AbortSignal.prototype);
    }
    signals.set(signal, { aborted: false, reason: undefined, listeners: [], hooks: [], dependents: [], sources: null, onabort: null, onabortEntry: null });
    return brand(signal, "AbortSignal");
  }
  function notify(callback, receiver, event) {
    try {
      if (typeof callback === "function") callback.call(receiver, event);
      else if (callback && typeof callback.handleEvent === "function") callback.handleEvent(event);
    } catch (e) {
      // Reporting must not let hostile coercion interrupt other abort algorithms.
      try { reportException(e); } catch (_) {}
    }
  }
  function hookIsAlive(hook) {
    if (!hook.alive) return true;
    try { return !!hook.alive(); }
    catch (e) { try { reportException(e); } catch (_) {} return false; }
  }
  function deliverHook(hook) {
    if (!hookIsAlive(hook)) return;
    try { hook.callback(); }
    catch (e) { try { reportException(e); } catch (_) {} }
  }
  function abort(signal, reason) {
    if (own(signal).aborted) return;
    reason = reason === undefined ? new DOMException("The operation was aborted", "AbortError") : reason;
    var pending = [];
    function mark(current) {
      var state = own(current);
      if (state.aborted) return;
      state.aborted = true;
      state.reason = reason;
      pending.push(current);
      state.dependents.slice().forEach(mark);
      state.dependents = [];
    }
    // All dependent signals are marked before the source dispatches its event.
    mark(signal);
    pending.forEach(function (current) {
      var state = own(current), stopped = false;
      state.hooks.splice(0).forEach(deliverHook);
      if (useEvents) {
        // The private firing path exists only while the native installer moves
        // from events.js to abort.js. Application code never receives it.
        if (typeof fireTrustedEvent === "function") fireTrustedEvent(current, "abort");
        else current.dispatchEvent(new EventCtor("abort"));
        return;
      }
      var event = { type: "abort", target: current, currentTarget: current,
        bubbles: false, cancelable: false, defaultPrevented: false,
        stopImmediatePropagation: function () { stopped = true; },
        stopPropagation: function () {}, preventDefault: function () {} };
      state.listeners.slice().forEach(function (entry) {
        if (stopped || state.listeners.indexOf(entry) < 0) return;
        if (entry.once) state.listeners.splice(state.listeners.indexOf(entry), 1);
        notify(entry.callback, current, event);
      });
      event.currentTarget = null;
    });
  }
  function subscribe(signal, callback, alive) {
    var state = own(signal);
    if (state.aborted) { deliverHook({ callback: callback, alive: alive }); return function () {}; }
    state.hooks = state.hooks.filter(hookIsAlive);
    var entry = { callback: callback, alive: alive };
    state.hooks.push(entry);
    return function () {
      var index = state.hooks.indexOf(entry);
      if (index >= 0) state.hooks.splice(index, 1);
    };
  }
  function AbortSignal() { throw new TypeError("Illegal constructor"); }
  if (useEvents) {
    AbortSignal.prototype = Object.create(global.EventTarget.prototype, {
      constructor: { value: AbortSignal, writable: true, configurable: true }
    });
  }
  Object.defineProperties(AbortSignal.prototype, {
    aborted: { get: function () { return own(this).aborted; }, enumerable: true },
    reason: { get: function () { return own(this).reason; }, enumerable: true },
    onabort: { get: function () { return own(this).onabort; }, set: function (v) {
      var signal = this, state = own(signal);
      state.onabort = typeof v === "function" ? v : null;
      if (state.onabort && !state.onabortEntry) {
        state.onabortEntry = function (event) { state.onabort.call(signal, event); };
        if (useEvents) signal.addEventListener("abort", state.onabortEntry);
        else state.listeners.push({ callback: state.onabortEntry });
      } else if (!state.onabort && state.onabortEntry) {
        if (useEvents) signal.removeEventListener("abort", state.onabortEntry);
        else state.listeners = state.listeners.filter(function (entry) { return entry.callback !== state.onabortEntry; });
        state.onabortEntry = null;
      }
    }, enumerable: true }
  });
  AbortSignal.prototype.throwIfAborted = function () { var state = own(this); if (state.aborted) throw state.reason; };
  if (!useEvents) {
    AbortSignal.prototype.addEventListener = function (type, callback, options) {
      var state = own(this);
      if (String(type) !== "abort" || callback == null) return;
      if (typeof callback !== "function" && typeof callback !== "object") throw new TypeError("invalid event listener");
      var capture = typeof options === "boolean" ? options : !!(options && options.capture);
      if (state.listeners.some(function (e) { return e.callback === callback && e.capture === capture; })) return;
      state.listeners.push({ callback: callback, capture: capture, once: !!(options && options.once) });
    };
    AbortSignal.prototype.removeEventListener = function (type, callback, options) {
      var state = own(this);
      if (String(type) !== "abort") return;
      var capture = typeof options === "boolean" ? options : !!(options && options.capture);
      state.listeners = state.listeners.filter(function (e) { return e.callback !== callback || e.capture !== capture; });
    };
  }
  AbortSignal.abort = function (reason) { var signal = create(); abort(signal, reason); return signal; };
  AbortSignal.timeout = function (milliseconds) {
    var delay = +milliseconds;
    if (!isFinite(delay) || delay < 0 || delay > Number.MAX_SAFE_INTEGER) throw new TypeError("invalid timeout");
    if (typeof global.setTimeout !== "function") {
      throw new DOMException("AbortSignal.timeout requires the TIMERS group", "NotSupportedError");
    }
    var signal = create();
    global.setTimeout(function () { abort(signal, new DOMException("The operation timed out", "TimeoutError")); }, Math.floor(delay));
    return signal;
  };
  AbortSignal.any = function (values) {
    if (values == null || (typeof values !== "object" && typeof values !== "function") || typeof values[Symbol.iterator] !== "function") throw new TypeError("signals must be an iterable");
    var inputs = Array.from(values), result = create(), releases = [], sources = [];
    inputs.forEach(own);
    for (var i = 0; i < inputs.length; i++) {
      if (own(inputs[i]).aborted) { abort(result, own(inputs[i]).reason); return result; }
    }
    inputs.forEach(function (input) {
      (own(input).sources || [input]).forEach(function (source) {
        if (sources.indexOf(source) < 0) sources.push(source);
      });
    });
    own(result).sources = sources;
    sources.forEach(function (input) {
      var state = own(input);
      if (state.dependents.indexOf(result) >= 0) return;
      state.dependents.push(result);
      releases.push(function () {
        var index = state.dependents.indexOf(result);
        if (index >= 0) state.dependents.splice(index, 1);
      });
    });
    subscribe(result, function () { releases.forEach(function (release) { release(); }); });
    return result;
  };
  function AbortController() {
    if (!new.target) throw new TypeError("AbortController requires new");
    controllers.set(this, create());
    brand(this, "AbortController");
  }
  Object.defineProperty(AbortController.prototype, "signal", { get: function () {
    var signal = controllers.get(this);
    if (!signal) throw new TypeError("not an AbortController");
    return signal;
  }, enumerable: true });
  AbortController.prototype.abort = function (reason) {
    var signal = controllers.get(this);
    if (!signal) throw new TypeError("not an AbortController");
    abort(signal, reason);
  };
  Object.defineProperty(AbortSignal.prototype, Symbol.toStringTag, { value: "AbortSignal" });
  Object.defineProperty(AbortController.prototype, Symbol.toStringTag, { value: "AbortController" });
  global.AbortSignal = AbortSignal;
  global.AbortController = AbortController;
  // Captured and deleted by the fetch binding before modules are evaluated.
  var hooks = { own: inspect, subscribe: subscribe };
  if (typeof setEventAbortHooks === "function") setEventAbortHooks(hooks);
  global.__ibex2_abort = hooks;
})(globalThis);
