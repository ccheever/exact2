// The web's standard globals a data module reasonably expects and Hermes
// lacks, in JavaScript: `structuredClone`, `queueMicrotask`, and ES2023's
// copying array methods Hermes has not built (`toSorted`, and the typed
// arrays' `toReversed`/`toSorted`/`with`). Hermes only: a browser realm has
// its own. `AbortController` is Ibex's (vendor/ibex/crates/ibex2/src/bindings/abort.js),
// evaluated before this file. The list of what every executor has is in
// docs/reference.md ("What a data module can use"; pomodoro F4, calc F2).
// @ref LLP 1027 D10 — what the module can use
(function (global) {
  'use strict';
  var toString = Object.prototype.toString, defineProperty = Object.defineProperty, keysOf = Object.keys;
  var NativeDate = global.Date, NativeMap = global.Map, NativeSet = global.Set, NativeRegExp = global.RegExp;
  var TypedArray = Object.getPrototypeOf(Uint8Array.prototype).constructor;
  function method(target, name, fn) {
    if (typeof target[name] !== 'function') defineProperty(target, name, { value: fn, writable: true, configurable: true });
  }
  // The intrinsics, captured before the app runs, so an object's own
  // `constructor`, `valueOf`, `forEach` or getters never steer a copy: a
  // value is what its internal slots say it is, as the web reads it.
  function getter(proto, name) { return Object.getOwnPropertyDescriptor(proto, name).get; }
  var typedName = getter(TypedArray.prototype, Symbol.toStringTag);
  var typedBuffer = getter(TypedArray.prototype, 'buffer'), typedOffset = getter(TypedArray.prototype, 'byteOffset'), typedLength = getter(TypedArray.prototype, 'length');
  var viewBuffer = getter(DataView.prototype, 'buffer'), viewOffset = getter(DataView.prototype, 'byteOffset'), viewLength = getter(DataView.prototype, 'byteLength');
  var bufferLength = getter(ArrayBuffer.prototype, 'byteLength'), bufferSlice = ArrayBuffer.prototype.slice;
  var regexpSource = getter(NativeRegExp.prototype, 'source');
  var flagGetters = [['d', 'hasIndices'], ['g', 'global'], ['i', 'ignoreCase'], ['m', 'multiline'], ['s', 'dotAll'], ['u', 'unicode'], ['v', 'unicodeSets'], ['y', 'sticky']]
    .filter(function (f) { return Object.getOwnPropertyDescriptor(NativeRegExp.prototype, f[1]); })
    .map(function (f) { return [f[0], getter(NativeRegExp.prototype, f[1])]; });
  function regexpFlags(re) { return flagGetters.map(function (f) { return f[1].call(re) ? f[0] : ''; }).join(''); }
  var mapSize = getter(NativeMap.prototype, 'size'), setSize = getter(NativeSet.prototype, 'size');
  var mapEach = NativeMap.prototype.forEach, setEach = NativeSet.prototype.forEach, mapSet = NativeMap.prototype.set, setAdd = NativeSet.prototype.add;
  var getTime = NativeDate.prototype.getTime, arraySort = Array.prototype.sort, typedSort = TypedArray.prototype.sort, typedReverse = TypedArray.prototype.reverse;
  var typedConstructors = {};
  ['Int8Array', 'Uint8Array', 'Uint8ClampedArray', 'Int16Array', 'Uint16Array', 'Int32Array', 'Uint32Array',
    'Float32Array', 'Float64Array', 'BigInt64Array', 'BigUint64Array'].forEach(function (name) {
    if (typeof global[name] === 'function') typedConstructors[name] = global[name];
  });
  var boxes = [[Boolean, Boolean.prototype.valueOf], [Number, Number.prototype.valueOf], [String, String.prototype.valueOf]];
  if (typeof BigInt === 'function') boxes.push([BigInt, BigInt.prototype.valueOf]);
  function brand(fn, value) { try { fn.call(value); return true; } catch (e) { return false; } }
  function own(object, key, value) { defineProperty(object, key, { value: value, writable: true, enumerable: true, configurable: true }); }

  // HTML's structured clone, without transfer: what a browser copies, with
  // shared references and cycles kept, and DataCloneError for the rest.
  function cloneError(what) {
    return new global.DOMException(what + ' could not be cloned.', 'DataCloneError');
  }
  var errors = ['Error', 'EvalError', 'RangeError', 'ReferenceError', 'SyntaxError', 'TypeError', 'URIError'];
  function clone(value, memo) {
    if (typeof value === 'symbol') throw cloneError(String(value));
    if (typeof value === 'function') throw cloneError(value.name ? 'function ' + value.name : 'A function');
    if (value === null || typeof value !== 'object') return value;
    if (memo.has(value)) return memo.get(value);
    var out, typed = typedName.call(value);
    for (var b = 0; b < boxes.length && out === undefined; b++) {
      if (brand(boxes[b][1], value)) out = Object(boxes[b][1].call(value));
    }
    if (out !== undefined) { /* a boxed primitive */ }
    else if (brand(getTime, value)) out = new NativeDate(getTime.call(value));
    else if (brand(regexpSource, value) && value !== NativeRegExp.prototype) out = new NativeRegExp(regexpSource.call(value), regexpFlags(value));
    else if (brand(bufferLength, value)) out = bufferSlice.call(value, 0);
    else if (typed !== undefined) {
      out = new typedConstructors[typed](clone(typedBuffer.call(value), memo), typedOffset.call(value), typedLength.call(value));
    } else if (brand(viewLength, value)) {
      out = new DataView(clone(viewBuffer.call(value), memo), viewOffset.call(value), viewLength.call(value));
    } else if (brand(mapSize, value)) {
      out = new NativeMap();
      memo.set(value, out);
      mapEach.call(value, function (v, k) { mapSet.call(out, clone(k, memo), clone(v, memo)); });
      return out;
    } else if (brand(setSize, value)) {
      out = new NativeSet();
      memo.set(value, out);
      setEach.call(value, function (v) { setAdd.call(out, clone(v, memo)); });
      return out;
    } else if (toString.call(value) === '[object Error]') {
      var name = errors.indexOf(value.name) < 0 ? 'Error' : value.name;
      out = new global[name](value.message === undefined ? undefined : String(value.message));
      memo.set(value, out);
      if (Object.prototype.hasOwnProperty.call(value, 'cause')) own(out, 'cause', clone(value.cause, memo));
      return out;
    } else if (value instanceof Promise || /^\[object (Promise|WeakMap|WeakSet|WeakRef)\]$/.test(toString.call(value))) {
      throw cloneError(value instanceof Promise ? '#<Promise>' : toString.call(value).replace(/^\[object (\w+)\]$/, '#<$1>'));
    } else {
      // An array keeps its length; anything else is a plain object of its
      // own enumerable string keys, read through getters, as a browser does.
      // Each is an own data property: a key named `__proto__` stays a key.
      out = Array.isArray(value) ? new Array(value.length) : {};
      memo.set(value, out);
      var keys = keysOf(value);
      for (var i = 0; i < keys.length; i++) own(out, keys[i], clone(value[keys[i]], memo));
      return out;
    }
    memo.set(value, out);
    return out;
  }
  method(global, 'structuredClone', function structuredClone(value, options) {
    if (!arguments.length) throw new TypeError('structuredClone requires a value');
    if (options && options.transfer && Array.from(options.transfer).length)
      throw cloneError('A transfer list (a data source has no other realm to transfer to)');
    return clone(value, new NativeMap());
  });

  // A microtask: a callback's throw is reported, as the web reports it, and
  // never rejects anything the module awaits.
  method(global, 'queueMicrotask', function queueMicrotask(callback) {
    if (typeof callback !== 'function') throw new TypeError('queueMicrotask requires a function');
    Promise.resolve().then(function () {
      try { callback(); } catch (e) { global.console.error('Uncaught ' + (e && e.stack ? e.stack : String(e))); }
    });
  });

  // ES2023's copying methods, as the spec's own steps: read every index
  // into a fresh ordinary array (a hole reads as undefined) or a typed array
  // of the same intrinsic type, then the intrinsic in-place method on it.
  method(Array.prototype, 'toSorted', function toSorted(compare) {
    if (compare !== undefined && typeof compare !== 'function') throw new TypeError('toSorted: the comparator must be a function');
    var object = Object(this), length = Math.min(Math.max(Math.trunc(+object.length) || 0, 0), Number.MAX_SAFE_INTEGER);
    var copy = new Array(length);
    for (var i = 0; i < length; i++) copy[i] = object[i];
    return arraySort.call(copy, compare);
  });
  function sameType(array) {
    var name = typedName.call(array);
    if (name === undefined) throw new TypeError('not a typed array');
    return new typedConstructors[name](array);
  }
  method(TypedArray.prototype, 'toReversed', function toReversed() { return typedReverse.call(sameType(this)); });
  method(TypedArray.prototype, 'toSorted', function toSorted(compare) {
    if (compare !== undefined && typeof compare !== 'function') throw new TypeError('toSorted: the comparator must be a function');
    return typedSort.call(sameType(this), compare);
  });
  method(TypedArray.prototype, 'with', function (index, value) {
    var copy = sameType(this), length = typedLength.call(copy), at = Math.trunc(+index) || 0;
    if (at < 0) at += length;
    var number = typeof value === 'bigint' ? value : +value;
    if (at < 0 || at >= length) throw new RangeError('with: index out of range');
    copy[at] = number;
    return copy;
  });

  // ECMA-402's `Intl.Locale`, which Hermes has not built (issue #118): a
  // tag parsed and canonicalized as UTS 35 says (case, sorted variants,
  // extensions and `-u-` keywords, a `true` value dropped), the options that
  // replace its parts, their getters, and the Intl Locale Info proposal's
  // `getWeekInfo()` as Chrome 154 answers it. The week comes from the region:
  // the tag's, its `-u-rg-`, or the likely region of its language (and
  // script), and `-u-fw-` names the first day. The tables are CLDR's
  // weekData and likely subtags, read once from Chrome (`und-XX`, the
  // maximized two-letter languages), keeping only what differs from the
  // default (Monday, a Saturday-Sunday weekend). Aliases are not
  // canonicalized (`iw` stays `iw`); no `maximize`, `minimize` or the
  // other `get…()` lists (docs/reference.md).
  if (global.Intl && typeof global.Intl.Locale !== 'function') (function () {
    var FIRST = { 7: 'AG AS BD BR BS BT BW BZ CA CO DM DO ET GT GU HK HN ID IL IN IS JM JP KE KH KR LA MH MM MO MT MX MZ NI NP PA PE PH PK PR PT PY SA SG SV TH TT TW UM US VE VI WS YE ZA ZW',
      6: 'AF BH DJ DZ EG IQ IR JO KW LY OM QA SD SY', 5: 'MV' };
    var WEEKEND = { 56: 'BH DZ EG IL IQ JO KW LY OM QA SA SD SY YE', 45: 'AF', 7: 'IN UG', 5: 'IR' };
    var LIKELY = 'aaET aeIR afZA amET arEG asIN bhIN bnBD chGU crCA dvMV dzBT enUS faIR gnPY guIN heIL hiIN idID ikUS inID isIS iuCA iwIL jaJP jvID jwID kiKE kmKH knIN koKR ksIN lgUG loLA mhMH mlIN mrIN mtMT myMM ndZW neNP nrZA nvUS ojCA omET orIN paIN psAF ptBR quPE saIN sdPK smWS snZW ssZA stZA suID taIN teIN thTH tiET tlPH tnZA tsZA urPK veZA xhZA zuZA ' +
      'undUS filPH yueHK ckbIQ hawUS chrUS cebPH kokIN maiIN satIN mniIN doiIN brxIN zh-HantTW zh-BopoTW yue-HansCN pa-ArabPK sd-DevaIN az-ArabIR ku-ArabIQ uz-ArabAF tg-ArabPK';
    function table(source) {
      var out = {};
      keysOf(source).forEach(function (value) { source[value].split(' ').forEach(function (region) { out[region] = value; }); });
      return out;
    }
    var first = table(FIRST), weekend = table(WEEKEND), likely = {};
    LIKELY.split(' ').forEach(function (entry) { likely[entry.slice(0, -2)] = entry.slice(-2); });
    var DAYS = ['mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun'];
    var slots = new WeakMap(), sorted = function (a, b) { return a < b ? -1 : a > b ? 1 : 0; };
    function invalid() { throw new RangeError('Incorrect locale information provided'); }
    var LANGUAGE = /^([a-z]{2,3}|[a-z]{5,8})$/, SCRIPT = /^[a-z]{4}$/, REGION = /^([a-z]{2}|\d{3})$/;
    var VARIANT = /^([a-z0-9]{5,8}|\d[a-z0-9]{3})$/, TYPE = /^[a-z0-9]{3,8}(-[a-z0-9]{3,8})*$/;
    // A unicode_locale_id, lower-cased, into its parts; RangeError if it is not one.
    function parse(tag) {
      var subtags = tag.toLowerCase().split('-'), i = 0, parts = { variants: [], keywords: {}, attributes: [], extensions: [], x: '' };
      var next = function (pattern) { return i < subtags.length && pattern.test(subtags[i]) ? subtags[i++] : undefined; };
      if (!(parts.language = next(LANGUAGE))) invalid();
      parts.script = next(SCRIPT);
      parts.region = next(REGION);
      for (var variant; (variant = next(VARIANT));) {
        if (parts.variants.indexOf(variant) >= 0) invalid();
        parts.variants.push(variant);
      }
      var seen = {};
      while (i < subtags.length) {
        var singleton = subtags[i++];
        if (!/^[0-9a-z]$/.test(singleton) || seen[singleton]) invalid();
        seen[singleton] = true;
        if (singleton === 'x') {
          parts.x = subtags.slice(i).join('-');
          if (!/^[a-z0-9]{1,8}(-[a-z0-9]{1,8})*$/.test(parts.x)) invalid();
          break;
        }
        var start = i;
        while (i < subtags.length && subtags[i].length > 1) {
          if (!/^[a-z0-9]{2,8}$/.test(subtags[i])) invalid();
          i++;
        }
        if (i === start) invalid();
        var body = subtags.slice(start, i);
        if (singleton !== 'u') { parts.extensions.push(singleton + '-' + body.join('-')); continue; }
        for (var j = 0; j < body.length && body[j].length > 2; j++) if (parts.attributes.indexOf(body[j]) < 0) parts.attributes.push(body[j]);
        while (j < body.length) {
          var key = body[j++], value = [];
          if (!/^[a-z0-9][a-z]$/.test(key)) invalid();
          while (j < body.length && body[j].length > 2) value.push(body[j++]);
          // A key with no type reads as `true`, as Chrome's getters answer; `kf` alone stays empty.
          if (!(key in parts.keywords)) parts.keywords[key] = value.length || key === 'kf' ? value.join('-') : 'true';
        }
      }
      return parts;
    }
    function baseName(p) {
      return [p.language, p.script && p.script[0].toUpperCase() + p.script.slice(1), p.region && p.region.toUpperCase()]
        .concat(p.variants.slice().sort(sorted)).filter(Boolean).join('-');
    }
    function serialize(p) {
      var keys = keysOf(p.keywords).sort(sorted), u = p.attributes.slice().sort(sorted);
      keys.forEach(function (key) { u.push(key); if (p.keywords[key] && p.keywords[key] !== 'true') u.push(p.keywords[key]); });
      var extensions = p.extensions.slice();
      if (u.length) extensions.push('u-' + u.join('-'));
      return [baseName(p)].concat(extensions.sort(sorted), p.x ? ['x-' + p.x] : []).join('-');
    }
    function option(options, name, values) {
      var value = options[name];
      if (value === undefined) return undefined;
      value = name === 'numeric' ? String(!!value) : String(value);
      if (values && values.indexOf(value) < 0) throw new RangeError('Value ' + value + ' out of range for locale options property ' + name);
      return value;
    }
    function Locale(tag) {
      var options = arguments[1];
      if (!new.target) throw new TypeError("Constructor Intl.Locale requires 'new'");
      if (typeof tag !== 'string' && (tag === null || typeof tag !== 'object')) throw new TypeError("First argument to Intl.Locale constructor can't be empty or missing");
      var text = slots.has(tag) ? slots.get(tag).tag : String(tag);
      if (text === '') throw new RangeError("First argument to Intl.Locale constructor can't be empty or missing");
      var parts = parse(text);
      if (options === null) throw new TypeError('Cannot convert undefined or null to object');
      options = options === undefined ? {} : Object(options);
      [['language', LANGUAGE], ['script', SCRIPT], ['region', REGION]].forEach(function (field) {
        var value = option(options, field[0]);
        if (value === undefined) return;
        if (!field[1].test(value.toLowerCase()) || value.indexOf('-') >= 0) invalid();
        parts[field[0]] = value.toLowerCase();
      });
      // `variants` replaces the tag's: each a variant subtag, none twice.
      var variants = option(options, 'variants');
      if (variants !== undefined) {
        variants = variants.toLowerCase().split('-');
        variants.forEach(function (variant, k) { if (!VARIANT.test(variant) || variants.indexOf(variant) !== k) invalid(); });
        parts.variants = variants;
      }
      [['calendar', 'ca'], ['collation', 'co'], ['firstDayOfWeek', 'fw'], ['hourCycle', 'hc', ['h11', 'h12', 'h23', 'h24']],
        ['caseFirst', 'kf', ['upper', 'lower', 'false']], ['numeric', 'kn'], ['numberingSystem', 'nu']].forEach(function (field) {
        var value = option(options, field[0], field[2]);
        if (value === undefined) return;
        if (field[1] === 'fw' && /^[0-7]$/.test(value)) value = DAYS[(+value + 6) % 7];
        if (!TYPE.test(value.toLowerCase())) invalid();
        parts.keywords[field[1]] = value.toLowerCase();
      });
      slots.set(this, { parts: parts, tag: serialize(parts) });
    }
    function slot(locale) {
      if (!slots.has(locale)) throw new TypeError('Method Intl.Locale.prototype called on an incompatible receiver');
      return slots.get(locale);
    }
    var proto = Locale.prototype;
    function define(name, value) { defineProperty(proto, name, { value: value, writable: true, configurable: true }); }
    function get(name, read) { defineProperty(proto, name, { get: function () { return read(slot(this).parts); }, configurable: true }); }
    define('toString', function toString() { return slot(this).tag; });
    get('baseName', baseName);
    get('language', function (p) { return p.language; });
    get('script', function (p) { return p.script && p.script[0].toUpperCase() + p.script.slice(1); });
    get('region', function (p) { return p.region && p.region.toUpperCase(); });
    get('variants', function (p) { return p.variants.length ? p.variants.slice().sort(sorted).join('-') : undefined; });
    [['calendar', 'ca'], ['caseFirst', 'kf'], ['collation', 'co'], ['firstDayOfWeek', 'fw'], ['hourCycle', 'hc'], ['numberingSystem', 'nu']].forEach(function (field) {
      get(field[0], function (p) { return p.keywords[field[1]]; });
    });
    get('numeric', function (p) { return p.keywords.kn === '' || p.keywords.kn === 'true'; });
    define('getWeekInfo', function getWeekInfo() {
      var p = slot(this).parts, rg = /^([a-z]{2})[a-z0-9]{1,4}$/.exec(p.keywords.rg || '');
      var region = rg ? rg[1].toUpperCase() : p.region ? p.region.toUpperCase() : p.script && likely[p.language + '-' + p.script[0].toUpperCase() + p.script.slice(1)] || likely[p.language];
      // The ISO 8601 calendar's week starts on Monday wherever it is.
      var fw = DAYS.indexOf(p.keywords.fw), end = weekend[region] || '67';
      return { firstDay: fw >= 0 ? fw + 1 : p.keywords.ca === 'iso8601' ? 1 : +(first[region] || 1), weekend: end.split('').map(Number) };
    });
    defineProperty(proto, Symbol.toStringTag, { value: 'Intl.Locale', configurable: true });
    defineProperty(global.Intl, 'Locale', { value: Locale, writable: true, configurable: true });
  })();

  // Ibex's AbortSignal times out on a timer; a data source has none (time
  // is an argument, LLP 1027.000), so the one timer-backed member refuses.
  // Its internal hooks (`__ibex2_abort`) stay for the prelude's `fetch`,
  // which takes and deletes them before any module runs.
  if (global.AbortSignal) global.AbortSignal.timeout = function () {
    throw new Error('AbortSignal.timeout() is unavailable in data sources: there are no timers; pass time as an argument');
  };
})(globalThis);
