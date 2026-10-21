// Grant-carrying fetch and demand-driven response bodies. Native handles never
// escape these closures: neither readers nor signals can forge network authority.
(function (global) {
  "use strict";
  var ObjectCtor = global.Object;
  var ObjectGetOwnPropertyDescriptor = ObjectCtor.getOwnPropertyDescriptor;
  var ObjectGetPrototypeOf = ObjectCtor.getPrototypeOf;
  var ArrayCtor = global.Array;
  var ArrayBufferCtor = global.ArrayBuffer;
  var Uint8ArrayCtor = global.Uint8Array;
  var ReflectApply = global.Reflect.apply;
  var TypedArrayPrototype = ObjectGetPrototypeOf(Uint8ArrayCtor.prototype);
  var typedArrayBuffer = ObjectGetOwnPropertyDescriptor(TypedArrayPrototype, "buffer").get;
  var typedArrayByteLength = ObjectGetOwnPropertyDescriptor(TypedArrayPrototype, "byteLength").get;
  var typedArraySet = Uint8ArrayCtor.prototype.set;
  function call(fn, receiver, args) { return ReflectApply(fn, receiver, args); }
  var brand = global.__ibex2_brand || function (value) { return value; };
  var field = global.__ibex2_response_field;
  var readBody = global.__ibex2_response_read;
  var control = global.__ibex2_fetch_control;
  var retain = global.__ibex2_response_own;
  var abort = global.__ibex2_abort;
  var blobHelpers = global.__ibex2_blob_helpers;
  var decode = global.__ibex2_text_decode;
  var encode = global.__ibex2_text_encode;
  var Headers = global.Headers;
  var URLSearchParams = global.URLSearchParams;
  var DOMException = global.DOMException;
  var freeHeaders = global.__ibex2_headers_free;
  ["__ibex2_response_field", "__ibex2_response_read", "__ibex2_response_own", "__ibex2_fetch_control", "__ibex2_abort",
   "__ibex2_headers_free", "__ibex2_text_encode", "__ibex2_text_decode", "__ibex2_text_encode_into",
   "__ibex2_blob_helpers"]
    .forEach(function (name) { delete global[name]; });
  var responses = new WeakMap(), streams = new WeakMap(), readers = new WeakMap(), requests = new WeakMap();
  function own(map, object, name) {
    var state = map.get(object);
    if (!state) throw new TypeError("not a " + name);
    return state;
  }
  function deferred() {
    var result = {};
    result.promise = new Promise(function (resolve, reject) { result.resolve = resolve; result.reject = reject; });
    result.promise.catch(function () {});
    return result;
  }
  function finish(state, error, failed) {
    if (state.terminal) return;
    state.terminal = true;
    state.failed = !!failed;
    state.error = error;
    field(state.handle, 8);
    state.cleanup();
    if (state.reader) {
      var reader = readers.get(state.reader);
      if (failed) reader.closed.reject(error); else reader.closed.resolve();
    }
  }
  function Response() { throw new TypeError("Response comes from fetch"); }
  // This is deliberately a response-body reader surface, not a general-purpose
  // ReadableStream constructor: user sources, BYOB and piping are not implemented.
  function ResponseBody() { throw new TypeError("ResponseBody comes from fetch"); }
  function ResponseBodyReader() { throw new TypeError("Reader comes from getReader"); }
  ResponseBody.prototype.getReader = function (options) {
    var state = own(streams, this, "response body");
    if (options && options.mode !== undefined) throw new TypeError("BYOB readers are not supported");
    if (state.reader) throw new TypeError("body is locked");
    var reader = Object.create(ResponseBodyReader.prototype), closed = deferred();
    readers.set(reader, { state: state, closed: closed, pending: [] });
    brand(reader, "ResponseBodyReader");
    state.reader = reader;
    if (state.terminal) { if (state.failed) closed.reject(state.error); else closed.resolve(); }
    return reader;
  };
  Object.defineProperty(ResponseBody.prototype, "locked", { get: function () { return !!own(streams, this, "response body").reader; } });
  function cancel(state) {
    state.used = true;
    if (state.failed) return Promise.reject(state.error);
    finish(state);
    return Promise.resolve();
  }
  ResponseBody.prototype.cancel = function () {
    var state = own(streams, this, "response body");
    if (state.reader) return Promise.reject(new TypeError("body is locked"));
    return cancel(state);
  };
  ResponseBodyReader.prototype.cancel = function () {
    var reader = own(readers, this, "reader");
    if (!reader.state) return Promise.reject(new TypeError("reader released"));
    return cancel(reader.state);
  };
  Object.defineProperty(ResponseBodyReader.prototype, "closed", { get: function () { return own(readers, this, "reader").closed.promise; } });
  ResponseBodyReader.prototype.releaseLock = function () {
    var reader = own(readers, this, "reader"), state = reader.state;
    if (!state) return;
    var error = new TypeError("reader released");
    reader.pending.forEach(function (pending) { pending.reject(error); });
    reader.pending = [];
    if (state.terminal) reader.closed = deferred();
    reader.closed.reject(error);
    reader.state = null;
    state.reader = null;
  };
  ResponseBodyReader.prototype.read = function () {
    var reader = own(readers, this, "reader"), state = reader.state;
    if (!state) return Promise.reject(new TypeError("reader released"));
    state.used = true;
    var pending = deferred();
    reader.pending.push(pending);
    // One native read at a time, including when readers release and reacquire.
    state.tail = state.tail.then(function () {
      if (reader.state !== state) throw new TypeError("reader released");
      if (state.terminal) {
        if (state.failed) throw state.error;
        return null;
      }
      if (state.saved) { var saved = state.saved; state.saved = null; return saved; }
      return readBody(state.handle);
    }).then(function (bytes) {
      if (state.failed) throw state.error;
      if (reader.state !== state) {
        // A released reader cannot consume the next reader's bytes.
        if (bytes !== null) state.saved = bytes;
        throw new TypeError("reader released");
      }
      if (state.terminal) bytes = null;
      if (bytes === null) finish(state);
      pending.resolve({ value: bytes === null ? undefined : new Uint8ArrayCtor(bytes), done: bytes === null });
    }, function (error) {
      if (reader.state === state && !state.terminal) finish(state, error, true);
      if (state.terminal && !state.failed && reader.state === state) pending.resolve({ value: undefined, done: true });
      else pending.reject(state.failed ? state.error : error);
    }).then(function () {
      var index = reader.pending.indexOf(pending);
      if (index >= 0) reader.pending.splice(index, 1);
    }, function (error) { pending.reject(error); });
    return pending.promise;
  };
  function observe(signal, weakBody) {
    return abort.subscribe(signal, function () {
      var body = weakBody();
      if (body) finish(streams.get(body), abort.own(signal).reason, true);
    }, weakBody);
  }
  function response(handle, cleanup, signal, method) {
    var result = Object.create(Response.prototype), body = Object.create(ResponseBody.prototype);
    var state = { handle: handle, used: false, terminal: false, failed: false, reader: null,
      tail: Promise.resolve(), cleanup: cleanup, body: body,
      status: field(handle, 0), ok: field(handle, 1), url: field(handle, 2),
      redirected: field(handle, 5), headers: new Headers(JSON.parse(field(handle, 7))) };
    responses.set(result, state);
    streams.set(body, state);
    brand(result, "Response");
    brand(body, "ResponseBody");
    var weakBody = retain(handle, body);
    if (method.toUpperCase() === "HEAD" || state.status === 204 || state.status === 205 || state.status === 304) {
      state.body = null;
      finish(state);
    } else if (signal) {
      var release = observe(signal, weakBody);
      state.cleanup = function () { release(); cleanup(); };
    }
    return result;
  }
  ["status", "ok", "url", "redirected", "headers", "body"].forEach(function (name) {
    Object.defineProperty(Response.prototype, name, { get: function () { return own(responses, this, "Response")[name]; }, enumerable: true });
  });
  Object.defineProperty(Response.prototype, "bodyUsed", { get: function () { return own(responses, this, "Response").used; }, enumerable: true });
  function consume(response) {
    var state = own(responses, response, "Response");
    if (state.body === null) return Promise.resolve(new ArrayBufferCtor(0));
    if (state.used || state.reader) return Promise.reject(new TypeError("body already consumed or locked"));
    var reader = state.body.getReader(), chunks = new ArrayCtor(), length = 0;
    function next() {
      return reader.read().then(function (chunk) {
        if (!chunk.done) {
          chunks[chunks.length] = chunk.value;
          length += call(typedArrayByteLength, chunk.value, []);
          return next();
        }
        // Do not read `.buffer`, `.byteLength`, or `.set` through caller-
        // controlled prototypes: this backing store becomes Blob private state.
        var bytes = new Uint8ArrayCtor(length), offset = 0;
        for (var i = 0; i < chunks.length; i++) {
          var part = chunks[i];
          call(typedArraySet, bytes, [part, offset]);
          offset += call(typedArrayByteLength, part, []);
        }
        return call(typedArrayBuffer, bytes, []);
      });
    }
    return next();
  }
  Response.prototype.arrayBuffer = function () { return consume(this); };
  Response.prototype.text = function () { return consume(this).then(function (bytes) { return decode(bytes); }); };
  Response.prototype.json = function () { return this.text().then(function (text) { return JSON.parse(text); }); };
  function httpWhitespace(code) {
    return code === 0x09 || code === 0x0a || code === 0x0d || code === 0x20;
  }
  function tokenCode(code) {
    return code >= 0x30 && code <= 0x39 || code >= 0x41 && code <= 0x5a ||
      code >= 0x61 && code <= 0x7a || "!#$%&'*+-.^_`|~".indexOf(String.fromCharCode(code)) >= 0;
  }
  function token(value) {
    if (!value) return false;
    for (var i = 0; i < value.length; i++) if (!tokenCode(value.charCodeAt(i))) return false;
    return true;
  }
  function trimHttp(value) {
    var first = 0, last = value.length;
    while (first < last && httpWhitespace(value.charCodeAt(first))) first++;
    while (last > first && httpWhitespace(value.charCodeAt(last - 1))) last--;
    return value.slice(first, last);
  }
  function quotedValueCode(code) {
    return code === 0x09 || code >= 0x20 && code <= 0x7e || code >= 0x80 && code <= 0xff;
  }
  function serializeMimeParameter(value) {
    if (token(value)) return value;
    var result = '"';
    for (var i = 0; i < value.length; i++) {
      var character = value[i];
      if (character === '"' || character === "\\") result += "\\";
      result += character;
    }
    return result + '"';
  }
  function splitMimeTypes(input) {
    var values = [], start = 0, quoted = false;
    for (var i = 0; i < input.length; i++) {
      var character = input[i];
      if (quoted && character === "\\" && i + 1 < input.length) {
        i++;
      } else if (character === '"') {
        quoted = !quoted;
      } else if (character === "," && !quoted) {
        values.push(input.slice(start, i));
        start = i + 1;
      }
    }
    values.push(input.slice(start));
    return values;
  }
  function parseMimeType(input) {
    var semicolon = input.indexOf(";");
    var essence = trimHttp(semicolon < 0 ? input : input.slice(0, semicolon));
    var slash = essence.indexOf("/");
    if (slash <= 0 || slash !== essence.lastIndexOf("/")) return null;
    var type = essence.slice(0, slash), subtype = essence.slice(slash + 1);
    if (!token(type) || !token(subtype)) return null;
    var parsed = {
      essence: type.toLowerCase() + "/" + subtype.toLowerCase(),
      parameters: []
    };
    var names = Object.create(null), position = semicolon < 0 ? input.length : semicolon;
    while (position < input.length) {
      if (input[position] === ";") position++;
      while (position < input.length && httpWhitespace(input.charCodeAt(position))) position++;
      var nameStart = position;
      while (position < input.length && input[position] !== ";" && input[position] !== "=") position++;
      var name = input.slice(nameStart, position).toLowerCase();
      if (position >= input.length || input[position] === ";") continue;
      position++;
      while (position < input.length && httpWhitespace(input.charCodeAt(position))) position++;
      var value = "", valid = true;
      if (input[position] === '"') {
        position++;
        var closed = false;
        while (position < input.length) {
          var character = input[position++];
          if (character === '"') { closed = true; break; }
          if (character === "\\" && position < input.length) character = input[position++];
          if (!quotedValueCode(character.charCodeAt(0))) valid = false;
          value += character;
        }
        if (!closed) valid = false;
        while (position < input.length && input[position] !== ";") position++;
      } else {
        var valueStart = position;
        while (position < input.length && input[position] !== ";") position++;
        value = trimHttp(input.slice(valueStart, position));
        if (!token(value)) valid = false;
      }
      if (valid && token(name) && names[name] === undefined) {
        names[name] = true;
        parsed.parameters.push([name, value]);
      }
    }
    return parsed;
  }
  function mimeParameter(mimeType, name) {
    for (var i = 0; i < mimeType.parameters.length; i++) {
      if (mimeType.parameters[i][0] === name) return mimeType.parameters[i][1];
    }
    return undefined;
  }
  function serializeMimeType(mimeType) {
    var result = mimeType.essence;
    for (var i = 0; i < mimeType.parameters.length; i++) {
      result += ";" + mimeType.parameters[i][0] + "=" +
        serializeMimeParameter(mimeType.parameters[i][1]);
    }
    return result;
  }
  // Fetch extracts the last valid, non-wildcard MIME type from the combined
  // Content-Type value. A charset carries through later values only while the
  // selected essence stays the same.
  // @ref LLP 0059.000#35-fetch--delegating-capability-bearing — Response.blob uses Fetch MIME extraction
  function extractMimeType(input) {
    if (input === null) return "";
    var values = splitMimeTypes(String(input));
    var mimeType = null, charset = null, essence = null;
    for (var i = 0; i < values.length; i++) {
      var parsed = parseMimeType(values[i]);
      if (parsed === null || parsed.essence === "*/*") continue;
      mimeType = parsed;
      var parsedCharset = mimeParameter(parsed, "charset");
      if (parsed.essence !== essence) {
        charset = parsedCharset === undefined ? null : parsedCharset;
        essence = parsed.essence;
      } else if (parsedCharset === undefined && charset !== null) {
        parsed.parameters.push(["charset", charset]);
      }
    }
    return mimeType === null ? "" : serializeMimeType(mimeType);
  }
  if (blobHelpers) {
    Response.prototype.blob = function () {
      var state = own(responses, this, "Response");
      var type = extractMimeType(state.headers.get("content-type"));
      return consume(this).then(function (bytes) { return blobHelpers.responseBlob(bytes, type); });
    };
    Response.prototype.formData = function () {
      return Promise.reject(new DOMException("Response.formData() parsing is not supported", "NotSupportedError"));
    };
  }
  Object.defineProperty(Response.prototype, Symbol.toStringTag, { value: "Response" });
  function convertBody(body, headers) {
    if (body === undefined || body === null) return undefined;
    if (blobHelpers) {
      // @ref LLP 0059.000#35-fetch--delegating-capability-bearing — authored Content-Type wins for Blob and FormData bodies
      var extracted = blobHelpers.extractBody(body);
      if (extracted) {
        if (extracted.type && !headers.has("content-type")) headers.set("Content-Type", extracted.type);
        return extracted.bytes;
      }
      var snapshot = blobHelpers.snapshotBufferSource(body);
      if (snapshot) return snapshot;
    }
    if (body instanceof URLSearchParams) {
      if (!headers.has("content-type")) {
        headers.set("Content-Type", "application/x-www-form-urlencoded;charset=UTF-8");
      }
      return encode(String(body));
    }
    if (body instanceof ArrayBuffer || ArrayBuffer.isView(body)) return body;
    return encode(String(body));
  }
  function Request(input, init) {
    if (!(this instanceof Request)) throw new TypeError("Request must be constructed with new");
    if (arguments.length === 0) throw new TypeError("Request expects a URL");
    init = init || {};
    var inherited = requests.get(input);
    var url = inherited ? inherited.url : String(input);
    var method = init.method === undefined ? (inherited ? inherited.method : "GET") : String(init.method);
    var redirect = init.redirect === undefined ? (inherited ? inherited.redirect : "follow") : String(init.redirect);
    var signal = init.signal === undefined ? (inherited ? inherited.signal : undefined) : init.signal;
    var headers = new Headers(init.headers === undefined && inherited ? inherited.headers : init.headers);
    var body;
    try {
      body = init.body === undefined && inherited ? inherited.body : convertBody(init.body, headers);
      if (body !== undefined && (method.toUpperCase() === "GET" || method.toUpperCase() === "HEAD")) {
        throw new TypeError("Request with GET/HEAD method cannot have a body");
      }
    } catch (error) {
      freeHeaders(headers._handle);
      throw error;
    }
    requests.set(this, { url: url, method: method, redirect: redirect, signal: signal, headers: headers, body: body });
    brand(this, "Request");
  }
  ["url", "method", "redirect", "signal", "headers"].forEach(function (name) {
    Object.defineProperty(Request.prototype, name, { get: function () { return own(requests, this, "Request")[name]; }, enumerable: true });
  });
  Object.defineProperty(Request.prototype, Symbol.toStringTag, { value: "Request" });
  if (blobHelpers) {
    global.Request = Request;
    global.Response = Response;
    Object.freeze(Request.prototype);
    Object.freeze(Request);
  }
  [Response, ResponseBody, ResponseBodyReader].forEach(function (type) { Object.freeze(type.prototype); Object.freeze(type); });
  return function makeFetch(raw) {
    return function fetch(input, init) {
      var headers, token, unsubscribe = function () {}, released = false;
      function cleanup() {
        if (released) return;
        released = true;
        unsubscribe();
        if (token !== undefined) control(2, token);
      }
      try {
        if (arguments.length === 0) throw new TypeError("fetch expects a URL");
        var signal, url, method, body, redirect;
        if (blobHelpers) {
          var request = new Request(input, init);
          var state = requests.get(request);
          // Request construction has already allocated this one-shot snapshot.
          // Publish it to the common cleanup path before signal validation or
          // the pre-abort check can throw or reject.
          headers = state.headers;
          signal = state.signal;
          if (signal != null && abort.own(signal).aborted) throw abort.own(signal).reason;
          url = state.url; method = state.method; body = state.body; redirect = state.redirect;
          // `request` is an internal one-shot snapshot. Passing its own Headers
          // handle avoids a second clone; every exit below frees this handle.
        } else {
          // Preserve the pre-BLOB FETCH contract byte for byte: only strings
          // are converted here, other values cross through the existing ABI,
          // and Request/Response constructors are not exposed.
          init = init || {};
          signal = init.signal;
          if (signal != null && abort.own(signal).aborted) return Promise.reject(abort.own(signal).reason);
          url = String(input);
          method = init.method === undefined ? "" : String(init.method);
          body = init.body;
          if (typeof body === "string") body = new TextEncoder().encode(body);
          redirect = init.redirect === undefined ? "follow" : String(init.redirect);
          headers = new Headers(init.headers);
        }
        token = control(0);
        if (signal != null) unsubscribe = abort.subscribe(signal, function () { control(1, token); });
        return raw(url, method, body, redirect, headers._handle, token).then(function (handle) {
          freeHeaders(headers._handle);
          if (signal != null && abort.own(signal).aborted) {
            field(handle, 8); cleanup(); throw abort.own(signal).reason;
          }
          unsubscribe();
          unsubscribe = function () {};
          return response(handle, cleanup, signal, method);
        }, function (error) {
          freeHeaders(headers._handle); cleanup();
          throw signal != null && abort.own(signal).aborted ? abort.own(signal).reason : error;
        });
      } catch (error) {
        if (headers) freeHeaders(headers._handle);
        cleanup();
        return Promise.reject(error);
      }
    };
  };
})(globalThis);
