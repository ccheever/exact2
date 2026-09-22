// The `Headers` binding.
//
// The CLASS SHAPE is here — constructor overloads, the iteration protocol, the
// TypeErrors — because that is engine-side object modelling. The SEMANTICS are
// in Rust: case-folding, name and value validation, normalization,
// comma-joining on append, and sorted iteration order. This file decides
// nothing a spec would call behaviour; it only presents it.
//
// The header list itself lives in Rust and is reached by handle, so a Headers
// object is a thin wrapper around an integer.
(function (global) {
  "use strict";

  // Captured, then removed from the global object: the ops take integer
  // handles, and a module that could reach them could read any header list
  // in the runtime by guessing one.
  var h = global.__ibex2_headers;
  delete global.__ibex2_headers;
  // @ref LLP 0059.000#35-fetch--delegating-capability-bearing — the JS binding owns request-snapshot cleanup
  // fetch.js captures and removes this during binding installation, before
  // any module runs. Only that binding may free its temporary request list.
  global.__ibex2_headers_free = h.free;

  function requireValidName(name) {
    if (!h.validName(name)) {
      throw new TypeError("Invalid header name: " + name);
    }
  }

  function requireValidValue(value) {
    if (!h.validValue(value)) {
      throw new TypeError("Invalid header value");
    }
  }

  function fill(headers, init) {
    if (init === undefined || init === null) {
      return;
    }
    if (init instanceof Headers) {
      for (var entry of init) {
        headers.append(entry[0], entry[1]);
      }
      return;
    }
    if (typeof init !== "object") {
      throw new TypeError("Headers init must be an object");
    }
    // A sequence of pairs, or a record of name -> value.
    if (typeof init.length === "number" || typeof init[Symbol.iterator] === "function") {
      for (var pair of init) {
        if (pair.length !== 2) {
          throw new TypeError("Headers init sequence entries must be name/value pairs");
        }
        headers.append(pair[0], pair[1]);
      }
      return;
    }
    var names = Object.keys(init);
    for (var i = 0; i < names.length; i++) {
      headers.append(names[i], init[names[i]]);
    }
  }

  function Headers(init) {
    if (!(this instanceof Headers)) {
      throw new TypeError("Headers must be constructed with new");
    }
    // `null` is not an absent init: the spec dictionary conversion throws.
    if (init === null || (init !== undefined && typeof init !== "object")) {
      throw new TypeError("Headers init must be an object");
    }
    Object.defineProperty(this, "_handle", {
      value: h.create(),
      enumerable: false,
      writable: false,
    });
    try {
      fill(this, init);
    } catch (e) {
      h.free(this._handle);
      throw e;
    }
  }

  Headers.prototype.append = function (name, value) {
    name = String(name);
    value = String(value);
    requireValidName(name);
    requireValidValue(value);
    h.append(this._handle, name, value);
  };

  Headers.prototype.set = function (name, value) {
    name = String(name);
    value = String(value);
    requireValidName(name);
    requireValidValue(value);
    h.set(this._handle, name, value);
  };

  Headers.prototype.get = function (name) {
    name = String(name);
    requireValidName(name);
    return h.get(this._handle, name);
  };

  Headers.prototype.has = function (name) {
    name = String(name);
    requireValidName(name);
    return h.has(this._handle, name);
  };

  Headers.prototype["delete"] = function (name) {
    name = String(name);
    requireValidName(name);
    h.remove(this._handle, name);
  };

  Headers.prototype.forEach = function (callback, thisArg) {
    for (var entry of this) {
      callback.call(thisArg, entry[1], entry[0], this);
    }
  };

  // WPT checks the prototype chain, not just that iteration works: a header
  // iterator must inherit from %IteratorPrototype%, and `next` must be a
  // configurable, enumerable, writable own property of its prototype. A plain
  // object literal has Object.prototype above it and fails that.
  var IteratorPrototype = Object.getPrototypeOf(
    Object.getPrototypeOf([][Symbol.iterator]())
  );
  var HeadersIteratorPrototype = Object.create(IteratorPrototype);
  HeadersIteratorPrototype.next = function () {
    var handle = this._headers._handle;
    if (this._index >= h.count(handle)) {
      return { done: true, value: undefined };
    }
    var name = h.nameAt(handle, this._index);
    var value = h.valueAt(handle, this._index);
    this._index += 1;
    return { done: false, value: this._pick(name, value) };
  };

  function makeIterator(headers, pick) {
    var iterator = Object.create(HeadersIteratorPrototype);
    Object.defineProperties(iterator, {
      _headers: { value: headers, enumerable: false },
      _index: { value: 0, enumerable: false, writable: true },
      _pick: { value: pick, enumerable: false },
    });
    return iterator;
  }

  Headers.prototype.entries = function () {
    return makeIterator(this, function (name, value) {
      return [name, value];
    });
  };
  Headers.prototype.keys = function () {
    return makeIterator(this, function (name) {
      return name;
    });
  };
  Headers.prototype.values = function () {
    return makeIterator(this, function (_, value) {
      return value;
    });
  };
  Headers.prototype[Symbol.iterator] = Headers.prototype.entries;

  Object.defineProperty(Headers.prototype, Symbol.toStringTag, {
    value: "Headers",
    configurable: true,
  });

  global.Headers = Headers;
})(globalThis);
