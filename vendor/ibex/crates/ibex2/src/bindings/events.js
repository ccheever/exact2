// DOM events for a worker-shaped runtime: one target, no node tree.
(function (global) {
  "use strict";

  // @ref LLP 0057.000#l3--events-abort-and-the-second-direction — trust and private state use bootstrap-captured intrinsics in unhardened borrowed runtimes
  var FunctionCall = Function.prototype.call;
  var FunctionBind = Function.prototype.bind;
  function uncurry(fn) { return FunctionCall.call(FunctionBind, FunctionCall, fn); }
  var functionCall = uncurry(Function.prototype.call);
  var functionApply = uncurry(Function.prototype.apply);
  var weakMapGet = uncurry(WeakMap.prototype.get);
  var weakMapSet = uncurry(WeakMap.prototype.set);
  var weakMapHas = uncurry(WeakMap.prototype.has);
  var arrayForEach = uncurry(Array.prototype.forEach);
  var arrayFrom = Array.from;
  var arrayIndexOf = uncurry(Array.prototype.indexOf);
  var arrayPush = uncurry(Array.prototype.push);
  var arraySlice = uncurry(Array.prototype.slice);
  var arraySome = uncurry(Array.prototype.some);
  var arraySplice = uncurry(Array.prototype.splice);
  var objectCreate = Object.create;
  var objectKeys = Object.keys;
  var objectDefineProperty = Object.defineProperty;
  var objectDefineProperties = Object.defineProperties;
  var objectFreeze = Object.freeze;
  var ObjectCtor = Object;
  var StringCtor = String;
  var NumberCtor = Number;
  var WeakMapCtor = WeakMap;
  var performanceObject = global.performance;
  var performanceNow = performanceObject && typeof performanceObject.now === "function"
    ? performanceObject.now
    : null;
  var dateNow = Date.now;

  // @ref LLP 0057.000#l3--events-abort-and-the-second-direction — private records have no prototype, so inherited setters never see them
  function privateRecord(fields) {
    var record = objectCreate(null);
    var keys = objectKeys(fields);
    for (var i = 0; i < keys.length; i++) record[keys[i]] = fields[keys[i]];
    return record;
  }
  function privateList(items) {
    var list = objectCreate(null);
    list.length = 0;
    if (items) for (var i = 0; i < items.length; i++) list[list.length++] = items[i];
    return list;
  }
  var eventStates = new WeakMapCtor();
  var targetStates = new WeakMapCtor();
  var brand = global.__ibex2_brand || function (value) { return value; };
  var abortSubscribe = null;
  var nativeReport = global.__ibex2_report_error;
  delete global.__ibex2_report_error;
  var reportingException = false;

  function domString(value) {
    if (typeof value === "symbol") throw new TypeError("cannot convert a Symbol to a string");
    return StringCtor(value);
  }

  function dictionary(value) {
    return value == null ? {} : ObjectCtor(value);
  }

  function eventState(event) {
    if (!weakMapHas(eventStates, event)) throw new TypeError("Illegal invocation");
    return weakMapGet(eventStates, event);
  }

  function targetState(target) {
    if (!weakMapHas(targetStates, target)) throw new TypeError("Illegal invocation");
    return weakMapGet(targetStates, target);
  }

  function isTrusted() {
    return eventState(this).trusted;
  }

  function initializeEvent(event, type, init, kind) {
    init = dictionary(init);
    weakMapSet(eventStates, event, privateRecord({
      type: domString(type),
      bubbles: !!init.bubbles,
      cancelable: !!init.cancelable,
      composed: !!init.composed,
      canceled: false,
      stop: false,
      stopImmediate: false,
      passive: false,
      dispatching: false,
      target: null,
      currentTarget: null,
      phase: 0,
      path: privateList(),
      trusted: false,
      timeStamp: performanceNow
        ? functionCall(performanceNow, performanceObject)
        : dateNow()
    }));
    // Web IDL's [LegacyUnforgeable] isTrusted is an own accessor. Keep the
    // getter shared across instances, as the platform descriptor requires.
    objectDefineProperty(event, "isTrusted", {
      get: isTrusted,
      enumerable: true
    });
    brand(event, kind || "Event");
  }

  function Event(type, init) {
    if (!new.target) throw new TypeError("Event requires new");
    if (arguments.length === 0) throw new TypeError("Event requires a type");
    initializeEvent(this, type, init, "Event");
  }

  objectDefineProperties(Event.prototype, {
    type: { get: function () { return eventState(this).type; }, enumerable: true },
    target: { get: function () { return eventState(this).target; }, enumerable: true },
    srcElement: { get: function () { return eventState(this).target; }, enumerable: true },
    currentTarget: { get: function () { return eventState(this).currentTarget; }, enumerable: true },
    eventPhase: { get: function () { return eventState(this).phase; }, enumerable: true },
    bubbles: { get: function () { return eventState(this).bubbles; }, enumerable: true },
    cancelable: { get: function () { return eventState(this).cancelable; }, enumerable: true },
    defaultPrevented: { get: function () { return eventState(this).canceled; }, enumerable: true },
    composed: { get: function () { return eventState(this).composed; }, enumerable: true },
    timeStamp: { get: function () { return eventState(this).timeStamp; }, enumerable: true },
    cancelBubble: {
      get: function () { return eventState(this).stop; },
      set: function (value) { if (value) eventState(this).stop = true; },
      enumerable: true
    },
    returnValue: {
      get: function () { return !eventState(this).canceled; },
      set: function (value) { if (!value) this.preventDefault(); },
      enumerable: true
    }
  });
  Event.prototype.preventDefault = function () {
    var state = eventState(this);
    if (state.cancelable && !state.passive) state.canceled = true;
  };
  Event.prototype.stopPropagation = function () { eventState(this).stop = true; };
  Event.prototype.stopImmediatePropagation = function () {
    var state = eventState(this);
    state.stop = true;
    state.stopImmediate = true;
  };
  Event.prototype.composedPath = function () { return arraySlice(eventState(this).path); };
  Event.prototype.initEvent = function (type, bubbles, cancelable) {
    var state = eventState(this);
    if (state.dispatching) return;
    state.type = domString(type);
    state.bubbles = !!bubbles;
    state.cancelable = !!cancelable;
    state.composed = false;
    state.canceled = false;
    state.stop = false;
    state.stopImmediate = false;
    state.passive = false;
    state.target = null;
    state.currentTarget = null;
    state.phase = Event.NONE;
    state.path = privateList();
    state.trusted = false;
  };

  var eventConstants = [
    ["NONE", 0], ["CAPTURING_PHASE", 1],
    ["AT_TARGET", 2], ["BUBBLING_PHASE", 3]
  ];
  arrayForEach(eventConstants, function (entry) {
    objectDefineProperty(Event, entry[0], { value: entry[1], enumerable: true });
    objectDefineProperty(Event.prototype, entry[0], { value: entry[1], enumerable: true });
  });

  function inherit(constructor) {
    constructor.prototype = objectCreate(Event.prototype, {
      constructor: { value: constructor, writable: true, configurable: true }
    });
  }

  function CustomEvent(type, init) {
    if (!new.target) throw new TypeError("CustomEvent requires new");
    if (arguments.length === 0) throw new TypeError("CustomEvent requires a type");
    init = dictionary(init);
    initializeEvent(this, type, init, "CustomEvent");
    eventState(this).detail = "detail" in init ? init.detail : null;
  }
  inherit(CustomEvent);
  objectDefineProperty(CustomEvent.prototype, "detail", {
    get: function () { return eventState(this).detail; }, enumerable: true
  });
  CustomEvent.prototype.initCustomEvent = function (type, bubbles, cancelable, detail) {
    var state = eventState(this);
    if (state.dispatching) return;
    this.initEvent(type, bubbles, cancelable);
    state.detail = detail;
  };

  function ErrorEvent(type, init) {
    if (!new.target) throw new TypeError("ErrorEvent requires new");
    if (arguments.length === 0) throw new TypeError("ErrorEvent requires a type");
    init = dictionary(init);
    initializeEvent(this, type, init, "ErrorEvent");
    var state = eventState(this);
    state.message = "message" in init ? domString(init.message) : "";
    state.filename = "filename" in init ? domString(init.filename) : "";
    state.lineno = "lineno" in init ? NumberCtor(init.lineno) >>> 0 : 0;
    state.colno = "colno" in init ? NumberCtor(init.colno) >>> 0 : 0;
    state.error = "error" in init ? init.error : null;
  }
  inherit(ErrorEvent);
  objectDefineProperties(ErrorEvent.prototype, {
    message: { get: function () { return eventState(this).message; }, enumerable: true },
    filename: { get: function () { return eventState(this).filename; }, enumerable: true },
    lineno: { get: function () { return eventState(this).lineno; }, enumerable: true },
    colno: { get: function () { return eventState(this).colno; }, enumerable: true },
    error: { get: function () { return eventState(this).error; }, enumerable: true }
  });

  function MessageEvent(type, init) {
    if (!new.target) throw new TypeError("MessageEvent requires new");
    if (arguments.length === 0) throw new TypeError("MessageEvent requires a type");
    init = dictionary(init);
    initializeEvent(this, type, init, "MessageEvent");
    var state = eventState(this);
    state.data = "data" in init ? init.data : null;
    state.origin = "origin" in init ? domString(init.origin) : "";
    state.lastEventId = "lastEventId" in init ? domString(init.lastEventId) : "";
    state.source = "source" in init ? init.source : null;
    state.ports = "ports" in init ? arrayFrom(init.ports) : [];
  }
  inherit(MessageEvent);
  objectDefineProperties(MessageEvent.prototype, {
    data: { get: function () { return eventState(this).data; }, enumerable: true },
    origin: { get: function () { return eventState(this).origin; }, enumerable: true },
    lastEventId: { get: function () { return eventState(this).lastEventId; }, enumerable: true },
    source: { get: function () { return eventState(this).source; }, enumerable: true },
    ports: { get: function () { return arraySlice(eventState(this).ports); }, enumerable: true }
  });
  MessageEvent.prototype.initMessageEvent = function (
      type, bubbles, cancelable, data, origin, lastEventId, source, ports) {
    var state = eventState(this);
    if (state.dispatching) return;
    this.initEvent(type, bubbles, cancelable);
    state.data = data;
    state.origin = domString(origin);
    state.lastEventId = domString(lastEventId);
    state.source = source == null ? null : source;
    state.ports = ports == null ? [] : arrayFrom(ports);
  };

  function CloseEvent(type, init) {
    if (!new.target) throw new TypeError("CloseEvent requires new");
    if (arguments.length === 0) throw new TypeError("CloseEvent requires a type");
    init = dictionary(init);
    initializeEvent(this, type, init, "CloseEvent");
    var state = eventState(this);
    state.wasClean = !!init.wasClean;
    state.code = "code" in init ? NumberCtor(init.code) & 0xffff : 0;
    state.reason = "reason" in init ? domString(init.reason) : "";
  }
  inherit(CloseEvent);
  objectDefineProperties(CloseEvent.prototype, {
    wasClean: { get: function () { return eventState(this).wasClean; }, enumerable: true },
    code: { get: function () { return eventState(this).code; }, enumerable: true },
    reason: { get: function () { return eventState(this).reason; }, enumerable: true }
  });

  function PromiseRejectionEvent(type, init) {
    if (!new.target) throw new TypeError("PromiseRejectionEvent requires new");
    if (arguments.length < 2 || init == null || !("promise" in ObjectCtor(init))) {
      throw new TypeError("PromiseRejectionEvent requires a promise");
    }
    init = ObjectCtor(init);
    initializeEvent(this, type, init, "PromiseRejectionEvent");
    var state = eventState(this);
    state.promise = init.promise;
    state.reason = init.reason;
  }
  inherit(PromiseRejectionEvent);
  objectDefineProperties(PromiseRejectionEvent.prototype, {
    promise: { get: function () { return eventState(this).promise; }, enumerable: true },
    reason: { get: function () { return eventState(this).reason; }, enumerable: true }
  });

  function EventTarget() {
    if (!new.target) throw new TypeError("EventTarget requires new");
    weakMapSet(targetStates, this, privateRecord({
      listeners: privateList(), listenerChange: null
    }));
    brand(this, "EventTarget");
  }

  function captureOf(options) {
    return typeof options === "boolean" ? options : !!(options && options.capture);
  }

  function notifyListenerChange(target, type) {
    var state = targetState(target);
    var hook = state.listenerChange;
    if (!hook) return;
    var present = false;
    for (var i = 0; i < state.listeners.length; i++) {
      var entry = state.listeners[i];
      if (!entry.removed && entry.type === type) {
        present = true;
        break;
      }
    }
    // @ref LLP 0059.000#312-websocket--delegating-capability-bearing-author-required — keepalive follows the real EventTarget mutation, including signal and once removal
    functionCall(hook, target, type, present);
  }

  function removeEntry(target, entry) {
    var listeners = targetState(target).listeners;
    var index = arrayIndexOf(listeners, entry);
    if (index < 0) return;
    entry.removed = true;
    arraySplice(listeners, index, 1);
    notifyListenerChange(target, entry.type);
    if (entry.abortRelease) {
      var release = entry.abortRelease;
      entry.abortRelease = null;
      release();
    }
  }

  EventTarget.prototype.addEventListener = function (type, callback, options) {
    var target = this;
    var state = targetState(target);
    type = domString(type);
    var capture = captureOf(options);
    var once = !!(options && typeof options !== "boolean" && options.once);
    var passive = !!(options && typeof options !== "boolean" && options.passive);
    var signal = options && typeof options !== "boolean" ? options.signal : undefined;
    if (signal !== undefined) {
      if (typeof global.AbortSignal !== "function" || !(signal instanceof global.AbortSignal)) {
        throw new TypeError("signal is not an AbortSignal");
      }
      if (signal.aborted) return;
    }
    if (callback == null) return;
    if (typeof callback !== "function" && typeof callback !== "object") {
      throw new TypeError("event listener must be a function or object");
    }
    if (arraySome(state.listeners, function (entry) {
      return !entry.removed && entry.type === type && entry.callback === callback && entry.capture === capture;
    })) return;
    var entry = privateRecord({ type: type, callback: callback, capture: capture, once: once,
      passive: passive, abortRelease: null, removed: false });
    arrayPush(state.listeners, entry);
    if (signal !== undefined) {
      if (typeof abortSubscribe !== "function") {
        removeEntry(target, entry);
        throw new TypeError("AbortSignal hooks are unavailable");
      }
      // @ref LLP 0057.000#l3--events-abort-and-the-second-direction — signal-bound listener removal is an abort algorithm, before abort event dispatch
      entry.abortRelease = abortSubscribe(signal, function () { removeEntry(target, entry); });
    }
    notifyListenerChange(target, type);
  };

  EventTarget.prototype.removeEventListener = function (type, callback, options) {
    var target = this;
    var state = targetState(target);
    type = domString(type);
    var capture = captureOf(options);
    for (var i = 0; i < state.listeners.length; i++) {
      var entry = state.listeners[i];
      if (!entry.removed && entry.type === type && entry.callback === callback && entry.capture === capture) {
        removeEntry(target, entry);
        return;
      }
    }
  };

  function invoke(target, event, capture) {
    var state = eventState(event);
    var listeners = arraySlice(targetState(target).listeners);
    for (var i = 0; i < listeners.length; i++) {
      var entry = listeners[i];
      if (state.stopImmediate) break;
      if (entry.removed || entry.type !== state.type || entry.capture !== capture) continue;
      if (entry.once) removeEntry(target, entry);
      state.passive = entry.passive;
      try {
        if (typeof entry.callback === "function") functionCall(entry.callback, target, event);
        else {
          var handleEvent = entry.callback.handleEvent;
          if (typeof handleEvent === "function") functionCall(handleEvent, entry.callback, event);
        }
      } catch (error) {
        reportException(error);
      }
      state.passive = false;
    }
  }

  // @ref LLP 0057.000#l3--events-abort-and-the-second-direction — only the captured host path may assert event trust
  function dispatch(target, event, trusted) {
    targetState(target);
    var state = eventState(event);
    if (state.dispatching) {
      throw new DOMException("The event is already being dispatched", "InvalidStateError");
    }
    state.trusted = trusted;
    state.dispatching = true;
    state.target = target;
    state.currentTarget = target;
    state.phase = Event.AT_TARGET;
    state.path = privateList([target]);
    try {
      invoke(target, event, true);
      // @ref LLP 0057.000#l3--events-abort-and-the-second-direction — at-target capture and bubble remain distinct propagation phases
      if (!state.stop) invoke(target, event, false);
      return !state.canceled;
    } finally {
      state.currentTarget = null;
      state.phase = Event.NONE;
      state.path = privateList();
      state.dispatching = false;
      state.stop = false;
      state.stopImmediate = false;
      state.passive = false;
    }
  }

  EventTarget.prototype.dispatchEvent = function (event) {
    // Public redispatch is always application dispatch, even when the event
    // was originally created and fired by the host.
    return dispatch(this, event, false);
  };

  function errorText(error) {
    try {
      if (error && typeof error.stack === "string") return error.stack;
      if (error && typeof error.message === "string") return error.message;
      return domString(error);
    } catch (_) {
      return "uncaught error";
    }
  }

  function reportException(error) {
    var text = errorText(error);
    if (reportingException) {
      nativeReport(text);
      return;
    }
    reportingException = true;
    try {
      var event = new ErrorEvent("error", {
        cancelable: true,
        message: error && typeof error.message === "string" ? error.message : text,
        error: error
      });
      if (dispatch(global, event, true)) nativeReport(text);
    } finally {
      reportingException = false;
    }
  }

  function reportError(error) {
    reportException(error);
  }

  function fireTrustedEvent(target, event) {
    return dispatch(target, typeof event === "string" ? new Event(event) : event, true);
  }

  function hasEventListener(target, type) {
    var listeners = targetState(target).listeners;
    for (var i = 0; i < listeners.length; i++) {
      if (!listeners[i].removed && listeners[i].type === type) return true;
    }
    return false;
  }

  function setAbortHooks(hooks) {
    if (!hooks || typeof hooks.subscribe !== "function") {
      throw new TypeError("invalid AbortSignal hooks");
    }
    abortSubscribe = hooks.subscribe;
  }

  function setListenerChangeHook(target, hook) {
    if (typeof hook !== "function") {
      throw new TypeError("invalid listener-change hook");
    }
    targetState(target).listenerChange = hook;
  }

  function defineEventHandler(name, type, errorHandler) {
    var callback = null;
    var wrapper = null;
    objectDefineProperty(global, name, {
      configurable: true,
      enumerable: true,
      get: function () { return callback; },
      set: function (value) {
        if (wrapper) functionCall(EventTarget.prototype.removeEventListener, global, type, wrapper);
        callback = typeof value === "function" ? value : null;
        wrapper = null;
        if (!callback) return;
        wrapper = function (event) {
          var result = errorHandler
            ? functionCall(callback, global, event.message, event.filename, event.lineno, event.colno, event.error)
            : functionCall(callback, global, event);
          if ((errorHandler && result === true) || (!errorHandler && result === false)) {
            event.preventDefault();
          }
        };
        functionCall(EventTarget.prototype.addEventListener, global, type, wrapper);
      }
    });
  }

  // @ref LLP 0057.000#l3--events-abort-and-the-second-direction — keep the engine global's prototype intact; only its private target record is new
  weakMapSet(targetStates, global, privateRecord({
    listeners: privateList(), listenerChange: null
  }));
  global.Event = Event;
  global.EventTarget = EventTarget;
  global.CustomEvent = CustomEvent;
  global.ErrorEvent = ErrorEvent;
  global.MessageEvent = MessageEvent;
  global.CloseEvent = CloseEvent;
  global.PromiseRejectionEvent = PromiseRejectionEvent;
  global.reportError = reportError;
  global.self = global;
  global.navigator = objectFreeze(brand({ userAgent: "Ibex/0.1.0" }, "Navigator"));
  global.addEventListener = function () {
    return functionApply(EventTarget.prototype.addEventListener, global, arguments);
  };
  global.removeEventListener = function () {
    return functionApply(EventTarget.prototype.removeEventListener, global, arguments);
  };
  global.dispatchEvent = function () {
    return functionApply(EventTarget.prototype.dispatchEvent, global, arguments);
  };
  defineEventHandler("onerror", "error", true);
  defineEventHandler("onunhandledrejection", "unhandledrejection", false);
  defineEventHandler("onrejectionhandled", "rejectionhandled", false);

  if (typeof Symbol === "function" && Symbol.toStringTag) {
    var taggedConstructors = [
      [Event, "Event"], [EventTarget, "EventTarget"], [CustomEvent, "CustomEvent"],
      [ErrorEvent, "ErrorEvent"], [MessageEvent, "MessageEvent"],
      [CloseEvent, "CloseEvent"], [PromiseRejectionEvent, "PromiseRejectionEvent"]
    ];
    arrayForEach(taggedConstructors, function (entry) {
      objectDefineProperty(entry[0].prototype, Symbol.toStringTag, {
        value: entry[1], configurable: true
      });
    });
  }

  return {
    reportException: reportException,
    fireTrustedEvent: fireTrustedEvent,
    hasEventListener: hasEventListener,
    setListenerChangeHook: setListenerChangeHook,
    setAbortHooks: setAbortHooks,
    onUnhandled: function (_, reason, promise) {
      var event = new PromiseRejectionEvent("unhandledrejection", {
        cancelable: true, promise: promise, reason: reason
      });
      // @ref LLP 0059.000#310-atob--btoa-structuredclone-blob--file--formdata-customevent--pure-ungated — stock Hermes exposes callable Promise tracker slots, so their events cannot authenticate host provenance
      if (dispatch(global, event, false)) nativeReport(errorText(reason));
    },
    onHandled: function (_, reason, promise) {
      dispatch(global,
        new PromiseRejectionEvent("rejectionhandled", { promise: promise, reason: reason }), false);
    }
  };
})(globalThis);
