// The local listener (LLP 1133.005 D1): `exactListen(address, options)`,
// answered as a stream. Native only (macOS and the Linux host; §8): build.rs
// evaluates this before the prelude, which hands the kit its seam at its end
// and deletes it, so a target without a listener has no `exactListen` global
// and a source tests `"exactListen" in globalThis`. The listener is a
// streamed `fetch` of its `unix:` address with its options under the
// prelude's private key; each connection's handle is the prelude's socket
// owner for that connection, so sends follow LLP 1016.000 D7 (D6 here).
(function (global) {
  "use strict";
  var MAX_TIMEOUT = 600000, MAX_LINE = 1048576, MAX_CONNECTIONS = 64;
  global.__exact_listen_kit = function (seam) {
    if (seam.host(12, "listen", "") !== "1") return;
    var fetch = global.fetch; // the prelude's
    // An option within D1's bounds, or its default when absent.
    function bounded(o, name, low, high, fallback) {
      var v = o[name];
      if (v === undefined) return fallback;
      if (!Number.isInteger(v) || v < low || v > high) throw new TypeError("exactListen: " + name + " must be an integer from " + low + " to " + high);
      return v;
    }
    // The event the mapper sees, and the handle of its connection (null
    // for `listening` and `error`), for one outcome of listener `state`.
    function heard(state, o) {
      var m = o.message, id = m ? Number(m.id) || 0 : 0, owner = m ? state.owners.get(id) || null : null;
      if (m && m.event === "listening") {
        var bound = {};
        try { bound = JSON.parse(m.data); } catch (e) { /* the host's own */ }
        return { event: { type: "listening", address: bound.address, port: bound.port || 0 }, owner: null };
      }
      if (m && m.event === "connection") {
        owner = seam.socketOwner(state.ticket, false, id);
        owner.state = 1;
        owner.handle = handle(owner);
        state.owners.set(id, owner);
        return { event: { type: "connection", id: id }, owner: owner };
      }
      if (m && m.event === "message") return { event: { type: "message", id: id, data: m.data }, owner: owner };
      if (m && m.event === "end") {
        if (owner) owner.state = 3;
        state.owners.delete(id);
        return { event: { type: "end", id: id, reason: m.data }, owner: owner };
      }
      // The end of the listener: every handle closes.
      forget(state);
      var failed = o.failed || { kind: "Network", message: o.response ? "HTTP " + o.response.status : "the listener ended" };
      // The executor words a bound address's refusal `AddressInUse: …` (exact-listen).
      var named = /^(AddressInUse): ([\s\S]*)$/.exec(failed.message);
      return { event: { type: "error", kind: named ? named[1] : failed.kind, message: named ? named[2] : failed.message }, owner: null };
    }
    function forget(state) { state.owners.forEach(function (owner) { owner.state = 3; }); state.owners.clear(); }
    // `ListenConnection`: a line each `send` (with its `\n`), and `end()`
    // once what was sent before it is written.
    function handle(owner) {
      return Object.freeze({
        get readyState() { return owner.state; },
        send: function (text) {
          if (typeof text !== "string") throw new TypeError("connection.send expects a string: one line");
          if (text.indexOf("\n") >= 0) throw new SyntaxError("connection.send takes one line: the peer would read a \\n as two");
          owner.stage({ op: "line", data: text, size: seam.utf8Length(text) });
        },
        end: function () { owner.stage({ op: "end", size: 0 }); },
      });
    }
    global.exactListen = function exactListen(address, options) {
      var call = seam.call();
      if (call && call.mutation) throw new DOMException("exactListen() answers a resource: a mutation's listener would have no end", "InvalidStateError");
      if (typeof address !== "string") throw new TypeError("exactListen(address, options): the address is a string, unix:<absolute path>");
      var o = options || {};
      if (typeof o.exactStream !== "function") throw new TypeError("exactListen maps each event to the answer: { exactStream(event, connection) { … } }");
      if (o.takeover !== undefined && typeof o.takeover !== "boolean") throw new TypeError("exactListen: takeover is a boolean");
      var state = { ticket: 0, owners: new Map() };
      var init = { exactStream: o.exactStream, exactIndependentHttp: { maxResponseBytes: 4194304 } };
      init[seam.LISTEN] = {
        options: {
          takeover: o.takeover === true,
          first_line_timeout_ms: bounded(o, "firstLineTimeout", 0, MAX_TIMEOUT, 0),
          max_line_bytes: bounded(o, "maxLineBytes", 1, MAX_LINE, 65536),
          max_connections: bounded(o, "maxConnections", 1, MAX_CONNECTIONS, 16),
        },
        opened: function (ticket) {
          state.ticket = ticket;
          return { heard: function (o) { return heard(state, o); }, forget: function () { forget(state); } };
        },
      };
      return fetch(address, init);
    };
  };
})(globalThis);
