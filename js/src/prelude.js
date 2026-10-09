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
  // 11 the agent's repeatable random bytes (hex; undefined outside the agent),
  // 12 the carrier's auth callback ("callback") or a web worker realm's placement.
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

  // No timers and no clock to read (LLP 1027.000): refused by name, as the
  // web's module bindings refuse them (host/web-js/ts-fetch.js), rather than
  // a bare "doesn't exist". Clearing what was never set is harmless.
  function noTimers(api) {
    return function () { throw new Error(api + " is unavailable in data sources: there are no timers; pass time as an argument"); };
  }
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
  // Every unhandled rejection in the module reaches the journal, as a
  // page's `unhandledrejection` reaches its console (LLP 1097 D8). Hermes's
  // tracker schedules its report with the global `setTimeout`, which a data
  // source has none of: the prelude's runs the tracker's own callback alone,
  // at the next checkpoint (`checkpoint` below), which is when a browser
  // fires the event, and refuses every other.
  var reports = [], nextReport = 1;
  var tracker = global.HermesInternal;
  if (tracker && typeof tracker.enablePromiseRejectionTracker === "function") {
    tracker.enablePromiseRejectionTracker({
      allRejections: true,
      onUnhandled: function (id, error) { journal("data: unhandled rejection: " + errorText(error)); leftOpen(); },
      onHandled: function () {},
    });
  }
  function checkpoint() {
    while (reports.length) {
      var report = reports.shift();
      try { report.run(); } catch (e) { /* the tracker's own; nothing to tell */ }
    }
  }
  // A browser realm (the wasm host's module realm) reports its own rejections (module-glue.js).
  if (!tracker && typeof global.addEventListener === "function") global.addEventListener("unhandledrejection", function () { leftOpen(); });
  var refuseTimeout = noTimers("setTimeout()");
  fixed(global, "setTimeout", function setTimeout(callback, delay) {
    if (typeof callback === "function" && callback.name === "bound onUnhandled") {
      reports.push({ id: nextReport, run: callback });
      return nextReport++;
    }
    return refuseTimeout();
  });
  fixed(global, "clearTimeout", function clearTimeout(id) {
    reports = reports.filter(function (r) { return r.id !== id; });
  });
  ["setInterval", "requestAnimationFrame", "requestIdleCallback"].forEach(function (name) {
    fixed(global, name, noTimers(name + "()"));
  });
  ["clearInterval", "cancelAnimationFrame", "cancelIdleCallback"].forEach(function (name) {
    fixed(global, name, function () {});
  });
  fixed(global, "performance", Object.freeze({ now: function () { return refuseAmbient("performance.now()"); } }));
  // No I/O of the module's own (LLP 1016.000 D3: a socket only listens,
  // opened by the runtime as a `fetch` with `exactStream`). Functions that
  // refuse by name, in every realm this prelude runs in (Hermes, the wasm
  // target's iframe and worker), as the JS target's bindings refuse them
  // (host/web-js/ts-fetch.js): `typeof` and the words are the same on every
  // host, and a `new WebSocket(url)` is not a bare "undefined".
  ["XMLHttpRequest", "WebSocket", "EventSource"].forEach(function (name) {
    fixed(global, name, function () { throw new Error(name + " is unavailable in data sources"); });
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

  // Intl.NumberFormat's `notation: "compact"` where the engine ignores it:
  // Hermes's Apple Intl printed `9,274,743` where a browser prints `9.3M`
  // (x2apps stocks #3). The short forms are ICU's, as Chrome 154 formats
  // `10^e` (formatToParts) for e = 3…14: per locale, each magnitude's power
  // of ten and suffix (`~` a no-break space), "" where it is not
  // abbreviated. ICU's steps follow (CompactHandler): the unrounded
  // magnitude picks the power, the scaled value rounds (half-expand, from
  // its shortest digits) to an integer but two significant digits, or to
  // the app's own digit options, and a carry into the next magnitude picks
  // again. Digits, signs and separators are the engine's own. A locale or
  // `compactDisplay: "long"` not here is formatted in full, and said once.
  (function () {
    var Native = global.Intl && global.Intl.NumberFormat;
    try { if (!Native || new Native("en-US", { notation: "compact" }).format(1000) === "1K") return; } catch (e) { return; }
    var SHORT = {
      "en": "3K|3K|3K|6M|6M|6M|9B|9B|9B|12T|12T|12T",
      "en-GB": "3k|3k|3k|6m|6m|6m|9bn|9bn|9bn|12tn|12tn|12tn",
      "en-IN": "3K|3K|5L|5L|7Cr|7Cr|7Cr|10KCr|10KCr|12LCr|12LCr|12LCr",
      "de": "|||6~Mio.|6~Mio.|6~Mio.|9~Mrd.|9~Mrd.|9~Mrd.|12~Bio.|12~Bio.|12~Bio.",
      "fr": "3~k|3~k|3~k|6~M|6~M|6~M|9~Md|9~Md|9~Md|12~Bn|12~Bn|12~Bn",
      "fr-CA": "3~k|3~k|3~k|6~M|6~M|6~M|9~G|9~G|9~G|12~T|12~T|12~T",
      "es": "3~mil|3~mil|3~mil|6~M|6~M|6~M|6~M|9~mil~M|9~mil~M|12~B|12~B|12~B",
      "es-MX": "3~k|3~k|3~k|6~M|6~M|6~M|6~M|9~mil~M|9~mil~M|12~B|12~B|12~B",
      "it": "3K|3K|3K|6~Mln|6~Mln|6~Mln|9~Mld|9~Mld|9~Mld|12~Bln|12~Bln|12~Bln",
      "pt": "3~mil|3~mil|3~mil|6~mi|6~mi|6~mi|9~bi|9~bi|9~bi|12~tri|12~tri|12~tri",
      "pt-PT": "3~mil|3~mil|3~mil|6~M|6~M|6~M|9~mM|9~mM|9~mM|12~Bi|12~Bi|12~Bi",
      "nl": "3K|3K|3K|6~mln.|6~mln.|6~mln.|9~mld.|9~mld.|9~mld.|12~bln.|12~bln.|12~bln.",
      "sv": "3~tn|3~tn|3~tn|6~mn|6~mn|6~mn|9~md|9~md|9~md|12~bn|12~bn|12~bn",
      "da": "3~t|3~t|3~t|6~mio.|6~mio.|6~mio.|9~mia.|9~mia.|9~mia.|12~bio.|12~bio.|12~bio.",
      "nb": "3k|3k|3k|6~mill.|6~mill.|6~mill.|9~mrd.|9~mrd.|9~mrd.|12~bill.|12~bill.|12~bill.",
      "fi": "3~t.|3~t.|3~t.|6~milj.|6~milj.|6~milj.|9~mrd.|9~mrd.|9~mrd.|12~bilj.|12~bilj.|12~bilj.",
      "pl": "3~tys.|3~tys.|3~tys.|6~mln|6~mln|6~mln|9~mld|9~mld|9~mld|12~bln|12~bln|12~bln",
      "ru": "3~тыс.|3~тыс.|3~тыс.|6~млн|6~млн|6~млн|9~млрд|9~млрд|9~млрд|12~трлн|12~трлн|12~трлн",
      "uk": "3~тис.|3~тис.|3~тис.|6~млн|6~млн|6~млн|9~млрд|9~млрд|9~млрд|12~трлн|12~трлн|12~трлн",
      "cs": "3~tis.|3~tis.|3~tis.|6~mil.|6~mil.|6~mil.|9~mld.|9~mld.|9~mld.|12~bil.|12~bil.|12~bil.",
      "tr": "3~B|3~B|3~B|6~Mn|6~Mn|6~Mn|9~Mr|9~Mr|9~Mr|12~Tn|12~Tn|12~Tn",
      "ja": "|4万|4万|4万|4万|8億|8億|8億|8億|12兆|12兆|12兆",
      "zh": "|4万|4万|4万|4万|8亿|8亿|8亿|8亿|12万亿|12万亿|12万亿",
      "zh-TW": "|4萬|4萬|4萬|4萬|8億|8億|8億|8億|12兆|12兆|12兆",
      "ko": "3천|4만|4만|4만|4만|8억|8억|8억|8억|12조|12조|12조",
      "hi": "3~हज़ार|3~हज़ार|5~लाख|5~लाख|7~क॰|7~क॰|9~अ॰|9~अ॰|11~ख॰|11~ख॰|13~नील|13~नील",
      "he": "3K‏|3K‏|3K‏|6M‏|6M‏|6M‏|9B‏|9B‏|9B‏|12T‏|12T‏|12T‏",
      "id": "3~rb|3~rb|3~rb|6~jt|6~jt|6~jt|9~M|9~M|9~M|12~T|12~T|12~T",
      "th": "3K|3K|3K|6M|6M|6M|9B|9B|9B|12T|12T|12T",
      "vi": "3~N|3~N|3~N|6~Tr|6~Tr|6~Tr|9~T|9~T|9~T|12~NT|12~NT|12~NT",
    };
    var said = {};
    function table(locale) {
      var parts = String(locale).split("-u-")[0].split("-"), lang = parts[0], region = "", script = "";
      for (var i = 1; i < parts.length; i++) {
        if (parts[i].length === 4) script = parts[i];
        else if (parts[i].length === 2 && !region) region = parts[i].toUpperCase();
      }
      var names = [lang + (region ? "-" + region : ""), lang === "zh" && (script === "Hant" || region === "HK" || region === "MO") ? "zh-TW" : "", lang];
      for (var j = 0; j < names.length; j++) {
        var row = names[j] && SHORT[names[j]];
        if (row) return row.split("|").map(function (cell) {
          var m = /^(\d+)(.*)$/.exec(cell);
          return m ? { power: Number(m[1]), suffix: m[2].replace(/~/g, " ") } : null;
        });
      }
      return null;
    }
    // A non-negative number's shortest digits, never exponential.
    function plain(x) {
      var s = String(x), e = s.indexOf("e");
      if (e < 0) return s;
      var mantissa = s.slice(0, e).split("."), digits = mantissa[0] + (mantissa[1] || ""), point = mantissa[0].length + Number(s.slice(e + 1));
      if (point <= 0) return "0." + new Array(1 - point).join("0") + digits;
      return point >= digits.length ? digits + new Array(point - digits.length + 1).join("0") : digits.slice(0, point) + "." + digits.slice(point);
    }
    // `x` (≥ 0) moved `shift` places left, rounded half-expand to `fraction`
    // digits after the point (a negative one rounds to tens, hundreds…),
    // as a decimal string.
    function roundAt(x, shift, fraction) {
      var s = plain(x).split("."), int = s[0], frac = s[1] || "";
      while (int.length <= shift - fraction) int = "0" + int;
      frac = int.slice(int.length - shift) + frac; int = int.slice(0, int.length - shift);
      var digits = (int + frac + new Array(Math.max(fraction, 0) + 2).join("0")).split("").map(Number), keep = int.length + fraction;
      var up = digits[keep] >= 5;
      digits.length = keep;
      for (var i = keep - 1; up && i >= 0; i--) { digits[i]++; up = digits[i] === 10; if (up) digits[i] = 0; }
      var text = (up ? "1" : "") + digits.join("") + new Array(Math.max(-fraction, 0) + 1).join("0");
      var cut = text.length - Math.max(fraction, 0), head = text.slice(0, cut).replace(/^0+(?=\d)/, "") || "0";
      return fraction > 0 ? head + "." + text.slice(cut) : head;
    }
    // Apple's engine has no `formatToParts`: the whole string is one part.
    function partsOf(format, n) {
      return typeof format.formatToParts === "function" ? format.formatToParts(n) : [{ type: "literal", value: format.format(n) }];
    }
    function Compact(locales, options) {
      this.native = new Native(locales, options);
      this.options = options;
      var resolved = this.native.resolvedOptions();
      this.cells = options.compactDisplay === "long" || resolved.style !== "decimal" ? null : table(resolved.locale);
      if (!this.cells && !said[resolved.locale] && global.console) {
        said[resolved.locale] = true;
        global.console.error("Intl.NumberFormat: notation \"compact\" (" + (options.compactDisplay || "short") + ", " + resolved.style + ") in " +
          resolved.locale + " is not available on this host; numbers are formatted in full");
      }
    }
    Compact.prototype = Object.create(Native.prototype);
    Compact.prototype.constructor = Compact;
    // The rounded scaled value as a decimal string, and its cell.
    Compact.prototype.scale = function (x) {
      var cells = this.cells, o = this.options;
      var cell = function (magnitude) { return magnitude < 3 ? null : cells[Math.min(magnitude, 14) - 3]; };
      var digits = function (shifted) {
        var int = shifted.split(".")[0].length;
        if (o.maximumSignificantDigits !== undefined || o.minimumSignificantDigits !== undefined) {
          var sd = o.maximumSignificantDigits !== undefined ? o.maximumSignificantDigits : 21;
          var zeros = /^0\.(0*)/.exec(shifted);
          return zeros ? zeros[1].length + sd : sd - int;
        }
        if (o.maximumFractionDigits !== undefined || o.minimumFractionDigits !== undefined)
          return o.maximumFractionDigits !== undefined ? o.maximumFractionDigits : Math.max(o.minimumFractionDigits, 3);
        // Compact rounding: an integer, but two significant digits.
        if (int >= 2 && shifted[0] !== "0") return 0;
        var lead = /^0\.0*/.exec(shifted);
        return lead ? lead[0].length - 1 + 1 : 1;
      };
      var magnitude = x === 0 ? 0 : plain(x).split(".")[0].replace(/^0+$/, "").length - 1;
      if (magnitude < 0) magnitude = 0;
      var c = cell(magnitude), shift = c ? c.power : 0;
      var unrounded = roundAt(x, shift, 30).replace(/\.?0+$/, "");
      var rounded = roundAt(x, shift, digits(unrounded));
      // A carry into the next magnitude picks its cell again.
      if (rounded.split(".")[0].length + shift - 1 > magnitude) {
        var next = cell(magnitude + 1);
        if ((next ? next.power : 0) !== shift) {
          c = next; shift = next ? next.power : 0;
          rounded = roundAt(x, shift, digits(roundAt(x, shift, 30).replace(/\.?0+$/, "")));
        }
      }
      return { text: rounded, cell: c };
    };
    Compact.prototype.parts = function (n) {
      n = Number(n);
      if (!this.cells || !isFinite(n)) return partsOf(this.native, n);
      var s = this.scale(Math.abs(n)), o = this.options, text = s.text;
      // Trailing zeros go, down to the app's minimum.
      var least = o.minimumFractionDigits !== undefined ? o.minimumFractionDigits : 0;
      if (o.minimumSignificantDigits === undefined)
        while (text.indexOf(".") >= 0 && text.split(".")[1].length > least && /0$/.test(text)) text = text.slice(0, -1);
      text = text.replace(/\.$/, "");
      var fraction = (text.split(".")[1] || "").length, value = Number(text);
      // Grouping as compact's (`min2`): only from five integer digits.
      var digits = partsOf(new Native(this.native.resolvedOptions().locale, {
        minimumFractionDigits: fraction, maximumFractionDigits: fraction,
        useGrouping: value >= 10000, signDisplay: o.signDisplay,
      }), n < 0 || (n === 0 && 1 / n < 0) ? -value : value);
      if (!s.cell) return digits;
      var suffix = s.cell.suffix, space = /^[\s  ]*/.exec(suffix)[0], tail = /[‎‏]*$/.exec(suffix)[0];
      var word = suffix.slice(space.length, suffix.length - tail.length);
      if (space) digits.push({ type: "literal", value: space });
      digits.push({ type: "compact", value: word });
      if (tail) digits.push({ type: "literal", value: tail });
      return digits;
    };
    Compact.prototype.formatToParts = function (n) { return this.parts(n); };
    Compact.prototype.resolvedOptions = function () {
      var r = this.native.resolvedOptions();
      if (this.cells) { r.notation = "compact"; r.compactDisplay = "short"; }
      return r;
    };
    Object.defineProperty(Compact.prototype, "format", {
      configurable: true,
      get: function () {
        var self = this;
        return function (n) { return self.parts(n).map(function (p) { return p.value; }).join(""); };
      },
    });
    function NumberFormat(locales, options) {
      if (options && options.notation === "compact") return new Compact(locales, options);
      return new.target ? construct(Native, [locales, options], new.target) : Native(locales, options);
    }
    NumberFormat.prototype = Native.prototype;
    NumberFormat.supportedLocalesOf = Native.supportedLocalesOf;
    Object.defineProperty(global.Intl, "NumberFormat", { value: NumberFormat, writable: true, configurable: true });
    var toLocale = Number.prototype.toLocaleString;
    Object.defineProperty(Number.prototype, "toLocaleString", {
      writable: true, configurable: true,
      value: function toLocaleString(locales, options) {
        if (options && options.notation === "compact") return new Compact(locales, options).format(Number(this));
        return toLocale.call(this, locales, options);
      },
    });
  })();

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
    if (!currentCall || currentCall.status !== "pending") {
      throw new Error(api + " is unavailable " + (initializing ? "during module initialization" : "outside an answer") + "; call it inside an answer");
    }
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
    return subtleCall(this, function () {
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
    return subtleCall(this, function () {
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
    return subtleCall(this, function () {
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
    return subtleCall(this, function () {
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

  // --- fetch: a request the host runs; a Promise for its reply -------------
  var nextTicket = 1;
  // ticket -> { resolve, reject, call, claimed, stream, signal, release }:
  // `claimed` once a settle has handed the executor its request, `release`
  // ends its watch on `signal`.
  var pending = new Map();
  var currentCall = null;    // the answer a fetch belongs to
  // A fetch's `signal` is watched through Ibex's own abort hooks natively,
  // which run before any listener and so cannot be stopped by one; a
  // browser realm's signal is the browser's, watched by a listener, and
  // checked again when the reply lands. Either watch ends with the fetch.
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
  var storageCounts = { done: 0, failed: 0, last: null };
  // Set before the first answer, never after: an app cannot turn it on.
  global.__exact_main_thread = function () { if (initializing) moving = true; };
  // Where a completion's count lives: the call that issued it, or the
  // background once that call's storage moved there.
  function owner(call) { return call.moved ? background : call; }
  // One queue for every storage operation of the module (D3), in the order
  // issued; one in flight at a time. So only the issuer of the operation in
  // flight waits on the store's shared completions: one completion wakes one
  // waiter, and the next answer's read sees the write issued before it.
  var MAX_QUEUED = 256;
  var queue = [];
  var head = null;
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

  function Response(r) {
    this.status = r.status;
    this.ok = r.status >= 200 && r.status < 300;
    this.headers = new global.Headers(r.headers);
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

  global.Response = Response;
  global.fetch = function (url, init) {
    var call = currentCall;
    // A continuation running as an answer already given still fetches: the
    // next answer to settle claims the request (see `settle`).
    if (!call) {
      var refused = new Error("fetch() called outside an answer: it was never run. Start a fetch inside an answer");
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
    var bodyFrom = init ? init.exactBodyFrom : undefined;
    if (bodyFrom !== undefined) {
      if (typeof bodyFrom !== "string" || bodyFrom.slice(0, 5) !== "app:/")
        return Promise.reject(new TypeError("exactBodyFrom must be an app:/ path"));
      if (utf8Length(bodyFrom) > 4096)
        return Promise.reject(new TypeError("exactBodyFrom: a path is at most 4096 bytes"));
      if (/(^|\/)\.\.?(\/|$)|\0/.test(bodyFrom.slice(5)))
        return Promise.reject(new TypeError("exactBodyFrom: an app:/ path has no . or .. segment"));
      if (method === "GET" || method === "HEAD")
        return Promise.reject(new TypeError("fetch: a " + method + " request cannot have a body"));
      if (raw != null)
        return Promise.reject(new TypeError("fetch: a request has one body: body or exactBodyFrom"));
      if (init.exactStream !== undefined && /^wss?:/i.test(String(url)))
        return Promise.reject(new TypeError("exactBodyFrom: a WebSocket sends no body"));
    }
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
    // A deadline for the whole exchange (headers and body), in milliseconds:
    // the host cancels the request when it passes and the fetch rejects with
    // a FetchError of kind "Timeout". A stream has none.
    var timeout = init ? init.exactTimeout : undefined;
    if (timeout !== undefined) {
      if (!Number.isInteger(timeout) || timeout < 1 || timeout > 3600000)
        return Promise.reject(new TypeError("exactTimeout must be an integer number of milliseconds from 1 to 3600000"));
      if (stream) return Promise.reject(new TypeError("exactTimeout: a stream has no timeout"));
    }
    // The web's `signal`: an aborted fetch rejects with its reason at once.
    // The host's request still runs; its reply is dropped (`__exact_fulfill`).
    var signal = init ? init.signal : undefined;
    if (signal != null && abortHooks) {
      try { abortHooks.subscribe(signal, function () {})(); }
      catch (e) { return Promise.reject(new TypeError("fetch: init.signal is not an AbortSignal")); }
    }
    if (signal && signal.aborted) return Promise.reject(signal.reason);
    var ticket = nextTicket++;
    var error = host(1, String(ticket), JSON.stringify({ method: method, url: String(url), headers: headers, body: body, body_base64: bytes ? toBase64(bytes) : undefined, max_response_bytes: ceiling, stream: stream ? true : undefined, timeout_ms: timeout, body_from: bodyFrom }));
    if (error !== undefined) return Promise.reject(new Error(error));
    call.tickets.push(ticket);
    if (stream) call.stream = stream;
    return new Promise(function (resolve, reject) {
      // `early`: made while its answer was still in flight (its body, or its
      // own continuation), so that answer's own, whoever else is pending.
      var p = { resolve: resolve, reject: reject, call: call, claimed: false, stream: !!stream, signal: signal, release: function () {}, early: call.status === "pending" };
      pending.set(ticket, p);
      if (signal) p.release = watchAbort(signal, function () { if (settled(ticket)) reject(signal.reason); });
    });
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
    if (!currentCall || currentCall.status !== "pending") throw new Error("authCallback() is unavailable " + (initializing ? "during module initialization" : "outside an answer") + "; call it inside an answer");
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
  var store = {
    get: function (name) { var v = host(2, String(name), ""); return v === undefined ? null : v; },
    set: function (name, value) { var e = host(3, String(name), String(value)); if (e !== undefined) throw new Error(e); },
    forget: function (name) { var e = host(4, String(name), ""); if (e !== undefined) throw new Error(e); },
    keepKey: function (name, pair) { return keepKey(name, pair); },
    key: function (name) { return keptKey(name); },
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
  var toldAgent = false;
  // Rust's error numbers are platform-specific. Never treat Windows access
  // denial (5) as EISDIR, or its 17/39 as Unix EEXIST/ENOTEMPTY.
  var windowsStorage = global.__exact_windows_storage === true;
  delete global.__exact_windows_storage;
  var ERRNO = windowsStorage
    ? { 2: "ENOENT", 3: "ENOENT", 32: "EBUSY", 33: "EBUSY", 80: "EEXIST", 145: "ENOTEMPTY", 170: "EBUSY", 183: "EEXIST", 267: "ENOTDIR" }
    : { 2: "ENOENT", 16: "EBUSY", 17: "EEXIST", 20: "ENOTDIR", 21: "EISDIR", 39: "ENOTEMPTY", 66: "ENOTEMPTY" };
  // `fs.compressImage`'s own failures (LLP 1069.002 A1.4), named in the
  // message natively; the web sets the code itself.
  var IMAGE_CODE = /^(?:filesystem: )?compressImage: (unsupported|undecodable|too-large|unfit|timeout): /;
  function storageCode(message) {
    if (/^denied: /.test(message)) return "denied";
    var image = IMAGE_CODE.exec(message);
    if (image) return image[1];
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
    // (`doc:/`) needs no app storage (LLP 1069.010 D1).
    var path = receiver && nativeStorage && receiver === nativeStorage.fs && typeof args[0] === "string" ? args[0] : "";
    try { refused = host(5, path, "") || (receiver ? undefined : "unsupported"); }
    catch (e) { refused = "bake"; } // no filesystem or database effects during bake
    // A drive with no scratch store is said once in the journal, as the web's (ts-data.js), where
    // the driver finds it for its note (LLP 1102 §3.17).
    if (refused === "agent" && !toldAgent) { toldAgent = true; journal("storage refused (agent): " + REFUSED.agent); }
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
  ["readFile", "writeFile", "atomicWriteFile", "appendFile", "readdir", "mkdir", "rm", "stat", "rename", "copyFile", "realpath"].forEach(function (method) {
    files[method] = function () { return storageCall(nativeStorage && nativeStorage.fs, method, arguments); };
  });
  // `fs.compressImage(from, to, {maxDimension, maxBytes})` (LLP 1069.002
  // A1): its options are checked here, before anything is queued, and travel
  // to the host as two numbers; natively the reply is `width\theight\tsize`,
  // on the web the record.
  var MAX_IMAGE_DIMENSION = 8192, MAX_IMAGE_BYTES = 67108864;
  files.compressImage = function (from, to, options) {
    var api = "storage.fs.compressImage()";
    try {
      if (typeof from !== "string" || typeof to !== "string") throw new TypeError(api + ": from and to must be app:/ paths");
      if (!options || typeof options !== "object") throw new TypeError(api + ": options must be {maxDimension, maxBytes}");
      var dimension = options.maxDimension, bytes = options.maxBytes;
      if (!Number.isInteger(dimension) || dimension < 1 || dimension > MAX_IMAGE_DIMENSION) throw new TypeError(api + ": maxDimension must be an integer from 1 to " + MAX_IMAGE_DIMENSION);
      if (!Number.isInteger(bytes) || bytes < 1 || bytes > MAX_IMAGE_BYTES) throw new TypeError(api + ": maxBytes must be an integer from 1 to " + MAX_IMAGE_BYTES);
    } catch (e) {
      return Promise.reject(e);
    }
    return storageCall(nativeStorage && nativeStorage.fs, "compressImage", [from, to, dimension, bytes], function (value) {
      var parts = typeof value === "string" ? value.split("\t") : [value.width, value.height, value.size];
      return { path: to, type: "image/jpeg", size: +parts[2], width: +parts[0], height: +parts[1] };
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
    return JSON.stringify({ tag: 2, kind: kind, code: e && e.code, failure: failureCode(e), message: String(message) });
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
    return JSON.stringify({ tag: 1, call: call.id, ticket: ticket });
  }
  // Whether an answer still has a fetch or storage step of its own in flight.
  function owes(call) {
    for (var i = 0; i < call.tickets.length; i++) {
      var p = pending.get(call.tickets[i]);
      if (p && p.call === call) return true;
    }
    return call.storage > 0 && !call.lost;
  }
  // An answer is given when its value is ready, as in a browser (LLP 1097
  // D1): the storage it started and did not await finishes after it, as the
  // background's, so nothing is stranded (kanban F22's reason to wait). On a
  // worker placement, which runs one turn to its end, it still waits.
  function reply(call, text) {
    call.replied = true;
    calls.delete(call.id);
    return text;
  }
  function settle(call, final) {
    // Its own work first: a fetch it made that no settle has handed out,
    // then its storage steps, which move to the background once its value
    // is ready. A fetch made while it was in
    // flight is its own (an unawaited POST), and it claims it whoever else
    // is pending. One made only after it finished ran in a continuation it
    // settled for another answer: with another answer in flight, which may
    // be that one, it is left to it (review r4a, finding 1); with none, it
    // waits for the fetch itself. Either way nothing is left behind (Grok's
    // batch 2 review).
    var finished = call.status === "done" || call.status === "failed", others = false;
    if (finished && moduleWide) calls.forEach(function (c) { if (c !== call && !c.letGo && c.status === "pending") others = true; });
    for (var i = 0; i < call.tickets.length; i++) {
      var own = pending.get(call.tickets[i]);
      if (own && !own.claimed && own.call === call && (own.early || !others)) return claim(call, call.tickets[i]);
    }
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
      return reply(call, call.status === "done" ? ok(call.value) : fail(call.error));
    }
    if (moduleWide) {
      var unclaimed;
      pending.forEach(function (p, ticket) { if (unclaimed === undefined && !p.claimed && !p.stream) unclaimed = ticket; });
      if (unclaimed !== undefined) return claim(call, unclaimed);
      var left = false;
      // Never the background's: only its own round holds the storage
      // ticket (D2); an answer awaiting it waits with the module below.
      storing.forEach(function (c) { if (c !== background && !calls.has(c.id)) left = true; });
      if (left) return JSON.stringify({ tag:1, call:call.id, ticket:0 });
      if (!final && (pending.size || storing.size)) return JSON.stringify({ tag: 1, call: call.id, ticket: 0, waiting: true });
    } else if (moving && !final && background.storage > 0) {
      // The page's realm runs one answer at a time, but background work
      // goes on beside it: an answer awaiting it parks and is asked again
      // after a background delivery (LLP 1097 D7).
      return JSON.stringify({ tag: 1, call: call.id, ticket: 0, waiting: true });
    }
    call.replied = true;
    calls.delete(call.id);
    if (moduleWide) return fail(new Error("the answer is pending on nothing: it awaits a promise that no fetch or storage step " +
      "in flight in this module will settle"));
    return fail(new Error("the answer is pending on nothing: no host operation it started will resolve it. " +
      "An answer that awaits a promise another answer started (a fetch shared between answers, or a queue chained through a " +
      "fetch another answer is waiting on) waits on work it does not own in the web's module realm; make each answer's own " +
      "fetch, or share the resolved value rather than the promise. Storage is different: an answer queued behind another's " +
      "storage turn waits for it"));
  }
  // The executor: `__exact_call(source, argsJson)` → tag 3 with a call id
  // (a value given at once too: see its end) — then it drains microtasks and asks
  // `__exact_settle(id)`, which is tag 0/2, or tag 1 with the ticket of the
  // fetch the answer is waiting on (0: its storage; 0 and `waiting`: another
  // answer's work). `__exact_fulfill(ticket, outcomeJson)` resolves that
  // fetch; drain and settle again. `__exact_settle(id, "final")` refuses an
  // answer still waiting on nothing of its own.
  global.__exact_call = function (source, argsJson) {
    initializing = false;
    var call = { id: nextCall++, status: "pending", value: undefined, error: undefined, tickets: [], storage: 0, replied: false };
    var result;
    currentCall = call;
    try {
      var nativeCall = function (request) {
        if (currentCall === background) {
          journal("data: native.call() called from background work: it was never run");
          throw new Error("native.call() called from background work: it was never run. Call it inside an answer");
        }
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
    catch (e) {
      calls.set(call.id, call);
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
  global.__exact_settle = function (id, final) {
    checkpoint();
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
    if (!call) return "";
    // Not a fetch another answer has since claimed: that one waits on it.
    var dropped = [];
    for (var i = 0; i < call.tickets.length; i++) {
      var p = pending.get(call.tickets[i]);
      // A stream's promise never settles (LLP 1016.000), ended or let go.
      if (p && p.call === call) { settled(call.tickets[i]); if (!p.stream) dropped.push(p); }
    }
    // Rejected now, run at the next drain with no answer current.
    dropped.forEach(function (p) {
      p.reject(new FetchError({ kind: "Aborted", message: "the answer was let go before this reply; the request may already have been sent" }));
    });
    var rejected = dropped.length ? " rejected" : "";
    if (call.storage > 0 && !call.lost) { call.letGo = true; return "storage" + rejected; }
    call.replied = true;
    calls.delete(call.id);
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
    });
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
  // The module's storage, after a call or a delivery (LLP 1097 D5, D8):
  // whether the operation in flight is the background's (`head`), the
  // operations queued behind it and in flight, an answer's or the
  // background's, and what landed and failed.
  global.__exact_background = function () {
    checkpoint();
    currentCall = null;
    return JSON.stringify({ head: headOwner() === background, queued: queue.length, inFlight: head ? 1 : 0,
      done: storageCounts.done, failed: storageCounts.failed, last: storageCounts.last });
  };
  // A background round delivers with the background current: what its
  // completion's reaction issues is the background's too.
  global.__exact_enter_background = function () { currentCall = background; };
  // A compression whose wait gave up, and whose right to write the waiter
  // took away (LLP 1069.002 A1.5): it can write nothing now, so it is
  // failed and the queue moves on. Any other operation whose wait ran out
  // may still write, so its queue waits for it, as before. Whether it was.
  var IMAGE_ABANDONED = /^compressImage: timeout: the storage wait ran out/;
  function failAbandonedImage(message) {
    if (!head || head.method !== "compressImage" || !IMAGE_ABANDONED.test(message)) return false;
    landed(head, false, storageError(message));
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
  global.__exact_fulfill = function (ticket, outcomeJson) {
    var p = settled(Number(ticket));
    if (!p) return;
    var o = JSON.parse(outcomeJson);
    // The continuation runs in the drain that follows, and a fetch it
    // makes belongs to this call.
    currentCall = p.call;
    // A browser realm's abort listener can be stopped by an earlier one;
    // the signal itself still says so.
    if (p.signal && p.signal.aborted) p.reject(p.signal.reason);
    else if (o.failed) p.reject(new FetchError(o.failed));
    else p.resolve(new Response(o.response));
  };
})(globalThis);
