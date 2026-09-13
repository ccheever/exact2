// One order-preserving byte encoding for index keys, byte-for-byte the
// server's (`crates/snapback4-core/src/key.rs`): a device scans the same
// bytes in the same order. Values on the device are plain JSON, so the
// column kind says whether a string is text or an identity.

export type Kind = "null" | "bool" | "int" | "string" | "id" | "bytes";

const T_NULL = 0x01, T_FALSE = 0x02, T_TRUE = 0x03, T_INT = 0x04, T_STRING = 0x05, T_ID = 0x06, T_BYTES = 0x07, T_END = 0xff;

const utf8 = new TextEncoder();

/** Encode a tuple of `(kind, value)` components. */
export function encodeKey(components: readonly (readonly [Kind, unknown])[]): Uint8Array {
  const out: number[] = [];
  for (const [kind, value] of components) encodeOne(kind, value, out);
  return Uint8Array.from(out);
}

function encodeOne(kind: Kind, value: unknown, out: number[]) {
  if (value === null || value === undefined) { out.push(T_NULL); return; }
  switch (kind) {
    case "null": out.push(T_NULL); return;
    case "bool": out.push(value ? T_TRUE : T_FALSE); return;
    case "int": {
      out.push(T_INT);
      // Flip the sign bit so two's complement sorts as unsigned bytes.
      const flipped = BigInt.asUintN(64, BigInt(value as number)) ^ (1n << 63n);
      for (let shift = 56n; shift >= 0n; shift -= 8n) out.push(Number((flipped >> shift) & 0xffn));
      return;
    }
    case "string": out.push(T_STRING); escape(utf8.encode(String(value)), out); return;
    case "id": out.push(T_ID); escape(utf8.encode(String(value)), out); return;
    case "bytes": out.push(T_BYTES); escape(hexBytes(String(value)), out); return;
  }
}

/** `0x00` inside a body becomes `0x00 0xff`; the body ends with `0x00 0x00`. */
function escape(bytes: Uint8Array, out: number[]) {
  for (const byte of bytes) {
    if (byte === 0) out.push(0, 0xff);
    else out.push(byte);
  }
  out.push(0, 0);
}

function hexBytes(hex: string): Uint8Array {
  const out = new Uint8Array(hex.length >> 1);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  return out;
}

/** The exclusive upper bound of every key that extends `prefix`. */
export function upperBound(prefix: Uint8Array): Uint8Array {
  const out = new Uint8Array(prefix.length + 1);
  out.set(prefix);
  out[prefix.length] = T_END;
  return out;
}

export function compareKeys(a: Uint8Array, b: Uint8Array): number {
  const n = Math.min(a.length, b.length);
  for (let i = 0; i < n; i++) {
    if (a[i]! !== b[i]!) return a[i]! < b[i]! ? -1 : 1;
  }
  return a.length - b.length;
}

export function hex(bytes: Uint8Array): string {
  let out = "";
  for (const byte of bytes) out += byte.toString(16).padStart(2, "0");
  return out;
}

/** The kind of an index column from its declared column type. */
export function kindOf(columnType: unknown): Kind {
  if (columnType === "Id" || columnType === "Principal") return "id";
  if (columnType === "Int" || columnType === "Time") return "int";
  if (columnType === "Bool") return "bool";
  if (typeof columnType === "object" && columnType !== null) {
    const key = Object.keys(columnType)[0];
    if (key === "Ref") return "id";
    if (key === "Text" || key === "Enum") return "string";
    if (key === "Decimal" || key === "Money") return "int";
    if (key === "Bytes") return "bytes";
    if (key === "Optional") return kindOf((columnType as { Optional: unknown }).Optional);
  }
  return "string";
}
