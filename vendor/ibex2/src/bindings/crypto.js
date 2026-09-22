// Object plumbing only: the Rust functions own entropy, quota, and UUID bits.
// @ref LLP 0059.000#39-crypto--ambient-partially-gated — the shared implementation
(function () {
  "use strict";
  const fill = globalThis.__ibex2_get_random_values;
  const uuid = globalThis.__ibex2_random_uuid;
  delete globalThis.__ibex2_get_random_values;
  delete globalThis.__ibex2_random_uuid;

  const isView = ArrayBuffer.isView;
  const proto = Object.getPrototypeOf(Uint8Array.prototype);
  const get = name => Object.getOwnPropertyDescriptor(proto, name).get;
  const tag = get(Symbol.toStringTag);
  const buffer = get("buffer");
  const offset = get("byteOffset");
  const length = get("byteLength");
  const arrayBufferLength = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "byteLength").get;
  const brands = new WeakSet();
  function receiver(value) {
    if (!brands.has(value)) throw new TypeError("Illegal invocation");
  }
  function operation(call) {
    try { return call(); }
    catch (error) {
      const message = error.message;
      for (const name of ["QuotaExceededError", "OperationError"]) {
        if (message.indexOf(name + ": ") === 0) {
          const detail = message.slice(name.length + 2);
          if (name === "QuotaExceededError") throw new QuotaExceededError(detail);
          throw new DOMException(detail, name);
        }
      }
      throw error;
    }
  }
  class Crypto {
    constructor() { throw new TypeError("Illegal constructor"); }
    getRandomValues(array) {
      receiver(this);
      if (!isView(array)) throw new TypeError("Expected an ArrayBufferView");
      const kind = tag.call(array);
      if (["Int8Array", "Uint8Array", "Uint8ClampedArray", "Int16Array",
           "Uint16Array", "Int32Array", "Uint32Array", "BigInt64Array",
           "BigUint64Array"].indexOf(kind) < 0) {
        throw new DOMException("Expected an integer TypedArray", "TypeMismatchError");
      }
      const backing = buffer.call(array);
      // WebIDL's ArrayBufferView excludes SharedArrayBuffer-backed views.
      arrayBufferLength.call(backing);
      operation(() => fill(backing, offset.call(array), length.call(array)));
      return array;
    }
    randomUUID() {
      receiver(this);
      return operation(() => uuid());
    }
  }
  for (const key of ["getRandomValues", "randomUUID"]) {
    Object.defineProperty(Crypto.prototype, key, {enumerable: true});
  }
  Object.defineProperty(Crypto.prototype, Symbol.toStringTag,
    {value: "Crypto", configurable: true});
  const crypto = Object.create(Crypto.prototype);
  brands.add(crypto);
  Object.freeze(crypto);
  globalThis.Crypto = Crypto;
  Object.defineProperty(globalThis, "crypto", {
    get() { return crypto; }, enumerable: true, configurable: true
  });
})();
