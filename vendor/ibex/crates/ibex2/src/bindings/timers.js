// setTimeout / setInterval / clearTimeout / clearInterval.
//
// The wheel is Rust's — deadlines, ordering, interval rescheduling. This file
// holds the one thing that cannot cross the boundary: the callback. Closures
// live here in a Map keyed by the integer handle Rust mints, and the adapter
// calls this script's private completion function when that handle is due.
(function (global) {
  "use strict";

  // Captured, then removed from the global object: `__ibex2_timer_clear`
  // takes an integer handle, and a module that could reach it could cancel
  // another module's timer by guessing one.
  var timerSet = global.__ibex2_timer_set;
  var timerSetRepeating = global.__ibex2_timer_set_repeating;
  var timerClear = global.__ibex2_timer_clear;
  var performanceNow = global.__ibex2_performance_now;
  delete global.__ibex2_timer_set;
  delete global.__ibex2_timer_set_repeating;
  delete global.__ibex2_timer_clear;
  delete global.__ibex2_performance_now;

  var apply = Reflect.apply;
  var callbacks = new Map();

  function schedule(repeating, handler, delay) {
    if (typeof handler !== "function") {
      // The string form of setTimeout compiles source, and dynamic code is
      // closed at construction (LLP 0060 D4). Refusing beats a confusing
      // "Parsing source code unsupported" from three frames down.
      throw new TypeError(
        "setTimeout/setInterval require a function; the string form needs eval, which is disabled"
      );
    }
    var ms = Number(delay);
    if (!isFinite(ms) || ms < 0) ms = 0;
    var extra = Array.prototype.slice.call(arguments, 3);
    var handle = repeating ? timerSetRepeating(ms) : timerSet(ms);
    callbacks.set(handle, { fn: handler, args: extra, repeating: repeating });
    return handle;
  }

  global.setTimeout = function (handler, delay) {
    return apply(
      schedule,
      null,
      [false, handler, delay].concat(Array.prototype.slice.call(arguments, 2))
    );
  };

  global.setInterval = function (handler, delay) {
    return apply(
      schedule,
      null,
      [true, handler, delay].concat(Array.prototype.slice.call(arguments, 2))
    );
  };

  function cancel(handle) {
    // clearTimeout(undefined) and clearTimeout(0) are no-ops in a browser.
    if (handle === undefined || handle === null) return;
    callbacks.delete(handle);
    timerClear(Number(handle));
  }

  global.clearTimeout = cancel;
  global.clearInterval = cancel;

  // Called by the pump, once per due timer.
  function fireTimer(handle) {
    var entry = callbacks.get(handle);
    if (!entry) return;
    // A one-shot is forgotten before it runs, so a callback that clears itself
    // — or throws — cannot leave a dead entry behind.
    if (!entry.repeating) callbacks.delete(handle);
    apply(entry.fn, undefined, entry.args);
  }

  // queueMicrotask: the one scheduling primitive the engine's Promise jobs
  // already provide, given its name. A callback that throws is reported as
  // an uncaught error, as the platform does, rather than becoming a rejected
  // promise nothing observes.
  var consoleError = global.console.error;
  global.queueMicrotask = function (callback) {
    if (typeof callback !== "function") {
      throw new TypeError("queueMicrotask requires a function");
    }
    Promise.resolve().then(function () {
      try {
        callback();
      } catch (e) {
        consoleError("Uncaught " + (e && e.stack ? e.stack : String(e)));
      }
    });
  };

  // The runtime's clock, replacing any earlier performance.now: timers and
  // performance.now read one clock (LLP 0071 D4).
  global.performance = global.performance || {};
  var runtimeNow = function () {
    return performanceNow();
  };
  global.performance.now = runtimeNow;
  // An accessor can accept the assignment and keep its own clock: refuse
  // installation rather than leave two clocks (LLP 0071 D4).
  if (global.performance.now !== runtimeNow) {
    throw new TypeError("performance.now cannot be replaced with the runtime's clock");
  }
  // The adapter retains this completion value in native state. It is not an
  // application global: only a host task already reserved by the pump may
  // choose the private timer handle passed here.
  return fireTimer;
})(globalThis);
