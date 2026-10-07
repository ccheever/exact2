// Locale-sensitive String case mapping for the Linux vanilla-Hermes profile.
// Observable coercion remains JavaScript; Unicode mapping crosses once to the
// host as exact UTF-16LE and is computed by ICU.
(function (global) {
  "use strict";

  var nativeCase = global.__ibex2_intl_case;
  delete global.__ibex2_intl_case;

  // Capture every intrinsic before application code and hardening. concat's
  // argument conversion is ECMAScript ToString (unlike String(Symbol()), it
  // rejects symbols) and therefore preserves generic-receiver behavior.
  var apply = Reflect.apply;
  var concat = String.prototype.concat;
  var canonicalize = Intl.getCanonicalLocales;
  var defineProperty = Object.defineProperty;
  var NumberFormat = Intl.NumberFormat;
  var resolvedOptions = NumberFormat.prototype.resolvedOptions;
  // Resolved on first use, not at install: opening ICU's first number
  // formatter costs about 6 ms on Windows's OS ICU, and a program that never
  // maps case without a locale should not pay it.
  // @ref LLP 0057.000#511-windows-intl-uses-the-os-icu — first INTL install cost
  var defaultLocale;

  function currentDefaultLocale() {
    if (defaultLocale === undefined) {
      defaultLocale = apply(resolvedOptions, new NumberFormat(), []).locale;
    }
    return defaultLocale;
  }

  function toString(value) {
    return apply(concat, "", [value]);
  }

  function selectLocale(locales) {
    // CanonicalizeLocaleList is observable and must run even when the string
    // being mapped is empty.
    var requested = apply(canonicalize, Intl, [locales]);
    if (requested.length === 0) return currentDefaultLocale();
    // ECMA-402 locale case mapping uses only the first requested locale. ICU
    // applies its casing-data fallback when that locale is unsupported; it is
    // incorrect to scan forward for a later locale supported by Collator.
    return requested[0];
  }

  function map(receiver, locales, mode) {
    if (receiver === null || receiver === undefined) {
      throw new TypeError("String locale case method called on null or undefined");
    }
    // ECMA-402 orders receiver ToString before CanonicalizeLocaleList.
    var value = toString(receiver);
    var locale = selectLocale(locales);
    return nativeCase(mode, locale, value);
  }

  function toLocaleLowerCase() {
    return map(this, arguments.length === 0 ? undefined : arguments[0], "lower");
  }

  function toLocaleUpperCase() {
    return map(this, arguments.length === 0 ? undefined : arguments[0], "upper");
  }

  defineProperty(String.prototype, "toLocaleLowerCase", {
    value: toLocaleLowerCase,
    writable: true,
    enumerable: false,
    configurable: true,
  });
  defineProperty(String.prototype, "toLocaleUpperCase", {
    value: toLocaleUpperCase,
    writable: true,
    enumerable: false,
    configurable: true,
  });
})(globalThis);
