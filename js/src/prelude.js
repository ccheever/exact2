// The executor's prelude (LLP 1027 D1a, D10): what a data module finds in
// its global object beyond the language — `fetch` over the host's ticket
// path, the store the seam hands to `answer`, and the three functions the
// executor calls. Compiled to bytecode by build.rs with the same hermesc as
// the module; evaluated once, before the module, at `Module::load`.
//
// Nothing here does I/O. `fetch` records a request through the one host
// function and returns a Promise the executor resolves when the host has
// run the request under the app's grants (LLP 1016 D1/D6); the store's
// reads are counted and its writes grant-checked in Rust (LLP 1018).
(function (global) {
  "use strict";
  // (op, a, b) -> string | undefined. Ops: 1 request(ticket, json),
  // 2 store.get(name), 3 store.set(name, value), 4 store.forget(name), 5 storage capability check.
  var host = global.__exact_host;
  delete global.__exact_host;

  // LLP 1027.000: time and seeds are source arguments, so bake and cache
  // see them. This VM belongs to one module; no page/guest globals change.
  // Install before the app can capture an alias, including Date's prototype
  // constructor. Keep explicit-value Date construction and UTC arithmetic.
  function refuseAmbient(api) {
    throw new Error(api + " is unavailable in data sources; pass time or a random seed as an argument");
  }
  function fixed(object, name, value) {
    Object.defineProperty(object, name, { value: value, writable: false, configurable: false });
  }
  var NativeDate = global.Date;
  var construct = Reflect.construct;
  var InputDate = new Proxy(NativeDate, {
    apply: function () { return refuseAmbient("Date()"); },
    construct: function (target, args, newTarget) {
      if (!args.length) return refuseAmbient("new Date()");
      return construct(target, args, newTarget);
    },
  });
  fixed(NativeDate, "now", function () { return refuseAmbient("Date.now()"); });
  fixed(NativeDate.prototype, "constructor", InputDate);
  fixed(global, "Date", InputDate);
  fixed(global.Math, "random", function () { return refuseAmbient("Math.random()"); });

  // Intl's formatting methods also default an omitted/undefined date to
  // machine time. Guard the prototype before an app can capture its bound
  // format getter or formatToParts method; explicit timestamps still use
  // the engine's locale/timezone implementation unchanged.
  if (global.Intl && global.Intl.DateTimeFormat) {
    var dateFormat = global.Intl.DateTimeFormat.prototype;
    var getFormat = Object.getOwnPropertyDescriptor(dateFormat, "format").get;
    var formats = new WeakMap();
    var apply = Reflect.apply;
    function explicitFormat(fn, name) {
      return new Proxy(fn, {
        apply: function (target, receiver, args) {
          if (args[0] === undefined) return refuseAmbient("Intl.DateTimeFormat." + name + "()");
          return apply(target, receiver, args);
        },
      });
    }
    Object.defineProperty(dateFormat, "format", {
      configurable: false,
      get: function () {
        var native = getFormat.call(this);
        if (!formats.has(native)) formats.set(native, explicitFormat(native, "format"));
        return formats.get(native);
      },
    });
    if (typeof dateFormat.formatToParts === "function") {
      fixed(dateFormat, "formatToParts", explicitFormat(dateFormat.formatToParts, "formatToParts"));
    }
  }

  function fromBase64(text) {
    return Uint8Array.from(global.atob(text), function (c) { return c.charCodeAt(0); });
  }

  // --- fetch: a request the host runs; a Promise for its reply -------------
  var nextTicket = 1;
  var pending = new Map();   // ticket -> { resolve, reject, call }
  var currentCall = null;    // the answer a fetch belongs to

  function Headers(init) {
    this._h = [];
    if (init instanceof Headers) init = init._h;
    if (Array.isArray(init)) for (var i = 0; i < init.length; i++) this.append(init[i][0], init[i][1]);
    else if (init && typeof init === "object") for (var k in init) if (Object.prototype.hasOwnProperty.call(init, k)) this.append(k, init[k]);
  }
  Headers.prototype.append = function (k, v) { this._h.push([String(k).toLowerCase(), String(v)]); };
  Headers.prototype.set = function (k, v) { this.delete(k); this.append(k, v); };
  Headers.prototype.delete = function (k) { k = String(k).toLowerCase(); this._h = this._h.filter(function (e) { return e[0] !== k; }); };
  Headers.prototype.get = function (k) {
    k = String(k).toLowerCase();
    var v = this._h.filter(function (e) { return e[0] === k; }).map(function (e) { return e[1]; });
    return v.length ? v.join(", ") : null;
  };
  Headers.prototype.has = function (k) { return this.get(k) !== null; };
  Headers.prototype.entries = function () { return this._h.slice(); };
  Headers.prototype.forEach = function (f) { this._h.forEach(function (e) { f(e[1], e[0]); }); };
  Headers.prototype.toJSON = function () { return this._h.slice(); };

  function Response(r) {
    this.status = r.status;
    this.ok = r.status >= 200 && r.status < 300;
    this.headers = new Headers(r.headers);
    this._text = r.body;
    this._b64 = r.bodyBase64;
  }
  Response.prototype.text = function () { return Promise.resolve(this._text); };
  Response.prototype.json = function () { var t = this._text; return new Promise(function (res) { res(JSON.parse(t)); }); };
  Response.prototype.arrayBuffer = function () { return Promise.resolve(fromBase64(this._b64).buffer); };

  function FetchError(f) {
    // Storage hardens Error.prototype. Define own fields instead of assigning
    // through its frozen inherited name/message properties.
    Object.defineProperties(this, {
      name: { value: "FetchError", configurable: true },
      message: { value: String(f.message), configurable: true },
      kind: { value: String(f.kind), configurable: true },
    });
  }
  FetchError.prototype = Object.create(Error.prototype);

  global.Headers = Headers;
  global.Response = Response;
  global.fetch = function (url, init) {
    var call = currentCall;
    if (!call) return Promise.reject(new Error("fetch called outside an answer"));
    var method = init && init.method ? String(init.method).toUpperCase() : "GET";
    var headers = new Headers(init && init.headers).entries();
    var body = init && init.body != null ? String(init.body) : "";
    // LLP 1041 §8.4: an explicit promise about both operation and settlement.
    // Browsers ignore this native scheduling hint; their admission is unchanged.
    var independent = init ? init.exactIndependentHttp : undefined;
    var ceiling;
    if (independent !== undefined) {
      ceiling = independent && independent.maxResponseBytes;
      if (!Number.isInteger(ceiling) || ceiling <= 0 || ceiling > 67108864)
        return Promise.reject(new TypeError("exactIndependentHttp.maxResponseBytes must be an integer from 1 to 67108864"));
    }
    var ticket = nextTicket++;
    var error = host(1, String(ticket), JSON.stringify({ method: method, url: String(url), headers: headers, body: body, max_response_bytes: ceiling }));
    if (error !== undefined) return Promise.reject(new Error(error));
    call.tickets.push(ticket);
    return new Promise(function (resolve, reject) { pending.set(ticket, { resolve: resolve, reject: reject, call: call }); });
  };

  // --- the store (LLP 1018): reads counted, writes grant-checked, in Rust -
  var store = {
    get: function (name) { var v = host(2, String(name), ""); return v === undefined ? null : v; },
    set: function (name, value) { var e = host(3, String(name), String(value)); if (e !== undefined) throw new Error(e); },
    forget: function (name) { var e = host(4, String(name), ""); if (e !== undefined) throw new Error(e); },
  };

  // Storage is a capability argument, never an ambient global. Native hosts
  // install Ibex2's objects during trusted initialization; bake and the browser
  // retain this explicit refusal surface until a provider is installed.
  var nativeStorage = null;
  global.__exact_install_storage = function () {
    nativeStorage = global.__exact_storage;
    delete global.__exact_storage;
    delete global.__exact_install_storage;
  };
  function storageError(message) {
    var error = new Error(message);
    error.kind = "Unavailable";
    return error;
  }
  function storageCall(receiver, method, args, convert) {
    var call = currentCall;
    if (!call || call.status !== "pending") return Promise.reject(storageError("storage called outside an answer"));
    try {
      host(5, "", ""); // no filesystem or database effects during bake
      if (!receiver) throw storageError("storage is unsupported by this host");
    } catch (e) { return Promise.reject(storageError(e.message || String(e))); }
    call.storage++;
    var promise;
    try { promise = receiver[method].apply(receiver, args); }
    catch (e) { promise = Promise.reject(e); }
    return promise.then(function (value) {
      currentCall = call;
      call.storage--;
      return convert ? convert(value) : value;
    }, function (error) {
      currentCall = call;
      call.storage--;
      throw error && error.kind ? error : storageError(error.message || String(error));
    });
  }
  function statement(raw) {
    return Object.freeze({
      execute: function (params) { return storageCall(raw, "execute", [params]); },
      query: function (params) { return storageCall(raw, "query", [params]); },
      close: function () { return storageCall(raw, "close", []); },
    });
  }
  function database(raw) {
    return Object.freeze({
      execute: function (sql, params) { return storageCall(raw, "execute", [sql, params]); },
      query: function (sql, params) { return storageCall(raw, "query", [sql, params]); },
      prepare: function (sql) { return storageCall(raw, "prepare", [sql], statement); },
      transaction: function (commands) { return storageCall(raw, "transaction", [commands]); },
      close: function () { return storageCall(raw, "close", []); },
    });
  }
  var files = { directories: Object.freeze({ data:"app:/data", cache:"app:/cache", temporary:"app:/tmp" }) };
  ["readFile", "writeFile", "atomicWriteFile", "appendFile", "readdir", "mkdir", "rm", "stat", "rename", "copyFile", "realpath"].forEach(function (method) {
    files[method] = function () { return storageCall(nativeStorage && nativeStorage.fs, method, arguments); };
  });
  var storage = Object.freeze({ fs:Object.freeze(files), sqlite:Object.freeze({
    open:function (path) { return storageCall(nativeStorage && nativeStorage.sqlite, "open", [path], database); },
  }) });

  // --- the seam ------------------------------------------------------------
  var calls = new Map();     // id -> { id, status, value, error, tickets }
  var nextCall = 1;
  // Large native strings travel alongside the JSON skeleton. The built-in
  // serializer still owns getters, toJSON, omissions, and cycle detection.
  // Explicit paths avoid reserving any property spelling in application data.
  var captureString = global.__exact_capture_string;
  delete global.__exact_capture_string;
  function ok(value) {
    var reply = { tag: 0, value: value === undefined ? null : value };
    if (typeof captureString !== "function") return JSON.stringify(reply);
    var paths = new WeakMap(), root = true;
    return JSON.stringify(reply, function (key, item) {
      var first = root;
      root = false;
      if (typeof item === "string" && item.length >= 65536) {
        var parent = first ? null : paths.get(this), depth = first ? 0 : 1;
        for (var link = parent; link; link = link.parent) depth++;
        var path = [];
        function pathKey(key) {
          Object.defineProperty(path, --depth, {value:key, enumerable:true, writable:true, configurable:true});
        }
        if (!first) pathKey(key);
        for (var link = parent; link; link = link.parent) pathKey(link.key);
        // Application toJSON hooks apply to its values, never our path metadata.
        Object.defineProperty(path, "toJSON", {value:undefined});
        captureString(JSON.stringify(path), item);
        return "";
      }
      // Retain one link per visited object; only a captured string needs a path.
      if (item !== null && typeof item === "object") paths.set(item, first ? null : {parent:paths.get(this), key:key});
      return item;
    });
  }
  function fail(e) {
    var kind = e && typeof e === "object" ? e.kind : undefined;
    if (kind !== "UnknownSource" && kind !== "BadArguments" && kind !== "Unavailable") kind = "Unavailable";
    var message = e && typeof e === "object" && e.message !== undefined ? e.message : e;
    return JSON.stringify({ tag: 2, kind: kind, message: String(message) });
  }
  function settle(call) {
    if (call.status === "done") { calls.delete(call.id); return ok(call.value); }
    if (call.status === "failed") { calls.delete(call.id); return fail(call.error); }
    for (var i = 0; i < call.tickets.length; i++) if (pending.has(call.tickets[i])) return JSON.stringify({ tag: 1, call: call.id, ticket: call.tickets[i] });
    if (call.storage > 0) return JSON.stringify({ tag:1, call:call.id, ticket:0 });
    calls.delete(call.id);
    return fail(new Error("the answer is pending on nothing: no host operation will resolve it"));
  }
  // The executor: `__exact_call(source, argsJson)` → tag 0/2 at once, or
  // tag 3 with a call id — then it drains microtasks and asks
  // `__exact_settle(id)`, which is tag 0/2, or tag 1 with the ticket of the
  // fetch the answer is waiting on. `__exact_fulfill(ticket, outcomeJson)`
  // resolves that fetch; drain and settle again.
  global.__exact_call = function (source, argsJson) {
    var call = { id: nextCall++, status: "pending", value: undefined, error: undefined, tickets: [], storage: 0 };
    var result;
    currentCall = call;
    try {
      var native = host(6, "available", "") === "native" ? Object.freeze({
        call: function (request) {
          if (!currentCall || currentCall.status !== "pending") throw new Error("native call outside an answer");
          return JSON.parse(host(6, "call", JSON.stringify(request)));
        },
      }) : null;
      result = global.exact.answer(source, JSON.parse(argsJson), store, storage, native);
    }
    catch (e) { currentCall = null; return fail(e); }
    if (result && typeof result.then === "function") {
      calls.set(call.id, call);
      result.then(function (v) { call.status = "done"; call.value = v; }, function (e) { call.status = "failed"; call.error = e; });
      return JSON.stringify({ tag: 3, call: call.id });
    }
    currentCall = null;
    return ok(result);
  };
  global.__exact_settle = function (id) {
    currentCall = null;
    var call = calls.get(Number(id));
    return call ? settle(call) : fail(new Error("no such call"));
  };
  global.__exact_storage_failed = function (id, outcomeJson) {
    var call = calls.get(Number(id));
    if (call) { call.status = "failed"; call.error = storageError(JSON.parse(outcomeJson).failed.message); }
  };
  global.__exact_fulfill = function (ticket, outcomeJson) {
    var p = pending.get(Number(ticket));
    if (!p) return;
    pending.delete(Number(ticket));
    var o = JSON.parse(outcomeJson);
    // The continuation runs in the drain that follows, and a fetch it
    // makes belongs to this call.
    currentCall = p.call;
    if (o.failed) p.reject(new FetchError(o.failed));
    else p.resolve(new Response(o.response));
  };
})(globalThis);
