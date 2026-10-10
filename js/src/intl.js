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
(function (global) {
  "use strict";
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
    return new.target ? Reflect.construct(Native, [locales, options], new.target) : Native(locales, options);
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
})(globalThis);
