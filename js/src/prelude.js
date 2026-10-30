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
  // (op, a, b) -> string | undefined, js/src/door.rs's ops: 1 a request, 2-4 and 18-20 the store, 5 storage,
  // 6 native, 7 pure, 8 secure randomness drawn, 9/10 canvas, 11 the agent's random bytes, 12 auth, 13 a journal line,
  // 16 a request withdrawn unsent, 21 the runner's clock as the call began, 22 module work's state (binary.js), 23 a handed-out
  // request the app aborted.
  var host = global.__exact_host;
  delete global.__exact_host;
  // The seam's bytes (binary.js): base64, UTF-8 lengths, a stream message's data.
  var binary = global.__exact_binary;
  delete global.__exact_binary;
  var fromBase64 = binary.fromBase64, toBase64 = binary.toBase64, utf8Length = binary.utf8Length, longPollOf = binary.longPollOf, fileRefusal = binary.fileRefusal, bodyWritten = binary.bodyWritten, uploadOf = binary.uploadOf;
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
  // The engine's own Date, through its prototype: a browser realm under a
  // page's fake clock (Playwright's `clock.install`) has a fake `Date` that
  // keeps the real prototype.
  var NativeDate = global.Date.prototype.constructor, construct = Reflect.construct;
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
    throw new Error("Math.random() is unavailable in data sources; seed from crypto.getRandomValues in a source (docs/recipes/random-and-shuffle.md), or pass a seed as an argument; a roll in the view needs no source (docs/recipes/random-rolls.md)");
  });
  // The runtime's own journal line (LLP 1097 D8): a failed storage
  // operation, an unhandled rejection. Natively it is the runner's journal
  // (`logs`) as it is; a realm without that door writes it to its console.
  function journal(line) {
    var written;
    try { written = host(13, String(line), ""); } catch (e) { written = undefined; }
    if (written !== "1" && global.console) global.console.error(String(line));
  }
  function errorText(e) {
    try { return e && typeof e === "object" && e.message !== undefined ? String(e.message) : String(e); }
    catch (_) { return "(an error that cannot be read)"; }
  }
  // An answer's own timers, on the runner's clock (op 21), and the timer
  // Hermes's rejection tracker asks for (binary.js `timers`). Every
  // unhandled rejection in the module reaches the journal, as a page's
  // `unhandledrejection` reaches its console (LLP 1097 D8).
  // The tracker's report is the timer it arms from inside its rejection
  // hook (Hermes's `Promise._C`), told by that place, not by its name alone.
  var reporting = 0, hooked = false;
  var timers = binary.timers(timerOwner, function () { return host(21, "", ""); }, journal, function (callback) {
    return (reporting > 0 || !hooked) && callback.name === "bound onUnhandled";
  }, function (signal) { try { return abortHooks ? abortHooks.own(signal).sources : null; } catch (e) { return null; } });
  var tracker = global.HermesInternal;
  if (tracker && typeof tracker.enablePromiseRejectionTracker === "function") {
    tracker.enablePromiseRejectionTracker({
      allRejections: true,
      onUnhandled: function (id, error) { journal("data: unhandled rejection: " + errorText(error)); leftOpen(); },
      onHandled: function () {},
    });
    var reject = global.Promise._C;
    if (typeof reject === "function") {
      hooked = true;
      global.Promise._C = function (promise, error) { reporting++; try { return reject.call(this, promise, error); } finally { reporting--; } };
    }
  }
  // A browser realm (the wasm host's module realm) reports its own rejections (module-glue.js).
  if (!tracker && typeof global.addEventListener === "function") global.addEventListener("unhandledrejection", function () { leftOpen(); });
  fixed(global, "performance", Object.freeze({ now: function () { return refuseAmbient("performance.now()"); } }));
  // Intl's formatting methods also default an omitted/undefined date to
  // machine time. Guard the prototype before an app can capture its bound
  // format getter or formatToParts method; explicit timestamps still use
  // the engine's locale/timezone implementation unchanged.
  if (global.Intl && global.Intl.DateTimeFormat) {
    // The engine's prototype, the one with the `format` accessor: a fake
    // clock's DateTimeFormat only inherits it, and answers an omitted date
    // from its clock, so the engine's constructor takes its place.
    var dateFormat = global.Intl.DateTimeFormat.prototype, engineFormat = dateFormat;
    while (engineFormat && !(Object.getOwnPropertyDescriptor(engineFormat, "format") || {}).get) engineFormat = Object.getPrototypeOf(engineFormat);
    if (engineFormat && engineFormat !== dateFormat) {
      dateFormat = engineFormat;
      global.Intl.DateTimeFormat = engineFormat.constructor;
    }
    var getFormat = Object.getOwnPropertyDescriptor(dateFormat, "format").get, formats = new WeakMap(), apply = Reflect.apply, engineOf = function (f) { return f; };
    // Natively intl.js follows, whose formatters share an engine formatter (`engineOf` finds it).
    if (tracker) Object.defineProperty(global, "__exact_dates", { configurable: true, value: { getFormat: getFormat, engineOf: function (f) { engineOf = f; } } });
    function explicitFormat(fn, name) {
      return new Proxy(fn, {
        apply: function (target, receiver, args) {
          if (args[0] === undefined) return refuseAmbient("Intl.DateTimeFormat." + name + "()");
          return apply(target, engineOf(receiver), args);
        },
      });
    }
    Object.defineProperty(dateFormat, "format", {
      configurable: false,
      get: function () { // a function for each object, as each has its own bound format
        var native = getFormat.call(engineOf(this));
        if (!formats.has(this)) formats.set(this, explicitFormat(native, "format"));
        return formats.get(this);
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
  var platformCrypto = global.crypto, fillRandom = platformCrypto.getRandomValues.bind(platformCrypto);
  var platformSubtle = platformCrypto.subtle; // the web's, in a secure context
  // Natively: `(op, name, arrayBuffer) -> string`, the byte work in Rust.
  var bytesDoor = global.__exact_bytes;
  delete global.__exact_bytes;
  // LLP 1133.003 §9's stub decode, natively and under the agent only (op
  // 11 answers there): the gate's fixture calls it until `VideoDecoder`.
  if (binary.gateDecode && host(11, "", "0") !== undefined) global.__exact_gate_decode = binary.gateDecode;
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
  function answerOnly(api) { if (!currentCall || currentCall.status !== "pending") throw new Error(api + " is unavailable " + (initializing ? "during module initialization" : "outside an answer") + "; call it inside an answer"); }
  var cryptoObject = Object.create(Crypto.prototype);
  function draw(receiver, api, take) {
    if (receiver !== cryptoObject) throw new TypeError("Illegal invocation");
    if (overlaying) throw inOverlay(api);
    answerOnly(api);
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
    storing.add(call);
    return work(promise).then(function (value) {
      currentCall = call;
      unstore(call);
      return value;
    }, function (error) {
      currentCall = call;
      unstore(call);
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
  function subtleCall(receiver, member, run) {
    try {
      if (receiver !== subtleObject) throw new TypeError("Illegal invocation");
      if (overlaying) throw inOverlay("crypto.subtle." + member + "()");
      return Promise.resolve(run());
    } catch (e) { return Promise.reject(e); }
  }
  // Every member the web has; each refuses by name until it is built here.
  ["encrypt", "decrypt", "sign", "verify", "digest", "generateKey", "deriveKey", "deriveBits",
    "importKey", "exportKey", "wrapKey", "unwrapKey"].forEach(function (member) {
    SubtleCrypto.prototype[member] = function () {
      return subtleCall(this, member, function () {
        throw new DOMException("crypto.subtle." + member + "() is unavailable in data sources", "NotSupportedError");
      });
    };
  });
  SubtleCrypto.prototype.digest = function digest(algorithm, data) {
    return subtleCall(this, "digest", function () {
      var given = algorithmName(algorithm), name = given.toUpperCase();
      if (!DIGESTS[name]) throw new DOMException("crypto.subtle.digest(): " + given + " is unavailable in data sources; use SHA-256, SHA-384 or SHA-512", "NotSupportedError");
      var bytes = copyBytes(data, "crypto.subtle.digest()");
      if (bytesDoor) return hexBuffer(bytesDoor(1, name, bytes));
      if (platformSubtle) return hostWork(platformSubtle.digest(name, bytes));
      if (name === "SHA-256" && pageDigest) return hostWork(pageDigest(new Uint8Array(bytes))).then(hexBuffer);
      throw new DOMException("crypto.subtle.digest(): " + name + " needs a secure context (HTTPS or localhost)", "NotSupportedError");
    });
  };
  // ECDSA P-256 for DPoP (D1b): generate, sign (ES256, raw r‖s), and JWK
  // import and export, with the web's names, argument shapes and errors.
  // Generating and signing draw a nonce: each is a counted read, and refuses
  // outside an answer. Natively the key's bytes stay in Rust behind a handle
  // (`exact_data::crypto`, bytes door ops 2–7); on the web a key is the
  // browser's own `CryptoKey` and the work its `subtle`, joined to the answer.
  var PlatformCryptoKey = global.CryptoKey;
  var keysDoor = global.__exact_keys; // the web realm's IndexedDB, for kept keys
  delete global.__exact_keys;
  var nativeKeys = new WeakMap(); // natively: CryptoKey -> its handle
  function CryptoKey() { throw new TypeError("Illegal constructor"); }
  function fromDoor(e) {
    var text = String(e && e.message !== undefined ? e.message : e), match = /^([A-Za-z]+Error): /.exec(text);
    return new DOMException(match ? text.slice(match[0].length) : text, match ? match[1] : "DataError");
  }
  function door(op, a, bytes) {
    try { return bytesDoor(op, a, bytes || new ArrayBuffer(0)); } catch (e) { throw fromDoor(e); }
  }
  function ecdsa(algorithm, api, needCurve) {
    var name = algorithmName(algorithm);
    if (name.toUpperCase() !== "ECDSA") throw new DOMException(api + ": " + name + " is unavailable in data sources; use ECDSA with P-256", "NotSupportedError");
    if (needCurve && algorithm.namedCurve !== "P-256") throw new DOMException(api + ": the curve " + algorithm.namedCurve + " is unavailable in data sources; use P-256", "NotSupportedError");
  }
  function usageList(usages, allowed, api) {
    if (!Array.isArray(usages)) throw new TypeError(api + ": usages is an array");
    var out = [];
    for (var i = 0; i < usages.length; i++) {
      if (allowed.indexOf(usages[i]) < 0) throw new DOMException(api + ": the usage " + usages[i] + " is not allowed for this key", "SyntaxError");
      if (out.indexOf(usages[i]) < 0) out.push(usages[i]);
    }
    return out;
  }
  function only(usages, which) { return usages.filter(function (u) { return u === which; }); }
  function entropyRead(api) {
    answerOnly(api);
    // Natively Rust marks the read where it draws (`exact_data::crypto`).
    if (!bytesDoor) host(8, "", "");
  }
  function makeKey(handle, type, extractable, usages) {
    var key = Object.create(CryptoKey.prototype);
    Object.defineProperties(key, {
      type: { value: type, enumerable: true },
      extractable: { value: !!extractable, enumerable: true },
      algorithm: { value: Object.freeze({ name: "ECDSA", namedCurve: "P-256" }), enumerable: true },
      usages: { value: Object.freeze(usages.slice()), enumerable: true },
    });
    nativeKeys.set(key, handle);
    return Object.freeze(key);
  }
  function nativePair(json, extractable, usages) {
    var r = JSON.parse(json);
    return {
      privateKey: makeKey(r["private"], "private", extractable, only(usages, "sign")),
      publicKey: makeKey(r["public"], "public", true, only(usages, "verify")),
    };
  }
  function isKey(key) { return bytesDoor ? nativeKeys.has(key) : !!PlatformCryptoKey && key instanceof PlatformCryptoKey; }
  function checkKey(key, api) {
    if (!isKey(key)) throw new TypeError(api + ": the key is not a CryptoKey");
    if (key.algorithm.name !== "ECDSA" || key.algorithm.namedCurve !== "P-256") throw new DOMException(api + ": the key is not ECDSA P-256", "InvalidAccessError");
  }
  function webSubtle(api) {
    if (!platformSubtle) throw new DOMException(api + ": ECDSA needs a secure context (HTTPS or localhost)", "NotSupportedError");
    return platformSubtle;
  }
  SubtleCrypto.prototype.generateKey = function generateKey(algorithm, extractable, usages) {
    return subtleCall(this, "generateKey", function () {
      var api = "crypto.subtle.generateKey()";
      ecdsa(algorithm, api, true);
      var list = usageList(usages, ["sign", "verify"], api);
      if (!only(list, "sign").length) throw new DOMException(api + ": a private key needs the sign usage", "SyntaxError");
      if (!bytesDoor) webSubtle(api);
      entropyRead(api);
      if (bytesDoor) return nativePair(door(2, extractable ? "1" : "0"), extractable, list);
      return hostWork(platformSubtle.generateKey({ name: "ECDSA", namedCurve: "P-256" }, !!extractable, list));
    });
  };
  SubtleCrypto.prototype.sign = function sign(algorithm, key, data) {
    return subtleCall(this, "sign", function () {
      var api = "crypto.subtle.sign()";
      ecdsa(algorithm, api, false);
      var hash = algorithm && algorithm.hash;
      if (algorithmName(hash === undefined ? "" : hash).toUpperCase() !== "SHA-256") throw new DOMException(api + ": ECDSA signs with SHA-256 (ES256) in data sources", "NotSupportedError");
      checkKey(key, api);
      if (key.type !== "private" || key.usages.indexOf("sign") < 0) throw new DOMException(api + ": the key is not for signing", "InvalidAccessError");
      var bytes = copyBytes(data, api);
      if (!bytesDoor) webSubtle(api);
      entropyRead(api);
      if (bytesDoor) return hexBuffer(door(3, String(nativeKeys.get(key)), bytes));
      return hostWork(platformSubtle.sign({ name: "ECDSA", hash: "SHA-256" }, key, bytes));
    });
  };
  SubtleCrypto.prototype.importKey = function importKey(format, keyData, algorithm, extractable, usages) {
    return subtleCall(this, "importKey", function () {
      var api = "crypto.subtle.importKey()";
      if (format !== "jwk") throw new DOMException(api + ": the format " + format + " is unavailable in data sources; use jwk", "NotSupportedError");
      ecdsa(algorithm, api, true);
      if (!keyData || typeof keyData !== "object") throw new TypeError(api + ": a JWK is an object");
      if (keyData.kty !== "EC" || keyData.crv !== "P-256") throw new DOMException(api + ": the JWK is not EC P-256", "DataError");
      var isPrivate = typeof keyData.d === "string";
      var list = usageList(usages, isPrivate ? ["sign"] : ["verify"], api);
      if (isPrivate && !list.length) throw new DOMException(api + ": a private key needs the sign usage", "SyntaxError");
      if (!bytesDoor) return hostWork(webSubtle(api).importKey("jwk", keyData, { name: "ECDSA", namedCurve: "P-256" }, !!extractable, list));
      var jwk = { kty: "EC", crv: "P-256", x: String(keyData.x), y: String(keyData.y) };
      if (isPrivate) jwk.d = keyData.d;
      var handle = Number(door(4, JSON.stringify({ jwk: jwk, extractable: !!extractable })));
      return makeKey(handle, isPrivate ? "private" : "public", isPrivate ? extractable : true, list);
    });
  };
  SubtleCrypto.prototype.exportKey = function exportKey(format, key) {
    return subtleCall(this, "exportKey", function () {
      var api = "crypto.subtle.exportKey()";
      if (format !== "jwk") throw new DOMException(api + ": the format " + format + " is unavailable in data sources; use jwk", "NotSupportedError");
      checkKey(key, api);
      if (!key.extractable) throw new DOMException(api + ": the key is not extractable", "InvalidAccessError");
      if (!bytesDoor) return hostWork(webSubtle(api).exportKey("jwk", key));
      var jwk = JSON.parse(door(5, String(nativeKeys.get(key))));
      jwk.ext = true;
      jwk.key_ops = key.usages.slice();
      return jwk;
    });
  };
  Object.defineProperty(CryptoKey.prototype, Symbol.toStringTag, { value: "CryptoKey", configurable: true });
  // "Keep this key" (D1b), one spelling on every host: `store.keepKey(name,
  // pair)` under the `secret.keep <name>` grant, `store.key(name)` →
  // `CryptoKeyPair | null` (a counted read), `store.forget(name)` drops it.
  // Natively Rust writes the pair's JWK itself, so the private scalar never
  // enters this heap; on the web the pair goes to the realm's IndexedDB and
  // the secret holds only its handle.
  function keepKey(name, pair) {
    var api = "store.keepKey()";
    if (overlaying) return Promise.reject(inOverlay(api));
    if (!currentCall || currentCall.status !== "pending") return Promise.reject(new Error(api + " called outside an answer"));
    try {
      if (!pair || !isKey(pair.privateKey) || !isKey(pair.publicKey) || pair.privateKey.type !== "private") throw new TypeError(api + ": a CryptoKeyPair from generateKey or store.key");
      if (bytesDoor) {
        door(6, JSON.stringify({ name: String(name), "private": nativeKeys.get(pair.privateKey), "public": nativeKeys.get(pair.publicKey) }));
        return Promise.resolve();
      }
      if (!keysDoor) throw new Error(api + ": this host keeps no keys");
      var handle = "exact.key:" + platformUuid();
      store.set(name, handle);
      return hostWork(keysDoor.put(handle, { privateKey: pair.privateKey, publicKey: pair.publicKey }));
    } catch (e) { return Promise.reject(e); }
  }
  function keptKey(name) {
    var api = "store.key()";
    if (overlaying) return Promise.reject(inOverlay(api));
    if (!currentCall || currentCall.status !== "pending") return Promise.reject(new Error(api + " called outside an answer"));
    try {
      if (bytesDoor) {
        var json = door(7, String(name));
        return Promise.resolve(json ? nativePair(json, false, ["sign", "verify"]) : null);
      }
      var handle = store.get(name);
      if (handle === null) return Promise.resolve(null);
      if (!keysDoor) throw new Error(api + ": this host keeps no keys");
      return hostWork(keysDoor.get(handle));
    } catch (e) { return Promise.reject(e); }
  }
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
  // The web's own `CryptoKey` in a browser realm; natively, this one.
  if (bytesDoor || !PlatformCryptoKey) Object.defineProperty(global, "CryptoKey", { value: CryptoKey, writable: true, configurable: true });
  Object.defineProperty(global, "SubtleCrypto", { value: SubtleCrypto, writable: true, configurable: true });
  Object.defineProperty(global, "Crypto", { value: Crypto, writable: true, configurable: true });
  fixed(global, "crypto", cryptoObject);
  // --- fetch: a request the host runs; a Promise for its reply -------------
  var nextTicket = 1;
  // ticket -> { resolve, reject, call, claimed, stream, signal, release }:
  // `claimed` once a settle has handed the executor its request, `release`
  // ends its watch on `signal`.
  var pending = new Map(), currentCall = null, mapping = null, inMapper = false; // the answer a fetch belongs to; a socket's mapper answering a frame (D7); any mapper call
  var work = global.__exact_module_work({ host: host, live: function (c) { return !!c && c !== background && !c.stream && !c.module && c.status === "pending" && !c.replied && !c.letGo; } }); delete global.__exact_module_work; // work that outlives its answer (LLP 1146 D2, binary.js): an adopted fetch's reply reaches its let-go answer in a module turn; `route` keeps a live answer's async step in its own turn
  global.__exact_async_context = Object.freeze({ current: function () { return currentCall; }, swap: function (call) { var before = currentCall; currentCall = call; return before; }, route: work.route }); // the bundle's async helper resumes each step under its answer (LLP 1133.002 D14)
  // Calls with storage steps in flight, settled or not: a step a settled
  // call left behind is still the module's work (see `settle`).
  var storing = new Set();
  function unstore(call) {
    call.storage--;
    if (!call.storage) storing.delete(call);
  }
  // --- background work (LLP 1097 D1–D3) ------------------------------------
  // An answer replies when its value is ready, as in a browser: storage it
  // started and did not await moves to `background`, the module's own owner,
  // never replied to, let go or claimed. The executor runs it under a ticket
  // of its own and delivers it with `currentCall = background`. Only on the
  // main thread (D6): a worker placement's answer still waits for its
  // storage, so the flag is set by the executor that runs inline alone.
  var moving = false;
  var background = { id: 0, status: "pending", value: undefined, error: undefined, tickets: [], storage: 0, replied: false, background: true };
  // The module's storage operations that landed and failed, and the last
  // failure's line (`state.background`, LLP 1097 D8).
  var storageCounts = { writes: 0, done: 0, failed: 0, last: null };
  // Set before the first answer, never after: an app cannot turn it on.
  global.__exact_main_thread = function () { if (initializing) moving = true; };
  // Where a completion's count lives: the call that issued it, or the
  // background once that call's storage moved there.
  function owner(call) { return call.moved ? background : call; }
  // One queue for every storage operation of the module (D3), in the order
  // issued; one in flight at a time. So only the issuer of the operation in
  // flight waits on the store's shared completions: one completion wakes one
  // waiter, and the next answer's read sees the write issued before it.
  var MAX_QUEUED = 256, queue = [], head = null;
  function issue(op) {
    head = op;
    var promise;
    try { promise = op.run(); } catch (e) { promise = Promise.reject(e); }
    promise.then(function (value) { landed(op, true, value); }, function (error) { landed(op, false, error); });
  }
  function landed(op, ok, value) {
    // A late completion of an operation already failed (a timed-out
    // continuation the store then finishes) must not clear the new head.
    if (op.settled) return;
    op.settled = true;
    if (head === op) {
      head = null;
      // The next operation was issued before anything this one's reaction
      // issues: it starts first.
      if (queue.length) issue(queue.shift());
    }
    op.settle(ok, value);
  }
  // The operation in flight's owner, or null.
  function headOwner() { return head ? owner(head.call) : null; }
  // A fetch's `signal` is watched through Ibex's own abort hooks natively, which no listener can stop; a browser
  // realm's is the browser's, watched by a listener and checked again when the reply lands. Either ends with the fetch.
  var abortHooks = global.__exact_ibex2_abort_hooks;
  delete global.__exact_ibex2_abort_hooks;
  function watchAbort(signal, aborted) {
    if (abortHooks) return abortHooks.subscribe(signal, aborted);
    signal.addEventListener("abort", aborted);
    return function () { signal.removeEventListener("abort", aborted); };
  }
  function settled(ticket) {
    var p = pending.get(ticket);
    if (p) { pending.delete(ticket); p.release(); }
    return p;
  }
  // --- scheduler.yield (WICG Prioritized Task Scheduling; LLP 1141.001 D10): `settle` parks the
  // answer as `yielded`, the host asks it again at once (`Dispatch::Again`), and `__exact_yielded`
  // resolves its yields in that new turn. Counted: an answer awaiting another's yielding work waits.
  var yielded = 0;
  function yieldTurn() {
    if (overlaying) return Promise.reject(inOverlay("scheduler.yield()"));
    var call = currentCall;
    if (call && (call.letGo || (call.replied && call.status === "pending"))) return Promise.reject(new DOMException("the answer was let go", "AbortError"));
    // No answer's turn to end (module evaluation, a mapper, background work, a continuation after its reply): go on.
    if (!call || call === background || inMapper || call.status !== "pending" || call.replied) return Promise.resolve();
    return new Promise(function (resolve, reject) { (call.yields || (call.yields = [])).push([resolve, reject]); yielded++; });
  }
  // End a call's yields, resolved or rejected with `error`: how many there were.
  function endYields(call, error) {
    var ys = call.yields || []; call.yields = []; yielded -= ys.length;
    ys.forEach(function (y) { if (error) y[1](error); else y[0](); }); return ys.length;
  }
  fixed(global, "scheduler", Object.freeze({ yield: yieldTurn, postTask: function () { throw new Error("scheduler.postTask() is unavailable in data sources; split long work with await scheduler.yield()"); } }));
  // `to`: the fetch's `exactBodyTo`; `exactBodyTo` is `{path, bytes}` once the host wrote the body there.
  function Response(r, to) {
    var written = to === undefined ? undefined : bodyWritten(r);
    this.status = r.status;
    this.ok = r.status >= 200 && r.status < 300;
    this.headers = new global.Headers(r.headers);
    this.exactBodyTo = written === undefined ? null : { path: to, bytes: written };
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

  // A mutation does one kind of effect, socket operations or a fetch: whichever
  // comes second refuses, and the mutation with it, before either leaves.
  var SOCKET_AND_FETCH = "a mutation cannot both send on a socket and fetch";
  // A resource's two-way socket (LLP 1016.000 D7) and a listener's connections (LLP 1133.005 D6): their handles (binary.js `sockets`).
  var sockets = binary.sockets({ host: host, copyBytes: copyBytes, current: function () { return currentCall; }, mapping: function () { return mapping; }, inMapper: function () { return inMapper; }, conflict: SOCKET_AND_FETCH });
  var socketOwner = sockets.socketOwner, listenOwner = sockets.listenOwner, handSocketOps = sockets.hand;
  // An aborted fetch's request, as a browser cancels one: a request its answer
  // awaits (handed out, `claimed`) is cancelled by the host, which resumes the
  // answer at once (door 23); one not yet handed out is withdrawn unsent (16).
  // A stream runs until the runner lets its ticket go, as before. A host that
  // knows neither door leaves the request running, the old behaviour.
  function abortRequest(ticket, p) {
    if (p.stream) return;
    try {
      if (p.claimed) host(23, String(ticket), "");
      else {
        var at = p.call.tickets.indexOf(ticket);
        if (at >= 0) p.call.tickets.splice(at, 1);
        host(16, String(ticket), "");
      }
    } catch (e) { /* an older host's door: the request runs, its reply dropped */ }
  }
  // An adopted fetch's request, cancelled by the host (door 23), which then
  // answers it aborted in a module turn: whether the host took it.
  function cancelAdopted(ticket) {
    try { host(23, String(ticket), ""); return true; } catch (e) { return false; }
  }
  // A refused mutation's fetches, withdrawn before any was handed out.
  function withdrawFetches(call) {
    for (var i = 0; i < call.tickets.length; i++) { settled(call.tickets[i]); host(16, String(call.tickets[i]), ""); }
    call.tickets = [];
    call.socketOps = null;
  }
  global.Response = Response;
  global.fetch = function (url, init) { return request(url, init); };
  // `listen`: a listener's options (`exactListen`, LLP 1133.005 D1); `own(ticket,
  // call)`: a child's stream owner (js/src/process.js).
  function request(url, init, listen, own) {
    if (overlaying || mapping) return Promise.reject(overlaying ? inOverlay("fetch()") : new Error("fetch() is unavailable in a socket's mapper: it answers each frame now"));
    var call = currentCall;
    // Refused with no answer running, and below in a let-go answer's continuation, as no answer waits for the reply
    // (where the web lets it run; a request with effects already sent runs to its end, LLP 1146 D2, §2.4).
    if (!call) {
      var refused = new Error(work.inModuleTurn() ? "fetch() called from a plain .then callback a let-go answer's request settled: it was never run. Await that result in an async function, which resumes in its own answer's turn" : "fetch() called outside an answer: it was never run. Start a fetch inside an answer");
      if (global.console) global.console.error(refused.message);
      return Promise.reject(refused);
    }
    // No fetch from background work (LLP 1097 D2; a background fetch is
    // deferred, §7): no answer waits for its reply, and no ticket is owed.
    if (call === background) {
      var orphan = new Error("fetch() called from background work: it was never run. Fetch inside an answer");
      journal("data: " + orphan.message);
      return Promise.reject(orphan);
    }
    if (call.letGo) return Promise.reject(new FetchError({ kind: "Aborted", message: "the answer was let go before this fetch" }));
    if (call.socketConflict || (call.socketOps && call.socketOps.length)) { call.socketConflict = true; return Promise.reject(new Error(SOCKET_AND_FETCH)); }
    var method = init && init.method ? String(init.method).toUpperCase() : "GET";
    var headers = Array.from(new global.Headers(init && init.headers).entries());
    // A BufferSource body goes as its bytes, in base64 beside the text body
    // (LLP 1069.002 D4: `readFile`'s bytes as an upload's body). Anything
    // else is a string, as before.
    var raw = init ? init.body : undefined, bytes = null;
    // Fetch's Request refuses a body on a GET or HEAD, an empty one too.
    if (raw != null && (method === "GET" || method === "HEAD"))
      return Promise.reject(new TypeError("fetch: a " + method + " request cannot have a body"));
    // Web IDL's BufferSource: a view on a SharedArrayBuffer and a resizable
    // buffer are refused; a detached buffer is a copy of no bytes.
    if (raw instanceof ArrayBuffer || ArrayBuffer.isView(raw)) {
      var backing = ArrayBuffer.isView(raw) ? raw.buffer : raw;
      if (typeof SharedArrayBuffer !== "undefined" && backing instanceof SharedArrayBuffer)
        return Promise.reject(new TypeError("fetch: a body on a SharedArrayBuffer is not a BufferSource"));
      if (backing.resizable === true)
        return Promise.reject(new TypeError("fetch: a body on a resizable ArrayBuffer is not a BufferSource"));
      bytes = raw.byteLength === 0 ? new Uint8Array(0) : new Uint8Array(copyBytes(raw, "fetch"));
    }
    var body = bytes || raw == null ? "" : String(raw);
    // A body from an app file (LLP 1108 D6 R2): only the path goes to the
    // host, which reads the file when it runs the request, under `fs.read`,
    // at most 64 MiB. The bytes never enter JavaScript. A missing file, a
    // denied path or one over the bound rejects the fetch before it is sent.
    // A body to an app file (`exactBodyTo`, 2026-10-26): the host writes a
    // 2xx body there as it arrives, under `fs.write`, at most the ceiling or
    // 1 GiB, and the file appears only whole; any other reply is read as usual.
    var bodyFrom = init ? init.exactBodyFrom : undefined, bodyTo = init ? init.exactBodyTo : undefined;
    var fileWhy = fileRefusal(init, method, url, raw); // binary.js
    if (fileWhy) return Promise.reject(new TypeError(fileWhy));
    // LLP 1041 §8.4: an explicit promise about both operation and settlement.
    // Browsers ignore this native scheduling hint; their admission is unchanged.
    var independent = init ? init.exactIndependentHttp : undefined;
    var ceiling;
    if (independent !== undefined) {
      ceiling = independent && independent.maxResponseBytes;
      var most = bodyTo === undefined ? 67108864 : 1073741824; // a body written to a file: 1 GiB
      if (!Number.isInteger(ceiling) || ceiling <= 0 || ceiling > most)
        return Promise.reject(new TypeError("exactIndependentHttp.maxResponseBytes must be an integer from 1 to " + most));
    }
    // An answer that keeps coming (LLP 1016.000): `exactStream` maps each
    // server-sent event to the answer, and the end too. The promise never
    // settles: the answer is what the mapper returns, message by message.
    var stream = init ? init.exactStream : undefined;
    if (stream !== undefined && typeof stream !== "function")
      return Promise.reject(new TypeError("exactStream maps each event to the answer: (event) => value"));
    if (stream && call.stream) return Promise.reject(new Error("an answer streams one request"));
    if (stream && ceiling === undefined) ceiling = 1048576;
    // An upload whose stream gets its progress, then its reply (`exactUploadProgress`, LLP 1016.000 D10).
    var upload = uploadOf(init, stream, method, url);
    if (typeof upload === "string") return Promise.reject(new TypeError(upload));
    // A deadline for the whole exchange (headers and body), in milliseconds: the host cancels the request
    // when it passes and the fetch rejects with a FetchError of kind "Timeout". A stream has none; an upload may.
    var timeout = init ? init.exactTimeout : undefined, held = init ? init.exactHeld : undefined;
    if (timeout !== undefined) {
      if (!Number.isInteger(timeout) || timeout < 1 || timeout > 3600000)
        return Promise.reject(new TypeError("exactTimeout must be an integer number of milliseconds from 1 to 3600000"));
      if (stream && !upload) return Promise.reject(new TypeError("exactTimeout: a stream has no timeout"));
    }
    if ((held !== undefined && held !== true && held !== false) || (held && stream)) return Promise.reject(new TypeError(held === true ? "exactHeld: a stream is never held; it stops counting at its first message" : "exactHeld is true or false")); var poll = longPollOf(init, stream, method, url); if (typeof poll === "string") return Promise.reject(new TypeError(poll)); // `exactHeld`: the server may hold the reply, and no clock waits on it (LLP 1016.000 D9); `exactLongPoll` (D8): the host asks again with each reply's cursor (binary.js `longPollOf`)
    // `redirect` (Fetch §5.4, the Request constructor): read and checked once,
    // before the signal, and a throw rejects. Rust follows redirects, checking
    // each hop's grant, so "manual" answers with the 3xx and its Location.
    var redirect;
    try { var asked = init ? init.redirect : undefined; redirect = asked !== undefined ? String(asked) : undefined; }
    catch (e) { return Promise.reject(e); }
    if (redirect !== undefined && redirect !== "follow" && redirect !== "manual" && redirect !== "error")
      return Promise.reject(new TypeError("redirect must be follow, manual or error"));
    if (upload && redirect !== undefined && redirect !== "follow") return Promise.reject(new TypeError("exactUploadProgress: an upload follows its redirects"));
    // The web's `signal`: an aborted fetch rejects with its reason at once,
    // and the host cancels its request (`abortRequest`).
    var signal = init ? init.signal : undefined;
    if (signal != null && abortHooks) {
      try { abortHooks.subscribe(signal, function () {})(); }
      catch (e) { return Promise.reject(new TypeError("fetch: init.signal is not an AbortSignal")); }
    }
    if (signal && signal.aborted) return Promise.reject(signal.reason);
    // An `AbortSignal.timeout`'s time left is the host's deadline, unless an earlier `exactTimeout` is; past the
    // longest a request waits (an hour), the hour is, and the signal does not abort when it passes.
    var left = stream ? undefined : timers.left(signal), timed = left !== undefined && !(timeout < left);
    if (timed) { timeout = Math.min(left, 3600000); timed = left <= 3600000; }
    var ticket = nextTicket++;
    // A WebSocket stream owns a two-way socket (LLP 1016.000 D7); an HTTP one's body that is not events is bytes.
    var socket = stream && (!!listen || !!own || /^wss?:/i.test(String(url)));
    var error = host(1, String(ticket), JSON.stringify({ method: method, url: String(url), headers: headers, body: body, body_base64: bytes ? toBase64(bytes) : undefined, max_response_bytes: ceiling, stream: stream ? true : undefined, socket: socket ? true : undefined, bytes: stream && !socket && !poll && !upload ? true : undefined, upload_progress: upload ? true : undefined, long_poll: poll, timeout_ms: timeout, body_from: bodyFrom, body_to: bodyTo, redirect: redirect, held: held ? true : undefined, listen: listen }));
    if (error !== undefined) return Promise.reject(new Error(error));
    call.tickets.push(ticket);
    if (stream) { call.stream = stream; call.upload = !!upload; }
    var owner = socket ? (call.socket = listen ? listenOwner(ticket) : own ? own(ticket, call) : socketOwner(ticket, call.mutation)) : null;
    return new Promise(function (resolve, reject) {
      // `early`: made while its answer was still in flight (its body, or its
      // own continuation), so that answer's own, whoever else is pending.
      var p = { resolve: resolve, reject: reject, call: call, claimed: false, stream: !!stream, signal: signal, timed: timed, release: function () {}, early: call.status === "pending", bodyTo: bodyTo };
      pending.set(ticket, p);
      // The caller's abort also makes its socket handle stale (D7); the
      // stream itself runs until the runner lets its ticket go.
      // An adopted fetch (LLP 1146 D2) stays pending: the host cancels it and
      // answers its kept ticket aborted, and `__exact_fulfill` rejects it in a
      // module turn, not in the turn that aborted it, so what its let-go
      // call's continuation writes stands when that turn's commit is refused.
      // A host without door 23 leaves it to settle here, as before.
      if (signal) p.release = watchAbort(signal, function () { if (owner) owner.state = 3; if (p.module && pending.get(ticket) === p && cancelAdopted(ticket)) return; var was = settled(ticket); if (was) { if (p.module) work.landed(); reject(signal.reason); abortRequest(ticket, was); } });
    });
  }
  // A local listener (LLP 1133.005 D1): a resource's answer, as `fetch(url, {exactStream})` is, whose events
  // are mapped now with the connection's handle beside them. Natively on desktop hosts only (door 12 `listen`);
  // elsewhere the global is absent, so `"exactListen" in globalThis` is the feature test (§8).
  if (host(12, "listen", "") === "1") global.exactListen = function exactListen(address, options) {
    var o = options || {}, call = currentCall;
    if (call && call.mutation) throw new DOMException("exactListen() is a resource's answer: a mutation's listener would have no end", "InvalidStateError");
    if (typeof o.exactStream !== "function") return Promise.reject(new TypeError("exactListen(address, {exactStream(event, connection)}) maps each event to the answer"));
    var bound = function (name, most) { var v = o[name]; if (v !== undefined && (!Number.isInteger(v) || v < 1 || v > most)) throw new TypeError("exactListen: " + name + " is an integer from 1 to " + most); return v; };
    try { var listen = { takeover: !!o.takeover, first_line_timeout_ms: bound("firstLineTimeout", 600000), max_line_bytes: bound("maxLineBytes", 1048576), max_connections: bound("maxConnections", 64) }; }
    catch (e) { return Promise.reject(e); }
    return request(String(address), { exactStream: o.exactStream, exactIndependentHttp: { maxResponseBytes: 1048576 } }, listen);
  };

  // --- signing in through the system browser (LLP 1069.006) ---------------
  // `authCallback()` is this carrier's callback, known before PAR: a native
  // build's granted `auth.callback`, the web's `<origin>/.exact/auth/callback`
  // (D2); a device fact, so bake compiles no answer that asks.
  // `openAuthSession(url, {callback, state, ephemeral})` is a request to
  // `exact-auth:` on the independent lane, as `native.later` is (D1): it
  // resolves with the callback URL (200) or rejects with the status and
  // message (499 cancelled, 403, 409, 428, 501, 502) as `error.status`.
  global.authCallback = function authCallback() {
    answerOnly("authCallback()");
    var callback = host(12, "callback", "");
    if (callback === undefined) throw new Error("authCallback(): the grants name no auth.callback");
    return callback;
  };
  global.openAuthSession = function openAuthSession(url, options) {
    var o = options || {};
    if (host(12, "placement", "") === "worker") {
      return Promise.reject(new Error("openAuthSession() is unavailable in a worker-placed source on the web: its popup opens in the press's call stack (LLP 1069.006); place this source on main"));
    }
    if (typeof o.callback !== "string" || typeof o.state !== "string" || !o.state) {
      return Promise.reject(new TypeError("openAuthSession(url, {callback, state}): callback is authCallback() and state a non-empty string"));
    }
    var body = JSON.stringify({ url: String(url), callback: o.callback, state: o.state, ephemeral: !!o.ephemeral });
    return global.fetch("exact-auth:", { method: "POST", body: body, exactIndependentHttp: { maxResponseBytes: 65536 } })
      .then(function (r) {
        return r.text().then(function (t) {
          if (r.status === 200) return t;
          var e = new Error(t || "the auth session failed");
          Object.defineProperty(e, "status", { value: r.status, enumerable: true });
          throw e;
        });
      });
  };

  // --- the store (LLP 1018): reads counted, writes grant-checked, in Rust -
  var presence = binary.presence(host, function (name) { return store.get(name); }, function (api) { if (overlaying) throw inOverlay(api); });
  var store = {
    get: function (name) { if (overlaying) throw inOverlay("store.get()"); var v = presence.door(2, String(name), ""); return v === undefined ? null : v; },
    set: function (name, value) { if (overlaying) throw inOverlay("store.set()"); var e = presence.door(3, String(name), String(value)); if (e !== undefined) throw new Error(e); },
    forget: function (name) { if (overlaying) throw inOverlay("store.forget()"); var e = host(4, String(name), ""); if (e !== undefined) throw new Error(e); },
    has: presence.has, locked: presence.locked, unlock: presence.unlock, protect: presence.protect, unprotect: presence.unprotect,
    keepKey: function (name, pair) { return keepKey(name, pair); }, key: function (name) { return keptKey(name); },
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
  // A storage refusal is `{kind: "Unavailable", code, message}`, the code
  // the same on every host (kanban F28; host/web-js/ts-data.js `coded`):
  // storage itself is unavailable ("bake", "agent", "unsupported"), the
  // grants do not cover the operation ("denied"), the filesystem's POSIX
  // name ("ENOENT", "EEXIST", "ENOTDIR", "EISDIR", "ENOTEMPTY", "EBUSY"),
  // else "failed". The message says more and differs by host.
  var REFUSED = {
    bake: "storage is unavailable during bake",
    agent: "storage is unavailable in agent mode unless the drive names a scratch store (--storage <name>)",
    unsupported: "storage is unsupported by this host",
  };
  var toldAgent = false, READS = { readFile: true, readdir: true, stat: true, realpath: true };
  // Rust's error numbers are platform-specific. Never treat Windows access
  // denial (5) as EISDIR, or its 17/39 as Unix EEXIST/ENOTEMPTY.
  var windowsStorage = global.__exact_windows_storage === true;
  delete global.__exact_windows_storage;
  var ERRNO = windowsStorage
    ? { 2: "ENOENT", 3: "ENOENT", 32: "EBUSY", 33: "EBUSY", 80: "EEXIST", 145: "ENOTEMPTY", 170: "EBUSY", 183: "EEXIST", 267: "ENOTDIR" }
    : { 2: "ENOENT", 16: "EBUSY", 17: "EEXIST", 20: "ENOTDIR", 21: "EISDIR", 39: "ENOTEMPTY", 66: "ENOTEMPTY" };
  // The image calls' own failures (LLP 1069.002 A1.4, LLP 1133.019 D17),
  // named in the message natively; the web sets the code itself.
  var IMAGE_CODE = /^(?:filesystem: )?(?:compress|transform)Image: (unsupported|undecodable|too-large|unfit|timeout|outside): /;
  function storageCode(message) {
    if (/^denied: /.test(message)) return "denied";
    var image = IMAGE_CODE.exec(message);
    if (image) return image[1];
    // Ibex's retained-document boundary supplies typed codes and only the
    // logical path. Do not infer a code from provider diagnostic prose.
    var document = /^document (ENOENT|EEXIST|ENOTDIR|EISDIR|ENOTEMPTY|EBUSY): doc:\//.exec(message);
    if (document) return document[1];
    // The Windows document adapter emits this only after same-handle type
    // inspection; incidental text and numeric access denial are insufficient.
    if (windowsStorage && /\(filesystem code EISDIR\)$/.test(message)) return "EISDIR";
    var errno = /\(os error (\d+)\)$/.exec(message);
    if (errno && ERRNO[errno[1]]) return ERRNO[errno[1]];
    return /\bbusy\b|database is locked/.test(message) ? "EBUSY" : "failed";
  }
  // As host/web/storage-fs.js `deniedPath` words it on the web (app farm round 2: a bare
  // `denied: sqlite.open` left builds guessing which per-persona file it wanted).
  function deniedPath(operation, path) {
    var dir = path.slice(0, path.lastIndexOf("/"));
    var directory = /^[a-z]+:\/[^/]/.test(dir) ? ", or `" + operation + " " + dir + "` for every file there" : "";
    return "denied: " + operation + " " + path + ": no grant covers it; grant `" + operation + " " + path + "`" + directory + " (a grant covers its path and what is below it, by whole names)";
  }
  function storageError(message, code) {
    var error = new Error(message);
    error.kind = "Unavailable";
    error.code = code || storageCode(String(message));
    return error;
  }
  function storageCall(receiver, method, args, convert) {
    if (overlaying) return Promise.reject(inOverlay("storage." + method + "()"));
    var call = currentCall;
    // Storage issued after its answer replied (a `.then` chained on a save,
    // a write begun when a background one lands) is background work (D2).
    // Before any answer, during module evaluation, it is refused, as it is
    // on a worker placement, where answers wait for their storage.
    if (moving && (call ? call.replied : !initializing)) call = background;
    if (!call || call.replied) {
      var refused = storageError("storage." + method + "() called outside an answer: it was never run. Start storage inside an answer");
      journal("storage refused: " + refused.message);
      return Promise.reject(refused);
    }
    var refused;
    // A file operation names its path: a document the person chose
    // (`doc:/`) needs no app storage (LLP 1069.010 D1); a copy into an app file does.
    var path = receiver && nativeStorage && receiver === nativeStorage.fs && typeof args[0] === "string" ? args[0] : "";
    try { refused = host(5, method === "copyFile" && path && /^app:\//.test(args[1]) ? args[1] : path, "") || (receiver ? undefined : "unsupported"); }
    catch (e) { refused = "bake"; } // no filesystem or database effects during bake
    // A drive with no scratch store is said once, at its first refused read or write, for the driver's note (LLP 1102 §3.17); a read finds an empty store, as on a fresh install (storage-environment.js `emptyRead`, word for word).
    if (refused === "agent" && !toldAgent) { toldAgent = true; journal("storage refused (agent): " + REFUSED.agent); }
    if (refused === "agent" && READS[method] && receiver === (nativeStorage && nativeStorage.fs)) return method === "readdir" && /^app:\/(data|cache|tmp)$/.test(args[0]) ? Promise.resolve([]) : Promise.reject(storageError("filesystem: no such file or directory: " + args[0] + " (a drive that names no scratch store reads an empty one; --storage <name> names one)", "ENOENT"));
    if (refused) return Promise.reject(storageError(REFUSED[refused], refused));
    var what = method + (path ? " " + path : "");
    // The bound (D3): refusal is the overload policy (LLP 1041 D2).
    if (head && queue.length >= MAX_QUEUED) {
      journal("storage refused: full (" + what + ")");
      return Promise.reject(storageError("storage queue full: " + MAX_QUEUED + " operations wait", "full"));
    }
    call.storage++;
    storing.add(call);
    // The web realm attributes the cell now, while this answer is current.
    // The adapter runs at issue, and a background round may be current then
    // (LLP 1097 D7). Hermes has no hook and runs the adapter at the call.
    var held = null;
    if (typeof global.__exact_reserve_storage === "function") held = global.__exact_reserve_storage();
    var argv = Array.prototype.slice.call(args);
    return new Promise(function (resolve, reject) {
      var op = { call: call, method: method, run: function () {
        try { return receiver[method].apply(receiver, argv); }
        catch (e) {
          if (held != null && typeof global.__exact_abandon_storage === "function") global.__exact_abandon_storage(held);
          throw e;
        }
      } };
      op.settle = function (ok, value) {
        var at = owner(op.call);
        currentCall = at;
        unstore(at);
        if (ok) {
          storageCounts.done++;
          if (/^(writeFile|atomicWriteFile|appendFile|mkdir|rm|rename|copyFile)$/.test(method) && [method === "copyFile" ? argv[1] : argv[0], method === "rename" ? argv[1] : null].some(function (path) {
            if (typeof path !== "string" || !/^app:\//.test(path)) return false;
            path = new URL(path).href.replace(/\/$/, "");
            return String(global.exact.grants).split("\n").some(function (line) {
              var grant = line.trim().split(/\s+/), root = (grant[1] || "").replace(/\/$/, "");
              return grant[0] === "fs.read" && /^app:\//.test(root) && (path === root || path.indexOf(root + "/") === 0 || root.indexOf(path + "/") === 0);
            });
          })) storageCounts.writes++;
          try { resolve(convert ? convert(value) : value); } catch (e) { reject(e); }
          return;
        }
        var error = value;
        // The host's grant refusal names no path: name it, and the lines that would admit it.
        if (method === "open" && typeof argv[0] === "string" && /^denied: sqlite\.open$/.test(error && error.message || String(error))) error = storageError(deniedPath("sqlite.open", argv[0]), "denied");
        else if (!error || !error.kind) error = storageError(error && error.message || String(error), error && error.code);
        else if (!error.code) try { error.code = storageCode(String(error.message)); } catch (_) { /* a frozen error keeps its own */ }
        // Every failed operation is journaled, an answer's or the
        // background's, so one nobody catches is still seen (D8).
        var line = "storage failed: " + what + ": " + error.code + " " + errorText(error);
        journal(line);
        storageCounts.failed++; storageCounts.last = line;
        reject(error);
      };
      if (head) queue.push(op);
      else issue(op);
    });
  }
  function statement(raw) {
    return Object.freeze({
      execute: function (params) { return storageCall(raw, "execute", [params]); },
      query: function (params) { return storageCall(raw, "query", [params]); },
      close: function () { return storageCall(raw, "close", []); },
    });
  }
  // The databases open now, each with the path and the work that opened it: an answer that
  // fails, its own or one let go (LLP 1097 D7), with one still open is journaled, since the host
  // does not close a handle an app may keep or share (Charlie, 2026-10-07), and the next open
  // would find it locked. A close counts from the moment it is issued.
  var openDatabases = new Set();
  // On a failure (an answer's own rejection, or a rejection nothing handled), each database
  // opened by an answer that has replied, failed or been let go, or by background work: work that
  // runs on in the background, which may have been the work that failed.
  function leftOpen() {
    openDatabases.forEach(function (h) {
      var by = h.by;
      if (h.told || !by || !(by === background || by.replied || by.status !== "pending" || by.letGo || by.lost)) return;
      h.told = true;
      journal("storage: " + h.path + " is still open after a failure in background work that opened it: if that work owns it, close it in a finally (finally { db.close() }), or the next open finds it locked (LLP 1097 D7)");
    });
  }
  function database(raw, path, by) {
    var handle = { path: path, by: by, told: false };
    openDatabases.add(handle);
    return Object.freeze({
      execute: function (sql, params) { return storageCall(raw, "execute", [sql, params]); },
      query: function (sql, params) { return storageCall(raw, "query", [sql, params]); },
      prepare: function (sql) { return storageCall(raw, "prepare", [sql], statement); },
      transaction: function (commands) { return storageCall(raw, "transaction", [commands]); },
      // A close counts once issued; one refused or failed leaves the database open, and tracked.
      close: function () {
        openDatabases.delete(handle);
        // A failed close still rejects for the caller, so one left unhandled is still reported.
        return storageCall(raw, "close", []).then(undefined, function (e) { openDatabases.add(handle); throw e; });
      },
    });
  }
  var files = { directories: Object.freeze({ data:"app:/data", cache:"app:/cache", temporary:"app:/tmp" }) };
  ["readFile", "writeFile", "atomicWriteFile", "appendFile", "readdir", "mkdir", "rm", "stat", "rename", "copyFile", "realpath", "lstat", "readlink", "access", "chmod", "symlink", "digest"].forEach(function (method) {
    files[method] = function () { return storageCall(nativeStorage && nativeStorage.fs, method, arguments); };
  });
  // LLP 1069.010.000 D4: a kept file the app replaces with its own atomic write stays kept: the host reads what the write replaces, then carries the entry to the file written.
  var atomic = files.atomicWriteFile; files.atomicWriteFile = function (path) { var t; try { t = typeof path === "string" && path.indexOf("doc:/") === 0 ? host(6, "kept", "replacing " + path) : undefined; } catch (e) { t = undefined; } var done = function (how) { if (t) try { host(6, "kept", how + t); } catch (e) {} }; return atomic.apply(null, arguments).then(function (v) { done("replaced "); return v; }, function (e) { done("dropped "); throw e; }); };
  // LLP 1069.010.000 D2a: `{expect: stat}` refuses a doc:/ write `ECHANGED` if the file changed since; the stat runs just before it.
  ["writeFile", "atomicWriteFile", "appendFile"].forEach(function (method) {
    var write = files[method], api = "storage.fs." + method + "()";
    files[method] = function (path, data, options) {
      var expect = options && options.expect;
      if (!expect) return write.apply(null, arguments);
      if (typeof path !== "string" || path.indexOf("doc:/") !== 0 || typeof expect.modifiedMs !== "number" || typeof expect.size !== "number") return Promise.reject(new TypeError(api + ": expect is a stat (modifiedMs and size) of a doc:/ path"));
      return files.stat(path).then(function (now) { if (now.modifiedMs !== expect.modifiedMs || now.size !== expect.size) throw storageError("document ECHANGED: " + path + " changed on disk since it was read", "ECHANGED"); return write(path, data); });
    };
  });
  // LLP 1069.010.000 D1: the answer is asked again when the doc:/ path (a folder's own entries, or `recursive` its subtree) changes on disk.
  // An advisory lock (LLP 1133.007 D6a), natively only: released by its `unlock()`, the end of the answer that took it (`freeLocks`), or the session's.
  files.flock = function (path, options) { var t = options && options.timeout, fs = nativeStorage && nativeStorage.fs, by = currentCall; return storageCall(fs, "flock", [path, t === undefined ? 0 : t], function (h) { var lock = { h: h, by: by }; heldLocks.add(lock); if (by && by.replied) freeLocks(); return Object.freeze({ unlock: function () { return heldLocks.delete(lock) ? storageCall(fs, "unlock", [h]) : Promise.resolve(); } }); }); };
  function freeLocks() { heldLocks.forEach(function (l) { if (!l.by || l.by === background || !l.by.replied) return; heldLocks.delete(l); try { Promise.resolve(nativeStorage.fs.unlock(l.h)).then(undefined, function () {}); } catch (e) {} }); }
  files.watch = function (path, options) {
    if (overlaying) throw inOverlay("storage.fs.watch()");
    if (!currentCall || currentCall.status !== "pending") throw new Error("storage.fs.watch() outside an answer");
    if (typeof path !== "string" || path.indexOf("doc:/") !== 0) throw new TypeError("storage.fs.watch(): path must be a doc:/ path");
    host(6, "watch", (options && options.recursive ? "doc-watch-recursive:" : "doc-watch:") + path);
  };
  files.recentDocuments = function () { return web("recentDocuments", []) || kept(""); }; files.forgetDocument = function (path) { return web("forgetDocument", [path]) || kept("forget " + path); }; function web(method, args) { var fs = nativeStorage && nativeStorage.fs; return fs && fs[method] ? storageCall(fs, method, args) : null; } function kept(ask) { try { var r = host(6, "kept", ask); return Promise.resolve(r ? JSON.parse(r) : undefined); } catch (e) { var m = String(e && e.message || e); return Promise.reject(m === "bake" ? storageError(REFUSED.bake, "bake") : storageError(m)); } } // LLP 1069.010.000 D5 (doc.keep): the host's kept list, which watch("doc:/") hears change, and the app's own removal
  // `fs.compressImage(from, to, {maxDimension, maxBytes, type, rotate, flip, crop})` (LLP 1069.002 A1,
  // A1.11; LLP 1133.019 D15) and `fs.transformImage(from, to, options)` (D14): their options are checked
  // before anything is queued and travel as binary.js `imageOptions` words them; natively the reply is
  // `width\theight\tsize`, a transform's then its path and the codec's report, and on the web the record.
  function imageCall(method, from, to, options, record) {
    var tail;
    try {
      if (typeof from !== "string" || typeof to !== "string") throw new TypeError("storage.fs." + method + "(): from and to must be app:/ paths");
      tail = binary.imageOptions("storage.fs." + method + "()", options, method === "transformImage");
    } catch (e) { return Promise.reject(e); }
    return storageCall(nativeStorage && nativeStorage.fs, method, [from, to].concat(tail), function (value) {
      return typeof value === "string" ? record(value.split("\t"), tail) : value;
    });
  }
  files.compressImage = function (from, to, options) { return imageCall("compressImage", from, to, options, function (parts, tail) { return { path: to, type: tail[2], size: +parts[2], width: +parts[0], height: +parts[1] }; }); };
  files.transformImage = function (from, to, options) {
    return imageCall("transformImage", from, to, options, function (parts) {
      var report = JSON.parse(parts.slice(4).join("\t"));
      return { path: parts[3], type: report.type, size: +parts[2], width: +parts[0], height: +parts[1], colorSpace: report.colorSpace, hdr: report.hdr, metadata: report.metadata };
    });
  };
  var storage = Object.freeze({ fs:Object.freeze(files), sqlite:Object.freeze({
    open:function (path) {
      // Its owner as storageCall reckons it: storage after an answer replied is the background's.
      var by = moving && (currentCall ? currentCall.replied : !initializing) ? background : currentCall;
      return storageCall(nativeStorage && nativeStorage.sqlite, "open", [path], function (raw) { return database(raw, path, by); });
    },
  }) });

  // --- the seam ------------------------------------------------------------
  var calls = new Map(), nextCall = 1, gcOwed = false; // calls: id -> { id, status, value, error, tickets }
  // Large native strings travel beside the JSON skeleton by explicit paths (no property
  // spelling reserved); the serializer owns getters, toJSON, omissions and cycles. One
  // whose answer outlived a collection owes a full one, once no answer awaits (shim.cc).
  var captureString = global.__exact_capture_string, gcEpoch = global.__exact_gc_epoch || function () { return 0; };
  delete global.__exact_capture_string; delete global.__exact_gc_epoch;
  var heldLocks = new Set();
  function cleanUp() { freeLocks(); var waiting = false; calls.forEach(function (c) { if (!c.stream && !c.letGo) waiting = true; }); if (gcOwed && !waiting) { gcOwed = false; gcEpoch(true); } }
  function ok(value, held) {
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
        captureString(JSON.stringify(path), item); if (held) gcOwed = true;
        return "";
      }
      // Only ancestors of the current object are needed for later capture paths.
      if (object) head = {value:item, parent:parent, key:key};
      return item;
    });
  }
  // `failure(x)`'s code for what a module let through (LLP 1109 D3), as
  // host/web-js/rt.js `failureCode` reads it: a fetch's rejection by its
  // kind, a coded storage refusal, else the module's own error.
  var FETCH_FAILURE = { Network: "offline", Timeout: "timeout", Refused: "refused" };
  function failureCode(e) {
    if (!e || typeof e !== "object") return "error";
    if (e.name === "FetchError") return FETCH_FAILURE[e.kind] || "error";
    return e.kind === "Unavailable" && typeof e.code === "string" ? "storage" : "error";
  }
  function fail(e) {
    var kind = e && typeof e === "object" ? e.kind : undefined;
    if (kind !== "UnknownSource" && kind !== "BadArguments" && kind !== "Unavailable") kind = "Unavailable";
    var message = e && typeof e === "object" && e.message !== undefined ? e.message : e;
    // `reason`: what `failure(x).reason` reads, a string the error carries (the snapback4 driver's refusal code).
    var reason = e && typeof e === "object" && typeof e.reason === "string" ? e.reason : "";
    return JSON.stringify({ tag: 2, kind: kind, code: e && e.code, failure: failureCode(e), reason: reason, message: String(message) });
  }
  // Natively, liveness is the module's, as a browser's event loop has it
  // (LLP 1027.003.000 §13; hn-reader F7): an answer awaiting a promise
  // another answer started (one memoized fetch, a queue chained through
  // another's storage) waits while any fetch or storage step of the module
  // is outstanding, and the executor asks again after each one lands. The
  // web's module realm (module-glue.js, the wasm target's) runs one answer
  // at a time and keeps the per-answer rule.
  //
  // A continuation runs as the answer whose reply settled the promise it
  // awaited, so a fetch an answer makes after awaiting another's lands among
  // that other's tickets (hn-reader F7, review finding 1). A fetch no settle
  // has handed the executor yet is the module's: the next answer to settle
  // without work of its own claims it and waits on it, so no request is left
  // behind. Storage a settled answer left is the background's (LLP 1097 D2),
  // which no answer claims.
  var moduleWide = bytesDoor !== undefined;
  function claim(call, ticket) {
    var p = pending.get(ticket);
    p.claimed = true;
    if (p.call !== call) { p.call = call; call.tickets.push(ticket); }
    // An answer waits on one thing at a time here: its timer runs after the fetch lands (the web races them).
    if (!p.timed && !call.toldRace && timers.armed(call)) { call.toldRace = true; journal("data: an answer awaits a fetch while a timer of its own is armed: the timer runs only after the fetch lands, so it cannot cut the fetch short as it can on the web; give the fetch AbortSignal.timeout or exactTimeout"); }
    return JSON.stringify({ tag: 1, call: call.id, ticket: ticket });
  }
  // An answer is given when its value is ready, as in a browser (LLP 1097
  // D1): the storage it started and did not await finishes after it, as the
  // background's, so nothing is stranded (kanban F22's reason to wait). On a
  // worker placement, which runs one turn to its end, it still waits.
  function reply(call, text) {
    endYields(call); // a branch its value did not wait for goes on, as a browser's does
    timers.drop(call); // its timers are its own (binary.js `timers`)
    call.replied = true;
    calls.delete(call.id); cleanUp();
    return text;
  }
  function settle(call, final) {
    // A fetch its continuation tried after it staged socket operations
    // (refused at the fetch) refuses the mutation too, before any frame.
    if (call.socketConflict && call.status === "done") {
      withdrawFetches(call);
      call.status = "failed"; call.error = new Error(SOCKET_AND_FETCH);
    }
    // Its own work first: a fetch it made that no settle has handed out,
    // then its storage steps, which move to the background once its value
    // is ready. A fetch made while it was in
    // flight is its own (an unawaited POST), and it claims it whoever else
    // is pending. One made only after it finished ran in a continuation it
    // settled for another answer: with another answer in flight, which may
    // be that one, it is left to it; with none, it waits for the fetch
    // itself. Either way nothing is left behind.
    var finished = call.status === "done" || call.status === "failed", others = false;
    if (finished && moduleWide) calls.forEach(function (c) { if (c !== call && !c.letGo && c.status === "pending") others = true; });
    for (var i = 0; i < call.tickets.length; i++) {
      var own = pending.get(call.tickets[i]);
      if (own && !own.claimed && own.call === call && (own.early || !others)) return claim(call, call.tickets[i]);
    }
    if (!finished && call.yields && call.yields.length) return JSON.stringify({ tag: 1, call: call.id, ticket: 0, yielded: true });
    if (call.storage > 0 && !call.lost) {
      if (finished && moving) {
        // Its value is ready: what it started and did not await is the
        // background's (D1, D2), and it replies now.
        background.storage += call.storage; call.storage = 0;
        storing.delete(call); storing.add(background);
        call.moved = true;
      } else {
        // It waits on its own storage: the one continuation is its when its
        // operation is in flight; queued behind another owner's, it waits
        // with the module and is asked again after each delivery (D3).
        var at = headOwner();
        if (!at || at === call) return JSON.stringify({ tag:1, call:call.id, ticket:0 });
        return JSON.stringify({ tag: 1, call: call.id, ticket: 0, waiting: true });
      }
    }
    if (finished) {
      // A fetch of its own already handed out and still in flight: asked
      // again once it lands (natively; the module realm runs one at a time).
      var inFlight = false;
      pending.forEach(function (p) { if (p.call === call && p.claimed) inFlight = true; });
      if (moduleWide && inFlight) return JSON.stringify({ tag: 1, call: call.id, ticket: 0, waiting: true });
      if (call.status === "done") handSocketOps(call);
      return reply(call, call.status === "done" ? ok(call.value, call.epoch !== gcEpoch()) : fail(call.error));
    }
    // Its own timers (binary.js `timers`): the earliest is due on the runner's clock.
    var due = timers.wait(call), later = due === undefined || due > timers.now();
    if (moduleWide) {
      var unclaimed;
      pending.forEach(function (p, ticket) { if (unclaimed === undefined && !p.claimed && !p.stream && !p.module) unclaimed = ticket; });
      if (unclaimed !== undefined) return claim(call, unclaimed);
      var left = false;
      // Never the background's: only its own round holds the storage
      // ticket (D2); an answer awaiting it waits with the module below.
      storing.forEach(function (c) { if (c !== background && !calls.has(c.id)) left = true; });
      if (left) return JSON.stringify({ tag:1, call:call.id, ticket:0 });
      // Another answer's work may settle it before its timer: whichever comes first asks it again (not on a
      // worker, which runs one turn to its end and so cannot wait on another's).
      if (!final && (due === undefined || moving && later) && (pending.size || storing.size || yielded || work.outstanding())) return JSON.stringify({ tag: 1, call: call.id, ticket: 0, waiting: true, timer: due });
    } else if (due === undefined && moving && !final && background.storage > 0) {
      // The page's realm runs one answer at a time, but background work
      // goes on beside it: an answer awaiting it parks and is asked again
      // after a background delivery (LLP 1097 D7).
      return JSON.stringify({ tag: 1, call: call.id, ticket: 0, waiting: true });
    }
    if (due !== undefined) return JSON.stringify({ tag: 1, call: call.id, ticket: 0, timer: due });
    call.replied = true;
    calls.delete(call.id); cleanUp();
    if (moduleWide) return fail(new Error("the answer is pending on nothing: it awaits a promise that no fetch or storage step " +
      "in flight in this module will settle"));
    return fail(new Error("the answer is pending on nothing: no host operation it started will resolve it. " +
      "An answer that awaits a promise another answer started (a fetch shared between answers, or a queue chained through a " +
      "fetch another answer is waiting on) waits on work it does not own in the web's module realm; make each answer's own " +
      "fetch, or share the resolved value rather than the promise. Storage is different: an answer queued behind another's " +
      "storage turn waits for it"));
  }
  var nativeCall = global.__exact_module_sync = function (request) { // `native.call`, and a module binding's synchronous form (LLP 1133.002 D14)
    if (overlaying) throw inOverlay("native.call()");
    if (currentCall === background) {
      journal("data: native.call() called from background work: it was never run");
      throw new Error("native.call() called from background work: it was never run. Call it inside an answer");
    }
    if (!currentCall || currentCall.status !== "pending") throw new Error("native call outside an answer");
    try { return JSON.parse(host(6, "call", JSON.stringify(request))); } // at bake, no module: storage's refusal (LLP 1067 D4)
    catch (e) { throw e && e.message === "bake" ? storageError("native.call() is unavailable during bake: the build links no native module", "bake") : e; }
  };
  // The executor: `__exact_call(source, argsJson)` → tag 3 with a call id (a value given at once too: see its end) — then
  // it drains microtasks and asks `__exact_settle(id)`, which is tag 0/2, or tag 1 with the ticket of the fetch the answer is
  // waiting on (0: its storage; 0 and `waiting`: another answer's work). `__exact_fulfill(ticket, outcomeJson)` resolves that
  // fetch; drain and settle again. `__exact_settle(id, "final")` refuses an answer still waiting on nothing of its own.
  // A handler a module calls back (LLP 1133.016 D9): the `tools` export's, its value as JSON text.
  function tool(name, args) { var f = global.exact.tools && global.exact.tools[name], text = function (v) { return JSON.stringify(v === undefined ? null : v); }, e, r;
    if (typeof f !== "function") { e = new Error("the data module's `tools` has no `" + name + "`"); e.kind = "UnknownSource"; throw e; } r = f(args, store, storage); return r && typeof r.then === "function" ? r.then(text) : text(r); }
  global.__exact_call = function (source, argsJson, mutation) {
    initializing = false;
    var call = { id: nextCall++, status: "pending", value: undefined, error: undefined, tickets: [], storage: 0, replied: false, mutation: mutation === "mutation", answer: mutation === "answer", sync: false, socketOps: null, epoch: gcEpoch() };
    var result;
    currentCall = call; work.setTurn(call);
    try {
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
          if (overlaying) throw inOverlay("native.watch()");
          if (!currentCall || currentCall.status !== "pending") throw new Error("native.watch outside an answer");
          host(6, "watch", String(topic));
        },
        // Long work: off the source's thread and outside its budget, settled
        // when the module's own work replies. It travels as a request the
        // host hands to the module (on the web, the app's page module), so
        // the answer waits for it as for a fetch. A module that takes no
        // long calls answers through `call`, now.
        later: function (request) {
          if (overlaying) return Promise.reject(inOverlay("native.later()"));
          if (host(6, "later", "") !== "later") {
            try { return Promise.resolve(nativeCall(request)); } catch (e) { return Promise.reject(e); }
          }
          return global.fetch("exact-native:", { method: "POST", body: JSON.stringify(request), exactIndependentHttp: { maxResponseBytes: 1048576 } })
            .then(function (r) { return r.text().then(function (t) { if (r.status === 200) return JSON.parse(t); throw new Error(t || "native call failed"); }); });
        },
      });
      call.sync = true;
      try { result = mutation === "tool" ? tool(source.slice(5), JSON.parse(JSON.parse(argsJson)[0])) : global.exact.answer(source, JSON.parse(argsJson), store, storage, native); }
      finally { call.sync = false; }
      // Staged socket operations go out with no fetch, a stream answer's included (D7); a source that answers with a promise is
      // an answer in flight from its call on, and what it staged is that turn's (the amendment of 2026-10-18).
      if (call.socketOps && call.tickets.length) call.socketConflict = true;
      if (call.socketConflict) {
        if (result && typeof result.then === "function") result.then(undefined, function () {});
        throw new Error(SOCKET_AND_FETCH);
      }
      if (call.socketOps && result && typeof result.then === "function") handSocketOps(call, 17);
    }
    catch (e) {
      calls.set(call.id, call);
      if (call.socketConflict) { withdrawFetches(call); e = new Error(SOCKET_AND_FETCH); }
      // A refused socket mutation makes no fetch later either: its
      // continuation's are refused as the second effect.
      if (call.socketOps) call.socketConflict = true;
      call.socketOps = null;
      call.status = "failed"; call.error = e;
      return JSON.stringify({ tag: 3, call: call.id });
    }
    if (result && typeof result.then === "function") {
      calls.set(call.id, call);
      result.then(function (v) {
        if (call.lost) return;
        call.status = "done"; call.value = v;
      }, function (e) {
        // A lost call's answer is not delivered, but a database it left open still is said.
        if (call.lost) { leftOpen(); return; }
        call.status = "failed"; call.error = e;
        leftOpen();
      });
      return JSON.stringify({ tag: 3, call: call.id });
    }
    // A value given at once is replied after the microtask checkpoint that
    // follows the call, as a browser runs one after each task, and work the
    // answer queued there is its own and lands first: a save chained behind
    // a resolved promise was never run on macOS, with nothing in the logs
    // (drums R10), because a reply given here left it queued with no answer
    // in flight to run it. The owner stays installed while that turn drains.
    // (The reply is serialized at `settle`, as a promised value is: the
    // engine keeps one call's large strings.)
    calls.set(call.id, call);
    call.status = "done"; call.value = result;
    return JSON.stringify({ tag: 3, call: call.id });
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
  // Calls still tracked: a diagnostic for tests (LLP 1041 §8.4 Q5).
  global.__exact_calls_open = function () { return String(calls.size); };
  global.__exact_settle = function (id, final) {
    timers.checkpoint();
    currentCall = null;
    var call = calls.get(Number(id));
    return call ? settle(call, final === "final") : fail(new Error("no such call"));
  };
  // The runner let this call's request go (LLP 1016 D5): drop the call, and
  // reject the fetches it waits on, so its continuation runs (a `finally`
  // clears what the call set) rather than vanishing: the runner drops the
  // reply when it comes. The request may already be on the wire. A send
  // that needs every reply is a `queue` mutation (LLP 1092).
  // One let go between storage steps answers "storage": its steps are
  // running and the chain behind them goes on, so the executor delivers them
  // until `__exact_let_go` says none is left, and its answer is never given
  // (ledger F12). A fetch it makes then is refused: no one wants its reply.
  global.__exact_forget = function (id) {
    var call = calls.get(Number(id));
    if (!call) return ""; call.letGo = true; // a fetch, yield or timer it makes from here is refused
    var moved = work.forgot(call); // its steps waiting for its own turn run in the let-go chain, with no store (LLP 1146 D2)
    // A released resource's socket handle is stale from now on (D7).
    if (call.socket) { call.socket.state = 3; if (call.socket.close) call.socket.close(); }
    // Not a fetch another answer has since claimed: that one waits on it.
    var dropped = [];
    for (var i = 0; i < call.tickets.length; i++) {
      var p = pending.get(call.tickets[i]);
      // A stream's promise never settles (LLP 1016.000), ended or let go.
      if (p && p.call === call && !p.module) { settled(call.tickets[i]); if (!p.stream) dropped.push(p); }
    }
    // Rejected now, run at the next drain with no answer current.
    dropped.forEach(function (p) {
      p.reject(new FetchError({ kind: "Aborted", message: "the answer was let go before this reply; the request may already have been sent" }));
    });
    var rejected = dropped.length + endYields(call, new DOMException("the answer was let go", "AbortError")) + moved ? " rejected" : "";
    timers.drop(call);
    if (call.storage > 0 && !call.lost) return "storage" + rejected;
    call.replied = true;
    calls.delete(call.id); cleanUp();
    return rejected.trim();
  };
  global.__exact_let_go = function (failed, message) {
    // A let-go call's compression so failed is settled, and what is queued
    // behind it issued; the call keeps its other storage (LLP 1069.002
    // A1.5). Said so, and the caller delivers on.
    if (failed && head && headOwner() && headOwner().letGo && failAbandonedImage(String(message))) return "settled";
    var owed = false;
    calls.forEach(function (c) {
      if (!c.letGo) return;
      if (failed) { c.lost = true; storing.delete(c); }
      if (c.storage > 0 && !c.lost) owed = true;
      else { c.replied = true; calls.delete(c.id); }
    }); cleanUp();
    if (!owed) return "";
    // Deliver only when the operation in flight is the let-go call's.
    // One queued behind a live answer stays there: delivering it here would
    // run the live answer's step with no store (a934686a0).
    var at = headOwner();
    return at && at.letGo ? "storage" : "queued";
  };
  // One message of the stream answer `id` began, or its end: the mapper's
  // value, now — a stream's answer never awaits (LLP 1016.000 D1). The end
  // is `{ type: "error" }` with the failure's kind and message, or the
  // status and body of a reply that was not 2xx; an upload's reply is `load`.
  global.__exact_message = function (id, outcomeJson) {
    var call = calls.get(Number(id));
    if (!call || !call.stream) return fail(new Error("no stream answer " + id));
    var o = JSON.parse(outcomeJson), event, socket = call.socket, listened = socket && socket.listen && o.message ? sockets.listenEvent(socket, o.message) : null;
    if (listened) event = listened.event;
    else if (call.upload && (o.response || o.message && o.message.event === "exact:progress")) event = binary.uploadEvent(o);
    else if (o.message) event = { type: o.message.event || "message", data: binary.messageData(o.message), lastEventId: o.message.id, coalesced: o.message.coalesced };
    else if (o.closed) event = { type: "close", data: "", lastEventId: "", coalesced: 0, code: o.closed.code, reason: o.closed.reason, wasClean: o.closed.wasClean };
    else if (o.failed) event = { type: "error", data: "", lastEventId: "", coalesced: 0, kind: o.failed.kind, message: o.failed.message, status: 0 };
    else event = { type: "error", data: o.response.body, lastEventId: "", coalesced: 0, kind: "Response", message: "HTTP " + o.response.status, status: o.response.status };
    // The socket's state as of this event (D7): `open` opens it, an end closes it. A listener's stream ends as its `error`.
    // `AddressInUse` travels as `Refused`, its message so prefixed (executor_listen.rs).
    if (socket && !o.message && socket.close) { var f = o.failed || { kind: "Network", message: event.message }, inUse = /^AddressInUse: /.test(f.message); socket.close(); event = { type: "error", kind: inUse ? "AddressInUse" : f.kind, message: inUse ? f.message.slice(14) : f.message }; }
    // A child's owner reads its own events (js/src/process.js).
    if (socket && socket.event) event = socket.event(o, event);
    else if (socket) socket.state = !o.message ? 3 : event.type === "open" && socket.state === 0 ? 1 : socket.state;
    currentCall = call; inMapper = true;
    // Its mapper may reply on it answering `open` or a `message` (never on wasm, where it never opens): handed over however the call ends, sent if the frame's commit stands.
    // A listener's, on the connection whose `connection` or `message` it answers (LLP 1133.005 D6).
    var own = listened && listened.connection && (event.type === "connection" || event.type === "message") ? listened.connection : null;
    var turn = mapping = socket && o.message && socket.state === 1 && (!socket.listen || own) ? { owner: socket, connection: own, socketOps: null, overload: null } : null;
    try {
      var value = !socket ? call.stream(event) : socket.listen ? call.stream(event, listened && listened.connection ? listened.connection.handle : undefined) : call.stream(event, socket.handle);
      if (value && typeof value.then === "function") throw new Error("exactStream answers each event now; it cannot await");
      if (turn && turn.overload) throw turn.overload;
      return ok(value);
    } catch (e) { return fail(e); }
    finally { currentCall = null; mapping = null; inMapper = false; if (turn) handSocketOps(turn); }
  };
  // The module's storage, after a call or a delivery (LLP 1097 D5, D8):
  // whether the operation in flight is the background's (`head`), the
  // operations queued behind it and in flight, an answer's or the
  // background's, and what landed and failed.
  global.__exact_background = function () {
    timers.checkpoint();
    currentCall = null;
    return JSON.stringify({ head: headOwner() === background, queued: queue.length, inFlight: head ? 1 : 0,
      writes: storageCounts.writes, done: storageCounts.done, failed: storageCounts.failed, last: storageCounts.last });
  };
  // An overlay (`overlay(source, args, answer, writes)`): what a resource
  // shows while writes affect it, called synchronously with no answer current. It has
  // no effects: `fetch`, storage, the store, `native` and `crypto` refuse.
  var overlaying = false;
  function inOverlay(api) {
    var e = new Error(api + " is unavailable in an overlay, which runs synchronously and has no effects; do the work in the source's answer or the mutation, and pass the overlay what it needs in the mutation's arguments");
    journal("overlay refused: " + e.message);
    return e;
  }
  // Whether the module has an overlay, read once when it has loaded (the host keeps the answer for the instance,
  // as the JS target decides at install): anything but `undefined`, or a read that throws, counts as one, so the
  // overlay is then asked each time as before. A getter or a proxy's trap runs here as initialization code.
  global.__exact_has_overlay = function () {
    try { return global.exact.overlay === undefined ? "" : "1"; } catch (e) { return "1"; }
  };
  global.__exact_overlay = function (source, argsJson, payloadJson) {
    initializing = false;
    // A realm whose answer spans tasks (the web's) has one current between them: put it back after. Both reads of
    // `exact.overlay` run guarded, so a getter's store call is refused as the overlay's is.
    var saved = currentCall;
    currentCall = null;
    overlaying = true;
    try {
      if (typeof global.exact.overlay !== "function") return JSON.stringify({ tag: 4 });
      var payload = JSON.parse(payloadJson);
      // An answered write the answer does not include yet goes on showing
      // when the overlay calls its `keep()`.
      var keep = [];
      var writes = payload.writes.map(function (w) {
        return Object.freeze(Object.assign({}, w, { keep: function () { if (keep.indexOf(w.id) < 0) keep.push(w.id); } }));
      });
      var value = global.exact.overlay(source, JSON.parse(argsJson), payload.answer, writes);
      if (value && typeof value.then === "function") throw new Error("an overlay returned a promise: it runs synchronously inside a commit and cannot await; compute its value from `answer` and `writes` and return it");
      return JSON.stringify(value === undefined ? { tag: 4 } : { tag: 0, value: value, keep: keep });
    } catch (e) {
      return JSON.stringify({ tag: 2, message: e && e.message ? String(e.message) : String(e) });
    } finally { overlaying = false; currentCall = saved; }
  };
  // A background round delivers with the background current: what its
  // completion's reaction issues is the background's too.
  global.__exact_enter_background = function () { currentCall = background; work.setTurn(null); };
  // A compression whose wait gave up, and whose right to write the waiter
  // took away (LLP 1069.002 A1.5): it can write nothing now, so it is
  // failed and the queue moves on. Any other operation whose wait ran out
  // may still write, so its queue waits for it, as before. Whether it was.
  var IMAGE_ABANDONED = /^compressImage: timeout: the storage wait ran out/;
  function failAbandonedImage(message) {
    if (!head || (head.method !== "compressImage" && head.method !== "transformImage") || !IMAGE_ABANDONED.test(message)) return false;
    landed(head, false, storageError(message.replace(/^compressImage/, head.method)));
    return true;
  }
  // The background's operation in flight, so failed: its rejection runs.
  global.__exact_background_failed = function (outcomeJson) {
    if (!head || owner(head.call) !== background) return "";
    return failAbandonedImage(JSON.parse(outcomeJson).failed.message) ? "1" : "";
  };
  global.__exact_storage_failed = function (id, outcomeJson) {
    var call = calls.get(Number(id));
    if (!call) return;
    // The continuation will not land. Settle the head now, as a failed
    // operation, so the queue advances. `lost` first: the answer's own
    // rejection must not report success, and a later delivery of this same
    // operation is ignored.
    var message = JSON.parse(outcomeJson).failed.message;
    call.lost = true;
    // The code its message names (`compressImage: timeout: `), else `failed`.
    if (head && head.call === call) landed(head, false, storageError(message));
    call.status = "failed";
    storing.delete(call);
    call.error = storageError(message);
  };
  // The answer a timer belongs to (binary.js `timers`), or the refusal.
  function timerOwner(api) {
    if (overlaying) throw inOverlay(api);
    if (inMapper || !currentCall || currentCall === background || currentCall.replied || currentCall.letGo)
      throw new Error(api + " is unavailable here: a timer belongs to the answer that arms it, and none is running (module initialization, a stream's mapper, background work, or an answer that has given its value or was let go)");
    return currentCall;
  }
  // A timer's turn: the answer's earliest callback due by the runner's clock `at` runs in the drain that follows;
  // `due`: the runner says the timer it waits for is due (where it has no clock, at bake, the time it waited for).
  global.__exact_timer = function (id, at, due) { var call = calls.get(Number(id)); if (call) { currentCall = call; timers.fire(call, at, due === "due"); } };
  // A yielded answer's new turn: its yields resolve in the drain that follows.
  global.__exact_yielded = function (id) { var call = calls.get(Number(id)); if (call) { currentCall = call; endYields(call); } };
  global.__exact_turn = function (id) { var t = id === "module" ? work.MODULE : id ? calls.get(Number(id)) || null : null; currentCall = t === work.MODULE ? null : t; work.setTurn(t); }; global.__exact_module_discarded = function () { work.runDiscarded(); }; // the executor's turns (js/src/module_work.rs): the turn's call is current, so a plain `.then` reaction runs as it (LLP 1146 D2)
  global.__exact_adopt = function (id, ticket) { var p = pending.get(Number(ticket)); if (!p || !p.claimed || p.stream || p.module || p.call !== calls.get(Number(id))) return ""; p.module = true; work.adopted(); return "adopted"; }; // a let-go call's fetch with effects the runner keeps (LLP 1146 D2): only one handed out (`claimed`, the one the call parked on, so its abort is a cancel, door 23); forgetting the call leaves it in flight and outstanding until its reply reaches the call's continuation in a module turn
  // The child-process module's seam (LLP 1133.007), natively only: js/src/process.js takes it, and it is gone.
  if (global.__exact_process_host) global.__exact_process_seam = Object.freeze({ request: request, socketOwner: socketOwner, host: host, copyBytes: copyBytes, toBase64: toBase64 });
  global.__exact_fulfill = function (ticket, outcomeJson) {
    var p = settled(Number(ticket));
    if (!p) return;
    var o = JSON.parse(outcomeJson);
    // The continuation runs in the drain that follows, and a fetch it
    // makes belongs to this call.
    currentCall = p.module ? null : p.call; if (p.module) work.landed(); // an adopted fetch's: its let-go call's steps run as it (the async helper), a plain reaction as no answer
    // Its `AbortSignal.timeout`'s deadline passed on the host: the signal aborts.
    if (o.failed && o.failed.kind === "Timeout" && p.timed) timers.expire(p.signal);
    // A browser realm's abort listener can be stopped by an earlier one;
    // the signal itself still says so.
    if (p.signal && p.signal.aborted) p.reject(p.signal.reason);
    else if (o.failed) p.reject(new FetchError(o.failed));
    else p.resolve(new Response(o.response, p.bodyTo));
  };
})(globalThis);
