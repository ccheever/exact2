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
  // 2 store.get(name), 3 store.set(name, value), 4 store.forget(name).
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

  // --- base64, for a response body's bytes (Hermes has no atob) -----------
  var B64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  var B64V = {};
  for (var i = 0; i < 64; i++) B64V[B64.charAt(i)] = i;
  function fromBase64(s) {
    var n = s.length;
    while (n > 0 && s.charAt(n - 1) === "=") n--;
    var out = new Uint8Array(Math.floor((n * 3) / 4));
    var o = 0, acc = 0, bits = 0;
    for (var k = 0; k < n; k++) {
      acc = (acc << 6) | B64V[s.charAt(k)];
      bits += 6;
      if (bits >= 8) { bits -= 8; out[o++] = (acc >> bits) & 255; }
    }
    return out;
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

  function FetchError(f) { this.name = "FetchError"; this.message = String(f.message); this.kind = String(f.kind); }
  FetchError.prototype = Object.create(Error.prototype);

  global.Headers = Headers;
  global.Response = Response;
  global.fetch = function (url, init) {
    var call = currentCall;
    if (!call) return Promise.reject(new Error("fetch called outside an answer"));
    var method = init && init.method ? String(init.method).toUpperCase() : "GET";
    var headers = new Headers(init && init.headers).entries();
    var body = init && init.body != null ? String(init.body) : "";
    var ticket = nextTicket++;
    host(1, String(ticket), JSON.stringify({ method: method, url: String(url), headers: headers, body: body }));
    call.tickets.push(ticket);
    return new Promise(function (resolve, reject) { pending.set(ticket, { resolve: resolve, reject: reject, call: call }); });
  };

  // --- the store (LLP 1018): reads counted, writes grant-checked, in Rust -
  var store = {
    get: function (name) { var v = host(2, String(name), ""); return v === undefined ? null : v; },
    set: function (name, value) { var e = host(3, String(name), String(value)); if (e !== undefined) throw new Error(e); },
    forget: function (name) { var e = host(4, String(name), ""); if (e !== undefined) throw new Error(e); },
  };

  // --- the seam ------------------------------------------------------------
  var calls = new Map();     // id -> { id, status, value, error, tickets }
  var nextCall = 1;
  function ok(value) { return JSON.stringify({ tag: 0, value: value === undefined ? null : value }); }
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
    calls.delete(call.id);
    return fail(new Error("the answer is pending on nothing: an awaited promise no fetch will resolve"));
  }
  // The executor: `__exact_call(source, argsJson)` → tag 0/2 at once, or
  // tag 3 with a call id — then it drains microtasks and asks
  // `__exact_settle(id)`, which is tag 0/2, or tag 1 with the ticket of the
  // fetch the answer is waiting on. `__exact_fulfill(ticket, outcomeJson)`
  // resolves that fetch; drain and settle again.
  global.__exact_call = function (source, argsJson) {
    var call = { id: nextCall++, status: "pending", value: undefined, error: undefined, tickets: [] };
    var result;
    currentCall = call;
    try { result = global.exact.answer(source, JSON.parse(argsJson), store); }
    catch (e) { currentCall = null; return fail(e); }
    currentCall = null;
    if (result && typeof result.then === "function") {
      calls.set(call.id, call);
      result.then(function (v) { call.status = "done"; call.value = v; }, function (e) { call.status = "failed"; call.error = e; });
      return JSON.stringify({ tag: 3, call: call.id });
    }
    return ok(result);
  };
  global.__exact_settle = function (id) {
    currentCall = null;
    var call = calls.get(Number(id));
    return call ? settle(call) : fail(new Error("no such call"));
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
