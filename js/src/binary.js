// The data seam's bytes, and the store's presence calls (below), for
// prelude.js: base64 for what crosses the seam
// as JSON, a string's UTF-8 length, and a stream message's data. Natively a
// binary message (a byte stream's bytes, a socket's frame; LLP 1016.000)
// names only its size, and the host fills an ArrayBuffer of it through the
// bytes door (op 9, js/src/crypto.rs): base64 decoded byte by byte in the
// interpreter cost 1.8 ms a 12 KB message. Evaluated just before prelude.js,
// which takes `__exact_binary` and deletes it: natively, and on the web in
// the module realm's iframe (module-glue.js) or its worker (module-worker.js).
(function (global) {
  "use strict";
  var door = global.__exact_bytes; // natively; prelude.js takes it from the global
  function fromBase64(text) {
    return Uint8Array.from(global.atob(text), function (c) { return c.charCodeAt(0); });
  }
  var BASE64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  function toBase64(bytes) {
    var out = [], n = bytes.length, i = 0, v;
    for (; i + 2 < n; i += 3) {
      v = bytes[i] << 16 | bytes[i + 1] << 8 | bytes[i + 2];
      out.push(BASE64[v >> 18 & 63] + BASE64[v >> 12 & 63] + BASE64[v >> 6 & 63] + BASE64[v & 63]);
    }
    if (i < n) {
      v = bytes[i] << 16 | (i + 1 < n ? bytes[i + 1] << 8 : 0);
      out.push(BASE64[v >> 18 & 63] + BASE64[v >> 12 & 63] + (i + 1 < n ? BASE64[v >> 6 & 63] : "=") + "=");
    }
    return out.join("");
  }
  // A string's UTF-8 bytes as TextEncoder makes them: a pair is 4, a lone
  // surrogate its replacement character's 3.
  function utf8Length(s) {
    var n = 0;
    for (var i = 0; i < s.length; i++) {
      var c = s.charCodeAt(i);
      if (c < 0x80) n += 1;
      else if (c < 0x800) n += 2;
      else if (c >= 0xd800 && c < 0xdc00 && i + 1 < s.length && (s.charCodeAt(i + 1) & 0xfc00) === 0xdc00) { n += 4; i++; }
      else n += 3;
    }
    return n;
  }
  // A stream message's `data`: the bytes the host fills, base64 where they
  // crossed as JSON (the web's module realm, a worker's turn), or text.
  function messageData(message) {
    if (message.size !== undefined) {
      var bytes = new ArrayBuffer(message.size);
      door(9, "", bytes);
      return bytes;
    }
    return message.binary !== undefined ? fromBase64(message.binary).buffer : message.data;
  }
  // LLP 1133.003 §9's gate, until `VideoDecoder`: a chunk's bytes copied
  // into the host and dropped (bytes door op 8). prelude.js installs it as
  // `__exact_gate_decode` under the agent only.
  function gateDecode(track, type, timestamp, data) {
    door(8, data.byteOffset + " " + data.byteLength, data.buffer);
  }
  // The store's presence calls (LLP 1133.010 D5), for prelude.js's `store`,
  // over its host door: a name's occupancy (op 18 kept, 19 locked or
  // pending, 20 this device keeps locked secrets now), and a call that waits
  // on the host's backend as a request of its own, whose reply carries an
  // unlocked value to its promise alone, never through the store. A refusal
  // whose message names its error (`InvalidAccessError: …`) carries that name.
  function presence(host, get, refuse) {
    // Own properties: a hardened realm's frozen `Error.prototype` refuses an
    // assigned `name`.
    function as(e, name) {
      Object.defineProperty(e, "name", { value: name, writable: true, configurable: true });
      Object.defineProperty(e, "code", { value: name, writable: true, enumerable: true, configurable: true });
      return e;
    }
    function named(e) {
      var m = /^(InvalidAccessError|NotSupportedError): /.exec(e && e.message);
      return m ? as(e, m[1]) : e;
    }
    function call(op, name, options) {
      name = String(name);
      try {
        refuse("store." + op + "()");
        var locked = host(19, name, "") === "1";
        if (op === "unlock" && !locked) return Promise.resolve(get(name));
        if (op === "unprotect" && !locked || op === "protect" && locked) return Promise.resolve();
        if (op === "protect" && host(20, "", "") !== "1") throw new Error("NotSupportedError: " + name + " cannot be locked: this device keeps no locked secret now");
      } catch (e) { return Promise.reject(named(e)); }
      var body = JSON.stringify({ presence: op, name: name, reason: options && options.reason != null ? String(options.reason) : "" });
      return global.fetch("exact-native:", { method: "POST", body: body, exactIndependentHttp: { maxResponseBytes: 1048576 } })
        .then(function (r) {
          return r.text().then(function (t) {
            var v = r.status === 200 ? JSON.parse(t) : { error: "UnknownError", message: t };
            if (v.error) throw as(new Error(v.message || v.error), v.error);
            return op === "unlock" ? (v.value == null ? null : v.value) : undefined;
          });
        });
    }
    return Object.freeze({
      door: function (op, a, b) { try { return host(op, a, b); } catch (e) { throw named(e); } },
      has: function (name) { refuse("store.has()"); return host(18, String(name), "") === "1"; },
      locked: function (name) { refuse("store.locked()"); return host(19, String(name), "") === "1"; },
      unlock: function (name, options) { return call("unlock", name, options); },
      protect: function (name) { return call("protect", name); },
      unprotect: function (name, options) { return call("unprotect", name, options); },
    });
  }
  global.__exact_binary = Object.freeze({
    fromBase64: fromBase64,
    toBase64: toBase64,
    utf8Length: utf8Length,
    messageData: messageData,
    gateDecode: door ? gateDecode : undefined,
    presence: presence,
  });
})(globalThis);
