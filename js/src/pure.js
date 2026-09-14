// Native class shapes over Ibex pure Rust algorithms. Browser modules use the
// browser's globals. No app code, clocks, grants, or I/O are involved.
// @ref LLP 1027.001 D1 — pure capabilities follow the web
(function (global) {
  'use strict';
  var host = global.__exact_host;
  var encodeBytes = global.__exact_encode, decodeBytes = global.__exact_decode;
  delete global.__exact_encode; delete global.__exact_decode;
  function call(op, args) { return JSON.parse(host(7, op, JSON.stringify(args))); }
  // Web IDL USVString conversion happens before JSON, which rejects lone
  // surrogates in Rust. Replacement preserves encodeInto's UTF-16 read count.
  function usv(input) {
    return String(input).replace(/[\ud800-\udbff][\udc00-\udfff]|[\ud800-\udfff]/g,
      function (match) { return match.length === 2 ? match : '\ufffd'; });
  }
  var encoders = new WeakSet();
  function TextEncoder() {
    if (!(this instanceof TextEncoder)) throw new TypeError('TextEncoder requires new');
    encoders.add(this);
  }
  Object.defineProperty(TextEncoder.prototype, 'encoding', { get: function () { if (!encoders.has(this)) throw new TypeError('not a TextEncoder'); return 'utf-8'; } });
  TextEncoder.prototype.encode = function (input) {
    if (!encoders.has(this)) throw new TypeError('not a TextEncoder');
    // The direct JSI UTF-8 conversion already replaces lone surrogates. Only
    // JSON-framed operations need the explicit USV scan above.
    return new Uint8Array(encodeBytes(input === undefined ? '' : String(input)));
  };
  TextEncoder.prototype.encodeInto = function (input, destination) {
    if (!encoders.has(this)) throw new TypeError('not a TextEncoder');
    if (!(destination instanceof Uint8Array)) throw new TypeError('encodeInto requires Uint8Array');
    var result = call('encodeInto', [usv(input), destination.length]);
    destination.set(result.bytes);
    return { read: result.read, written: result.written };
  };
  var decoders = new WeakMap();
  function TextDecoder(label, options) {
    if (!(this instanceof TextDecoder)) throw new TypeError('TextDecoder requires new');
    label = label === undefined ? 'utf-8' : String(label).replace(/^[\t\n\f\r ]+|[\t\n\f\r ]+$/g, '').toLowerCase();
    if (['utf-8', 'utf8', 'unicode-1-1-utf-8', 'unicode11utf8', 'unicode20utf8', 'x-unicode20utf8'].indexOf(label) < 0)
      throw new RangeError('TextDecoder supports UTF-8 only');
    decoders.set(this, { fatal: !!(options && options.fatal), ignoreBOM: !!(options && options.ignoreBOM), pending: [], started: false });
  }
  ['fatal', 'ignoreBOM', 'encoding'].forEach(function (name) {
    Object.defineProperty(TextDecoder.prototype, name, { get: function () {
      var state = decoders.get(this);
      if (!state) throw new TypeError('not a TextDecoder');
      return name === 'encoding' ? 'utf-8' : state[name];
    } });
  });
  TextDecoder.prototype.decode = function (input, options) {
    var state = decoders.get(this);
    if (!state) throw new TypeError('not a TextDecoder');
    var bytes;
    if (input === undefined) bytes = new Uint8Array(0);
    else if (input instanceof ArrayBuffer) bytes = new Uint8Array(input);
    else if (ArrayBuffer.isView(input)) bytes = new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
    else throw new TypeError('decode requires an ArrayBuffer or view');
    var stream = !!(options && options.stream), result;
    try {
      result = !stream && state.pending.length === 0
        ? {text: decodeBytes(bytes, state.fatal, true), pending: []}
        : call('decode', [state.pending.concat(Array.from(bytes)), stream, state.fatal]);
    }
    catch (error) { state.pending = []; state.started = false; throw new TypeError(String(error.message)); }
    state.pending = result.pending;
    var text = result.text;
    if (!state.started && text.length) {
      state.started = true;
      if (!state.ignoreBOM && text.charCodeAt(0) === 0xfeff) text = text.slice(1);
    }
    if (!stream) { state.pending = []; state.started = false; }
    return text;
  };
  Object.defineProperty(TextEncoder.prototype, Symbol.toStringTag, {value:'TextEncoder', configurable:true});
  Object.defineProperty(TextDecoder.prototype, Symbol.toStringTag, {value:'TextDecoder', configurable:true});
  global.TextEncoder = TextEncoder;
  global.TextDecoder = TextDecoder;
  function invalid(message) {
    var error = new Error(message);
    Object.defineProperty(error, 'name', { value: 'InvalidCharacterError' });
    return error;
  }
  global.btoa = function (input) {
    var text = String(input);
    for (var i = 0; i < text.length; i++) if (text.charCodeAt(i) > 255) throw invalid('btoa requires Latin-1');
    return call('btoa', [text]);
  };
  global.atob = function (input) {
    // WHATWG forgiving-base64 normalization; Ibex owns the byte algorithm.
    var text = String(input).replace(/[\t\n\f\r ]/g, '');
    if (text.length % 4 === 0) text = text.replace(/==?$/, '');
    if (text.length % 4 === 1 || /[^A-Za-z0-9+/]/.test(text)) throw invalid('invalid base64');
    var alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
    var tail = text.length % 4;
    if (tail) {
      var value = alphabet.indexOf(text[text.length - 1]);
      text = text.slice(0, -1) + alphabet[value & (tail === 2 ? 48 : 60)];
      while (text.length % 4) text += '=';
    }
    return call('atob', [text]);
  };
  ['url_parse', 'url_set', 'sp_normalize', 'sp_get', 'sp_get_all', 'sp_has', 'sp_set', 'sp_append', 'sp_delete', 'sp_sort', 'sp_entries'].forEach(function (op) {
    var name = op.replace(/^sp_/, 'search_params_');
    global['__ibex2_' + name] = function () {
      return call(op, Array.from(arguments).map(usv));
    };
  });
})(globalThis);
// Ibex's URL shape currently snapshots iteration. Web iterators are live:
// re-read the canonical Rust pairs at each next(), including in forEach.
(function (global) {
  global.__exact_finish_pure = function () {
    delete global.__exact_finish_pure;
    var proto = global.URLSearchParams.prototype;
    var snapshot = proto.entries;
    function iterator(instance, field) {
      snapshot.call(instance); // Validate the receiver before returning an iterator.
      var at = 0, done = false;
      var result = { next: function () {
        if (done) return { value: undefined, done: true };
        var pairs = Array.from(snapshot.call(instance));
        if (at >= pairs.length) { done = true; return { value: undefined, done: true }; }
        var pair = pairs[at++];
        return { value: field === undefined ? pair : pair[field], done: false };
      } };
      result[Symbol.iterator] = function () { return this; };
      return result;
    }
    proto.entries = function () { return iterator(this); };
    proto.keys = function () { return iterator(this, 0); };
    proto.values = function () { return iterator(this, 1); };
    proto[Symbol.iterator] = proto.entries;
    proto.forEach = function (callback, receiver) {
      if (typeof callback !== 'function') throw new TypeError('forEach requires a function');
      var entries = this.entries(), entry;
      while (!(entry = entries.next()).done) callback.call(receiver, entry.value[1], entry.value[0], this);
    };
  };
})(globalThis);
