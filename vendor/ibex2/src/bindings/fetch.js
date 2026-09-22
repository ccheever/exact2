// Grant-carrying fetch and demand-driven response bodies. Native handles never
// escape these closures: neither readers nor signals can forge network authority.
(function (global) {
  "use strict";
  var field = global.__ibex2_response_field;
  var readBody = global.__ibex2_response_read;
  var control = global.__ibex2_fetch_control;
  var retain = global.__ibex2_response_own;
  var abort = global.__ibex2_abort;
  var decode = global.__ibex2_text_decode;
  var Headers = global.Headers;
  var freeHeaders = global.__ibex2_headers_free;
  ["__ibex2_response_field", "__ibex2_response_read", "__ibex2_response_own", "__ibex2_fetch_control", "__ibex2_abort",
   "__ibex2_headers_free", "__ibex2_text_encode", "__ibex2_text_decode", "__ibex2_text_encode_into"]
    .forEach(function (name) { delete global[name]; });
  var responses = new WeakMap(), streams = new WeakMap(), readers = new WeakMap();
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
      pending.resolve({ value: bytes === null ? undefined : new Uint8Array(bytes), done: bytes === null });
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
    if (state.body === null) return Promise.resolve(new ArrayBuffer(0));
    if (state.used || state.reader) return Promise.reject(new TypeError("body already consumed or locked"));
    var reader = state.body.getReader(), chunks = [], length = 0;
    function next() {
      return reader.read().then(function (chunk) {
        if (!chunk.done) { chunks.push(chunk.value); length += chunk.value.byteLength; return next(); }
        var bytes = new Uint8Array(length), offset = 0;
        chunks.forEach(function (part) { bytes.set(part, offset); offset += part.byteLength; });
        return bytes.buffer;
      });
    }
    return next();
  }
  Response.prototype.arrayBuffer = function () { return consume(this); };
  Response.prototype.text = function () { return consume(this).then(function (bytes) { return decode(bytes); }); };
  Response.prototype.json = function () { return this.text().then(function (text) { return JSON.parse(text); }); };
  Object.defineProperty(Response.prototype, Symbol.toStringTag, { value: "Response" });
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
        init = init || {};
        var signal = init.signal;
        if (signal != null && abort.own(signal).aborted) return Promise.reject(abort.own(signal).reason);
        var url = String(input), method = init.method === undefined ? "" : String(init.method), body = init.body;
        if (typeof body === "string") body = new TextEncoder().encode(body);
        var redirect = init.redirect === undefined ? "follow" : String(init.redirect);
        headers = new Headers(init.headers);
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
