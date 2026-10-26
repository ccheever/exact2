// The data seam's bytes, and the store's presence calls (below), for
// prelude.js: base64 for what crosses the seam
// as JSON, a string's UTF-8 length, a stream message's data, a long
// poll's terms as the seam carries them (LLP 1016.000 D8), a fetch's file
// options, and an image call's options (here, beside the prelude, under its
// line cap). Natively a
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
  // `exactLongPoll` (LLP 1016.000 D8), checked as the web's ts-stream.js checks it (names in code points, `from` a member
  // or two joined by one `.`): the op's `long_poll`, nothing when there is none, or why it is refused.
  function longPollOf(init, stream, method, url) {
    var poll = init ? init.exactLongPoll : undefined, name = function (k) { var v = poll[k], n = typeof v === "string" ? Array.from(v).length : 0; return n > 0 && n <= 256 ? v : null; };
    if (poll === undefined) return undefined;
    if (!stream) return "exactLongPoll asks again for an answer that keeps coming: give exactStream too";
    if (/^wss?:/i.test(String(url))) return "exactLongPoll: a long poll is HTTP, not a WebSocket";
    if (method !== "GET") return "exactLongPoll: a long poll asks with GET";
    if (poll === null || typeof poll !== "object") return "exactLongPoll is { cursor, from }";
    if (!name("cursor")) return "exactLongPoll.cursor is the query parameter the cursor goes in, a name";
    var parts = name("from") ? poll.from.split(".") : [];
    if (!parts.length || parts.length > 2 || parts.some(function (p) { return !p; })) return "exactLongPoll.from is the reply's field the cursor comes from, a name, or two joined by one `.`";
    if (poll.sameCursor !== undefined && poll.sameCursor !== "heartbeat" && poll.sameCursor !== "event") return "exactLongPoll.sameCursor is \"heartbeat\" (the default) or \"event\"";
    return { cursor: poll.cursor, from: poll.from, same_cursor_event: poll.sameCursor === "event" ? true : undefined };
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
  // The image calls' options (prelude.js `imageCall`), checked before
  // anything is queued, as the host's arguments after `from` and `to`:
  // `compressImage`'s two numbers and type (LLP 1069.002 A1, A1.11), then its
  // edit, and `transformImage`'s all, each as the one JSON value the codec
  // reads (LLP 1133.019 D17).
  var IMAGE_WORDS = { flip: ["horizontal", "vertical"], resizeQuality: ["pixelated", "low", "medium", "high"], metadata: ["keep", "no-location", "none"] };
  function imageOptions(api, options, transform) {
    var out = {}, crop;
    function fail(says) { throw new TypeError(api + ": " + says); }
    function take(name, ok, says) {
      var value = options[name];
      if (value === undefined) return;
      if (!ok(value)) fail(name + " must be " + says);
      out[name] = value;
    }
    function word(name) { take(name, function (w) { return IMAGE_WORDS[name].indexOf(w) >= 0; }, IMAGE_WORDS[name].map(function (w) { return JSON.stringify(w); }).join(" or ")); }
    function finite(n) { return typeof n === "number" && isFinite(n); }
    function side(n) { return Number.isInteger(n) && n >= 1 && n <= 16384; }
    if (transform ? options !== undefined && (!options || typeof options !== "object") : !options || typeof options !== "object") fail(transform ? "options must be an object" : "options must be {maxDimension, maxBytes}");
    options = options || {};
    crop = options.crop;
    take("rotate", finite, "a finite number of degrees");
    word("flip");
    if (crop !== undefined) {
      if (!crop || typeof crop !== "object" || ![crop.x, crop.y, crop.width, crop.height].every(finite) || !(crop.width > 0 && crop.height > 0)) fail("crop must be {x, y, width, height}, finite, with a positive width and height");
      out.crop = { x: crop.x, y: crop.y, width: crop.width, height: crop.height };
    }
    if (!transform) {
      var dimension = options.maxDimension, bytes = options.maxBytes, type = options.type === undefined ? "image/jpeg" : options.type;
      if (!Number.isInteger(dimension) || dimension < 1 || dimension > 8192) fail("maxDimension must be an integer from 1 to 8192");
      if (!Number.isInteger(bytes) || bytes < 1 || bytes > 67108864) fail("maxBytes must be an integer from 1 to 67108864");
      if (type !== "image/jpeg" && type !== "image/png") fail("type must be \"image/jpeg\" or \"image/png\"");
      return [dimension, bytes, type].concat(Object.keys(out).length ? [JSON.stringify(out)] : []);
    }
    take("resizeWidth", side, "an integer from 1 to 16384");
    take("resizeHeight", side, "an integer from 1 to 16384");
    word("resizeQuality");
    take("type", function (t) { return typeof t === "string" && /^image\/[a-z0-9.+-]+$/.test(t); }, "a media type such as \"image/jpeg\"");
    take("quality", function (q) { return typeof q === "number" && q >= 0 && q <= 1; }, "a number from 0 to 1");
    word("metadata");
    return [JSON.stringify(out)];
  }
  // `exactBodyFrom` and `exactBodyTo` (LLP 1108 D6 R2), checked as the web's
  // ts-body-from.js checks them: a refusal's words, or undefined.
  function filePath(name, path) {
    if (typeof path !== "string" || path.slice(0, 5) !== "app:/") return name + " must be an app:/ path";
    if (utf8Length(path) > 4096) return name + ": a path is at most 4096 bytes";
    if (/(^|\/)\.\.?(\/|$)|\0/.test(path.slice(5))) return name + ": an app:/ path has no . or .. segment";
  }
  function fileRefusal(init, method, url, raw) {
    var from = init ? init.exactBodyFrom : undefined, to = init ? init.exactBodyTo : undefined, why;
    if (from !== undefined) {
      if ((why = filePath("exactBodyFrom", from))) return why;
      if (method === "GET" || method === "HEAD") return "fetch: a " + method + " request cannot have a body";
      if (raw != null) return "fetch: a request has one body: body or exactBodyFrom";
      if (init.exactStream !== undefined && /^wss?:/i.test(String(url))) return "exactBodyFrom: a WebSocket sends no body";
    }
    if (to !== undefined) {
      if ((why = filePath("exactBodyTo", to))) return why;
      if (init.exactStream !== undefined) return "exactBodyTo: a stream's body is its messages, not a file";
      if (method === "HEAD") return "exactBodyTo: a HEAD reply has no body to write";
    }
  }
  // The bytes `exactBodyTo` wrote, from the host's `:exact-body-to` pair
  // (the runner's `BODY_WRITTEN`), taken out of the reply's headers: no HTTP
  // header name has a colon. Undefined when the body was not written.
  function bodyWritten(reply) {
    for (var i = 0; i < reply.headers.length; i++)
      if (reply.headers[i][0] === ":exact-body-to") return Number(reply.headers.splice(i, 1)[0][1]);
  }
  global.__exact_binary = Object.freeze({
    fileRefusal: fileRefusal,
    bodyWritten: bodyWritten,
    imageOptions: imageOptions,
    fromBase64: fromBase64,
    toBase64: toBase64,
    utf8Length: utf8Length,
    messageData: messageData,
    gateDecode: door ? gateDecode : undefined,
    longPollOf: longPollOf,
    presence: presence,
  });
})(globalThis);
