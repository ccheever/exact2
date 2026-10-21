// Blob, File, and FormData are JavaScript shapes over ordinary ArrayBuffers.
// Rust is involved only when an ordered FormData entry list becomes RFC 7578
// wire bytes. No Blob or File handle table exists.
// @ref LLP 0057.000#l6--blob-file-formdata — JS owns shapes and bytes; Rust owns multipart wire encoding
(function (global) {
  "use strict";

  var brand = global.__ibex2_brand || function (value) { return value; };

  // Capture every intrinsic the implementation calls. Application code may
  // replace prototype methods after bootstrap without reaching private bytes
  // or changing the result of an already-installed binding.
  var ObjectCtor = global.Object;
  var ObjectCreate = ObjectCtor.create;
  var ObjectDefineProperty = ObjectCtor.defineProperty;
  var ObjectDefineProperties = ObjectCtor.defineProperties;
  var ObjectGetOwnPropertyDescriptor = ObjectCtor.getOwnPropertyDescriptor;
  var ObjectGetPrototypeOf = ObjectCtor.getPrototypeOf;
  var ArrayCtor = global.Array;
  var ArrayIsArray = ArrayCtor.isArray;
  var arrayIterator = ArrayCtor.prototype[global.Symbol.iterator];
  var arrayPush = ArrayCtor.prototype.push;
  var arraySplice = ArrayCtor.prototype.splice;
  var ArrayBufferCtor = global.ArrayBuffer;
  var ArrayBufferIsView = ArrayBufferCtor.isView;
  var Uint8ArrayCtor = global.Uint8Array;
  var DataViewCtor = global.DataView;
  var WeakMapCtor = global.WeakMap;
  var PromiseCtor = global.Promise;
  var PromiseResolve = PromiseCtor.resolve;
  var StringCtor = global.String;
  var NumberCtor = global.Number;
  var TypeErrorCtor = global.TypeError;
  var RangeErrorCtor = global.RangeError;
  var MathFloor = global.Math.floor;
  var MathMax = global.Math.max;
  var MathMin = global.Math.min;
  var DateNow = global.Date.now;
  var ReflectApply = global.Reflect.apply;
  var SymbolIterator = global.Symbol.iterator;
  var SymbolToStringTag = global.Symbol.toStringTag;
  var stringCharCodeAt = global.String.prototype.charCodeAt;
  var stringFromCharCode = global.String.fromCharCode;
  var stringToLowerCase = global.String.prototype.toLowerCase;
  var intrinsicHasInstance = global.Function.prototype[global.Symbol.hasInstance];
  var weakGet = WeakMapCtor.prototype.get;
  var weakSet = WeakMapCtor.prototype.set;
  var arrayBufferLength = ObjectGetOwnPropertyDescriptor(ArrayBufferCtor.prototype, "byteLength").get;
  var typedArrayPrototype = ObjectGetPrototypeOf(Uint8ArrayCtor.prototype);
  var typedBuffer = ObjectGetOwnPropertyDescriptor(typedArrayPrototype, "buffer").get;
  var typedOffset = ObjectGetOwnPropertyDescriptor(typedArrayPrototype, "byteOffset").get;
  var typedLength = ObjectGetOwnPropertyDescriptor(typedArrayPrototype, "byteLength").get;
  var dataBuffer = ObjectGetOwnPropertyDescriptor(DataViewCtor.prototype, "buffer").get;
  var dataOffset = ObjectGetOwnPropertyDescriptor(DataViewCtor.prototype, "byteOffset").get;
  var dataLength = ObjectGetOwnPropertyDescriptor(DataViewCtor.prototype, "byteLength").get;
  var encodeMultipart = global.__ibex2_multipart_encode;
  var newBoundary = global.__ibex2_multipart_boundary;
  var encodeText = global.__ibex2_text_encode;
  var decodeText = global.__ibex2_text_decode;
  delete global.__ibex2_multipart_encode;
  delete global.__ibex2_multipart_boundary;

  var blobs = new WeakMapCtor();
  var files = new WeakMapCtor();
  var forms = new WeakMapCtor();
  var formIterators = new WeakMapCtor();

  function call(fn, receiver, args) {
    return ReflectApply(fn, receiver, args);
  }

  function get(map, value) {
    return call(weakGet, map, [value]);
  }

  function set(map, value, state) {
    call(weakSet, map, [value, state]);
  }

  function requireBrand(map, value, name) {
    var state = get(map, value);
    if (!state) throw new TypeErrorCtor("not a " + name);
    return state;
  }

  function domString(value) {
    if (typeof value === "symbol") throw new TypeErrorCtor("cannot convert a Symbol to a string");
    return StringCtor(value);
  }

  // Web IDL USVString conversion: preserve scalar values and replace each
  // unpaired surrogate with U+FFFD before UTF-8 encoding or name exposure.
  function usv(value) {
    var input = domString(value), output = "";
    for (var i = 0; i < input.length; i++) {
      var first = call(stringCharCodeAt, input, [i]);
      if (first >= 0xd800 && first <= 0xdbff) {
        if (i + 1 < input.length) {
          var second = call(stringCharCodeAt, input, [i + 1]);
          if (second >= 0xdc00 && second <= 0xdfff) {
            output += call(stringFromCharCode, null, [first, second]);
            i++;
            continue;
          }
        }
        output += "\ufffd";
      } else if (first >= 0xdc00 && first <= 0xdfff) {
        output += "\ufffd";
      } else {
        output += call(stringFromCharCode, null, [first]);
      }
    }
    return output;
  }

  function normalizedType(value) {
    var type = domString(value === undefined ? "" : value);
    for (var i = 0; i < type.length; i++) {
      var code = call(stringCharCodeAt, type, [i]);
      if (code < 0x20 || code > 0x7e) return "";
    }
    return call(stringToLowerCase, type, []);
  }

  // Web IDL `[Clamp] long long`: clamp infinities and out-of-range values,
  // turn NaN into zero, and round finite fractions to nearest with ties even.
  function clampLongLong(value) {
    var number = NumberCtor(value);
    if (number !== number || number === 0) return 0;
    if (number <= -9223372036854775808) return -9223372036854775808;
    if (number >= 9223372036854775807) return 9223372036854775807;
    var lower = MathFloor(number);
    var fraction = number - lower;
    if (fraction < 0.5) return lower;
    if (fraction > 0.5) return lower + 1;
    return lower % 2 === 0 ? lower : lower + 1;
  }

  function longLong(value) {
    var number = NumberCtor(value);
    if (number !== number || number === 0 || number === Infinity || number === -Infinity) return 0;
    var truncated = number < 0 ? -MathFloor(-number) : MathFloor(number);
    var modulus = 18446744073709551616;
    var converted = truncated % modulus;
    if (converted < 0) converted += modulus;
    return converted >= 9223372036854775808 ? converted - modulus : converted;
  }

  function copy(buffer, offset, length) {
    var source = new Uint8ArrayCtor(buffer, offset, length);
    var target = new Uint8ArrayCtor(length);
    for (var i = 0; i < length; i++) target[i] = source[i];
    return call(typedBuffer, target, []);
  }

  function arrayBufferSpan(value) {
    try {
      return { buffer: value, offset: 0, length: call(arrayBufferLength, value, []) };
    } catch (_) {}
    if (!ArrayBufferIsView(value)) return null;
    try {
      return {
        buffer: call(typedBuffer, value, []),
        offset: call(typedOffset, value, []),
        length: call(typedLength, value, [])
      };
    } catch (_) {
      return {
        buffer: call(dataBuffer, value, []),
        offset: call(dataOffset, value, []),
        length: call(dataLength, value, [])
      };
    }
  }

  function sequenceIterator(value) {
    if ((typeof value !== "object" || value === null) && typeof value !== "function") {
      throw new TypeErrorCtor("Blob parts must be a sequence");
    }
    var method = ArrayIsArray(value) ? arrayIterator : value[SymbolIterator];
    if (typeof method !== "function") throw new TypeErrorCtor("Blob parts must be iterable");
    var iterator = call(method, value, []);
    if (iterator === null || typeof iterator !== "object") {
      throw new TypeErrorCtor("Blob parts iterator is invalid");
    }
    return iterator;
  }

  function makeBlobBytes(parts) {
    var chunks = [], length = 0, iterator = sequenceIterator(parts), step;
    // Convert each yielded part before asking the iterator for the next one.
    // Web IDL sequence conversion is observably interleaved with a mutable
    // input array (the upstream Blob constructor WPT exercises pop/unshift).
    while (!(step = iterator.next()).done) {
      var part = step.value, bytes, state = get(blobs, part);
      if (state) {
        bytes = state.bytes;
        var blobLength = call(arrayBufferLength, bytes, []);
        bytes = copy(bytes, 0, blobLength);
      } else {
        var span = arrayBufferSpan(part);
        if (span) bytes = copy(span.buffer, span.offset, span.length);
        else bytes = encodeText(usv(part));
      }
      var chunkLength = call(arrayBufferLength, bytes, []);
      call(arrayPush, chunks, [bytes]);
      length += chunkLength;
      if (length > 9007199254740991) throw new RangeErrorCtor("Blob is too large");
    }
    var target = new Uint8ArrayCtor(length), at = 0;
    for (var c = 0; c < chunks.length; c++) {
      var chunk = new Uint8ArrayCtor(chunks[c]);
      for (var j = 0; j < chunk.length; j++) target[at++] = chunk[j];
    }
    return call(typedBuffer, target, []);
  }

  function initializeBlob(object, parts, options) {
    if (parts === undefined) parts = [];
    if (options === undefined || options === null) options = {};
    if (typeof options !== "object" && typeof options !== "function") {
      throw new TypeErrorCtor("Blob options must be an object");
    }
    // Web IDL converts arguments from left to right. In particular, a failing
    // BlobPart conversion must happen before any option getter is observed.
    var bytes = makeBlobBytes(parts);
    var endingsValue = options.endings;
    var endings = endingsValue === undefined ? "transparent" : domString(endingsValue);
    if (endings === "native") {
      throw new TypeErrorCtor("Blob endings 'native' is not supported; use 'transparent'");
    }
    if (endings !== "transparent") throw new TypeErrorCtor("invalid Blob endings value");
    installBlobState(object, {
      bytes: bytes,
      size: call(arrayBufferLength, bytes, []),
      type: normalizedType(options.type)
    });
  }

  function installBlobState(object, state) {
    set(blobs, object, state);
    brand(object, "Blob", {
      clone: function () { return trustedBlobState(state); }
    });
    return object;
  }

  function Blob() {
    if (!call(intrinsicHasInstance, Blob, [this])) throw new TypeErrorCtor("Blob must be constructed with new");
    initializeBlob(this, arguments[0], arguments[1]);
  }

  ObjectDefineProperties(Blob.prototype, {
    size: {
      get: function () { return requireBrand(blobs, this, "Blob").size; },
      enumerable: true,
      configurable: true
    },
    type: {
      get: function () { return requireBrand(blobs, this, "Blob").type; },
      enumerable: true,
      configurable: true
    }
  });

  Blob.prototype.slice = function (start, end, contentType) {
    var state = requireBrand(blobs, this, "Blob"), size = state.size;
    var relativeStart = start === undefined ? 0 : clampLongLong(start);
    var first = relativeStart < 0 ? MathMax(size + relativeStart, 0) : MathMin(relativeStart, size);
    var relativeEnd = end === undefined ? size : clampLongLong(end);
    var last = relativeEnd < 0 ? MathMax(size + relativeEnd, 0) : MathMin(relativeEnd, size);
    var length = MathMax(last - first, 0);
    return trustedBlob(copy(state.bytes, first, length), normalizedType(contentType));
  };

  Blob.prototype.arrayBuffer = function () {
    var state = requireBrand(blobs, this, "Blob");
    return call(PromiseResolve, PromiseCtor, [copy(state.bytes, 0, state.size)]);
  };

  Blob.prototype.bytes = function () {
    var state = requireBrand(blobs, this, "Blob");
    return call(PromiseResolve, PromiseCtor, [new Uint8ArrayCtor(copy(state.bytes, 0, state.size))]);
  };

  Blob.prototype.text = function () {
    var state = requireBrand(blobs, this, "Blob");
    return call(PromiseResolve, PromiseCtor, [decodeText(state.bytes)]);
  };

  ObjectDefineProperty(Blob.prototype, SymbolToStringTag, {
    value: "Blob",
    configurable: true
  });
  var BlobPrototype = Blob.prototype;

  function trustedBlob(bytes, type) {
    return trustedBlobState({
      bytes: bytes,
      size: call(arrayBufferLength, bytes, []),
      type: normalizedType(type)
    });
  }

  function trustedBlobState(state) {
    return installBlobState(ObjectCreate(BlobPrototype), state);
  }

  function installFileState(object, blobState, fileState) {
    set(blobs, object, blobState);
    set(files, object, fileState);
    brand(object, "File", {
      clone: function () { return trustedFileState(blobState, fileState); }
    });
    return object;
  }

  function trustedFileState(blobState, fileState) {
    return installFileState(ObjectCreate(FilePrototype), blobState, fileState);
  }

  function initializeFile(object, parts, name, options) {
    // File's required fileBits does not have Blob's optional [] default.
    var bytes = makeBlobBytes(parts);
    name = usv(name);
    if (options === undefined || options === null) options = {};
    if (typeof options !== "object" && typeof options !== "function") {
      throw new TypeErrorCtor("File options must be an object");
    }
    // Web IDL dictionary keys are converted in lexicographic order.
    var endingsValue = options.endings;
    var endings = endingsValue === undefined ? "transparent" : domString(endingsValue);
    if (endings === "native") {
      throw new TypeErrorCtor("File endings 'native' is not supported; use 'transparent'");
    }
    if (endings !== "transparent") throw new TypeErrorCtor("invalid File endings value");
    var lastModifiedValue = options.lastModified;
    var lastModified = lastModifiedValue === undefined ? DateNow() : longLong(lastModifiedValue);
    var type = normalizedType(options.type);
    var blobState = {
      bytes: bytes,
      size: call(arrayBufferLength, bytes, []),
      type: type
    };
    installFileState(object, blobState, { name: name, lastModified: lastModified });
  }

  function File(parts, name) {
    if (!call(intrinsicHasInstance, File, [this])) throw new TypeErrorCtor("File must be constructed with new");
    if (arguments.length < 2) throw new TypeErrorCtor("File requires fileBits and fileName");
    initializeFile(this, parts, name, arguments[2]);
  }

  File.prototype = ObjectCreate(Blob.prototype);
  ObjectDefineProperty(File.prototype, "constructor", {
    value: File,
    writable: true,
    configurable: true
  });
  ObjectDefineProperties(File.prototype, {
    name: {
      get: function () { return requireBrand(files, this, "File").name; },
      enumerable: true,
      configurable: true
    },
    lastModified: {
      get: function () { return requireBrand(files, this, "File").lastModified; },
      enumerable: true,
      configurable: true
    }
  });
  ObjectDefineProperty(File.prototype, SymbolToStringTag, {
    value: "File",
    configurable: true
  });
  var FilePrototype = File.prototype;

  function fileFromBlob(blob, name) {
    var state = requireBrand(blobs, blob, "Blob");
    var sourceFile = get(files, blob);
    return installFileState(ObjectCreate(File.prototype), state, {
      name: usv(name),
      lastModified: sourceFile ? sourceFile.lastModified : DateNow()
    });
  }

  function formEntry(name, value, filename, filenameGiven) {
    name = usv(name);
    if (get(blobs, value)) {
      if (filenameGiven) value = fileFromBlob(value, filename);
      else if (!get(files, value)) value = fileFromBlob(value, "blob");
      return { name: name, value: value };
    }
    if (filenameGiven) {
      throw new TypeErrorCtor("a filename requires a Blob value");
    }
    return { name: name, value: usv(value) };
  }

  function FormData(form) {
    if (!call(intrinsicHasInstance, FormData, [this])) throw new TypeErrorCtor("FormData must be constructed with new");
    if (arguments.length !== 0 && form !== undefined) {
      throw new TypeErrorCtor("FormData from an HTML form is not supported without a DOM");
    }
    set(forms, this, []);
    brand(this, "FormData");
  }

  FormData.prototype.append = function (name, value, filename) {
    if (arguments.length < 2) throw new TypeErrorCtor("FormData.append requires name and value");
    call(arrayPush, requireBrand(forms, this, "FormData"), [
      formEntry(name, value, filename, arguments.length >= 3 && filename !== undefined)
    ]);
  };

  FormData.prototype["delete"] = function (name) {
    if (arguments.length < 1) throw new TypeErrorCtor("FormData.delete requires a name");
    name = usv(name);
    var entries = requireBrand(forms, this, "FormData");
    for (var i = entries.length - 1; i >= 0; i--) {
      if (entries[i].name === name) call(arraySplice, entries, [i, 1]);
    }
  };

  FormData.prototype.get = function (name) {
    if (arguments.length < 1) throw new TypeErrorCtor("FormData.get requires a name");
    name = usv(name);
    var entries = requireBrand(forms, this, "FormData");
    for (var i = 0; i < entries.length; i++) {
      if (entries[i].name === name) return entries[i].value;
    }
    return null;
  };

  FormData.prototype.getAll = function (name) {
    if (arguments.length < 1) throw new TypeErrorCtor("FormData.getAll requires a name");
    name = usv(name);
    var entries = requireBrand(forms, this, "FormData"), values = [];
    for (var i = 0; i < entries.length; i++) {
      if (entries[i].name === name) call(arrayPush, values, [entries[i].value]);
    }
    return values;
  };

  FormData.prototype.has = function (name) {
    if (arguments.length < 1) throw new TypeErrorCtor("FormData.has requires a name");
    name = usv(name);
    var entries = requireBrand(forms, this, "FormData");
    for (var i = 0; i < entries.length; i++) if (entries[i].name === name) return true;
    return false;
  };

  FormData.prototype.set = function (name, value, filename) {
    if (arguments.length < 2) throw new TypeErrorCtor("FormData.set requires name and value");
    var entry = formEntry(name, value, filename, arguments.length >= 3 && filename !== undefined);
    var entries = requireBrand(forms, this, "FormData"), first = -1;
    for (var i = 0; i < entries.length; i++) {
      if (entries[i].name !== entry.name) continue;
      if (first < 0) {
        first = i;
        entries[i] = entry;
      } else {
        call(arraySplice, entries, [i--, 1]);
      }
    }
    if (first < 0) call(arrayPush, entries, [entry]);
  };

  var IteratorPrototype = ObjectGetPrototypeOf(ObjectGetPrototypeOf([][SymbolIterator]()));
  var FormDataIteratorPrototype = ObjectCreate(IteratorPrototype);
  FormDataIteratorPrototype.next = function () {
    var iterator = requireBrand(formIterators, this, "FormData iterator");
    var entries = requireBrand(forms, iterator.form, "FormData");
    if (iterator.index >= entries.length) return { done: true, value: undefined };
    var entry = entries[iterator.index++];
    if (iterator.kind === 0) return { done: false, value: [entry.name, entry.value] };
    return { done: false, value: iterator.kind === 1 ? entry.name : entry.value };
  };

  function formIterator(form, kind) {
    requireBrand(forms, form, "FormData");
    var iterator = ObjectCreate(FormDataIteratorPrototype);
    set(formIterators, iterator, { form: form, kind: kind, index: 0 });
    brand(iterator, "FormDataIterator");
    return iterator;
  }

  FormData.prototype.entries = function () { return formIterator(this, 0); };
  FormData.prototype.keys = function () { return formIterator(this, 1); };
  FormData.prototype.values = function () { return formIterator(this, 2); };
  FormData.prototype[SymbolIterator] = FormData.prototype.entries;
  FormData.prototype.forEach = function (callback, thisArg) {
    if (typeof callback !== "function") throw new TypeErrorCtor("callback must be a function");
    var entries = requireBrand(forms, this, "FormData"), index = 0;
    while (index < entries.length) {
      var entry = entries[index++];
      call(callback, thisArg, [entry.value, entry.name, this]);
    }
  };
  ObjectDefineProperty(FormData.prototype, SymbolToStringTag, {
    value: "FormData",
    configurable: true
  });

  function extractBody(value) {
    var blob = get(blobs, value);
    if (blob) return { bytes: blob.bytes, type: blob.type };
    var entries = get(forms, value);
    if (!entries) return null;
    var boundary = newBoundary(), args = [boundary];
    for (var i = 0; i < entries.length; i++) {
      var entry = entries[i], file = get(files, entry.value);
      call(arrayPush, args, [entry.name]);
      if (file) {
        var state = requireBrand(blobs, entry.value, "Blob");
        call(arrayPush, args, [1, state.bytes, file.name, state.type]);
      } else {
        call(arrayPush, args, [0, entry.value, undefined, undefined]);
      }
    }
    return {
      bytes: ReflectApply(encodeMultipart, undefined, args),
      type: 'multipart/form-data; boundary="' + boundary + '"'
    };
  }

  global.Blob = Blob;
  global.File = File;
  global.FormData = FormData;

  // Retained only by the native adapter and handed directly to fetch.js while
  // trusted bootstrap is still running. It never enters application globals.
  return {
    extractBody: extractBody,
    snapshotBufferSource: function (value) {
      var span = arrayBufferSpan(value);
      return span ? copy(span.buffer, span.offset, span.length) : null;
    },
    responseBlob: function (bytes, type) {
      var span = arrayBufferSpan(bytes);
      if (!span) throw new TypeError("Response bytes are not an ArrayBuffer");
      return trustedBlob(copy(span.buffer, span.offset, span.length), type);
    }
  };
})(globalThis);
