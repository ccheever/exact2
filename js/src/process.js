// @exact/child-process (LLP 1133.007): Node's `child_process` names over
// Exact's streams. The package's binding re-exports `__exact_child_process`,
// built here from the prelude's seam (js/src/prelude.js), which only a
// native executor whose host starts programs installs: macOS and Linux
// (`__exact_process_host`, js/build.rs). Everywhere else every name rejects
// NotSupportedError and `processStatus().available` is false (§8).
//
// A child is a stream resource (D2): `spawn` is a process request to
// `exact-process:` that the host answers with ordered events, and its
// handle stages stdin writes, ends and kills on the stream as a socket's
// sends are staged (LLP 1016.000 D7), each addressed to its incarnation.
(function (global) {
  var seam = global.__exact_process_seam, os = global.__exact_process_host;
  delete global.__exact_process_seam; delete global.__exact_process_host;
  var available = !!seam && (os === "macos" || os === "linux");
  var HIGH_WATER = 65536, SIGNALS = ["SIGTERM", "SIGINT", "SIGHUP", "SIGQUIT", "SIGUSR1", "SIGUSR2", "SIGSTOP", "SIGCONT", "SIGKILL"];
  var REFUSED = { shell: "a shell command line runs any program, which no grant could bound: name the program", detached: "a daemon that outlives the app is a login item's", uid: "another user's identity is out", gid: "another user's identity is out", windowsHide: "a child never shows a console", argv0: "a child's argv[0] is its program", serialization: "there is no IPC channel" };
  var WORDS = ["apple", "linux", "windows"];

  function coded(message, code, name) {
    var e = new Error(message);
    Object.defineProperties(e, { code: { value: code, configurable: true }, name: { value: name || "Error", configurable: true } });
    return e;
  }
  function absent(api) { return coded(api + ": NotSupportedError: this platform starts no programs (LLP 1133.007)", "NotSupportedError", "NotSupportedError"); }
  function invalid(message) { return new DOMException(message, "InvalidStateError"); }

  // The options every call shares, checked and written as the host reads them.
  function common(api, options) {
    var o = options || {}, body = {};
    if (typeof o !== "object") throw new TypeError(api + ": options must be an object");
    Object.keys(REFUSED).forEach(function (k) { if (o[k] !== undefined && o[k] !== false) throw new TypeError(api + ": `" + k + "` is refused: " + REFUSED[k]); });
    WORDS.forEach(function (k) { if (o[k] !== undefined) throw new TypeError(api + ": `" + k + "` is declared and not built yet (platform-word-later)"); });
    if (o.cwd !== undefined) { if (typeof o.cwd !== "string") throw new TypeError(api + ": cwd must be a path"); body.cwd = o.cwd; }
    if (o.env !== undefined && o.env !== null) {
      if (typeof o.env !== "object") throw new TypeError(api + ": env must be an object of strings");
      body.env = Object.keys(o.env).filter(function (k) { return o.env[k] !== undefined; }).map(function (k) { return [k, String(o.env[k])]; });
    }
    if (o.encoding !== undefined && o.encoding !== "utf8" && o.encoding !== "utf-8" && o.encoding !== "buffer") throw new TypeError(api + ": encoding is \"utf8\" or \"buffer\"");
    body.encoding = o.encoding === undefined || o.encoding === "buffer" ? "buffer" : "utf8";
    if (o.killSignal !== undefined) { if (SIGNALS.indexOf(o.killSignal) < 0) throw new TypeError(api + ": killSignal: " + SIGNALS.join(", ")); body.killSignal = o.killSignal; }
    if (o.exactStdinFrom !== undefined) { if (typeof o.exactStdinFrom !== "string") throw new TypeError(api + ": exactStdinFrom must be an app:/ or home:/ path"); body.stdinFrom = o.exactStdinFrom; }
    return body;
  }
  function millis(api, name, value, max) {
    if (value === undefined) return undefined;
    if (!Number.isInteger(value) || value < 0 || value > max) throw new TypeError(api + ": " + name + " must be an integer number of milliseconds from 0 to " + max);
    return value;
  }
  function argv(api, file, args) {
    if (typeof file !== "string" || !file) throw new TypeError(api + ": the program is a string, as its grant line names it");
    if (args !== undefined && (!Array.isArray(args) || args.some(function (a) { return typeof a !== "string"; }))) throw new TypeError(api + ": args must be an array of strings");
    return args ? args.slice() : [];
  }

  // A child's events, as its mapper reads them, and its handle per incarnation (D2, D4).
  function childOwner(stdinIgnored) {
    return function (ticket, call) {
      var owner = seam.socketOwner(ticket, call.mutation), incarnation = 0, child = null;
      function handle(n, pid) {
        var state = { exitCode: null, signalCode: null, ended: stdinIgnored, over: false };
        function live(api) {
          if (owner.state !== 1 || incarnation !== n || child !== state) throw invalid("child." + api + ": the child's incarnation has ended");
          if (api !== "kill" && state.ended) throw invalid("child." + api + ": stdin is " + (stdinIgnored ? "ignored" : "ended"));
        }
        state.handle = Object.freeze({
          get pid() { return pid; },
          get exitCode() { return state.exitCode; },
          get signalCode() { return state.signalCode; },
          get writableLength() { return owner.amount(); },
          write: function (data) {
            live("write");
            var bytes;
            if (typeof data === "string") bytes = new TextEncoder().encode(data);
            else if (data instanceof ArrayBuffer || ArrayBuffer.isView(data)) bytes = new Uint8Array(seam.copyBytes(data, "child.write"));
            else throw new TypeError("child.write expects a string, ArrayBuffer or ArrayBufferView");
            // Past the mark, the host says `drain` once stdin's queue is empty (D2, as Node's Writable).
            var ok = owner.amount() + bytes.length <= HIGH_WATER;
            owner.stage({ op: "write", size: bytes.length, wire: { op: "write", data: seam.toBase64(bytes), incarnation: n, drain: ok ? undefined : true } });
            return ok;
          },
          end: function () {
            live("end");
            owner.stage({ op: "end", size: 0, wire: { op: "end", incarnation: n } });
            state.ended = true;
          },
          kill: function (signal) {
            live("kill");
            if (signal === undefined) signal = "SIGTERM";
            if (SIGNALS.indexOf(signal) < 0) throw new TypeError("child.kill: " + SIGNALS.join(", "));
            owner.stage({ op: "kill", size: 0, wire: { op: "kill", signal: signal, incarnation: n } });
            return true;
          },
        });
        return state;
      }
      owner.handle = handle(0, null).handle;
      owner.event = function (o, event) {
        var data = {};
        if (o.message && typeof o.message.data === "string" && o.message.data && o.message.event !== "stdout" && o.message.event !== "stderr") {
          try { data = JSON.parse(o.message.data); } catch (e) { data = {}; }
        }
        if (o.closed) { try { data = JSON.parse(o.closed.reason); } catch (e) { data = {}; } }
        if (o.message && o.message.event === "spawn") {
          incarnation = data.incarnation || incarnation + 1;
          child = handle(incarnation, data.pid === undefined ? null : data.pid);
          owner.handle = child.handle;
          owner.state = 1;
          return { type: "spawn", pid: child.handle.pid, restarts: data.restarts || 0 };
        }
        if (o.message && (o.message.event === "stdout" || o.message.event === "stderr")) return { type: o.message.event, data: event.data };
        if (o.message && o.message.event === "exit") {
          if (child) { child.exitCode = data.code === undefined ? null : data.code; child.signalCode = data.signal || null; }
          return { type: "exit", code: data.code === undefined ? null : data.code, signal: data.signal || null };
        }
        if (o.message && o.message.event === "close" || o.closed) {
          // The incarnation ends: its handle refuses from here, and a restart's spawn brings the next.
          owner.state = o.closed ? 3 : 0;
          child = null;
          return { type: "close", code: data.code === undefined ? null : data.code, signal: data.signal || null, drained: data.drained !== false };
        }
        if (o.message) return { type: o.message.event };
        owner.state = 3;
        child = null;
        return event.type === "error" ? { type: "error", kind: event.kind, message: event.message } : event;
      };
      return owner;
    };
  }

  function spawn(file, args, options) {
    if (args !== undefined && !Array.isArray(args)) { options = args; args = undefined; }
    if (!available) return Promise.reject(absent("spawn"));
    var o = options || {}, body;
    try {
      args = argv("spawn", file, args);
      body = common("spawn", o);
      if (typeof o.exactStream !== "function") throw new TypeError("spawn: exactStream maps each event to the answer: (event, child) => value");
      var stdio = o.stdio === undefined ? ["pipe", "pipe", "pipe"] : typeof o.stdio === "string" ? [o.stdio, o.stdio, o.stdio] : o.stdio;
      if (!Array.isArray(stdio) || stdio.length > 3 || stdio.some(function (s) { return s !== "pipe" && s !== "ignore"; }))
        throw new TypeError("spawn: stdio entries are \"pipe\" or \"ignore\": a child inherits no terminal, and has no IPC channel or extra pipes");
      while (stdio.length < 3) stdio.push("pipe");
      body.kind = "spawn"; body.program = file; body.args = args; body.stdio = stdio;
      body.forceKillAfter = millis("spawn", "exactForceKillAfter", o.exactForceKillAfter, 600000);
      body.drainTimeout = millis("spawn", "exactDrainTimeout", o.exactDrainTimeout, 600000);
      if (o.exactRestart !== undefined) {
        var r = o.exactRestart || {};
        if (r.when !== "always" && r.when !== "on-failure") throw new TypeError("spawn: exactRestart.when is \"always\" or \"on-failure\"");
        if (r.preventExitStatus !== undefined && (!Array.isArray(r.preventExitStatus) || r.preventExitStatus.some(function (c) { return !Number.isInteger(c); }))) throw new TypeError("spawn: exactRestart.preventExitStatus is a list of exit codes");
        body.restart = { when: r.when, delay: millis("spawn", "exactRestart.delay", r.delay, 3600000), maxDelay: millis("spawn", "exactRestart.maxDelay", r.maxDelay, 3600000), preventExitStatus: r.preventExitStatus || [] };
      }
      if (o.exactReady !== undefined) {
        var y = o.exactReady || {};
        if (typeof y.url !== "string") throw new TypeError("spawn: exactReady.url is the URL to poll");
        if (y.accept !== undefined && (!Array.isArray(y.accept) || y.accept.length !== 2)) throw new TypeError("spawn: exactReady.accept is [lowest, highest] status");
        if (y.onTimeout !== undefined && y.onTimeout !== "retry" && y.onTimeout !== "fail") throw new TypeError("spawn: exactReady.onTimeout is \"retry\" or \"fail\"");
        body.ready = { url: y.url, interval: millis("spawn", "exactReady.interval", y.interval, 3600000), timeout: millis("spawn", "exactReady.timeout", y.timeout, 3600000), accept: y.accept, onTimeout: y.onTimeout || "retry" };
      }
    } catch (e) { return Promise.reject(e); }
    return seam.request("exact-process:", { method: "POST", body: JSON.stringify(body), exactStream: o.exactStream }, undefined, childOwner(body.stdio[0] === "ignore"));
  }

  // A one-shot request to the host's process work: its JSON answer.
  function once(api, body, ceiling, signal) {
    var init = { method: "POST", body: JSON.stringify(body), exactIndependentHttp: { maxResponseBytes: ceiling } };
    if (signal !== undefined) init.signal = signal;
    return seam.request("exact-process:", init).then(function (r) {
      return r.text().then(function (t) { if (r.status === 200) return JSON.parse(t); throw coded(api + ": " + (t || "HTTP " + r.status), "ERR_CHILD_PROCESS"); });
    }, function (e) {
      if (e && e.name === "FetchError" && e.kind === "NotFound") throw coded(api + ": spawn " + (body.program || "") + " ENOENT: " + e.message, "ENOENT");
      if (e && e.name === "FetchError") throw coded(api + ": " + e.message, e.kind === "Refused" ? "EACCES" : "ERR_CHILD_PROCESS", "Error");
      throw e;
    });
  }

  function execFile(file, args, options) {
    if (args !== undefined && !Array.isArray(args)) { options = args; args = undefined; }
    if (!available) return Promise.reject(absent("execFile"));
    var o = options || {}, body;
    try {
      args = argv("execFile", file, args);
      body = common("execFile", o);
      body.kind = "execFile"; body.program = file; body.args = args;
      body.timeout = millis("execFile", "timeout", o.timeout, 3600000) || undefined;
      var max = o.maxBuffer === undefined ? 1048576 : o.maxBuffer;
      if (!Number.isInteger(max) || max < 1 || max > 16777216) throw new TypeError("execFile: maxBuffer is 1 to 16777216 bytes");
      body.maxBuffer = max;
      if (o.input !== undefined) {
        var bytes = typeof o.input === "string" ? new TextEncoder().encode(o.input) : new Uint8Array(seam.copyBytes(o.input, "execFile input"));
        if (bytes.length > 16777216) throw new TypeError("execFile: input is at most 16 MiB");
        body.input = seam.toBase64(bytes);
      }
    } catch (e) { return Promise.reject(e); }
    var ceiling = Math.min(67108864, 2 * (4 * Math.ceil(body.maxBuffer / 3)) + 65536);
    return once("execFile", body, ceiling, o.signal).then(function (r) {
      var text = function (v) { return body.encoding === "utf8" ? v : Uint8Array.from(global.atob(v), function (c) { return c.charCodeAt(0); }).buffer; };
      var out = { stdout: text(r.stdout), stderr: text(r.stderr), code: r.code, signal: r.signal };
      if (r.code === 0 && !r.signal && !r.timedOut && !r.overflow) return out;
      var why = r.overflow ? r.overflow + " maxBuffer length exceeded" : "Command failed: " + [file].concat(args).join(" ") + (body.encoding === "utf8" && out.stderr ? "\n" + out.stderr : "");
      var e = coded(why, r.overflow ? "ERR_CHILD_PROCESS_STDIO_MAXBUFFER" : r.code, "Error");
      Object.defineProperties(e, { signal: { value: r.signal, configurable: true }, killed: { value: !!r.timedOut, configurable: true }, stdout: { value: out.stdout, configurable: true }, stderr: { value: out.stderr, configurable: true } });
      throw e;
    });
  }

  function execPath(target) {
    if (!available) return Promise.reject(absent("execPath"));
    if (typeof target !== "string" || !target) return Promise.reject(new TypeError("execPath: the program, as its grant line names it"));
    return once("execPath", { kind: "execPath", program: target }, 65536).then(function (r) { return r.path === undefined ? null : r.path; });
  }
  function environment(api, kind) {
    return function () {
      if (!available) return Promise.reject(absent(api));
      return once(api, { kind: kind }, 1048576);
    };
  }
  // A fact (§6): granted where a program may start and a `process.spawn` line names one; never `prompt`.
  function processStatus() {
    var grants = String(global.exact && global.exact.grants || "").split("\n").some(function (line) { return /^\s*process\.spawn\s/.test(line); });
    // Stage 0 runs only the agent's substitutes (§7): outside the agent nothing starts yet.
    var starts = available && seam.agent();
    return { available: starts, state: !starts ? "unknown" : grants ? "granted" : "denied" };
  }
  global.__exact_child_process = Object.freeze({
    spawn: spawn, execFile: execFile, execPath: execPath, processStatus: processStatus,
    loginShellEnvironment: environment("loginShellEnvironment", "loginEnv"), appEnvironment: environment("appEnvironment", "appEnv"),
  });
})(globalThis);
