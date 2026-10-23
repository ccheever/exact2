// The data seam's bytes, for prelude.js: base64 for what crosses the seam
// as JSON, a string's UTF-8 length, a stream message's data, and a long
// poll's terms as the seam carries them (LLP 1016.000 D8; here, beside the
// prelude, under its line cap). Natively a
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
  global.__exact_binary = Object.freeze({
    fromBase64: fromBase64,
    toBase64: toBase64,
    utf8Length: utf8Length,
    messageData: messageData,
    gateDecode: door ? gateDecode : undefined,
    longPollOf: longPollOf,
  });
})(globalThis);
