// The data seam's bytes, and the store's presence calls (below), for
// prelude.js: base64 for what crosses the seam
// as JSON, a string's UTF-8 length, a stream message's data, a long
// poll's terms as the seam carries them (LLP 1016.000 D8), an upload's
// option and events, a fetch's file
// options, an image call's options and an answer's timers (here, beside the
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
  // `exactUploadProgress` (LLP 1016.000 D10), checked as the runner and the web's
  // ts-stream.js check it: true for an upload whose stream reports its progress, nothing, or why it is refused.
  function uploadOf(init, stream, method, url) {
    var upload = init ? init.exactUploadProgress : undefined;
    if (upload === undefined || upload === false) return undefined;
    if (upload !== true) return "exactUploadProgress is true or false";
    if (!stream) return "exactUploadProgress reports an upload's progress to its stream: give exactStream too";
    if (/^wss?:/i.test(String(url))) return "exactUploadProgress: a WebSocket sends no body";
    if (/^[a-z][a-z0-9+.-]*:/i.test(String(url)) && !/^https?:/i.test(String(url))) return "exactUploadProgress: an upload is an http: or https: request";
    if (init.exactLongPoll !== undefined) return "exactUploadProgress: a long poll sends no body";
    if (method === "GET" || method === "HEAD") return "exactUploadProgress: a GET or HEAD request sends no body";
    return true;
  }
  // An upload's event for its mapper, as XMLHttpRequest names them: `progress` for the host's count of the body's
  // bytes sent, and `load` for the reply, whatever its status, which ends the stream.
  function uploadEvent(o) {
    if (o.response) return { type: "load", status: o.response.status, data: o.response.body, lastEventId: "", coalesced: 0 };
    var sent = JSON.parse(o.message.data);
    return { type: "progress", loaded: sent.loaded, total: sent.total, lengthComputable: true, data: "", lastEventId: "", coalesced: o.message.coalesced };
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
      } catch (e) {
        // The lock is read first: a refusal for want of a store names the operation called
        if (e && /^store\.locked: no store here/.test(e.message)) e.message = e.message.replace("store.locked", "store." + op);
        return Promise.reject(named(e));
      }
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

  // A resource's two-way socket (LLP 1016.000 D7): the handle its mapper
  // receives. Its state is what the mapper has seen: CONNECTING until
  // `open`, OPEN until the end, CLOSED once the stream ends or its answer
  // is let go. A mutation's synchronous source and the socket's own mapper
  // answering `open` or a `message` may send or close: each operation is
  // copied now, handed over with that turn's answer (`settle`,
  // `__exact_message`) and published once the commit that settles it stands.
  // An answer in flight's go to the host as each is made, in any turn of it (amendment of 2026-10-18).
  // A local listener's connections (LLP 1133.005 D6) take the same rules, each
  // connection a target of its own: its mapper sends only on the connection
  // whose `connection` or `message` event it answers. `env` is the prelude's
  // door and turn: the answer running, the mapper call, and any mapper call.
  var SOCKET_OPS = 256, SOCKET_BYTES = 1048576;
  function sockets(env) {
    var host = env.host;
    var closed = function (what) { return new DOMException("the " + (what || "socket") + " is not open", "InvalidStateError"); }, quota = function (what) { return new DOMException(what, "QuotaExceededError"); };
    // The transport's bytes as this turn began, earlier turns' unsent ones and
    // an answer's this turn (door op 15; none on wasm, whose handle never
    // opens), as WHATWG's `bufferedAmount` counts them at a task's start. A
    // listener's connections are bounded by the host (D3).
    function buffered(owner) { if (owner.listen) return 0; try { return Number(host(15, String(owner.ticket), "")) || 0; } catch (e) { return 0; } }
    function turn(owner) { var m = env.mapping(); return m && m.owner === owner ? m : env.current(); } // its own mapper's call, else the turn's
    // Stage `op` on `target` (the socket, or one connection of a listener),
    // whose `state` 1 is open; `byMutation`: its stream is a mutation's.
    function stage(owner, target, op, byMutation) {
      var call = turn(owner), mapping = env.mapping(), mapper = !!mapping && call === mapping, what = owner.listen ? "connection" : "socket";
      // A turn of an answer the runner still holds, in no mapper call: a resource's, or a mutation's after a source that answered with a promise.
      var answer = !mapper && !env.inMapper() && !!call && !call.replied && !call.letGo && (call.answer || call.mutation && !call.sync && call.status === "pending");
      if (!call || !mapper && !answer && (!call.mutation || !call.sync)) throw new Error(what + "." + op.op.replace("text", "send").replace("binary", "send").replace("line", "send") + " is only available in a mutation's synchronous source, its " + (owner.listen ? "listener" : "socket") + "'s own mapper or an answer in flight");
      if (owner.state !== 1 || target.state !== 1 || answer && byMutation) throw closed(what);
      if (mapper && owner.listen && mapping.connection !== target) throw closed(what);
      if (answer) return sendNow(owner, target, call, op);
      if (!mapper && call.tickets.length) { call.socketConflict = true; throw new Error(env.conflict); }
      var over = function (what) { var e = quota(what); if (mapper) call.overload = e; return e; }; // fails a mapper's snapshot, caught or not
      var ops = call.socketOps || (call.socketOps = []), bytes = op.size, mine = op.size, kind = mapper ? "mapper call" : "mutation";
      if (ops.length >= SOCKET_OPS) throw over("socket " + kind + " exceeds 256 operations");
      for (var i = 0; i < ops.length; i++) {
        if (ops[i].target === target && ops[i].ends) throw closed(what);
        bytes += ops[i].size;
        if (ops[i].owner === owner) mine += ops[i].size;
      }
      // The turn's payload as a whole; this socket's buffer with what it stages on it.
      if (bytes > SOCKET_BYTES) throw over("socket " + kind + " exceeds 1 MiB");
      if (buffered(owner) + mine > SOCKET_BYTES) throw over("socket buffer exceeds 1 MiB");
      op.owner = owner; op.target = target;
      ops.push(op);
    }
    // An answer's operation goes to the host now; D7's bounds count over all its turns.
    function sendNow(owner, target, call, op) {
      var sent = call.socketSent || (call.socketSent = { ops: 0, bytes: 0, closed: [] });
      if (sent.closed.indexOf(target) >= 0) throw closed(owner.listen ? "connection" : "socket");
      if (sent.ops >= SOCKET_OPS) throw quota("socket answer exceeds 256 operations");
      if (sent.bytes + op.size > SOCKET_BYTES) throw quota("socket answer exceeds 1 MiB");
      if (buffered(owner) + op.size > SOCKET_BYTES) throw quota("socket buffer exceeds 1 MiB");
      sent.ops++; sent.bytes += op.size;
      if (op.ends) sent.closed.push(target);
      host(17, String(owner.ticket), wire(op));
    }
    function wire(op) { return JSON.stringify({ op: op.op, data: op.data, code: op.code, reason: op.reason, connection: op.connection }); }
    function socketOwner(ticket, byMutation) {
      var owner = { ticket: ticket, state: 0 };
      owner.handle = Object.freeze({
        get readyState() { return owner.state; },
        get bufferedAmount() {
          var n = buffered(owner), call = turn(owner), ops = call && call.socketOps;
          for (var i = 0; ops && i < ops.length; i++) if (ops[i].owner === owner) n += ops[i].size;
          return n;
        },
        send: function (data) {
          if (typeof data === "string") return stage(owner, owner, { op: "text", data: data, size: utf8Length(data) }, byMutation);
          if (!(data instanceof ArrayBuffer) && !ArrayBuffer.isView(data)) throw new TypeError("socket.send expects a string, ArrayBuffer or ArrayBufferView");
          var bytes = new Uint8Array(env.copyBytes(data, "socket.send"));
          stage(owner, owner, { op: "binary", data: toBase64(bytes), size: bytes.length }, byMutation);
        },
        close: function (code, reason) {
          if (code === undefined) code = 1000; if (reason === undefined) reason = "";
          if (code !== 1000 && (!Number.isInteger(code) || code < 3000 || code > 4999)) throw new DOMException("invalid socket close code", "InvalidAccessError");
          if (typeof reason !== "string" || utf8Length(reason) > 123) throw new SyntaxError("socket close reason exceeds 123 UTF-8 bytes");
          stage(owner, owner, { op: "close", code: code, reason: reason, size: 0, ends: true }, byMutation);
        },
      });
      return owner;
    }
    // A listener (LLP 1133.005 D1): its state is the stream's (OPEN from
    // `listening`), and each connection's handle is OPEN from its
    // `connection` event to its `end`, CLOSED once the stream ends.
    function listenOwner(ticket) {
      var owner = { ticket: ticket, state: 0, listen: true, connections: new Map() };
      // The connection `id`'s handle, made at its `connection` event.
      owner.connection = function (id) {
        var c = owner.connections.get(id);
        if (c) return c;
        c = { id: id, state: 1 };
        c.handle = Object.freeze({
          get readyState() { return c.state; },
          send: function (text) {
            if (typeof text !== "string") throw new TypeError("ListenConnection.send expects a string");
            // The peer would read a line with a newline as two.
            if (text.indexOf("\n") >= 0) throw new SyntaxError("ListenConnection.send: a line has no \\n");
            stage(owner, c, { op: "line", connection: id, data: text, size: utf8Length(text) + 1 }, false);
          },
          end: function () { stage(owner, c, { op: "end", connection: id, size: 0, ends: true }, false); },
        });
        owner.connections.set(id, c);
        return c;
      };
      // The stream's end, or its answer let go: every handle is closed.
      owner.close = function () { owner.state = 3; owner.connections.forEach(function (c) { c.state = 3; }); owner.connections.clear(); };
      return owner;
    }
    // One message for a listener's mapper: the event and the connection
    // handle beside it (none for `listening`). `end` closes that handle.
    function listenEvent(owner, message) {
      var type = message.event, id = Number(message.id), c = type === "listening" ? null : owner.connection(id);
      if (type === "listening") { owner.state = 1; return { event: { type: type, address: message.data, port: id || 0 } }; }
      if (type === "end") { c.state = 3; owner.connections.delete(id); return { event: { type: type, id: id, reason: message.data }, connection: c }; }
      if (type === "connection") return { event: { type: type, id: id }, connection: c };
      return { event: { type: "message", id: id, data: message.data }, connection: c };
    }
    // A turn's staged operations to the host, with its answer.
    function hand(call, door) { // door 17: a promise-answering mutation's first answer turn, counted toward its bounds
      var ops = call.socketOps || [], sent = door === 17 && (call.socketSent = { ops: ops.length, bytes: 0, closed: [] }); call.socketOps = null;
      ops.forEach(function (op) { if (sent) { sent.bytes += op.size; if (op.ends) sent.closed.push(op.target); } host(door || 14, String(op.owner.ticket), wire(op)); });
    }
    return { socketOwner: socketOwner, listenOwner: listenOwner, listenEvent: listenEvent, hand: hand };
  }
  // An answer's own timers (LLP 1027.000 D1 as amended 2026-10-28), for
  // prelude.js, here under its line cap. `setTimeout` arms one due on the
  // runner's clock as the call began (`clock`, host door op 21), owned by the
  // answer running (`owner`, which refuses one outside an answer). An answer
  // that awaits only its timers parks on the earliest (`wait`), and the
  // runner asks it again once its clock is there (`fire`): one callback a
  // turn, so the microtasks each queues run before the next, as a browser
  // runs them between tasks. An answer that gives its value or is let go
  // clears the rest (`drop`): their callbacks never run. `setInterval`, the
  // frame callbacks and the clock reads stay refused. Hermes's rejection
  // tracker schedules its report with `setTimeout` too: that one runs alone,
  // at the prelude's next `checkpoint`, which is when a browser fires
  // `unhandledrejection`.
  function timers(owner, clock, journal, report, sources) {
    var armed = new Map(), reports = [], signals = new WeakMap(), next = 1, now = 0;
    function fixed(name, value) { Object.defineProperty(global, name, { value: value, writable: false, configurable: false }); }
    // The runner's clock as the call began; where none is (at bake), the last time a timer ran at. Not kept
    // monotonic: a runner that restarts at zero beside this realm (a dev reload) is the clock from then on.
    function time() { var c = clock(), t = Number(c); if (c !== undefined && c !== "" && t === t) now = t; return now; }
    function arm(api, run, ms, args) {
      var call = owner(api), id = next++;
      armed.set(id, { call: call, due: Math.ceil(time() + ms), run: run, args: args });
      return id;
    }
    fixed("setTimeout", function setTimeout(callback, delay) {
      if (typeof callback === "function" && report(callback)) { reports.push({ id: next, run: callback }); return next++; }
      if (typeof callback !== "function") throw new TypeError("setTimeout(): the callback must be a function; a string of code is never run");
      return arm("setTimeout()", callback, Math.max(Number(delay) | 0, 0), Array.prototype.slice.call(arguments, 2)); // `long`, as WebIDL converts it
    });
    fixed("clearTimeout", function clearTimeout(id) {
      armed.delete(id);
      reports = reports.filter(function (r) { return r.id !== id; });
    });
    ["setInterval", "requestAnimationFrame", "requestIdleCallback"].forEach(function (name) {
      fixed(name, function () { throw new Error(name + "() is unavailable in data sources: an answer waits with setTimeout; pass time as an argument"); });
    });
    ["clearInterval", "cancelAnimationFrame", "cancelIdleCallback"].forEach(function (name) { fixed(name, function () {}); });
    if (global.AbortSignal) global.AbortSignal.timeout = function timeout(ms) {
      var n = Number(ms);
      if (!(n >= 0 && n <= 9007199254740991)) throw new TypeError("AbortSignal.timeout(): the delay is a number of milliseconds from 0");
      var controller = new global.AbortController();
      var abort = function () { controller.abort(new global.DOMException("signal timed out", "TimeoutError")); };
      var id = arm("AbortSignal.timeout()", abort, Math.floor(n), []);
      // Its deadline outlives the timer, which its answer's end drops: a request carrying it still has it.
      signals.set(controller.signal, { id: id, due: armed.get(id).due, abort: abort });
      return controller.signal;
    };
    // A timeout signal's deadline, or the earliest among the signals one was composed of (`sources`).
    function deadline(signal) {
      var s = signal && signals.get(signal), composed = !s && signal && sources(signal);
      if (composed) composed.forEach(function (input) { var t = signals.get(input); if (t && (!s || t.due < s.due)) s = t; });
      return s;
    }
    function first(call, by) {
      var found;
      armed.forEach(function (t, id) { if (t.call === call && t.due <= by && (!found || t.due < found[1].due)) found = [id, t]; });
      return found;
    }
    return {
      // The earliest of `call`'s timers: when it is due, or undefined.
      wait: function (call) { var t = first(call, Infinity); return t ? (call.timerDue = t[1].due) : undefined; },
      armed: function (call) { return first(call, Infinity) !== undefined; },
      now: time,
      // `call`'s earliest timer due by the clock `at` runs: the runner's, or,
      // where none is (at bake), the last time a timer ran at. `due`: the
      // runner says the timer it waited for is due, so that one runs.
      fire: function (call, at, due) {
        var t = Number(at);
        if (at !== "" && t === t) now = t;
        if (due) now = Math.max(now, call.timerDue || 0);
        t = first(call, now);
        if (!t) return;
        armed.delete(t[0]);
        try { t[1].run.apply(undefined, t[1].args); }
        catch (e) { journal("data: a timer's callback threw: " + (e && e.message !== undefined ? e.message : String(e))); }
      },
      drop: function (call) { armed.forEach(function (t, id) { if (t.call === call) armed.delete(id); }); },
      // An `AbortSignal.timeout` still to fire, or one composed of it: the ms
      // left, the deadline a fetch carrying it gives the host; `expire` aborts
      // it when that passes, its timer dropped or not, as a browser's fires:
      // a fetch its answer let go runs to its end (LLP 1146 D2), and another
      // answer may carry the same signal.
      left: function (signal) { var s = deadline(signal); return s && !signal.aborted ? Math.max(1, s.due - time()) : undefined; },
      expire: function (signal) { var s = deadline(signal); if (s && !signal.aborted) { armed.delete(s.id); s.abort(); } },
      checkpoint: function () {
        while (reports.length) {
          var report = reports.shift();
          try { report.run(); } catch (e) { /* the tracker's own; nothing to tell */ }
        }
      },
    };
  }
  global.__exact_binary = Object.freeze({
    timers: timers,
    fileRefusal: fileRefusal,
    bodyWritten: bodyWritten,
    imageOptions: imageOptions,
    fromBase64: fromBase64,
    toBase64: toBase64,
    utf8Length: utf8Length,
    messageData: messageData,
    gateDecode: door ? gateDecode : undefined,
    longPollOf: longPollOf,
    uploadOf: uploadOf,
    uploadEvent: uploadEvent,
    presence: presence,
    sockets: sockets,
  });
})(globalThis);

// Work that outlives its answer (LLP 1146 D2): a fetch with effects whose
// answer was let go is adopted, not rejected (`__exact_adopt`), and its reply
// reaches the let-go answer's continuation in a module turn, which the
// executor runs outside every answer and commit (js/src/module_work.rs), so
// what that continuation writes to the store stands on its own. A live
// answer's step that such a turn settles runs in that answer's own turn, so
// its reads, writes and fetches are its own. prelude.js takes
// `__exact_module_work` and deletes it; it is here, in binary.js, so that
// every realm that evaluates the prelude has it.
(function (global) {
  "use strict";
  // The prelude's hooks: `host` (the door) and `live(call)` (a call still answering).
  global.__exact_module_work = function (hooks) {
    var host = hooks.host;
    // The turn running now, by its call's id: an answer's, MODULE's, or null
    // (a let-go chain, a background delivery, module evaluation). An id, so
    // that no ended call's value is kept alive by it.
    var MODULE = Object.freeze({ id: -1 });
    var turn = null;
    var ready = new Map();           // live answer's call -> [steps for its own turn]
    var discarded = [];              // steps of answers let go before their turn, run with no store
    var adopted = 0;                 // adopted fetches in flight
    function waiting() { var n = 0; ready.forEach(function (steps) { n += steps.length; }); return n; }
    // What may still settle an answer waiting on module work, for the executor.
    function note() { host(22, "state", JSON.stringify({ outstanding: adopted + waiting() })); }
    return Object.freeze({
      MODULE: MODULE,
      // The turn now (the executor's `__exact_turn`): an answer's call, MODULE or null.
      setTurn: function (t) {
        turn = t ? t.id : null;
        var steps = t && t !== MODULE ? ready.get(t) : null;
        if (steps) { ready.delete(t); steps.forEach(function (s) { s(); }); note(); }
      },
      // The async helper asks before each step whether it runs here. In a
      // module turn a live answer's step waits for that answer's own turn;
      // anything else runs where it is, the let-go answer's continuation
      // included. `step(key, arg)` is the helper's next step, bound only when
      // deferred: this runs before every step of every async function.
      route: function (at, step, key, arg) {
        if (!at || turn !== MODULE.id || !hooks.live(at)) return false;
        if (!ready.has(at)) ready.set(at, []);
        ready.get(at).push(function () { step(key, arg); });
        note();
        return true;
      },
      // After a module turn, with no store: what let-go answers had waiting.
      runDiscarded: function () {
        turn = null;
        for (var n = 0; discarded.length && n < 1024; n++) discarded.shift()();
      },
      // An answer let go: its steps waiting for its turn move to the steps
      // the let-go chain runs with no store, and stop counting as
      // outstanding. How many moved.
      forgot: function (call) {
        var steps = ready.get(call);
        if (!steps) return 0;
        ready.delete(call);
        steps.forEach(function (s) { discarded.push(s); });
        note();
        return steps.length;
      },
      inModuleTurn: function () { return turn === MODULE.id; },
      adopted: function () { adopted++; note(); },
      landed: function () { adopted--; note(); },
      outstanding: function () { return adopted + waiting() > 0; },
    });
  };
})(globalThis);
