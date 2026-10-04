// The HTML structured clone algorithm over the value space Hermes exposes.
// @ref LLP 0059.000#310-atob--btoa-structuredclone-blob-customevent--pure-ungated — clone stays inside the engine and v1 refuses transfer
(function (global) {
  "use strict";

  // Capture every intrinsic the clone uses while bootstrap still owns the
  // realm. Application changes after this point cannot redirect traversal,
  // brand checks, allocation, or collection mutation.
  var FunctionCall = Function.prototype.call;
  var FunctionBind = Function.prototype.bind;
  function uncurry(fn) { return FunctionCall.call(FunctionBind, FunctionCall, fn); }
  var functionCall = uncurry(Function.prototype.call);

  var ObjectCtor = Object;
  var ObjectPrototype = Object.prototype;
  var objectKeys = Object.keys;
  var objectGetPrototypeOf = Object.getPrototypeOf;
  var objectDefineProperty = Object.defineProperty;
  var objectGetOwnPropertyDescriptor = Object.getOwnPropertyDescriptor;
  var objectHasOwn = uncurry(Object.prototype.hasOwnProperty);
  var objectIsPrototypeOf = uncurry(Object.prototype.isPrototypeOf);

  var ArrayCtor = Array;
  var arrayIsArray = Array.isArray;
  var arrayBufferIsView = ArrayBuffer.isView;
  var iteratorSymbol = Symbol.iterator;

  var MapCtor = Map;
  var mapHas = uncurry(Map.prototype.has);
  var mapGet = uncurry(Map.prototype.get);
  var mapSet = uncurry(Map.prototype.set);
  var mapForEach = uncurry(Map.prototype.forEach);
  var SetCtor = Set;
  var setHas = uncurry(Set.prototype.has);
  var setAdd = uncurry(Set.prototype.add);
  var setForEach = uncurry(Set.prototype.forEach);

  var WeakMapCtor = WeakMap;
  var weakMapHas = uncurry(WeakMap.prototype.has);
  var WeakSetCtor = WeakSet;
  var weakSetHas = uncurry(WeakSet.prototype.has);
  var weakRefDeref = typeof WeakRef === "function" ? uncurry(WeakRef.prototype.deref) : null;
  var registryUnregisterMethod = typeof FinalizationRegistry === "function"
    ? uncurry(FinalizationRegistry.prototype.unregister) : null;
  var registryToken = {};
  var registryUnregister = registryUnregisterMethod
    ? function (value) { registryUnregisterMethod(value, registryToken); }
    : null;
  var PromisePrototype = Promise.prototype;

  var ArrayBufferCtor = ArrayBuffer;
  var arrayBufferLength = uncurry(
    Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "byteLength").get
  );
  var arrayBufferDetachedDescriptor = Object.getOwnPropertyDescriptor(
    ArrayBuffer.prototype, "detached"
  );
  var arrayBufferDetached = arrayBufferDetachedDescriptor && arrayBufferDetachedDescriptor.get
    ? uncurry(arrayBufferDetachedDescriptor.get)
    : null;
  var Uint8ArrayCtor = Uint8Array;
  var typedArraySet = uncurry(Uint8Array.prototype.set);
  var TypedArrayPrototype = objectGetPrototypeOf(Uint8Array.prototype);
  var typedArrayTag = uncurry(
    Object.getOwnPropertyDescriptor(TypedArrayPrototype, Symbol.toStringTag).get
  );
  var typedArrayBuffer = uncurry(
    Object.getOwnPropertyDescriptor(TypedArrayPrototype, "buffer").get
  );
  var typedArrayOffset = uncurry(
    Object.getOwnPropertyDescriptor(TypedArrayPrototype, "byteOffset").get
  );
  var typedArrayLength = uncurry(
    Object.getOwnPropertyDescriptor(TypedArrayPrototype, "length").get
  );
  var DataViewCtor = DataView;
  var dataViewBuffer = uncurry(
    Object.getOwnPropertyDescriptor(DataView.prototype, "buffer").get
  );
  var dataViewOffset = uncurry(
    Object.getOwnPropertyDescriptor(DataView.prototype, "byteOffset").get
  );
  var dataViewLength = uncurry(
    Object.getOwnPropertyDescriptor(DataView.prototype, "byteLength").get
  );

  var typedArrayConstructors = {};
  ["Int8Array", "Uint8Array", "Uint8ClampedArray", "Int16Array",
   "Uint16Array", "Int32Array", "Uint32Array", "Float32Array",
   "Float64Array", "BigInt64Array", "BigUint64Array", "Float16Array"]
    .forEach(function (name) {
      if (typeof global[name] === "function") typedArrayConstructors[name] = global[name];
    });

  var DateCtor = Date;
  var dateValue = uncurry(Date.prototype.valueOf);
  var RegExpCtor = RegExp;
  var regexpSource = uncurry(
    Object.getOwnPropertyDescriptor(RegExp.prototype, "source").get
  );
  var regexpFlagNames = [
    ["hasIndices", "d"], ["global", "g"], ["ignoreCase", "i"],
    ["multiline", "m"], ["dotAll", "s"], ["unicode", "u"],
    ["unicodeSets", "v"], ["sticky", "y"]
  ];
  var regexpFlags = [];
  regexpFlagNames.forEach(function (entry) {
    var descriptor = Object.getOwnPropertyDescriptor(RegExp.prototype, entry[0]);
    if (descriptor && descriptor.get) regexpFlags.push([uncurry(descriptor.get), entry[1]]);
  });

  var booleanValue = uncurry(Boolean.prototype.valueOf);
  var numberValue = uncurry(Number.prototype.valueOf);
  var stringValue = uncurry(String.prototype.valueOf);
  var bigintValue = typeof BigInt === "function" ? uncurry(BigInt.prototype.valueOf) : null;
  var symbolValue = uncurry(Symbol.prototype.valueOf);
  var textEncoderEncodeDescriptor = typeof global.TextEncoder === "function"
    ? objectGetOwnPropertyDescriptor(global.TextEncoder.prototype, "encode")
    : null;
  var textEncoderEncode = textEncoderEncodeDescriptor && textEncoderEncodeDescriptor.value
    ? uncurry(textEncoderEncodeDescriptor.value)
    : null;
  var textDecoderEncodingDescriptor = typeof global.TextDecoder === "function"
    ? objectGetOwnPropertyDescriptor(global.TextDecoder.prototype, "encoding")
    : null;
  var textDecoderEncoding = textDecoderEncodingDescriptor && textDecoderEncodingDescriptor.get
    ? uncurry(textDecoderEncodingDescriptor.get)
    : null;

  // Engine Intl objects are platform objects too (not serializable), but only
  // the Linux replacements write the brand registry; Apple's Hermes provides
  // Intl natively. ECMA-402 requires each of these methods to throw on a value
  // without the matching internal slots, so a captured call is a slot check.
  var intlChecks = [];
  if (typeof global.Intl === "object" && global.Intl !== null) {
    var intlNames = ["Collator", "DateTimeFormat", "DisplayNames", "ListFormat",
      "NumberFormat", "PluralRules", "RelativeTimeFormat", "Segmenter"];
    for (var n = 0; n < intlNames.length; n++) {
      var IntlCtor = global.Intl[intlNames[n]];
      var resolved = typeof IntlCtor === "function" && IntlCtor.prototype
        ? objectGetOwnPropertyDescriptor(IntlCtor.prototype, "resolvedOptions")
        : null;
      if (resolved && typeof resolved.value === "function") intlChecks.push(uncurry(resolved.value));
    }
    var IntlLocale = global.Intl.Locale;
    var maximize = typeof IntlLocale === "function" && IntlLocale.prototype
      ? objectGetOwnPropertyDescriptor(IntlLocale.prototype, "maximize")
      : null;
    if (maximize && typeof maximize.value === "function") intlChecks.push(uncurry(maximize.value));
  }

  var errorIsError = Error.isError;
  var errorConstructors = {
    Error: Error,
    EvalError: EvalError,
    RangeError: RangeError,
    ReferenceError: ReferenceError,
    SyntaxError: SyntaxError,
    TypeError: TypeError,
    URIError: URIError
  };
  var StringCtor = String;
  var TypeErrorPrototype = TypeError.prototype;
  var DOMExceptionCtor = global.DOMException;
  var QuotaExceededErrorCtor = global.QuotaExceededError;
  // Headers creates the one bootstrap-only identity registry before any
  // platform factory runs. Every factory captures its writer. This final PURE
  // script captures the reader and erases both helpers before application
  // code. A public tag, prototype, getter, or native handle is never consulted
  // to decide whether a value is a platform object.
  var platformBrand = global.__ibex2_platform_brand;
  delete global.__ibex2_brand;
  delete global.__ibex2_platform_brand;

  function dataCloneError(message) {
    throw new DOMExceptionCtor(message, "DataCloneError");
  }

  function hasBrand(check, value) {
    try { check(value); return true; } catch (_) { return false; }
  }

  function platformRecord(value) {
    return typeof platformBrand === "function" ? platformBrand(value) : undefined;
  }

  function mapFallbackDetachedError(error, message) {
    if (objectIsPrototypeOf(TypeErrorPrototype, error)) dataCloneError(message);
    throw error;
  }

  function copyArrayBuffer(source) {
    if (arrayBufferDetached) {
      if (arrayBufferDetached(source)) {
        dataCloneError("Detached ArrayBuffers cannot be cloned");
      }
      var result = new ArrayBufferCtor(arrayBufferLength(source));
      typedArraySet(new Uint8ArrayCtor(result), new Uint8ArrayCtor(source));
      return result;
    }
    try {
      var fallbackResult = new ArrayBufferCtor(arrayBufferLength(source));
      typedArraySet(new Uint8ArrayCtor(fallbackResult), new Uint8ArrayCtor(source));
      return fallbackResult;
    } catch (error) {
      // Engines without the `detached` getter expose detachment here instead.
      mapFallbackDetachedError(error, "Detached ArrayBuffers cannot be cloned");
    }
  }

  function tryBoxed(valueOf, value) {
    if (!valueOf) return null;
    try { return { value: valueOf(value) }; } catch (_) { return null; }
  }

  function cloneError(value, memory) {
    var name = StringCtor(value.name);
    if (!objectHasOwn(errorConstructors, name)) name = "Error";
    var hasMessage = objectHasOwn(value, "message");
    var result = hasMessage
      ? new errorConstructors[name](StringCtor(value.message))
      : new errorConstructors[name]();
    mapSet(memory, value, result);
    if (objectHasOwn(value, "cause")) {
      objectDefineProperty(result, "cause", {
        value: cloneAny(value.cause, memory),
        writable: true, configurable: true
      });
    }
    if (objectHasOwn(value, "stack")) {
      objectDefineProperty(result, "stack", {
        value: StringCtor(value.stack), writable: true, configurable: true
      });
    }
    return result;
  }

  function cloneObject(value, memory) {
    if (mapHas(memory, value)) return mapGet(memory, value);

    // All observable checks below either accept an ordinary live Proxy as the
    // documented limitation or use an internal slot. A revoked Proxy is
    // unambiguously detectable: even [[GetPrototypeOf]] throws.
    try { objectGetPrototypeOf(value); }
    catch (_) { dataCloneError("Revoked Proxy objects cannot be cloned"); }

    // Values whose prototype can look ordinary but whose data lives in engine
    // slots are classified before the ordinary-object path.
    var boxed = tryBoxed(booleanValue, value);
    if (boxed) {
      var boxedBoolean = new Boolean(boxed.value);
      mapSet(memory, value, boxedBoolean);
      return boxedBoolean;
    }
    boxed = tryBoxed(numberValue, value);
    if (boxed) {
      var boxedNumber = new Number(boxed.value);
      mapSet(memory, value, boxedNumber);
      return boxedNumber;
    }
    boxed = tryBoxed(stringValue, value);
    if (boxed) {
      var boxedString = new String(boxed.value);
      mapSet(memory, value, boxedString);
      return boxedString;
    }
    boxed = tryBoxed(bigintValue, value);
    if (boxed) {
      var boxedBigInt = ObjectCtor(boxed.value);
      mapSet(memory, value, boxedBigInt);
      return boxedBigInt;
    }
    if (hasBrand(symbolValue, value)) dataCloneError("Symbol objects cannot be cloned");

    var platform = platformRecord(value);
    if (platform !== undefined) {
      if (platform.kind === "DOMException") {
        var domData = platform.data;
        var domResult = new DOMExceptionCtor(domData.message, domData.name);
        mapSet(memory, value, domResult);
        return domResult;
      }
      if (platform.kind === "QuotaExceededError") {
        var quotaData = platform.data;
        var quotaOptions = {};
        if (quotaData.quota !== null) quotaOptions.quota = quotaData.quota;
        if (quotaData.requested !== null) quotaOptions.requested = quotaData.requested;
        var quotaResult = new QuotaExceededErrorCtor(quotaData.message, quotaOptions);
        mapSet(memory, value, quotaResult);
        return quotaResult;
      }
      dataCloneError("This platform object cannot be cloned");
    }

    if (arrayIsArray(value)) {
      // Array.isArray also returns true for a non-revoked Proxy around an
      // array. JavaScript exposes no general Proxy predicate; those proxies
      // are the documented detection limit of this engine-side shim.
      var arrayResult = new ArrayCtor(value.length);
      mapSet(memory, value, arrayResult);
      cloneEnumerableProperties(value, arrayResult, memory);
      return arrayResult;
    }

    if (hasBrand(arrayBufferLength, value)) {
      var bufferResult = copyArrayBuffer(value);
      mapSet(memory, value, bufferResult);
      return bufferResult;
    }

    if (arrayBufferIsView(value)) {
      if (hasBrand(dataViewBuffer, value)) {
        var dataBuffer = dataViewBuffer(value);
        var clonedDataView;
        if (arrayBufferDetached) {
          if (arrayBufferDetached(dataBuffer)) {
            dataCloneError("Views on detached ArrayBuffers cannot be cloned");
          }
          clonedDataView = new DataViewCtor(
            cloneObject(dataBuffer, memory),
            dataViewOffset(value),
            dataViewLength(value)
          );
        } else {
          try {
            clonedDataView = new DataViewCtor(
              cloneObject(dataBuffer, memory),
              dataViewOffset(value),
              dataViewLength(value)
            );
          } catch (error) {
            mapFallbackDetachedError(
              error, "Views on detached ArrayBuffers cannot be cloned"
            );
          }
        }
        mapSet(memory, value, clonedDataView);
        return clonedDataView;
      }
      var typedBuffer = typedArrayBuffer(value);
      var tag = typedArrayTag(value);
      var Type = typedArrayConstructors[tag];
      if (!Type) dataCloneError("Unsupported typed array");
      var clonedView;
      if (arrayBufferDetached) {
        if (arrayBufferDetached(typedBuffer)) {
          dataCloneError("Views on detached ArrayBuffers cannot be cloned");
        }
        clonedView = new Type(
          cloneObject(typedBuffer, memory),
          typedArrayOffset(value),
          typedArrayLength(value)
        );
      } else {
        try {
          clonedView = new Type(
            cloneObject(typedBuffer, memory),
            typedArrayOffset(value),
            typedArrayLength(value)
          );
        } catch (error) {
          mapFallbackDetachedError(
            error, "Views on detached ArrayBuffers cannot be cloned"
          );
        }
      }
      mapSet(memory, value, clonedView);
      return clonedView;
    }

    if (hasBrand(mapHas, value)) {
      var mapResult = new MapCtor();
      mapSet(memory, value, mapResult);
      mapForEach(value, function (entryValue, entryKey) {
        mapSet(mapResult, cloneAny(entryKey, memory), cloneAny(entryValue, memory));
      });
      return mapResult;
    }

    if (hasBrand(setHas, value)) {
      var setResult = new SetCtor();
      mapSet(memory, value, setResult);
      setForEach(value, function (entryValue) {
        setAdd(setResult, cloneAny(entryValue, memory));
      });
      return setResult;
    }

    if (hasBrand(dateValue, value)) {
      var dateResult = new DateCtor(dateValue(value));
      mapSet(memory, value, dateResult);
      return dateResult;
    }

    if (hasBrand(regexpSource, value)) {
      var flags = "";
      for (var f = 0; f < regexpFlags.length; f++) {
        if (regexpFlags[f][0](value)) flags += regexpFlags[f][1];
      }
      var regexpResult = new RegExpCtor(regexpSource(value), flags);
      mapSet(memory, value, regexpResult);
      return regexpResult;
    }

    if (hasBrand(weakMapHas, value) || hasBrand(weakSetHas, value) ||
        hasBrand(weakRefDeref, value) || hasBrand(registryUnregister, value)) {
      dataCloneError("Weak collections and references cannot be cloned");
    }
    if (hasBrand(textEncoderEncode, value) ||
        hasBrand(textDecoderEncoding, value)) {
      dataCloneError("Text codec objects cannot be cloned");
    }
    for (var c = 0; c < intlChecks.length; c++) {
      if (hasBrand(intlChecks[c], value)) dataCloneError("Intl objects cannot be cloned");
    }
    if (objectIsPrototypeOf(PromisePrototype, value)) {
      dataCloneError("Promise objects cannot be cloned");
    }

    if (errorIsError(value)) return cloneError(value, memory);

    // Ordinary instances deserialize as plain objects. This intentionally
    // drops a user-defined or null prototype; only enumerable own string keys
    // cross the clone.
    var objectResult = {};
    mapSet(memory, value, objectResult);
    cloneEnumerableProperties(value, objectResult, memory);
    return objectResult;
  }

  function cloneEnumerableProperties(source, target, memory) {
    var keys = objectKeys(source);
    for (var i = 0; i < keys.length; i++) {
      var key = keys[i];
      // HTML step 26.4 re-checks ownership after every preceding getter: the
      // key snapshot is not permission to serialize a property since deleted.
      if (!objectHasOwn(source, key)) continue;
      // Reading invokes an enumerable own getter exactly once. Defining a data
      // property avoids the legacy __proto__ setter and discards descriptors,
      // as StructuredSerialize/Deserialize requires.
      var cloned = cloneAny(source[key], memory);
      objectDefineProperty(target, key, {
        value: cloned, enumerable: true, writable: true, configurable: true
      });
    }
  }

  function cloneAny(value, memory) {
    var type = typeof value;
    if (type === "function") dataCloneError("Functions cannot be cloned");
    if (type === "symbol") dataCloneError("Symbols cannot be cloned");
    if (value === null || type !== "object") return value;
    return cloneObject(value, memory);
  }

  function convertTransferSequence(transfer) {
    var transferType = typeof transfer;
    if (transfer === null ||
        (transferType !== "object" && transferType !== "function")) {
      throw new TypeError("transfer must be a sequence of objects");
    }

    // WebIDL GetMethod reads @@iterator exactly once. Iterating through this
    // wrapper uses that one result, and for-of performs IteratorClose if the
    // element conversion below throws.
    var iteratorMethod = transfer[iteratorSymbol];
    if (typeof iteratorMethod !== "function") {
      throw new TypeError("transfer must be a sequence of objects");
    }
    var iterator = functionCall(iteratorMethod, transfer);
    var iteratorType = typeof iterator;
    if (iterator === null ||
        (iteratorType !== "object" && iteratorType !== "function")) {
      throw new TypeError("transfer iterator must be an object");
    }
    var iterable = {};
    objectDefineProperty(iterable, iteratorSymbol, {
      value: function () { return iterator; }
    });
    var hasEntries = false;
    for (var item of iterable) {
      var itemType = typeof item;
      if (item === null || (itemType !== "object" && itemType !== "function")) {
        throw new TypeError("transfer entries must be objects");
      }
      hasEntries = true;
    }
    return hasEntries;
  }

  function structuredClone(value) {
    var options = arguments.length > 1 ? arguments[1] : undefined;
    if (options !== undefined && options !== null) {
      var optionsType = typeof options;
      if (optionsType !== "object" && optionsType !== "function") {
        throw new TypeError("options must be a dictionary");
      }
      var transfer = options.transfer;
      if (transfer !== undefined) {
        if (convertTransferSequence(transfer)) {
          dataCloneError("transfer is not supported");
        }
      }
    }
    return cloneAny(value, new MapCtor());
  }

  global.structuredClone = structuredClone;
})(globalThis);
