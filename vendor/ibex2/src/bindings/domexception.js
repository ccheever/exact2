// WebIDL's DOMException: shared by WebCrypto errors and future web APIs.
// @ref https://webidl.spec.whatwg.org/#idl-DOMException — names and legacy codes
(function () {
  "use strict";
  const slots = new WeakMap();
  const codes = [
    ["IndexSizeError", "INDEX_SIZE_ERR"],
    [null, "DOMSTRING_SIZE_ERR"],
    ["HierarchyRequestError", "HIERARCHY_REQUEST_ERR"],
    ["WrongDocumentError", "WRONG_DOCUMENT_ERR"],
    ["InvalidCharacterError", "INVALID_CHARACTER_ERR"],
    [null, "NO_DATA_ALLOWED_ERR"],
    ["NoModificationAllowedError", "NO_MODIFICATION_ALLOWED_ERR"],
    ["NotFoundError", "NOT_FOUND_ERR"],
    ["NotSupportedError", "NOT_SUPPORTED_ERR"],
    ["InUseAttributeError", "INUSE_ATTRIBUTE_ERR"],
    ["InvalidStateError", "INVALID_STATE_ERR"],
    ["SyntaxError", "SYNTAX_ERR"],
    ["InvalidModificationError", "INVALID_MODIFICATION_ERR"],
    ["NamespaceError", "NAMESPACE_ERR"],
    ["InvalidAccessError", "INVALID_ACCESS_ERR"],
    [null, "VALIDATION_ERR"],
    ["TypeMismatchError", "TYPE_MISMATCH_ERR"],
    ["SecurityError", "SECURITY_ERR"],
    ["NetworkError", "NETWORK_ERR"],
    ["AbortError", "ABORT_ERR"],
    ["URLMismatchError", "URL_MISMATCH_ERR"],
    ["QuotaExceededError", "QUOTA_EXCEEDED_ERR"],
    ["TimeoutError", "TIMEOUT_ERR"],
    ["InvalidNodeTypeError", "INVALID_NODE_TYPE_ERR"],
    ["DataCloneError", "DATA_CLONE_ERR"]
  ];
  function state(receiver) {
    const value = slots.get(receiver);
    if (!value) throw new TypeError("Illegal invocation");
    return value;
  }
  class DOMException extends Error {
    constructor(message = "", name = "Error") {
      // Template substitution performs ToString with the string hint.
      message = `${message}`;
      name = `${name}`;
      super(message);
      delete this.message;
      let code = 0;
      for (let i = 0; i < codes.length; i++) {
        if (codes[i][0] === name) code = i + 1;
      }
      slots.set(this, {message, name, code});
    }
    get message() { return state(this).message; }
    get name() { return state(this).name; }
    get code() { return state(this).code; }
  }
  for (const key of ["message", "name", "code"]) {
    Object.defineProperty(DOMException.prototype, key, {enumerable: true});
  }
  Object.defineProperty(DOMException.prototype, Symbol.toStringTag,
    {value: "DOMException", configurable: true});
  for (let i = 0; i < codes.length; i++) {
    const descriptor = {value: i + 1, enumerable: true};
    Object.defineProperty(DOMException, codes[i][1], descriptor);
    Object.defineProperty(DOMException.prototype, codes[i][1], descriptor);
  }
  const quotaSlots = new WeakMap();
  function quotaState(receiver) {
    const value = quotaSlots.get(receiver);
    if (!value) throw new TypeError("Illegal invocation");
    return value;
  }
  class QuotaExceededError extends DOMException {
    constructor(message = "", options = {}) {
      super(message, "QuotaExceededError");
      if (options == null) options = {};
      if (typeof options !== "object" && typeof options !== "function") {
        throw new TypeError("Expected a dictionary");
      }
      const values = {};
      for (const key of ["quota", "requested"]) {
        const input = options[key];
        const value = input === undefined ? null : +input;
        if (value !== null && !Number.isFinite(value)) {
          throw new TypeError("Expected a finite double");
        }
        values[key] = value;
      }
      if (values.quota < 0 || values.requested < 0 ||
          (values.quota !== null && values.requested !== null &&
           values.requested < values.quota)) {
        throw new RangeError("Invalid quota or requested size");
      }
      quotaSlots.set(this, values);
    }
    get quota() { return quotaState(this).quota; }
    get requested() { return quotaState(this).requested; }
  }
  for (const key of ["quota", "requested"]) {
    Object.defineProperty(QuotaExceededError.prototype, key, {enumerable: true});
  }
  Object.defineProperty(QuotaExceededError.prototype, Symbol.toStringTag,
    {value: "QuotaExceededError", configurable: true});
  globalThis.QuotaExceededError = QuotaExceededError;
  globalThis.DOMException = DOMException;
})();
