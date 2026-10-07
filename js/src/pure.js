// Ibex owns URL, URLSearchParams, Headers, and text. Preserve Exact's base64
// edge behavior and live URLSearchParams iteration while Ibex's iterator snapshots.
(function (global) {
  "use strict";
  var host = global.__exact_host;
  function call(op, args) { return JSON.parse(host(7, op, JSON.stringify(args))); }
  function invalid(message) {
    var error = new Error(message);
    Object.defineProperty(error, "name", { value: "InvalidCharacterError" });
    return error;
  }
  global.btoa = function (input) {
    var text = String(input);
    for (var i = 0; i < text.length; i++) {
      if (text.charCodeAt(i) > 255) throw invalid("btoa requires Latin-1");
    }
    return call("btoa", [text]);
  };
  global.atob = function (input) {
    var text = String(input).replace(/[\t\n\f\r ]/g, "");
    if (text.length % 4 === 0) text = text.replace(/==?$/, "");
    if (text.length % 4 === 1 || /[^A-Za-z0-9+/]/.test(text)) throw invalid("invalid base64");
    var alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    var tail = text.length % 4;
    if (tail) {
      var value = alphabet.indexOf(text[text.length - 1]);
      text = text.slice(0, -1) + alphabet[value & (tail === 2 ? 48 : 60)];
      while (text.length % 4) text += "=";
    }
    return call("atob", [text]);
  };
  var proto = global.URLSearchParams.prototype;
  var snapshot = proto.entries;
  function iterator(instance, field) {
    snapshot.call(instance); // validate the receiver now
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
    if (typeof callback !== "function") throw new TypeError("forEach requires a function");
    var entries = this.entries(), entry;
    while (!(entry = entries.next()).done) callback.call(receiver, entry.value[1], entry.value[0], this);
  };
})(globalThis);
