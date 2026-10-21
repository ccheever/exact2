// A bound length's layout regions (LLP 1123 D10) as the browser's safe area:
// the name in `env(-exact-<region>-inset-<edge>)` becomes
// `safe-area-inset-<edge>`, as css.rs writes a literal one through the
// kernel's `browser_css`. A browser keeps an `env()` value's text as written,
// escapes included, so the text is read as CSS Syntax §4 tokenizes it, after
// its preprocessing (§3.3): comments, strings, numbers, hashes, at-keywords,
// CDO and CDC and `url()` contents are passed over, and an identifier is compared with its
// escapes decoded. Only `env()`'s first argument is renamed; everything else is
// kept as written.
const REGION = /^-exact-(?:safe-area-corners|layout-margins(?:-corners)?|readable-content(?:-corners)?)-inset-(top|right|bottom|left)$/;

export function regions(text) {
  // Return a value without a region's prefix spelled out or an escape as it is: almost every value
  if (!text.includes("-exact-") && !text.includes("\\")) return text;
  const s = text.includes("\r") || text.includes("\f") ? text.replace(/\r\n?|\f/g, "\n") : text;
  let out = "", copied = 0, i = 0, argument = false; // whether the next token is `env()`'s first
  while (i < s.length) {
    const start = i, c = s.charCodeAt(i);
    if (c === 0x2f && s.charCodeAt(i + 1) === 0x2a) { // a comment
      const end = s.indexOf("*/", i + 2);
      i = end < 0 ? s.length : end + 2;
    } else if (space(c)) {
      i++;
    } else if (c === 0x22 || c === 0x27) {
      i = stringEnd(s, i);
      argument = false;
    } else if (s.startsWith("<!--", i) || s.startsWith("-->", i)) {
      i += c === 0x3c ? 4 : 3; // CDO or CDC
      argument = false;
    } else if (startsNumber(s, i)) {
      i = numberEnd(s, i);
      argument = false;
    } else if ((c === 0x23 && (nameChar(s.charCodeAt(i + 1)) || escapes(s, i + 1))) || (c === 0x40 && startsIdent(s, i + 1))) {
      i = identEnd(s, i + 1); // a hash or an at-keyword
      argument = false;
    } else if (!startsIdent(s, i)) {
      i++;
      argument = false;
    } else {
      i = identEnd(s, i);
      if (s.charCodeAt(i) === 0x28) { // a function
        const name = decode(s.slice(start, i++)).toLowerCase();
        argument = name === "env";
        if (name === "url") i = urlEnd(s, i);
      } else {
        const region = argument && REGION.exec(decode(s.slice(start, i)));
        argument = false;
        if (region) {
          out += s.slice(copied, start) + `safe-area-inset-${region[1]}`;
          copied = i;
        }
      }
    }
  }
  return copied === 0 ? s : out + s.slice(copied);
}

const space = (c) => c === 0x20 || c === 0x09 || c === 0x0a;
const hex = (c) => (c >= 0x30 && c <= 0x39) || (c >= 0x41 && c <= 0x46) || (c >= 0x61 && c <= 0x66);
const digit = (c) => c >= 0x30 && c <= 0x39;
/** A letter, `_`, a non-ASCII code unit or NUL, which preprocessing makes U+FFFD (§4.2 ident-start code point). */
const identStart = (c) => (c >= 0x41 && c <= 0x5a) || (c >= 0x61 && c <= 0x7a) || c === 0x5f || c >= 0x80 || c === 0;
/** An ident-start code point, a digit or `-` (§4.2 ident code point). */
const nameChar = (c) => identStart(c) || digit(c) || c === 0x2d;
/** Whether a valid escape starts at `i` (§4.3.8): a backslash not before a newline (at the end, it reads as U+FFFD). */
const escapes = (s, i) => s.charCodeAt(i) === 0x5c && s.charCodeAt(i + 1) !== 0x0a;

/** Whether an identifier starts at `i` (§4.3.9). */
function startsIdent(s, i) {
  if (s.charCodeAt(i) !== 0x2d) return identStart(s.charCodeAt(i)) || escapes(s, i);
  const next = s.charCodeAt(i + 1);
  return identStart(next) || next === 0x2d || escapes(s, i + 1);
}

/** Where the identifier starting at `i` ends (§4.3.11). */
function identEnd(s, i) {
  for (;;) {
    if (nameChar(s.charCodeAt(i))) i++;
    else if (escapes(s, i)) i = escapeEnd(s, i);
    else return i;
  }
}

/** Whether a number starts at `i` (§4.3.10). */
function startsNumber(s, i) {
  let c = s.charCodeAt(i);
  if (c === 0x2b || c === 0x2d) c = s.charCodeAt(++i);
  return digit(c) || (c === 0x2e && digit(s.charCodeAt(i + 1)));
}

/** Where the number starting at `i` ends, with its unit or `%` (§4.3.3, §4.3.12). */
function numberEnd(s, i) {
  if (s.charCodeAt(i) === 0x2b || s.charCodeAt(i) === 0x2d) i++;
  while (digit(s.charCodeAt(i))) i++;
  if (s.charCodeAt(i) === 0x2e && digit(s.charCodeAt(i + 1))) for (i++; digit(s.charCodeAt(i)); i++);
  const e = s.charCodeAt(i), sign = s.charCodeAt(i + 1) === 0x2b || s.charCodeAt(i + 1) === 0x2d ? 1 : 0;
  if ((e === 0x45 || e === 0x65) && digit(s.charCodeAt(i + 1 + sign))) for (i += 1 + sign; digit(s.charCodeAt(i)); i++);
  return startsIdent(s, i) ? identEnd(s, i) : s.charCodeAt(i) === 0x25 ? i + 1 : i;
}

/** Where the valid escape at `i` ends (§4.3.7): up to six hex digits and one whitespace after them, or one character. */
function escapeEnd(s, i) {
  let j = i + 1;
  if (!hex(s.charCodeAt(j))) return j + 1;
  while (j < i + 7 && hex(s.charCodeAt(j))) j++;
  return space(s.charCodeAt(j)) ? j + 1 : j;
}

/** An identifier's text with its escapes decoded (§4.3.7). */
function decode(word) {
  if (!word.includes("\\")) return word;
  return word.replace(/\\([0-9a-f]{1,6})[ \t\n]?|\\([\s\S])/gi, (_, digits, char) => {
    if (!digits) return char;
    const n = parseInt(digits, 16);
    return String.fromCodePoint(n === 0 || n > 0x10ffff || (n >= 0xd800 && n <= 0xdfff) ? 0xfffd : n);
  });
}

/** Where the string opening at `i` ends (§4.3.5): after its closing quote, or before an unescaped newline. */
function stringEnd(s, i) {
  const quote = s.charCodeAt(i++);
  while (i < s.length) {
    const c = s.charCodeAt(i);
    if (c === quote) return i + 1;
    if (c === 0x0a) return i;
    // A backslash before a newline continues the string
    i = escapes(s, i) ? escapeEnd(s, i) : c === 0x5c ? i + 2 : i + 1;
  }
  return s.length;
}

/** Where an unquoted `url(` token's contents end, after its `)` (§4.3.6); `i` itself when a quote follows, the contents then being tokens. */
function urlEnd(s, i) {
  let j = i;
  while (space(s.charCodeAt(j))) j++;
  if (s.charCodeAt(j) === 0x22 || s.charCodeAt(j) === 0x27) return i;
  while (j < s.length && s.charCodeAt(j) !== 0x29) j = escapes(s, j) ? escapeEnd(s, j) : j + 1;
  return Math.min(j + 1, s.length);
}
