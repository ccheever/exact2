// Linux Intl.DateTimeFormat object/coercion layer. ICU never leaks into the
// JavaScript model: `owner` is an opaque JSI NativeState object, and every
// operation reaches the same Rust-owned formatter through a private function.
//
// This file is compiled to bytecode and evaluated as a completion-value
// factory during trusted initialization. It creates no temporary globals.
//
// @ref LLP 0057#31-what-goes-in-rust-and-what-does-not — JavaScript owns observable object shape and value plumbing
(function (create, format, parts, partType, partValue, resolved, supported, canonicalTimeZone) {
  "use strict";

  var IntlObject = Intl;
  var canonicalLocales = IntlObject.getCanonicalLocales;
  var DateIntrinsic = Date;
  var dateNow = DateIntrinsic.now;
  var dateGetTime = DateIntrinsic.prototype.getTime;
  var apply = Reflect.apply;
  var stringConcat = String.prototype.concat;
  var ObjectIntrinsic = Object;
  var defineProperty = ObjectIntrinsic.defineProperty;
  var createObject = ObjectIntrinsic.create;
  var states = new WeakMap();

  function isObject(value) {
    return value !== null && (typeof value === "object" || typeof value === "function");
  }

  function toStringValue(value) {
    // concat performs abstract ToString on its arguments: string-hint object
    // conversion and Symbol rejection, unlike the String constructor.
    return apply(stringConcat, "", [value]);
  }

  function stringOption(options, name, allowed, fallback) {
    var value = options[name];
    if (value === undefined) return fallback;
    value = toStringValue(value);
    if (allowed !== undefined && allowed.indexOf(value) < 0)
      throw new RangeError(name + " is not a supported value");
    return value;
  }

  function booleanOption(options, name) {
    var value = options[name];
    return value === undefined ? undefined : !!value;
  }

  function validUnicodeType(value) {
    return /^[0-9A-Za-z]{3,8}(?:-[0-9A-Za-z]{3,8})*$/.test(value);
  }

  function dataProperty(object, name, value) {
    defineProperty(object, name, {
      value: value,
      writable: true,
      enumerable: true,
      configurable: true,
    });
  }

  // ES2020 13.1.2 plus the dateStyle/timeStyle proposal's same-year extension.
  // These Gets are deliberate: InitializeDateTimeFormat later reads the
  // component properties again. Doing either pass a third time is observably
  // wrong for accessors and proxies.
  function toDateTimeOptions(input, required, defaults) {
    var base;
    if (input === undefined) base = null;
    else {
      if (input === null) throw new TypeError("Intl.DateTimeFormat options must not be null");
      base = ObjectIntrinsic(input);
    }
    var options = createObject(base);
    var needDefaults = true;
    var i;
    if (required === "date" || required === "any") {
      var dateNames = ["weekday", "year", "month", "day"];
      for (i = 0; i < dateNames.length; i++) {
        if (options[dateNames[i]] !== undefined) needDefaults = false;
      }
    }
    if (required === "time" || required === "any") {
      var timeNames = ["hour", "minute", "second"];
      for (i = 0; i < timeNames.length; i++) {
        if (options[timeNames[i]] !== undefined) needDefaults = false;
      }
    }
    var dateStyle = options.dateStyle;
    var timeStyle = options.timeStyle;
    if (dateStyle !== undefined || timeStyle !== undefined) needDefaults = false;
    if (required === "date" && timeStyle !== undefined)
      throw new TypeError("timeStyle is not allowed in Date.prototype.toLocaleDateString");
    if (required === "time" && dateStyle !== undefined)
      throw new TypeError("dateStyle is not allowed in Date.prototype.toLocaleTimeString");
    if (needDefaults && (defaults === "date" || defaults === "all")) {
      dataProperty(options, "year", "numeric");
      dataProperty(options, "month", "numeric");
      dataProperty(options, "day", "numeric");
    }
    if (needDefaults && (defaults === "time" || defaults === "all")) {
      dataProperty(options, "hour", "numeric");
      dataProperty(options, "minute", "numeric");
      dataProperty(options, "second", "numeric");
    }
    return options;
  }

  function localeList(locales) {
    return apply(canonicalLocales, IntlObject, [locales]).join("\n");
  }

  function normalized(locales, input, required, defaults) {
    var requested = localeList(locales);
    var options = toDateTimeOptions(input, required, defaults);
    var matcher = stringOption(options, "localeMatcher", ["lookup", "best fit"], "best fit");
    var calendar = stringOption(options, "calendar", undefined, undefined);
    if (calendar !== undefined && !validUnicodeType(calendar))
      throw new RangeError("calendar is not a Unicode locale type");
    var numbering = stringOption(options, "numberingSystem", undefined, undefined);
    if (numbering !== undefined && !validUnicodeType(numbering))
      throw new RangeError("numberingSystem is not a Unicode locale type");
    var hour12 = booleanOption(options, "hour12");
    var hourCycle = stringOption(options, "hourCycle", ["h11", "h12", "h23", "h24"], undefined);

    // Validation belongs here, before any component Get. The native helper
    // returns the canonical IANA name (or the host default with no argument).
    var rawZone = options.timeZone;
    var timeZone = rawZone === undefined
      ? canonicalTimeZone()
      : canonicalTimeZone(toStringValue(rawZone));

    var weekday = stringOption(options, "weekday", ["narrow", "short", "long"], undefined);
    var era = stringOption(options, "era", ["narrow", "short", "long"], undefined);
    var year = stringOption(options, "year", ["2-digit", "numeric"], undefined);
    var month = stringOption(options, "month", ["2-digit", "numeric", "narrow", "short", "long"], undefined);
    var day = stringOption(options, "day", ["2-digit", "numeric"], undefined);
    var hour = stringOption(options, "hour", ["2-digit", "numeric"], undefined);
    var minute = stringOption(options, "minute", ["2-digit", "numeric"], undefined);
    var second = stringOption(options, "second", ["2-digit", "numeric"], undefined);
    var timeZoneName = stringOption(options, "timeZoneName", ["short", "long"], undefined);
    var formatMatcher = stringOption(options, "formatMatcher", ["basic", "best fit"], "best fit");
    var dateStyle = stringOption(options, "dateStyle", ["full", "long", "medium", "short"], undefined);
    var timeStyle = stringOption(options, "timeStyle", ["full", "long", "medium", "short"], undefined);

    return [
      requested, matcher, calendar, numbering, hour12, hourCycle, timeZone,
      weekday, era, year, month, day, hour, minute, second, timeZoneName,
      formatMatcher, dateStyle, timeStyle,
    ];
  }

  function initialize(object, locales, options, required, defaults) {
    var owner = apply(create, undefined, normalized(locales, options, required, defaults));
    states.set(object, { owner: owner, bound: undefined });
    return object;
  }

  function direct(value) {
    if (!isObject(value)) throw new TypeError("incompatible Intl.DateTimeFormat receiver");
    var state = states.get(value);
    if (state === undefined) throw new TypeError("incompatible Intl.DateTimeFormat receiver");
    return state;
  }

  function timeValue(date) {
    return date === undefined ? apply(dateNow, DateIntrinsic, []) : +date;
  }

  function DateTimeFormat() {
    var locales = arguments[0];
    var options = arguments[1];
    if (new.target === undefined) return new DateTimeFormat(locales, options);
    return initialize(this, locales, options, "any", "date");
  }

  // Concise methods and arrows are not constructors. The public Intl methods
  // reject `new`, while DateTimeFormat itself remains callable/constructable.
  var publicMethods = {
    supportedLocalesOf(locales) {
      var requested = localeList(locales);
      var options;
      if (arguments.length < 2 || arguments[1] === undefined) options = createObject(null);
      else {
        if (arguments[1] === null) throw new TypeError("options must not be null");
        options = ObjectIntrinsic(arguments[1]);
      }
      var matcher = stringOption(options, "localeMatcher", ["lookup", "best fit"], "best fit");
      var result = supported(requested, matcher);
      return result === "" ? [] : result.split("\n");
    },
    getFormat() {
      var state = direct(this);
      if (state.bound === undefined) {
        var owner = state.owner;
        var bound = (date) => format(owner, timeValue(date));
        defineProperty(bound, "name", { value: "", configurable: true });
        state.bound = bound;
      }
      return state.bound;
    },
    formatToParts(date) {
      // Brand first: a fake receiver must fail before coercing `date`.
      var owner = direct(this).owner;
      var count = parts(owner, timeValue(date));
      var result = [];
      for (var i = 0; i < count; i++) {
        result.push({ type: partType(owner, i), value: partValue(owner, i) });
      }
      return result;
    },
    resolvedOptions() {
      var owner = direct(this).owner;
      var result = {};
      var names = [
        "locale", "calendar", "numberingSystem", "timeZone", "hourCycle", "hour12",
        "weekday", "era", "year", "month", "day", "hour", "minute", "second",
        "timeZoneName", "dateStyle", "timeStyle",
      ];
      for (var i = 0; i < names.length; i++) {
        var value = resolved(owner, i);
        if (value !== undefined) result[names[i]] = value;
      }
      return result;
    }
  };
  var supportedLocalesOf = publicMethods.supportedLocalesOf;
  var getFormat = publicMethods.getFormat;
  var formatToParts = publicMethods.formatToParts;
  var resolvedOptions = publicMethods.resolvedOptions;

  defineProperty(DateTimeFormat, "supportedLocalesOf", {
    value: supportedLocalesOf,
    writable: true,
    enumerable: false,
    configurable: true,
  });

  var prototype = DateTimeFormat.prototype;
  defineProperty(getFormat, "name", { value: "get format", configurable: true });
  defineProperty(prototype, "format", {
    get: getFormat,
    enumerable: false,
    configurable: true,
  });

  defineProperty(prototype, "formatToParts", {
    value: formatToParts,
    writable: true,
    enumerable: false,
    configurable: true,
  });

  defineProperty(prototype, "resolvedOptions", {
    value: resolvedOptions,
    writable: true,
    enumerable: false,
    configurable: true,
  });

  defineProperty(prototype, Symbol.toStringTag, {
    value: "Intl.DateTimeFormat",
    writable: false,
    enumerable: false,
    configurable: true,
  });

  defineProperty(IntlObject, "DateTimeFormat", {
    value: DateTimeFormat,
    writable: true,
    enumerable: false,
    configurable: true,
  });
  defineProperty(DateTimeFormat, "prototype", { writable: false });

  function dateLocale(locales, options, required, defaults) {
    // Date brand checking precedes all locale/options observation.
    var millis = apply(dateGetTime, this, []);
    if (millis !== millis) return "Invalid Date";
    // §16.4 performs ToDateTimeOptions before constructing DateTimeFormat.
    // Construction then performs its own any/date pass; both are observable.
    var prepared = toDateTimeOptions(options, required, defaults);
    var vector = normalized(locales, prepared, "any", "date");
    return format(apply(create, undefined, vector), millis);
  }

  var dateMethods = {
    toLocaleString() {
      return dateLocale.call(this, arguments[0], arguments[1], "any", "all");
    },
    toLocaleDateString() {
      return dateLocale.call(this, arguments[0], arguments[1], "date", "date");
    },
    toLocaleTimeString() {
      return dateLocale.call(this, arguments[0], arguments[1], "time", "time");
    }
  };
  defineProperty(DateIntrinsic.prototype, "toLocaleString", {
    value: dateMethods.toLocaleString,
    writable: true,
    enumerable: false,
    configurable: true,
  });
  defineProperty(DateIntrinsic.prototype, "toLocaleDateString", {
    value: dateMethods.toLocaleDateString,
    writable: true,
    enumerable: false,
    configurable: true,
  });
  defineProperty(DateIntrinsic.prototype, "toLocaleTimeString", {
    value: dateMethods.toLocaleTimeString,
    writable: true,
    enumerable: false,
    configurable: true,
  });
});
