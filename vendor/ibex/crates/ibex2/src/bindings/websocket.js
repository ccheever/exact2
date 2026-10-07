// WHATWG WebSocket over the Rust WebSocket library and L3 event subscription.
// The completion value is a factory: caller-owned runtimes invoke it once with
// their installer endowment, while the secure loader invokes it per grant set.
// @ref LLP 0067#1-five-properties — WebSocket authority is a lexical module binding
(function (global) {
  "use strict";

  // Capture every intrinsic used after bootstrap. Application code may
  // replace globals without changing parsing, payload conversion, events, or
  // the private state of an already-installed constructor.
  var ObjectCtor = global.Object;
  var ObjectCreate = ObjectCtor.create;
  var ObjectDefineProperty = ObjectCtor.defineProperty;
  var ObjectGetOwnPropertyDescriptor = ObjectCtor.getOwnPropertyDescriptor;
  var ObjectGetPrototypeOf = ObjectCtor.getPrototypeOf;
  var ArrayCtor = global.Array;
  var ArrayFrom = ArrayCtor.from;
  var arrayJoin = ArrayCtor.prototype.join;
  var arrayPush = ArrayCtor.prototype.push;
  var ArrayBufferCtor = global.ArrayBuffer;
  var ArrayBufferIsView = ArrayBufferCtor.isView;
  var arrayBufferSlice = ArrayBufferCtor.prototype.slice;
  var Uint8ArrayCtor = global.Uint8Array;
  var typedArrayPrototype = ObjectGetPrototypeOf(Uint8ArrayCtor.prototype);
  var typedBuffer = ObjectGetOwnPropertyDescriptor(typedArrayPrototype, "buffer").get;
  var typedOffset = ObjectGetOwnPropertyDescriptor(typedArrayPrototype, "byteOffset").get;
  var typedLength = ObjectGetOwnPropertyDescriptor(typedArrayPrototype, "byteLength").get;
  var WeakMapCtor = global.WeakMap;
  var weakGet = WeakMapCtor.prototype.get;
  var weakSet = WeakMapCtor.prototype.set;
  var URLCtor = global.URL;
  var StringCtor = global.String;
  var stringIndexOf = StringCtor.prototype.indexOf;
  var stringToLowerCase = StringCtor.prototype.toLowerCase;
  var stringCharCodeAt = StringCtor.prototype.charCodeAt;
  var NumberCtor = global.Number;
  var NumberIsNaN = NumberCtor.isNaN;
  var MathFloor = global.Math.floor;
  var ReflectApply = global.Reflect.apply;
  var ReflectConstruct = global.Reflect.construct;
  var TextDecoderCtor = global.TextDecoder;
  var TextEncoderCtor = global.TextEncoder;
  var textDecode = TextDecoderCtor.prototype.decode;
  var textEncode = TextEncoderCtor.prototype.encode;
  var EventCtor = global.Event;
  var MessageEventCtor = global.MessageEvent;
  var CloseEventCtor = global.CloseEvent;
  var EventTargetCtor = global.EventTarget;
  var eventAdd = EventTargetCtor.prototype.addEventListener;
  var eventRemove = EventTargetCtor.prototype.removeEventListener;
  var BlobCtor = global.Blob;
  var DOMExceptionCtor = global.DOMException;
  var ErrorCtor = global.Error;
  var TypeErrorCtor = global.TypeError;
  var SymbolCtor = global.Symbol;
  var SymbolToStringTag = SymbolCtor && SymbolCtor.toStringTag;
  var decoder = new TextDecoderCtor();
  var encoder = new TextEncoderCtor();
  var fireTrustedEvent = global.__ibex2_fire_trusted_event;
  var blobHelpers = global.__ibex2_blob_helpers;
  var brand = global.__ibex2_brand || function (value) { return value; };
  delete global.__ibex2_fire_trusted_event;
  delete global.__ibex2_blob_helpers;

  if (typeof fireTrustedEvent !== "function") {
    throw new ErrorCtor("WebSocket event hooks are unavailable");
  }

  function call(fn, receiver, args) {
    return ReflectApply(fn, receiver, args);
  }

  function privateRecord() {
    return ObjectCreate(null);
  }

  function domString(value) {
    if (typeof value === "symbol") {
      throw new TypeErrorCtor("cannot convert a Symbol to a string");
    }
    return StringCtor(value);
  }

  function syntax(message) {
    return new DOMExceptionCtor(message, "SyntaxError");
  }

  // Web IDL's [Clamp] unsigned short conversion (including ties-to-even).
  function closeCodeValue(value) {
    var number = NumberCtor(value);
    if (NumberIsNaN(number) || number <= 0) return 0;
    if (number >= 65535) return 65535;
    var lower = MathFloor(number);
    var fraction = number - lower;
    if (fraction < 0.5) return lower;
    if (fraction > 0.5) return lower + 1;
    return lower % 2 === 0 ? lower : lower + 1;
  }

  function parseURL(value) {
    var parsed;
    try {
      parsed = new URLCtor(domString(value));
    } catch (_) {
      throw syntax("The WebSocket URL is invalid");
    }
    if (parsed.protocol === "http:") parsed.protocol = "ws:";
    else if (parsed.protocol === "https:") parsed.protocol = "wss:";
    if (parsed.protocol !== "ws:" && parsed.protocol !== "wss:") {
      throw syntax("The WebSocket URL must use ws: or wss:");
    }
    if (call(stringIndexOf, parsed.href, ["#"]) !== -1) {
      throw syntax("The WebSocket URL must not contain a fragment");
    }
    if (parsed.username || parsed.password) {
      throw syntax("The WebSocket URL must not contain credentials");
    }
    return parsed;
  }

  function token(value) {
    if (value.length === 0) return false;
    for (var i = 0; i < value.length; i++) {
      var code = call(stringCharCodeAt, value, [i]);
      var alpha = (code >= 48 && code <= 57) ||
        (code >= 65 && code <= 90) || (code >= 97 && code <= 122);
      if (alpha || call(stringIndexOf, "!#$%&'*+-.^_`|~", [value[i]]) !== -1) continue;
      return false;
    }
    return true;
  }

  function protocolList(value, present) {
    if (!present || value === undefined) return [];
    var values = typeof value === "string"
      ? [value]
      : call(ArrayFrom, ArrayCtor, [value]);
    var result = [];
    for (var i = 0; i < values.length; i++) {
      var protocol = domString(values[i]);
      if (!token(protocol)) throw syntax("A WebSocket protocol is not an HTTP token");
      var folded = call(stringToLowerCase, protocol, []);
      for (var j = 0; j < result.length; j++) {
        if (call(stringToLowerCase, result[j], []) === folded) {
          throw syntax("WebSocket protocols must not contain duplicates");
        }
      }
      call(arrayPush, result, [protocol]);
    }
    return result;
  }

  function text(packet, offset) {
    return call(textDecode, decoder, [new Uint8ArrayCtor(packet, offset)]);
  }

  function binary(packet) {
    return call(arrayBufferSlice, packet, [1]);
  }

  function trackedType(type) {
    return type === "open" || type === "message" ||
      type === "error" || type === "close";
  }

  return function makeWebSocket(hooks, setListenerChangeHook) {
    if (!hooks || typeof hooks.open !== "function") {
      throw new ErrorCtor("WebSocket native hooks are unavailable");
    }
    if (typeof setListenerChangeHook !== "function") {
      throw new ErrorCtor("EventTarget listener hooks are unavailable");
    }

    var states = new WeakMapCtor();

    function stateOf(socket) {
      var state = call(weakGet, states, [socket]);
      if (!state) throw new TypeErrorCtor("Illegal invocation");
      return state;
    }

    function hasListener(state, type) {
      return state.listeners[type] === true;
    }

    function updateKeepalive(socket) {
      var state = stateOf(socket);
      if (!state.handle || typeof hooks.setKeepalive !== "function") return;
      var keep = false;
      if (state.readyState === 0) {
        keep = hasListener(state, "open") || hasListener(state, "message") ||
          hasListener(state, "error") || hasListener(state, "close");
      } else if (state.readyState === 1) {
        keep = hasListener(state, "message") || hasListener(state, "error") ||
          hasListener(state, "close");
      } else if (state.readyState === 2) {
        keep = hasListener(state, "error") || hasListener(state, "close");
      }
      hooks.setKeepalive(state.handle, keep);
    }

    function receive(socket, packet) {
      var state = stateOf(socket);
      var bytes = new Uint8ArrayCtor(packet);
      var kind = bytes[0];
      // Host events are tasks. State can change between admission and
      // delivery, so stale open/message tasks are suppressed here.
      if (kind === 0) {
        if (state.readyState !== 0) return;
        state.protocol = text(packet, 1);
        state.readyState = 1;
        updateKeepalive(socket);
        fireTrustedEvent(socket, new EventCtor("open"));
        return;
      }
      if (kind === 1) {
        if (state.readyState !== 1) return;
        fireTrustedEvent(socket, new MessageEventCtor("message", {
          data: text(packet, 1), origin: state.origin
        }));
        return;
      }
      if (kind === 2) {
        if (state.readyState !== 1) return;
        var data = binary(packet);
        if (state.binaryType === "blob" && typeof BlobCtor === "function") {
          data = new BlobCtor([data]);
        }
        fireTrustedEvent(socket, new MessageEventCtor("message", {
          data: data, origin: state.origin
        }));
        return;
      }
      if (kind === 3) {
        if (state.closeFired) return;
        state.readyState = 3;
        updateKeepalive(socket);
        fireTrustedEvent(socket, new EventCtor("error"));
        return;
      }
      if (kind === 4) {
        if (state.closeFired) return;
        state.readyState = 3;
        state.closeFired = true;
        updateKeepalive(socket);
        fireTrustedEvent(socket, new CloseEventCtor("close", {
          wasClean: bytes[1] !== 0,
          code: (bytes[2] << 8) | bytes[3],
          reason: text(packet, 4)
        }));
      }
    }

    function WebSocket(url, protocols) {
      if (!new.target) throw new TypeErrorCtor("WebSocket requires new");
      if (arguments.length === 0) throw new TypeErrorCtor("WebSocket requires a URL");
      if (!hooks.supported) {
        throw new DOMExceptionCtor(
          "WebSocket was omitted from this build", "NotSupportedError");
      }
      var parsed = parseURL(url);
      var offered = protocolList(protocols, arguments.length > 1);
      var socket = ReflectConstruct(EventTargetCtor, [], new.target);
      var state = privateRecord();
      state.handle = 0;
      state.url = parsed.href;
      state.origin = parsed.origin;
      state.protocol = "";
      state.binaryType = "blob";
      state.readyState = 0;
      state.handlers = privateRecord();
      state.listeners = privateRecord();
      state.closeFired = false;
      call(weakSet, states, [socket, state]);
      brand(socket, "WebSocket");
      call(setListenerChangeHook, null, [socket, function (type, present) {
        if (!trackedType(type)) return;
        state.listeners[type] = present;
        updateKeepalive(socket);
      }]);
      state.handle = hooks.open(
        socket, receive, state.url, call(arrayJoin, offered, ["\n"]));
      updateKeepalive(socket);
      return socket;
    }

    WebSocket.prototype = ObjectCreate(EventTargetCtor.prototype, {
      constructor: { value: WebSocket, writable: true, configurable: true },
      url: { get: function () { return stateOf(this).url; }, enumerable: true },
      readyState: {
        get: function () { return stateOf(this).readyState; }, enumerable: true
      },
      bufferedAmount: {
        get: function () {
          var state = stateOf(this);
          return hooks.bufferedAmount(state.handle);
        }, enumerable: true
      },
      extensions: { get: function () { stateOf(this); return ""; }, enumerable: true },
      protocol: { get: function () { return stateOf(this).protocol; }, enumerable: true },
      binaryType: {
        get: function () { return stateOf(this).binaryType; },
        set: function (value) {
          var state = stateOf(this);
          value = domString(value);
          if (value === "blob" || value === "arraybuffer") state.binaryType = value;
        },
        enumerable: true
      }
    });

    ObjectDefineProperty(WebSocket.prototype, "send", {
      value: function (data) {
      var state = stateOf(this);
      if (state.readyState === 0) {
        throw new DOMExceptionCtor(
          "The WebSocket is still connecting", "InvalidStateError");
      }
      if (data instanceof ArrayBufferCtor) {
        hooks.sendBinary(state.handle, data);
      } else if (ArrayBufferIsView(data)) {
        hooks.sendBinary(state.handle, new Uint8ArrayCtor(
          call(typedBuffer, data, []), call(typedOffset, data, []),
          call(typedLength, data, [])));
      } else if (typeof BlobCtor === "function" && data instanceof BlobCtor) {
        if (!blobHelpers || typeof blobHelpers.extractBody !== "function") {
          throw new DOMExceptionCtor(
            "Blob WebSocket send is unavailable", "NotSupportedError");
        }
        hooks.sendBinary(state.handle, blobHelpers.extractBody(data).bytes);
      } else {
        hooks.sendText(state.handle, domString(data));
      }
      updateKeepalive(this);
      }, writable: true, configurable: true
    });

    ObjectDefineProperty(WebSocket.prototype, "close", {
      value: function (code, reason) {
      var state = stateOf(this);
      var closeCode = 0;
      if (arguments.length > 0 && code !== undefined) {
        closeCode = closeCodeValue(code);
        if (closeCode !== 1000 && (closeCode < 3000 || closeCode > 4999)) {
          throw new DOMExceptionCtor(
            "The close code is not allowed", "InvalidAccessError");
        }
      }
      var closeReason = arguments.length > 1 ? domString(reason) : "";
      if (closeCode === 0 && closeReason !== "") {
        throw syntax("A close reason requires a close code");
      }
      if (call(textEncode, encoder, [closeReason]).byteLength > 123) {
        throw syntax("The close reason exceeds 123 UTF-8 bytes");
      }
      if (state.readyState === 2 || state.readyState === 3) return;
      state.readyState = 2;
      updateKeepalive(this);
      hooks.close(state.handle, closeCode, closeReason);
      }, writable: true, configurable: true
    });

    var handlerTypes = ["open", "message", "error", "close"];
    for (var handlerIndex = 0; handlerIndex < handlerTypes.length; handlerIndex++) {
      (function (type) {
      ObjectDefineProperty(WebSocket.prototype, "on" + type, {
        get: function () { return stateOf(this).handlers[type] || null; },
        set: function (value) {
          var state = stateOf(this);
          var previous = state.handlers[type];
          if (previous) call(eventRemove, this, [type, previous]);
          if (typeof value === "function") {
            state.handlers[type] = value;
            call(eventAdd, this, [type, value]);
          } else {
            delete state.handlers[type];
          }
        },
        enumerable: true,
        configurable: true
      });
      })(handlerTypes[handlerIndex]);
    }

    var constants = [
      ["CONNECTING", 0], ["OPEN", 1], ["CLOSING", 2], ["CLOSED", 3]
    ];
    for (var constantIndex = 0; constantIndex < constants.length; constantIndex++) {
      var entry = constants[constantIndex];
      ObjectDefineProperty(WebSocket, entry[0], { value: entry[1], enumerable: true });
      ObjectDefineProperty(WebSocket.prototype, entry[0], {
        value: entry[1], enumerable: true
      });
    }
    if (SymbolToStringTag) {
      ObjectDefineProperty(WebSocket.prototype, SymbolToStringTag, {
        value: "WebSocket", configurable: true
      });
    }
    return WebSocket;
  };
})(globalThis);
