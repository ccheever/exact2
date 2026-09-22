// ECMA-402 2020 NumberFormat object plumbing for Linux Hermes.
// Observable JavaScript coercion order stays here; all normalized policy and
// formatting semantics live in Rust and ICU through the private binding.
(function (global) {
  "use strict";

  var raw = global.__ibex2_intl_number_format;
  delete global.__ibex2_intl_number_format;

  var IntlObject = global.Intl;
  var canonicalLocales = IntlObject.getCanonicalLocales;
  var reflectApply = Reflect.apply;
  var objectCreate = Object.create;
  var objectDefineProperty = Object.defineProperty;
  var objectDefineProperties = Object.defineProperties;
  var numberValueOf = Number.prototype.valueOf;
  var bigintValueOf = typeof BigInt === "function" ? BigInt.prototype.valueOf : null;
  var bigintToString = typeof BigInt === "function" ? BigInt.prototype.toString : null;
  var stringConcat = String.prototype.concat;
  var floor = Math.floor;
  var boundFormats = new WeakMap();

  var simpleUnits = Object.create(null);
  var unitNames = [
    "acre", "bit", "byte", "celsius", "centimeter", "day", "degree",
    "fahrenheit", "fluid-ounce", "foot", "gallon", "gigabit", "gigabyte",
    "gram", "hectare", "hour", "inch", "kilobit", "kilobyte", "kilogram",
    "kilometer", "liter", "megabit", "megabyte", "meter", "mile",
    "mile-scandinavian", "milliliter", "millimeter", "millisecond", "minute",
    "month", "ounce", "percent", "petabyte", "pound", "second", "stone",
    "terabit", "terabyte", "week", "yard", "year"
  ];
  for (var unitIndex = 0; unitIndex < unitNames.length; unitIndex++) {
    simpleUnits[unitNames[unitIndex]] = true;
  }

  function optionObject(options) {
    if (options === undefined) return objectCreate(null);
    if (options === null) throw new TypeError("options must not be null");
    return Object(options);
  }

  function stringOption(options, name, allowed, fallback) {
    var value = options[name];
    if (value === undefined) return fallback;
    // Unlike the String constructor, abstract ToString rejects Symbols.
    value = reflectApply(stringConcat, "", [value]);
    if (allowed !== undefined && allowed.indexOf(value) < 0) {
      throw new RangeError(name + " is not a supported value");
    }
    return value;
  }

  function defaultNumberOption(value, minimum, maximum, fallback, name) {
    if (value === undefined) return fallback;
    // Unary plus is ToNumber: unlike the Number constructor, it rejects both
    // primitive and boxed BigInt values.
    value = +value;
    if (value !== value || value < minimum || value > maximum) {
      throw new RangeError(name + " is outside its allowed range");
    }
    return floor(value);
  }

  function validUnicodeType(value) {
    var parts = value.split("-");
    for (var i = 0; i < parts.length; i++) {
      if (!/^[0-9A-Za-z]{3,8}$/.test(parts[i])) return false;
    }
    return true;
  }

  function validCurrency(value) {
    return /^[A-Za-z]{3}$/.test(value);
  }

  function asciiUpper(value) {
    return value.replace(/[a-z]/g, function (letter) {
      return String.fromCharCode(letter.charCodeAt(0) - 32);
    });
  }

  function validUnit(value) {
    if (simpleUnits[value]) return true;
    var marker = "-per-";
    var first = value.indexOf(marker);
    return first > 0 && first === value.lastIndexOf(marker) &&
      simpleUnits[value.slice(0, first)] === true &&
      simpleUnits[value.slice(first + marker.length)] === true;
  }

  function canonicalize(locales) {
    return reflectApply(canonicalLocales, IntlObject, [locales]);
  }

  function normalize(locales, options) {
    var requested = canonicalize(locales);
    var opts = optionObject(options);
    var localeMatcher = stringOption(opts, "localeMatcher", ["lookup", "best fit"], "best fit");
    var numberingSystem = stringOption(opts, "numberingSystem", undefined, undefined);
    if (numberingSystem !== undefined && !validUnicodeType(numberingSystem)) {
      throw new RangeError("numberingSystem is not a Unicode locale type");
    }
    var style = stringOption(opts, "style", ["decimal", "percent", "currency", "unit"], "decimal");
    var currency = stringOption(opts, "currency", undefined, undefined);
    if (currency !== undefined && !validCurrency(currency)) {
      throw new RangeError("currency is not a well-formed currency code");
    }
    if (style === "currency" && currency === undefined) {
      throw new TypeError("currency is required with currency style");
    }
    var currencyDisplay = stringOption(opts, "currencyDisplay", ["code", "symbol", "narrowSymbol", "name"], "symbol");
    var currencySign = stringOption(opts, "currencySign", ["standard", "accounting"], "standard");
    var unit = stringOption(opts, "unit", undefined, undefined);
    if (unit !== undefined && !validUnit(unit)) {
      throw new RangeError("unit is not a sanctioned unit identifier");
    }
    if (style === "unit" && unit === undefined) {
      throw new TypeError("unit is required with unit style");
    }
    var unitDisplay = stringOption(opts, "unitDisplay", ["short", "narrow", "long"], "short");
    if (currency !== undefined) currency = asciiUpper(currency);

    var minimumFractionDefault = style === "currency" ? raw.currencyDigits(currency) : 0;
    var maximumFractionDefault = style === "currency" ? minimumFractionDefault :
      (style === "percent" ? 0 : 3);

    var notation = stringOption(opts, "notation", ["standard", "scientific", "engineering", "compact"], "standard");
    var minimumIntegerDigits = defaultNumberOption(opts.minimumIntegerDigits, 1, 21, 1, "minimumIntegerDigits");
    // ECMA-402 2020 reads all four properties in this order, then coerces only
    // the significant pair or the fraction pair selected by precedence.
    var minimumFractionDigits = opts.minimumFractionDigits;
    var maximumFractionDigits = opts.maximumFractionDigits;
    var minimumSignificantDigits = opts.minimumSignificantDigits;
    var maximumSignificantDigits = opts.maximumSignificantDigits;
    if (minimumSignificantDigits !== undefined || maximumSignificantDigits !== undefined) {
      minimumSignificantDigits = defaultNumberOption(minimumSignificantDigits, 1, 21, 1, "minimumSignificantDigits");
      maximumSignificantDigits = defaultNumberOption(maximumSignificantDigits, minimumSignificantDigits, 21, 21, "maximumSignificantDigits");
      minimumFractionDigits = undefined;
      maximumFractionDigits = undefined;
    } else if (minimumFractionDigits !== undefined || maximumFractionDigits !== undefined) {
      minimumFractionDigits = defaultNumberOption(minimumFractionDigits, 0, 20, minimumFractionDefault, "minimumFractionDigits");
      var actualMaximumDefault = Math.max(minimumFractionDigits, maximumFractionDefault);
      maximumFractionDigits = defaultNumberOption(maximumFractionDigits, minimumFractionDigits, 20, actualMaximumDefault, "maximumFractionDigits");
    } else if (notation !== "compact") {
      minimumFractionDigits = minimumFractionDefault;
      maximumFractionDigits = maximumFractionDefault;
    } else {
      minimumFractionDigits = undefined;
      maximumFractionDigits = undefined;
    }
    var compactDisplay = stringOption(opts, "compactDisplay", ["short", "long"], "short");
    var useGroupingValue = opts.useGrouping;
    var useGrouping = useGroupingValue === undefined ? true : Boolean(useGroupingValue);
    var signDisplay = stringOption(opts, "signDisplay", ["auto", "never", "always", "exceptZero"], "auto");

    return [requested.join("\n"), localeMatcher, numberingSystem, style,
      currency, currencyDisplay, currencySign, unit, unitDisplay, notation,
      minimumIntegerDigits, minimumFractionDigits, maximumFractionDigits,
      minimumSignificantDigits, maximumSignificantDigits, compactDisplay,
      useGrouping, signDisplay];
  }

  function toPrimitiveNumber(value) {
    if (value === null || (typeof value !== "object" && typeof value !== "function")) {
      return value;
    }
    var exotic = value[Symbol.toPrimitive];
    if (exotic !== undefined && exotic !== null) {
      if (typeof exotic !== "function") throw new TypeError("Symbol.toPrimitive is not callable");
      var result = reflectApply(exotic, value, ["number"]);
      if (result === null || (typeof result !== "object" && typeof result !== "function")) return result;
      throw new TypeError("cannot convert object to primitive value");
    }
    for (var i = 0; i < 2; i++) {
      var method = value[i === 0 ? "valueOf" : "toString"];
      if (typeof method === "function") {
        var primitive = reflectApply(method, value, []);
        if (primitive === null || (typeof primitive !== "object" && typeof primitive !== "function")) return primitive;
      }
    }
    throw new TypeError("cannot convert object to primitive value");
  }

  function numeric(value) {
    var primitive = toPrimitiveNumber(value);
    if (typeof primitive === "bigint") {
      return ["bigint", reflectApply(bigintToString, primitive, [])];
    }
    return ["number", Number(primitive)];
  }

  function NumberFormat() {
    var locales = arguments[0];
    var options = arguments[1];
    if (new.target === undefined) return new NumberFormat(locales, options);
    var normalized = normalize(locales, options);
    raw.initialize.apply(raw, [this].concat(normalized));
  }

  // Concise methods and arrows are not constructors. The selected Intl
  // methods must reject `new` while the Intl constructor above remains an
  // ordinary callable constructor.
  var publicMethods = {
    getFormat() {
      raw.assertReceiver(this);
      var bound = boundFormats.get(this);
      if (bound === undefined) {
        var receiver = this;
        bound = (value) => {
          var input = numeric(value);
          return raw.format(receiver, input[0], input[1]);
        };
        objectDefineProperty(bound, "name", { value: "", configurable: true });
        boundFormats.set(this, bound);
      }
      return bound;
    },
    formatToParts(value) {
      raw.assertReceiver(this);
      var input = numeric(value);
      return raw.formatToParts(this, input[0], input[1]);
    },
    resolvedOptions() {
      return raw.resolvedOptions(this);
    },
    supportedLocalesOf(locales) {
      var requested = canonicalize(locales);
      var opts = optionObject(arguments[1]);
      var matcher = stringOption(opts, "localeMatcher", ["lookup", "best fit"], "best fit");
      var supported = raw.supportedLocalesOf(requested.join("\n"), matcher);
      return supported === "" ? [] : supported.split("\n");
    }
  };
  var getFormat = publicMethods.getFormat;
  var formatToParts = publicMethods.formatToParts;
  var resolvedOptions = publicMethods.resolvedOptions;
  var supportedLocalesOf = publicMethods.supportedLocalesOf;

  objectDefineProperties(NumberFormat.prototype, {
    constructor: { value: NumberFormat, writable: true, configurable: true },
    format: { get: getFormat, configurable: true },
    formatToParts: { value: formatToParts, writable: true, configurable: true },
    resolvedOptions: { value: resolvedOptions, writable: true, configurable: true }
  });
  objectDefineProperty(getFormat, "name", { value: "get format", configurable: true });
  objectDefineProperty(NumberFormat, "prototype", { writable: false });
  objectDefineProperty(NumberFormat.prototype, Symbol.toStringTag, {
    value: "Intl.NumberFormat", configurable: true
  });
  objectDefineProperty(NumberFormat, "supportedLocalesOf", {
    value: supportedLocalesOf, writable: true, configurable: true
  });
  objectDefineProperty(IntlObject, "NumberFormat", {
    value: NumberFormat, writable: true, configurable: true
  });

  var numberToLocaleString = {
    toLocaleString() {
      var value = reflectApply(numberValueOf, this, []);
      return new NumberFormat(arguments[0], arguments[1]).format(value);
    }
  }.toLocaleString;
  objectDefineProperty(Number.prototype, "toLocaleString", {
    value: numberToLocaleString, writable: true, configurable: true
  });

  if (bigintValueOf !== null) {
    var bigintToLocaleString = {
      toLocaleString() {
        var value = reflectApply(bigintValueOf, this, []);
        return new NumberFormat(arguments[0], arguments[1]).format(value);
      }
    }.toLocaleString;
    objectDefineProperty(BigInt.prototype, "toLocaleString", {
      value: bigintToLocaleString, writable: true, configurable: true
    });
  }
})(globalThis);
