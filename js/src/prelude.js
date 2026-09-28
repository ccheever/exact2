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
  // 2 store.get(name), 3 store.set(name, value), 4 store.forget(name), 5 storage capability check,
  // 6 native, 7 pure, 8 the answer drew secure randomness, 9/10 canvas text and images,
  // 11 the agent's repeatable random bytes (hex; undefined outside the agent).
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
  // Not a second door to randomness: `crypto` below is the secure one.
  fixed(global.Math, "random", function () {
    throw new Error("Math.random() is unavailable in data sources; pass time or a random seed as an argument, or use crypto.getRandomValues");
  });

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

  // --- crypto (LLP 1069.005): digests are pure; entropy is a device read ---
  // Each draw inside an answer counts as an external read (op 8), as a secret
  // or SQLite read does: bake compiles no value that drew one, and the host
  // asks again on the device. A draw outside an answer (during module
  // initialization) refuses: it would be one hidden input every answer shares
  // (D2, D3). `subtle` has what is built, the same on every executor: over
  // ibex2's `crypto` and the bytes door natively, over the realm's own on the
  // web. Every other `subtle` member refuses by name (D1).
  var platformCrypto = global.crypto;
  var fillRandom = platformCrypto.getRandomValues.bind(platformCrypto);
  var platformSubtle = platformCrypto.subtle; // the web's, in a secure context
  // Natively: `(op, name, arrayBuffer) -> string`, the byte work in Rust.
  var bytesDoor = global.__exact_bytes;
  delete global.__exact_bytes;
  // A LAN dev page (no secure context): the dev protocol's SHA-256, which
  // module integrity already uses there. `(Uint8Array) -> Promise<hex>`.
  var pageDigest = global.__exact_digest;
  delete global.__exact_digest;
  function formatUuid(b) {
    var text = "";
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    for (var i = 0; i < 16; i++) text += (i === 4 || i === 6 || i === 8 || i === 10 ? "-" : "") + (b[i] < 16 ? "0" : "") + b[i].toString(16);
    return text;
  }
  // A LAN dev page is not a secure context, so it has no `randomUUID`: the
  // same v4 bits as ibex2's `format_uuid`, from `getRandomValues`.
  var platformUuid = typeof platformCrypto.randomUUID === "function"
    ? platformCrypto.randomUUID.bind(platformCrypto)
    : function () { return formatUuid(fillRandom(new Uint8Array(16))); };
  var initializing = true; // until the executor first calls in
  function Crypto() { throw new TypeError("Illegal constructor"); }
  var cryptoObject = Object.create(Crypto.prototype);
  function draw(receiver, api, take) {
    if (receiver !== cryptoObject) throw new TypeError("Illegal invocation");
    if (!currentCall || currentCall.status !== "pending") {
      throw new Error(api + " is unavailable " + (initializing ? "during module initialization" : "outside an answer") + "; call it inside an answer");
    }
    var value = take();
    host(8, "", "");
    return value;
  }
  // Under the agent, the host's repeatable stream (D2b): `n` bytes, or
  // null outside the agent, where the draw is the platform's.
  function agentBytes(n) {
    var hex = host(11, "", String(n));
    return hex === undefined ? null : new Uint8Array(hexBuffer(hex));
  }
  Crypto.prototype.getRandomValues = function getRandomValues(view) {
    return draw(this, "crypto.getRandomValues()", function () {
      // The platform's call checks the view and its quota, as it always does.
      var filled = fillRandom(view), stream = agentBytes(filled.byteLength);
      if (stream) new Uint8Array(filled.buffer, filled.byteOffset, filled.byteLength).set(stream);
      return filled;
    });
  };
  Crypto.prototype.randomUUID = function randomUUID() {
    return draw(this, "crypto.randomUUID()", function () {
      var stream = agentBytes(16);
      return stream ? formatUuid(stream) : platformUuid();
    });
  };

  // Host work an answer waits on that is neither a fetch nor storage (the
  // browser's digest settles in a later task): on the web it joins the
  // answer's storage turn, so the answer is pending on it, not on nothing.
  // Natively the work is already done and the promise settles in the drain.
  function hostWork(promise) {
    var call = currentCall;
    var work = nativeStorage && nativeStorage.work;
    if (!call || call.status !== "pending" || typeof work !== "function") return promise;
    call.storage++;
    return work(promise).then(function (value) {
      currentCall = call;
      call.storage--;
      return value;
    }, function (error) {
      currentCall = call;
      call.storage--;
      throw error;
    });
  }
  function hexBuffer(hex) {
    var bytes = new Uint8Array(hex.length / 2);
    for (var i = 0; i < bytes.length; i++) bytes[i] = parseInt(hex.substr(i * 2, 2), 16);
    return bytes.buffer;
  }
  // A BufferSource's bytes, copied now: later writes to it change nothing.
  function copyBytes(data, api) {
    if (data instanceof ArrayBuffer) return data.slice(0);
    if (ArrayBuffer.isView(data)) return data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength);
    throw new TypeError(api + ": the data is not an ArrayBuffer or ArrayBufferView");
  }
  function algorithmName(algorithm) {
    var name = typeof algorithm === "string" ? algorithm : algorithm && algorithm.name;
    if (typeof name !== "string") throw new TypeError("an algorithm is a name or an object with a name");
    return name;
  }
  var DIGESTS = { "SHA-256": 1, "SHA-384": 1, "SHA-512": 1 };
  function SubtleCrypto() { throw new TypeError("Illegal constructor"); }
  var subtleObject = Object.create(SubtleCrypto.prototype);
  function subtleCall(receiver, run) {
    try {
      if (receiver !== subtleObject) throw new TypeError("Illegal invocation");
      return Promise.resolve(run());
    } catch (e) {
      return Promise.reject(e);
    }
  }
  // Every member the web has; each refuses by name until it is built here.
  ["encrypt", "decrypt", "sign", "verify", "digest", "generateKey", "deriveKey", "deriveBits",
    "importKey", "exportKey", "wrapKey", "unwrapKey"].forEach(function (member) {
    SubtleCrypto.prototype[member] = function () {
      return subtleCall(this, function () {
        throw new DOMException("crypto.subtle." + member + "() is unavailable in data sources", "NotSupportedError");
      });
    };
  });
  SubtleCrypto.prototype.digest = function digest(algorithm, data) {
    return subtleCall(this, function () {
      var given = algorithmName(algorithm), name = given.toUpperCase();
      if (!DIGESTS[name]) throw new DOMException("crypto.subtle.digest(): " + given + " is unavailable in data sources; use SHA-256, SHA-384 or SHA-512", "NotSupportedError");
      var bytes = copyBytes(data, "crypto.subtle.digest()");
      if (bytesDoor) return hexBuffer(bytesDoor(1, name, bytes));
      if (platformSubtle) return hostWork(platformSubtle.digest(name, bytes));
      if (name === "SHA-256" && pageDigest) return hostWork(pageDigest(new Uint8Array(bytes))).then(hexBuffer);
      throw new DOMException("crypto.subtle.digest(): " + name + " needs a secure context (HTTPS or localhost)", "NotSupportedError");
    });
  };
  Object.defineProperty(SubtleCrypto.prototype, Symbol.toStringTag, { value: "SubtleCrypto", configurable: true });
  Object.freeze(subtleObject);
  Object.defineProperty(Crypto.prototype, "subtle", {
    get: function () {
      if (this !== cryptoObject) throw new TypeError("Illegal invocation");
      return subtleObject;
    },
    enumerable: true,
    configurable: true,
  });
  Object.defineProperty(Crypto.prototype, Symbol.toStringTag, { value: "Crypto", configurable: true });
  Object.freeze(cryptoObject);
  delete global.CryptoKey; // until its keys are built (D1b)
  Object.defineProperty(global, "SubtleCrypto", { value: SubtleCrypto, writable: true, configurable: true });
  Object.defineProperty(global, "Crypto", { value: Crypto, writable: true, configurable: true });
  fixed(global, "crypto", cryptoObject);

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
    // An answer that keeps coming (LLP 1016.000): `exactStream` maps each
    // server-sent event to the answer, and the end too. The promise never
    // settles: the answer is what the mapper returns, message by message.
    var stream = init ? init.exactStream : undefined;
    if (stream !== undefined && typeof stream !== "function")
      return Promise.reject(new TypeError("exactStream maps each event to the answer: (event) => value"));
    if (stream && call.stream) return Promise.reject(new Error("an answer streams one request"));
    if (stream && ceiling === undefined) ceiling = 1048576;
    var ticket = nextTicket++;
    var error = host(1, String(ticket), JSON.stringify({ method: method, url: String(url), headers: headers, body: body, max_response_bytes: ceiling, stream: stream ? true : undefined }));
    if (error !== undefined) return Promise.reject(new Error(error));
    call.tickets.push(ticket);
    if (stream) call.stream = stream;
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
    var head = null, root = true;
    return JSON.stringify(reply, function (key, item) {
      var first = root;
      root = false;
      var large = typeof item === "string" && item.length >= 65536;
      var object = item !== null && typeof item === "object";
      var parent = null;
      if (!first && (large || object)) {
        parent = head;
        while (parent && parent.value !== this) parent = parent.parent;
      }
      if (large) {
        head = parent;
        // The root link identifies its holder but has no result-path key.
        var depth = first ? 0 : 1;
        for (var link = parent; link && link.parent; link = link.parent) depth++;
        var path = [];
        function pathKey(key) {
          Object.defineProperty(path, --depth, {value:key, enumerable:true, writable:true, configurable:true});
        }
        if (!first) pathKey(key);
        for (var link = parent; link && link.parent; link = link.parent) pathKey(link.key);
        // Application toJSON hooks apply to its values, never our path metadata.
        Object.defineProperty(path, "toJSON", {value:undefined});
        captureString(JSON.stringify(path), item);
        return "";
      }
      // Only ancestors of the current object are needed for later capture paths.
      if (object) head = {value:item, parent:parent, key:key};
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
    return fail(new Error("the answer is pending on nothing: no host operation it started will resolve it. " +
      "An answer that awaits a promise another answer started (a fetch shared between answers, or a queue chained through a " +
      "fetch another answer is waiting on) waits on work it does not own; make each answer's own fetch, or share the resolved " +
      "value rather than the promise. Storage is different: an answer queued behind another's storage turn waits for it"));
  }
  // The executor: `__exact_call(source, argsJson)` → tag 0/2 at once, or
  // tag 3 with a call id — then it drains microtasks and asks
  // `__exact_settle(id)`, which is tag 0/2, or tag 1 with the ticket of the
  // fetch the answer is waiting on. `__exact_fulfill(ticket, outcomeJson)`
  // resolves that fetch; drain and settle again.
  global.__exact_call = function (source, argsJson) {
    initializing = false;
    var call = { id: nextCall++, status: "pending", value: undefined, error: undefined, tickets: [], storage: 0 };
    var result;
    currentCall = call;
    try {
      var nativeCall = function (request) {
        if (!currentCall || currentCall.status !== "pending") throw new Error("native call outside an answer");
        return JSON.parse(host(6, "call", JSON.stringify(request)));
      };
      // Null only where no module can be (a page with no page module). Else an
      // object; whether a module is linked is the device's fact, not the
      // build's, so asking `available` marks the answer as the device's: the
      // bake leaves it uncompiled and the host asks it again.
      var native = host(6, "kind", "") !== "native" ? null : Object.freeze({
        get available() { return host(6, "available", "") === "native"; },
        call: nativeCall,
        // This answer depends on a device topic the module announces when it
        // changes (a level, a step, new words): the host asks it again then,
        // instead of the app polling (LLP 1016.002).
        watch: function (topic) {
          if (!currentCall || currentCall.status !== "pending") throw new Error("native.watch outside an answer");
          host(6, "watch", String(topic));
        },
        // Long work: off the source's thread and outside its budget, settled
        // when the module's own work replies. It travels as a request the
        // host hands to the module (on the web, the app's page module), so
        // the answer waits for it as for a fetch. A module that takes no
        // long calls answers through `call`, now.
        later: function (request) {
          if (host(6, "later", "") !== "later") {
            try { return Promise.resolve(nativeCall(request)); } catch (e) { return Promise.reject(e); }
          }
          return global.fetch("exact-native:", { method: "POST", body: JSON.stringify(request), exactIndependentHttp: { maxResponseBytes: 1048576 } })
            .then(function (r) { return r.text().then(function (t) { if (r.status === 200) return JSON.parse(t); throw new Error(t || "native call failed"); }); });
        },
      });
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
  // Canvas 2D (LLP 1056 D1): the module's draw seam, when it exports `draw`.
  // Text is measured and image handles are answered where the draw runs
  // (LLP 1056 D8, D9): the page's functions on the web, else the host door.
  global.__exact_draw = function (request, measure, image) {
    initializing = false;
    if (!global.exact.drawCanvas) throw new Error("the module exports no draw");
    return global.exact.drawCanvas(request, {
      measure: measure || function (json) { return host(9, json, ""); },
      image: image || function (json) { var r = host(10, json, ""); return r === undefined ? "" : r; },
    });
  };
  global.__exact_retire = function (retired) {
    if (global.exact.retireCanvases) global.exact.retireCanvases(retired);
    return "";
  };
  global.__exact_settle = function (id) {
    currentCall = null;
    var call = calls.get(Number(id));
    return call ? settle(call) : fail(new Error("no such call"));
  };
  // The runner let this call's request go (LLP 1016 D5): drop the call and
  // the fetches it waits on, so nothing keeps them alive.
  global.__exact_forget = function (id) {
    var call = calls.get(Number(id));
    if (!call) return "";
    calls.delete(call.id);
    for (var i = 0; i < call.tickets.length; i++) pending.delete(call.tickets[i]);
    return "";
  };
  // One message of the stream answer `id` began, or its end: the mapper's
  // value, now — a stream's answer never awaits (LLP 1016.000 D1). The end
  // is `{ type: "error" }` with the failure's kind and message, or the
  // status and body of a reply that was not an event stream.
  global.__exact_message = function (id, outcomeJson) {
    var call = calls.get(Number(id));
    if (!call || !call.stream) return fail(new Error("no stream answer " + id));
    var o = JSON.parse(outcomeJson), event;
    if (o.message) event = { type: o.message.event || "message", data: o.message.data, lastEventId: o.message.id, coalesced: o.message.coalesced };
    else if (o.failed) event = { type: "error", data: "", lastEventId: "", coalesced: 0, kind: o.failed.kind, message: o.failed.message, status: 0 };
    else event = { type: "error", data: o.response.body, lastEventId: "", coalesced: 0, kind: "Response", message: "HTTP " + o.response.status, status: o.response.status };
    currentCall = call;
    try {
      var value = call.stream(event);
      if (value && typeof value.then === "function") throw new Error("exactStream answers each event now; it cannot await");
      return ok(value);
    } catch (e) { return fail(e); }
    finally { currentCall = null; }
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
