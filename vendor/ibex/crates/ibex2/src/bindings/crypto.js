// Object plumbing only: Rust owns entropy, algorithms, key material, and errors.
// @ref LLP 0059.000#314-cryptosubtle--pure-ungated-author-required — thin projection over Rust key handles
(function () {
  "use strict";
  const fill = globalThis.__ibex2_get_random_values;
  const uuid = globalThis.__ibex2_random_uuid;
  const native = globalThis.__ibex2_subtle;
  const brand = globalThis.__ibex2_brand || (value => value);
  delete globalThis.__ibex2_get_random_values;
  delete globalThis.__ibex2_random_uuid;
  delete globalThis.__ibex2_subtle;

  const isView = ArrayBuffer.isView;
  const typedProto = Object.getPrototypeOf(Uint8Array.prototype);
  const get = name => Object.getOwnPropertyDescriptor(typedProto, name).get;
  const tag = get(Symbol.toStringTag);
  const viewBuffer = get("buffer");
  const viewOffset = get("byteOffset");
  const viewLength = get("byteLength");
  const dataViewProto = DataView.prototype;
  const dataViewBuffer = Object.getOwnPropertyDescriptor(dataViewProto, "buffer").get;
  const dataViewOffset = Object.getOwnPropertyDescriptor(dataViewProto, "byteOffset").get;
  const dataViewLength = Object.getOwnPropertyDescriptor(dataViewProto, "byteLength").get;
  const arrayBufferLength = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "byteLength").get;
  const freeze = Object.freeze;
  const create = Object.create;
  const arrayFrom = Array.from;
  const resolve = Promise.resolve.bind(Promise);
  const reject = Promise.reject.bind(Promise);
  const empty = new ArrayBuffer(0);

  const cryptoBrands = new WeakSet();
  const subtleBrands = new WeakSet();
  const keyHandles = new WeakMap();
  const keyTypes = new WeakMap();
  const keyAlgorithms = new WeakMap();
  const keyExtractable = new WeakMap();
  const keyUsages = new WeakMap();

  function receiver(brands, value) {
    if (!brands.has(value)) throw new TypeError("Illegal invocation");
  }
  function domString(value, label) {
    if (typeof value === "symbol") throw new TypeError(label + " cannot be a Symbol");
    return String(value);
  }
  function webError(error) {
    const message = error && typeof error.message === "string" ? error.message : String(error);
    if (message.indexOf("TypeError: ") === 0) return new TypeError(message.slice(11));
    for (const name of ["NotSupportedError", "InvalidAccessError", "DataError",
                        "OperationError", "SyntaxError", "QuotaExceededError"]) {
      if (message.indexOf(name + ": ") === 0) {
        const detail = message.slice(name.length + 2);
        if (name === "QuotaExceededError") return new QuotaExceededError(detail);
        return new DOMException(detail, name);
      }
    }
    return error;
  }
  function operation(call) {
    try { return call(); }
    catch (error) { throw webError(error); }
  }
  function promised(call) {
    try { return resolve(call()); }
    catch (error) { return reject(webError(error)); }
  }
  function unsupported(message) {
    throw new DOMException(message, "NotSupportedError");
  }
  function algorithmName(value) {
    const name = value !== null && (typeof value === "object" || typeof value === "function")
      ? value.name : value;
    if (name === undefined) throw new TypeError("algorithm name is required");
    return domString(name, "algorithm name").toUpperCase();
  }
  function keyFormat(value) {
    const format = domString(value, "format");
    if (["raw", "jwk", "spki", "pkcs8"].indexOf(format) < 0) {
      throw new TypeError("key format " + format + " is not valid");
    }
    return format;
  }
  function hashName(value) {
    const name = algorithmName(value);
    if (name !== "SHA-256" && name !== "SHA-384" && name !== "SHA-512") {
      unsupported("hash algorithm " + name + " is not supported");
    }
    return name;
  }
  function enforceRange(value, label, maximum) {
    if (typeof value === "bigint" || typeof value === "symbol") {
      throw new TypeError(label + " must be a number");
    }
    const result = Number(value);
    if (!Number.isFinite(result)) {
      throw new TypeError(label + " must be a finite number");
    }
    const integer = Math.trunc(result);
    if (integer < 0 || integer > maximum) {
      throw new TypeError(label + " is outside the accepted range");
    }
    return integer;
  }
  function unsignedLong(value, label) {
    return enforceRange(value, label, 4294967295);
  }
  function nullableUnsignedLong(value, label) {
    return value === null || value === undefined ? null : unsignedLong(value, label);
  }
  function unsignedShort(value, label) {
    return enforceRange(value, label, 65535);
  }
  function octet(value, label) {
    return enforceRange(value, label, 255);
  }
  function bufferSource(value, label) {
    try {
      arrayBufferLength.call(value);
      return value;
    } catch (_) {}
    if (!isView(value)) throw new TypeError(label + " must be a BufferSource");
    let backing, offset, length;
    try {
      backing = viewBuffer.call(value);
      offset = viewOffset.call(value);
      length = viewLength.call(value);
    } catch (_) {
      backing = dataViewBuffer.call(value);
      offset = dataViewOffset.call(value);
      length = dataViewLength.call(value);
    }
    arrayBufferLength.call(backing);
    return new Uint8Array(backing, offset, length);
  }
  function byteLength(value) {
    try { return arrayBufferLength.call(value); }
    catch (_) { return viewLength.call(value); }
  }
  function usageList(value) {
    let list;
    try { list = arrayFrom(value); }
    catch (_) { throw new TypeError("keyUsages must be a sequence"); }
    const result = [];
    for (const item of list) {
      const usage = domString(item, "key usage");
      if (["encrypt", "decrypt", "wrapKey", "unwrapKey", "sign", "verify", "deriveKey", "deriveBits"].indexOf(usage) < 0) {
        throw new TypeError("Unknown key usage " + usage);
      }
      if (result.indexOf(usage) < 0) result.push(usage);
    }
    return result;
  }
  function jwkKeyOps(value) {
    let list;
    try { list = arrayFrom(value); }
    catch (_) { throw new TypeError("JWK key_ops must be a sequence"); }
    return list.map(item => domString(item, "JWK key operation"));
  }
  function optionalLength(algorithm) {
    if (algorithm === null || (typeof algorithm !== "object" && typeof algorithm !== "function")) return -1;
    return algorithm.length === undefined ? -1 : unsignedLong(algorithm.length, "length");
  }
  function normalizeKeyAlgorithm(algorithm, generating) {
    const name = algorithmName(algorithm);
    if (name === "HMAC") {
      if (algorithm === null || (typeof algorithm !== "object" && typeof algorithm !== "function")) {
        throw new TypeError("HMAC requires a hash");
      }
      return {name, hash: hashName(algorithm.hash), length: optionalLength(algorithm)};
    }
    if (name === "AES-GCM") {
      const length = generating
        ? unsignedShort(algorithm && algorithm.length, "AES-GCM length") : -1;
      if (generating && length !== 128 && length !== 256) {
        if (length === 192) unsupported("AES-GCM-192 is not supported by ring");
        throw new DOMException("AES-GCM keys must be 128 or 256 bits", "OperationError");
      }
      return {name, hash: "", length};
    }
    if (!generating && (name === "HKDF" || name === "PBKDF2")) {
      return {name, hash: "", length: -1, namedCurve: ""};
    }
    if (name === "ECDSA") {
      if (algorithm === null || (typeof algorithm !== "object" && typeof algorithm !== "function")) {
        throw new TypeError("ECDSA requires namedCurve");
      }
      const namedCurve = domString(algorithm.namedCurve, "namedCurve");
      if (namedCurve !== "P-256") unsupported("named curve " + namedCurve + " is not supported");
      return {name, hash: "", length: -1, namedCurve};
    }
    if (name === "ED25519") {
      return {name, hash: "", length: -1, namedCurve: ""};
    }
    unsupported("key algorithm " + name + " is not supported");
  }
  function publicAlgorithm(normalized, materialBits) {
    if (normalized.name === "HMAC") {
      return freeze({
        name: "HMAC",
        hash: freeze({name: normalized.hash}),
        length: normalized.length < 0 ? materialBits : normalized.length
      });
    }
    if (normalized.name === "AES-GCM") {
      return freeze({name: "AES-GCM", length: normalized.length < 0 ? materialBits : normalized.length});
    }
    if (normalized.name === "ECDSA") {
      return freeze({name: "ECDSA", namedCurve: normalized.namedCurve});
    }
    if (normalized.name === "ED25519") return freeze({name: "Ed25519"});
    return freeze({name: normalized.name});
  }
  function keyRecord(value) {
    if (!keyHandles.has(value)) throw new TypeError("Expected a CryptoKey");
    return {
      handle: keyHandles.get(value),
      type: keyTypes.get(value),
      algorithm: keyAlgorithms.get(value),
      extractable: keyExtractable.get(value),
      usages: keyUsages.get(value)
    };
  }
  function makeKey(handle, type, algorithm, extractable, usages) {
    const key = create(CryptoKey.prototype);
    native.own(handle, key);
    keyHandles.set(key, handle);
    keyTypes.set(key, type);
    keyAlgorithms.set(key, algorithm);
    keyExtractable.set(key, extractable);
    keyUsages.set(key, freeze(usages.slice()));
    return brand(key, "CryptoKey");
  }
  function jwkAlg(algorithm) {
    if (algorithm.name === "HMAC") return "HS" + algorithm.hash.name.slice(4);
    if (algorithm.name === "ECDSA") return "ES256";
    if (algorithm.name === "Ed25519") return "Ed25519";
    return "A" + algorithm.length + "GCM";
  }
  function base64urlBits(value) {
    return Math.floor(value.length * 6 / 8) * 8;
  }
  function normalizeSignature(algorithm) {
    const name = algorithmName(algorithm);
    if (name === "HMAC") return {name, hash: ""};
    if (name === "ECDSA") {
      if (algorithm === null || (typeof algorithm !== "object" && typeof algorithm !== "function")) {
        throw new TypeError("ECDSA requires a hash");
      }
      return {name, hash: hashName(algorithm.hash)};
    }
    if (name === "ED25519") return {name, hash: ""};
    unsupported("signature algorithm " + name + " is not supported");
  }
  function normalizeDerivation(algorithm) {
    const name = algorithmName(algorithm);
    if (algorithm === null || (typeof algorithm !== "object" && typeof algorithm !== "function")) {
      throw new TypeError(name + " requires parameters");
    }
    if (name === "HKDF") {
      return {
        name,
        hash: hashName(algorithm.hash),
        salt: bufferSource(algorithm.salt, "HKDF salt"),
        info: bufferSource(algorithm.info, "HKDF info"),
        iterations: 0
      };
    }
    if (name === "PBKDF2") {
      return {
        name,
        hash: hashName(algorithm.hash),
        salt: bufferSource(algorithm.salt, "PBKDF2 salt"),
        info: empty,
        iterations: unsignedLong(algorithm.iterations, "PBKDF2 iterations")
      };
    }
    unsupported("derivation algorithm " + name + " is not supported");
  }
  function normalizeAes(algorithm) {
    if (algorithmName(algorithm) !== "AES-GCM") unsupported("only AES-GCM is supported");
    if (algorithm === null || (typeof algorithm !== "object" && typeof algorithm !== "function")) {
      throw new TypeError("AES-GCM requires parameters");
    }
    return {
      iv: bufferSource(algorithm.iv, "AES-GCM iv"),
      additionalData: algorithm.additionalData === undefined
        ? empty : bufferSource(algorithm.additionalData, "AES-GCM additionalData"),
      tagLength: algorithm.tagLength === undefined ? 128 : octet(algorithm.tagLength, "tagLength")
    };
  }

  class CryptoKey {
    constructor() { throw new TypeError("Illegal constructor"); }
    get type() { return keyRecord(this).type; }
    get extractable() { return keyRecord(this).extractable; }
    get algorithm() { return keyRecord(this).algorithm; }
    get usages() { return keyRecord(this).usages; }
  }
  for (const name of ["type", "extractable", "algorithm", "usages"]) {
    const descriptor = Object.getOwnPropertyDescriptor(CryptoKey.prototype, name);
    descriptor.enumerable = true;
    Object.defineProperty(CryptoKey.prototype, name, descriptor);
  }
  Object.defineProperty(CryptoKey.prototype, Symbol.toStringTag,
    {value: "CryptoKey", configurable: true});

  class SubtleCrypto {
    constructor() { throw new TypeError("Illegal constructor"); }
    digest(algorithm, data) {
      receiver(subtleBrands, this);
      return promised(() => native.digest(hashName(algorithm), bufferSource(data, "data")));
    }
    importKey(formatValue, keyData, algorithmValue, extractableValue, usagesValue) {
      receiver(subtleBrands, this);
      return promised(() => {
        const format = keyFormat(formatValue);
        const algorithm = normalizeKeyAlgorithm(algorithmValue, false);
        if (format === "jwk" && (algorithm.name === "HKDF" || algorithm.name === "PBKDF2")) {
          unsupported(algorithm.name + " accepts raw keys only");
        }
        const extractable = Boolean(extractableValue);
        const usages = usageList(usagesValue);
        let material, kty, alg, use, keyOps, ext, crv, x, y, d, bits;
        if (format !== "jwk") {
          material = bufferSource(keyData, "keyData");
          bits = byteLength(material) * 8;
        } else {
          if (keyData === null || typeof keyData !== "object") throw new TypeError("JWK keyData must be an object");
          material = keyData.k === undefined ? undefined : domString(keyData.k, "JWK k");
          kty = domString(keyData.kty, "JWK kty");
          alg = keyData.alg === undefined ? undefined : domString(keyData.alg, "JWK alg");
          use = keyData.use === undefined ? undefined : domString(keyData.use, "JWK use");
          keyOps = keyData.key_ops === undefined ? undefined : jwkKeyOps(keyData.key_ops);
          ext = keyData.ext === undefined ? -1 : (Boolean(keyData.ext) ? 1 : 0);
          crv = keyData.crv === undefined ? undefined : domString(keyData.crv, "JWK crv");
          x = keyData.x === undefined ? undefined : domString(keyData.x, "JWK x");
          y = keyData.y === undefined ? undefined : domString(keyData.y, "JWK y");
          d = keyData.d === undefined ? undefined : domString(keyData.d, "JWK d");
          bits = material === undefined ? 0 : base64urlBits(material);
        }
        const handle = native.importKey(
          format, material, algorithm.name, algorithm.hash, algorithm.length,
          extractable, usages.join(","), kty, alg, use,
          keyOps === undefined ? -1 : keyOps.length, ext,
          crv, x, y, d, algorithm.namedCurve,
          ...(keyOps === undefined ? [] : keyOps)
        );
        const type = algorithm.name === "HMAC" || algorithm.name === "AES-GCM" ||
          algorithm.name === "HKDF" || algorithm.name === "PBKDF2"
          ? "secret" : (format === "pkcs8" || (format === "jwk" && d !== undefined) ? "private" : "public");
        return makeKey(handle, type, publicAlgorithm(algorithm, bits), extractable, usages);
      });
    }
    exportKey(formatValue, keyValue) {
      receiver(subtleBrands, this);
      return promised(() => {
        const format = keyFormat(formatValue);
        const key = keyRecord(keyValue);
        const exported = native.exportKey(key.handle, format);
        if (format === "raw") return exported;
        if (format === "spki" || format === "pkcs8") return exported;
        if (key.algorithm.name === "ECDSA") {
          const fields = exported.split(".");
          const result = {kty: "EC", crv: "P-256", x: fields[0], y: fields[1],
            alg: "ES256", key_ops: key.usages.slice(), ext: key.extractable};
          if (key.type === "private") result.d = fields[2];
          return result;
        }
        if (key.algorithm.name === "Ed25519") {
          const fields = exported.split(".");
          const result = {kty: "OKP", crv: "Ed25519", x: fields[0],
            alg: "Ed25519", key_ops: key.usages.slice(), ext: key.extractable};
          if (key.type === "private") result.d = fields[1];
          return result;
        }
        return {
          kty: "oct",
          k: exported,
          alg: jwkAlg(key.algorithm),
          key_ops: key.usages.slice(),
          ext: key.extractable
        };
      });
    }
    generateKey(algorithmValue, extractableValue, usagesValue) {
      receiver(subtleBrands, this);
      return promised(() => {
        const algorithm = normalizeKeyAlgorithm(algorithmValue, true);
        const extractable = Boolean(extractableValue);
        const usages = usageList(usagesValue);
        if ((algorithm.name === "ECDSA" || algorithm.name === "ED25519") &&
            usages.indexOf("sign") < 0) {
          throw new DOMException("generated private keys need at least one usage", "SyntaxError");
        }
        const handle = native.generateKey(
          algorithm.name, algorithm.hash, algorithm.length, extractable, usages.join(","), algorithm.namedCurve
        );
        if (algorithm.name === "ECDSA" || algorithm.name === "ED25519") {
          const handles = handle.split(",");
          const publicUsages = usages.indexOf("verify") < 0 ? [] : ["verify"];
          const privateUsages = usages.indexOf("sign") < 0 ? [] : ["sign"];
          return {
            publicKey: makeKey(Number(handles[0]), "public", publicAlgorithm(algorithm, 0), true, publicUsages),
            privateKey: makeKey(Number(handles[1]), "private", publicAlgorithm(algorithm, 0), extractable, privateUsages)
          };
        }
        const bits = algorithm.name === "HMAC" && algorithm.length < 0
          ? (algorithm.hash === "SHA-256" ? 512 : 1024) : algorithm.length;
        return makeKey(handle, "secret", publicAlgorithm(algorithm, bits), extractable, usages);
      });
    }
    sign(algorithm, keyValue, data) {
      receiver(subtleBrands, this);
      return promised(() => {
        const normalized = normalizeSignature(algorithm);
        return native.sign(keyRecord(keyValue).handle, normalized.name, normalized.hash, bufferSource(data, "data"));
      });
    }
    verify(algorithm, keyValue, signature, data) {
      receiver(subtleBrands, this);
      return promised(() => {
        const normalized = normalizeSignature(algorithm);
        return native.verify(
          keyRecord(keyValue).handle, normalized.name, normalized.hash,
          bufferSource(signature, "signature"),
          bufferSource(data, "data")
        );
      });
    }
    encrypt(algorithm, keyValue, data) {
      receiver(subtleBrands, this);
      return promised(() => {
        const params = normalizeAes(algorithm);
        return native.encrypt(
          keyRecord(keyValue).handle, params.iv, params.additionalData, params.tagLength,
          bufferSource(data, "data")
        );
      });
    }
    decrypt(algorithm, keyValue, data) {
      receiver(subtleBrands, this);
      return promised(() => {
        const params = normalizeAes(algorithm);
        return native.decrypt(
          keyRecord(keyValue).handle, params.iv, params.additionalData, params.tagLength,
          bufferSource(data, "data")
        );
      });
    }
    deriveBits(algorithmValue, baseKeyValue, lengthValue) {
      receiver(subtleBrands, this);
      return promised(() => {
        const algorithm = normalizeDerivation(algorithmValue);
        return native.deriveBits(
          keyRecord(baseKeyValue).handle, algorithm.name, algorithm.hash,
          algorithm.salt, algorithm.info, algorithm.iterations,
          nullableUnsignedLong(lengthValue, "length")
        );
      });
    }
    deriveKey(algorithmValue, baseKeyValue, derivedValue, extractableValue, usagesValue) {
      receiver(subtleBrands, this);
      return promised(() => {
        const algorithm = normalizeDerivation(algorithmValue);
        const derived = normalizeKeyAlgorithm(derivedValue, true);
        if (derived.name === "HMAC" && derived.length === 0) {
          throw new TypeError("HMAC derived-key length must be greater than zero");
        }
        const extractable = Boolean(extractableValue);
        const usages = usageList(usagesValue);
        const handle = native.deriveKey(
          keyRecord(baseKeyValue).handle, algorithm.name, algorithm.hash,
          algorithm.salt, algorithm.info, algorithm.iterations,
          derived.name, derived.hash, derived.length,
          extractable, usages.join(",")
        );
        const bits = derived.name === "HMAC" && derived.length < 0
          ? (derived.hash === "SHA-256" ? 512 : 1024) : derived.length;
        return makeKey(handle, "secret", publicAlgorithm(derived, bits), extractable, usages);
      });
    }
    wrapKey() {
      receiver(subtleBrands, this);
      return promised(() => unsupported("wrapKey is not supported"));
    }
    unwrapKey() {
      receiver(subtleBrands, this);
      return promised(() => unsupported("unwrapKey is not supported"));
    }
  }
  for (const key of ["digest", "importKey", "exportKey", "generateKey", "sign", "verify",
                     "encrypt", "decrypt", "deriveBits", "deriveKey", "wrapKey", "unwrapKey"]) {
    Object.defineProperty(SubtleCrypto.prototype, key, {enumerable: true});
  }
  Object.defineProperty(SubtleCrypto.prototype, Symbol.toStringTag,
    {value: "SubtleCrypto", configurable: true});
  const subtle = create(SubtleCrypto.prototype);
  subtleBrands.add(subtle);
  brand(subtle, "SubtleCrypto");
  freeze(subtle);

  class Crypto {
    constructor() { throw new TypeError("Illegal constructor"); }
    get subtle() { receiver(cryptoBrands, this); return subtle; }
    getRandomValues(array) {
      receiver(cryptoBrands, this);
      if (!isView(array)) throw new TypeError("Expected an ArrayBufferView");
      const kind = tag.call(array);
      if (["Int8Array", "Uint8Array", "Uint8ClampedArray", "Int16Array",
           "Uint16Array", "Int32Array", "Uint32Array", "BigInt64Array",
           "BigUint64Array"].indexOf(kind) < 0) {
        throw new DOMException("Expected an integer TypedArray", "TypeMismatchError");
      }
      const backing = viewBuffer.call(array);
      arrayBufferLength.call(backing);
      operation(() => fill(backing, viewOffset.call(array), viewLength.call(array)));
      return array;
    }
    randomUUID() {
      receiver(cryptoBrands, this);
      return operation(() => uuid());
    }
  }
  for (const key of ["subtle", "getRandomValues", "randomUUID"]) {
    Object.defineProperty(Crypto.prototype, key, {enumerable: true});
  }
  Object.defineProperty(Crypto.prototype, Symbol.toStringTag,
    {value: "Crypto", configurable: true});
  const crypto = create(Crypto.prototype);
  cryptoBrands.add(crypto);
  brand(crypto, "Crypto");
  freeze(crypto);
  globalThis.CryptoKey = CryptoKey;
  globalThis.SubtleCrypto = SubtleCrypto;
  globalThis.Crypto = Crypto;
  Object.defineProperty(globalThis, "crypto", {
    get() { return crypto; }, enumerable: true, configurable: true
  });
})();
