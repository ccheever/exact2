// Base64 (RFC 4648 §4) for bytes, written out because `btoa` and `atob` are not in every runtime the app runs in.
const ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
const LOOKUP = new Map([...ALPHABET].map((char, index) => [char, index]));

export function toBase64(bytes: Uint8Array): string {
  const out: string[] = [];
  for (let index = 0; index < bytes.length; index += 3) {
    const a = bytes[index]!, b = bytes[index + 1], c = bytes[index + 2];
    const triple = (a << 16) | ((b ?? 0) << 8) | (c ?? 0);
    out.push(ALPHABET[(triple >> 18) & 63]! + ALPHABET[(triple >> 12) & 63]! + (b === undefined ? '=' : ALPHABET[(triple >> 6) & 63]!) + (c === undefined ? '=' : ALPHABET[triple & 63]!));
  }
  return out.join('');
}

/** Characters outside the alphabet (padding, line breaks) are skipped. */
export function fromBase64(text: string): Uint8Array {
  const clean = text.replace(/[^A-Za-z0-9+/]/g, '');
  const out = new Uint8Array(Math.floor(clean.length * 3 / 4));
  let bits = 0, value = 0, at = 0;
  for (const char of clean) {
    value = (value << 6) | (LOOKUP.get(char) ?? 0); bits += 6;
    if (bits >= 8) { bits -= 8; out[at++] = (value >> bits) & 255; }
  }
  return out.subarray(0, at);
}
