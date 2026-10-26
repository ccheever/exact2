// Intl.NumberFormat's `notation: "compact"` where the engine ignores it:
// Hermes's Apple Intl printed `9,274,743` where a browser prints `9.3M`
// (x2apps stocks #3). The short forms are ICU's, as Chrome 154 formats
// `10^e` (formatToParts) for e = 3…14: per locale, each magnitude's power
// of ten and suffix (`~` a no-break space), "" where it is not
// abbreviated. ICU's steps follow (CompactHandler): the unrounded
// magnitude picks the power, the scaled value rounds (by `roundingMode`,
// from its shortest digits) to an integer but two significant digits, or to
// the app's own digit options, and a carry into the next magnitude picks
// again. Digits, signs and separators are the engine's own. A locale or
// `compactDisplay: "long"` not here is formatted in full, and said once.
//
// Its rounding too, where the engine rounds a half to even: Hermes's Apple
// Intl leaves NSNumberFormatter's `.halfEven` and ignores `roundingMode`, so
// USD 0.825 printed `$0.82` where a browser prints `$0.83` (X72). A browser
// rounds the number's shortest digits by the formatter's `roundingMode`,
// half-expand unless it names another (ECMA-402 FormatNumericToString). So
// this rounds them first, to the digits the engine prints, and hands the
// engine a number its own rounding leaves alone; `resolvedOptions()` names
// the mode.
(function (global) {
  "use strict";
  // The cache key of an Intl formatter's arguments when they read the same
  // whenever they are read: locales as a string or strings, and options as a
  // plain object of finite primitives in own enumerable data properties;
  // null for anything else.
  function formatKey(locales, options) {
    if (locales !== undefined && typeof locales !== "string" && !(Array.isArray(locales) && locales.every(function (l) { return typeof l === "string"; }))) return null;
    if (options !== undefined) {
      if (options === null || typeof options !== "object") return null;
      var proto = Object.getPrototypeOf(options);
      if (proto !== Object.prototype && proto !== null) return null;
      var names = Object.getOwnPropertyNames(options);
      for (var i = 0; i < names.length; i++) {
        var d = Object.getOwnPropertyDescriptor(options, names[i]), t = typeof d.value;
        if (!d.enumerable || !("value" in d) || (t === "object" && d.value !== null) || t === "function" || t === "symbol" || (t === "number" && !isFinite(d.value))) return null;
      }
    }
    return JSON.stringify([locales === undefined ? 0 : locales, options === undefined ? 0 : options]);
  }
  dates(global.__exact_dates);
  delete global.__exact_dates;
  // Apple's engine makes an NSDateFormatter for every DateTimeFormat and
  // every Date `toLocale…String` call, which a module labelling a few
  // hundred rows a turn pays for each (500 dates twice: 130 ms, 7 ms kept).
  // A formatter made with locales and options that read the same whenever
  // they are read (`formatKey`) is made once; each `new` is still an object
  // of its own, which the prototype's methods read through to the one made
  // (`shared`; the prelude's guarded `format` and `formatToParts` too).
  function dates(guard) {
    var Engine = global.Intl && global.Intl.DateTimeFormat;
    if (!guard || !Engine) return;
    var proto = Engine.prototype, apply = Reflect.apply, made = new Map(), shared = new WeakMap();
    var engineOf = function (format) { return shared.get(format) || format; };
    guard.engineOf(engineOf);
    var madeFor = function (prefix, locales, options) {
      var key = formatKey(locales, options);
      if (key === null) return null;
      var format = made.get(prefix + key);
      if (!format) {
        if (made.size >= 64) made.clear();
        made.set(prefix + key, (format = new Engine(locales, options)));
      }
      return format;
    };
    ["resolvedOptions", "formatRange", "formatRangeToParts"].forEach(function (name) {
      var method = proto[name];
      if (typeof method !== "function") return;
      Object.defineProperty(proto, name, { writable: true, configurable: true, value: { [name]: function () { return apply(method, engineOf(this), arguments); } }[name] });
    });
    // Reading `arguments` keeps the constructor's and methods' `length` 0, as ECMA-402's.
    var DateTimeFormat = function DateTimeFormat() {
      var locales = arguments[0], options = arguments[1], format = madeFor("", locales, options);
      if (!format) return new.target ? Reflect.construct(Engine, [locales, options], new.target) : Engine(locales, options);
      var own = Object.create(new.target ? new.target.prototype : proto);
      shared.set(own, format);
      return own;
    };
    DateTimeFormat.prototype = proto;
    DateTimeFormat.supportedLocalesOf = Engine.supportedLocalesOf;
    Object.defineProperty(proto, "constructor", { value: DateTimeFormat, writable: true, configurable: true });
    Object.defineProperty(global.Intl, "DateTimeFormat", { value: DateTimeFormat, writable: true, configurable: true });
    // A Date's `toLocaleDateString`, `toLocaleTimeString` and
    // `toLocaleString` are a DateTimeFormat's `format` whose options gain
    // the method's default fields when they name none of its kind
    // (ECMA-402 §21.4.1, CreateDateTimeFormat's `required` and `defaults`).
    var DATE_FIELDS = ["weekday", "year", "month", "day"], TIME_FIELDS = ["dayPeriod", "hour", "minute", "second", "fractionalSecondDigits"];
    var withDefaults = function (options, required, defaults) {
      var o = options === undefined ? {} : options, named = false;
      if (required !== "time") DATE_FIELDS.forEach(function (k) { if (o[k] !== undefined) named = true; });
      if (required !== "date") TIME_FIELDS.forEach(function (k) { if (o[k] !== undefined) named = true; });
      // A style the method refuses is the engine's own TypeError.
      if ((required === "date" && o.timeStyle !== undefined) || (required === "time" && o.dateStyle !== undefined)) return null;
      if (named || o.dateStyle !== undefined || o.timeStyle !== undefined) return o;
      var full = Object.assign({}, o);
      if (defaults !== "time") full.year = full.month = full.day = "numeric";
      if (defaults !== "date") full.hour = full.minute = full.second = "numeric";
      return full;
    };
    var dateProto = global.Date.prototype, timeOf = dateProto.getTime;
    [["toLocaleString", "any", "all"], ["toLocaleDateString", "date", "date"], ["toLocaleTimeString", "time", "time"]].forEach(function (m) {
      var name = m[0], engineMethod = dateProto[name];
      Object.defineProperty(dateProto, name, {
        writable: true, configurable: true,
        value: { [name]: function () {
          var locales = arguments[0], options = arguments[1], time = timeOf.call(this);
          var full = formatKey(locales, options) === null ? null : withDefaults(options, m[1], m[2]);
          var format = full && madeFor(name + ":", locales, full);
          if (!format) return apply(engineMethod, this, arguments);
          return time !== time ? "Invalid Date" : guard.getFormat.call(format)(time);
        } }[name],
      });
    });
  }
  var Native = global.Intl && global.Intl.NumberFormat, hasCompact, roundsEven, probed = false;
  if (!Native) return;
  // One formatter answers both: an engine without compact formats 1000
  // in full, and one that rounds to even prints 0.125 as `0.12`; one that
  // refuses it is left alone. It is made at the first number formatted,
  // not as the prelude runs: Apple's engine reads every locale's name and
  // makes an NSNumberFormatter for it, which an app that formats no number
  // should not keep.
  function probe() {
    if (probed) return;
    probed = true;
    try {
      var formatter = new Native("en-US", { notation: "compact", maximumFractionDigits: 2 });
      hasCompact = formatter.format(1000) === "1K";
      roundsEven = formatter.format(0.125) === "0.12";
    } catch (e) { hasCompact = true; roundsEven = false; }
  }
  var MODES = ["ceil", "floor", "expand", "trunc", "halfCeil", "halfFloor", "halfExpand", "halfTrunc", "halfEven"];
  function modeOf(options) {
    var mode = options == null || options.roundingMode === undefined ? "halfExpand" : String(options.roundingMode);
    if (MODES.indexOf(mode) < 0) throw new RangeError("Value " + mode + " out of range for Intl.NumberFormat options property roundingMode");
    return mode;
  }
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
  // Whether a rounding moves the kept digits away from zero: `first` is
  // the first digit dropped, `rest` whether any after it is not 0, `last`
  // the last digit kept, `negative` the number's sign.
  function away(mode, first, rest, last, negative) {
    var exact = !first && !rest, half = first > 5 || (first === 5 && rest) ? 1 : first === 5 ? 0 : -1;
    if (mode === "ceil") return !exact && !negative;
    if (mode === "floor") return !exact && negative;
    if (mode === "expand") return !exact;
    if (mode === "trunc") return false;
    if (half) return half > 0;
    return mode === "halfExpand" || (mode === "halfCeil" && !negative) || (mode === "halfFloor" && negative) || (mode === "halfEven" && last % 2 === 1);
  }
  // `x` (≥ 0) moved `shift` places left, rounded to `fraction` digits
  // after the point (a negative one rounds to tens, hundreds…) by `mode`
  // (half-expand when none) for a number of sign `negative`, as a
  // decimal string.
  function roundAt(x, shift, fraction, mode, negative) {
    var s = plain(x).split("."), int = s[0], frac = s[1] || "";
    while (int.length <= shift - fraction) int = "0" + int;
    frac = int.slice(int.length - shift) + frac; int = int.slice(0, int.length - shift);
    var digits = (int + frac + new Array(Math.max(fraction, 0) + 2).join("0")).split("").map(Number), keep = int.length + fraction;
    var up = away(mode || "halfExpand", digits[keep], digits.slice(keep + 1).some(Boolean), digits[keep - 1], negative);
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
  // Each formatter's roundingMode, and its digits and `format` once read.
  var state = new WeakMap();
  function stateOf(format) {
    var s = state.get(format);
    if (!s) state.set(format, (s = { mode: "halfExpand" }));
    return s;
  }
  function Compact(locales, options) {
    this.native = new Native(locales, options);
    this.options = options;
    this.mode = modeOf(options);
    if (roundsEven) stateOf(this.native).mode = this.mode;
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
  Compact.prototype.scale = function (x, negative) {
    var cells = this.cells, o = this.options, mode = this.mode;
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
    var rounded = roundAt(x, shift, digits(unrounded), mode, negative);
    // A carry into the next magnitude picks its cell again.
    if (rounded.split(".")[0].length + shift - 1 > magnitude) {
      var next = cell(magnitude + 1);
      if ((next ? next.power : 0) !== shift) {
        c = next; shift = next ? next.power : 0;
        rounded = roundAt(x, shift, digits(roundAt(x, shift, 30).replace(/\.?0+$/, "")), mode, negative);
      }
    }
    return { text: rounded, cell: c };
  };
  Compact.prototype.parts = function (n) {
    n = Number(n);
    if (!this.cells || !isFinite(n)) return partsOf(this.native, n);
    var s = this.scale(Math.abs(n), n < 0), o = this.options, text = s.text;
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
    r.roundingMode = this.mode;
    return r;
  };
  Object.defineProperty(Compact.prototype, "format", {
    configurable: true,
    get: function () {
      var self = this;
      return function (n) { return self.parts(n).map(function (p) { return p.value; }).join(""); };
    },
  });
  var bind = (Object.getOwnPropertyDescriptor(Native.prototype, "format") || {}).get, resolve = Native.prototype.resolvedOptions;
  if (bind) {
    // The digits the engine prints: significant digits; or fraction digits,
    // of the percent for a percent and of the mantissa in scientific
    // notation (which the engine prints only for a decimal). Compact with
    // no digit options, which the engine prints in full, names none: it
    // keeps its style's own digits, those a formatter without `notation`
    // names. A unit is left alone: the engine converts it to the locale's
    // unit first.
    var digitsOf = function (o) {
      if (o.style === "unit") return null;
      if (o.maximumSignificantDigits !== undefined) return { significant: o.maximumSignificantDigits };
      if (o.maximumFractionDigits !== undefined)
        return { fraction: o.maximumFractionDigits + (o.style === "percent" ? 2 : 0), scientific: o.notation === "scientific" && o.style === "decimal" };
      if (o.notation !== "compact") return null;
      return digitsOf(resolve.call(new Native(o.locale, { style: o.style, currency: o.currency, currencyDisplay: o.currencyDisplay })));
    };
    // `n` rounded to those digits.
    var roundFor = function (format, s, n) {
      if (typeof n === "bigint") return n;
      var x = +n;
      if (s.digits === undefined) s.digits = digitsOf(resolve.call(format));
      var d = s.digits;
      if (!d || x === 0 || !isFinite(x)) return x;
      var text = plain(Math.abs(x)), int = text.split(".")[0];
      // Most numbers need nothing: half-to-even rounds as half-expand but
      // at a tie, and up to 15 digits every conversion agrees on them.
      if (s.mode === "halfExpand" && d.fraction !== undefined && !d.scientific && text.length <= 16) {
        var decimals = text.length - int.length - 1;
        if (decimals !== d.fraction + 1 || text.charAt(text.length - 1) !== "5") return x;
      }
      var magnitude = int !== "0" ? int.length - 1 : -(/^0\.(0*)/.exec(text)[1].length + 1);
      var fraction = d.significant !== undefined ? d.significant - 1 - magnitude : d.scientific ? d.fraction - magnitude : d.fraction;
      var rounded = Number(roundAt(Math.abs(x), 0, fraction, s.mode, x < 0));
      // Past the largest double, the engine's own rounding prints it.
      return !isFinite(rounded) ? x : x < 0 ? -rounded : rounded;
    };
    Object.defineProperty(Native.prototype, "format", {
      configurable: true,
      get: function () {
        probe();
        if (!roundsEven) return bind.call(this);
        var s = stateOf(this), self = this;
        if (s.format) return s.format;
        var bound = bind.call(this);
        return (s.format = function (n) { return bound(roundFor(self, s, n)); });
      },
    });
    Object.defineProperty(Native.prototype, "resolvedOptions", {
      writable: true, configurable: true,
      value: function resolvedOptions() {
        probe();
        var r = resolve.call(this);
        if (roundsEven) r.roundingMode = stateOf(this).mode;
        return r;
      },
    });
  }
  function NumberFormat(locales, options) {
    probe();
    if (!hasCompact && options && options.notation === "compact") return new Compact(locales, options);
    var format = new.target ? Reflect.construct(Native, [locales, options], new.target) : Native(locales, options);
    if (roundsEven) stateOf(format).mode = modeOf(options);
    return format;
  }
  NumberFormat.prototype = Native.prototype;
  NumberFormat.supportedLocalesOf = Native.supportedLocalesOf;
  Object.defineProperty(global.Intl, "NumberFormat", { value: NumberFormat, writable: true, configurable: true });
  // `toLocaleString` is `new Intl.NumberFormat(locales, options).format(x)`
  // (ECMA-402 §16.4.1), and Apple's engine makes an NSNumberFormatter for
  // each call: a data module formatting a few hundred prices a second spent
  // most of its turn there. So a formatter is kept by the locales and
  // options it was made with, when they read the same whenever they are
  // read: strings, and a plain object of primitives.
  var kept = new Map();
  function formatterFor(locales, options) {
    var key = formatKey(locales, options);
    if (key === null) return new NumberFormat(locales, options);
    var f = kept.get(key);
    if (!f) {
      if (kept.size >= 64) kept.clear();
      kept.set(key, (f = new NumberFormat(locales, options)));
    }
    return f;
  }
  Object.defineProperty(Number.prototype, "toLocaleString", {
    writable: true, configurable: true,
    value: function toLocaleString(locales, options) {
      return formatterFor(locales, options).format(Number.prototype.valueOf.call(this));
    },
  });
})(globalThis);
