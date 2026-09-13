// Opaque cursors, byte-for-byte the server's (`cursor.rs`): base64url over
// canonical JSON of `[1, table, index, order, suffix, values]`, values in
// the tagged form. A page position minted here resumes on the server and
// vice versa.

export type ColumnTypeJson = unknown;

export interface CursorPosition {
  table: string;
  index: string;
  order: "Asc" | "Desc";
  suffix: { name: string; kind: ColumnTypeJson }[];
  values: unknown[];
}

/** Canonical JSON: compact, object keys sorted, as serde_json writes a BTreeMap. */
export function canonicalJson(value: unknown): string {
  if (value === null || value === undefined) return "null";
  if (typeof value === "number") return Number.isInteger(value) ? String(value) : JSON.stringify(value);
  if (typeof value === "string" || typeof value === "boolean") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  const keys = Object.keys(value as Record<string, unknown>).sort();
  return `{${keys.map((k) => `${JSON.stringify(k)}:${canonicalJson((value as Record<string, unknown>)[k])}`).join(",")}}`;
}

function tagFor(kind: ColumnTypeJson, value: unknown): unknown {
  if (value === null || value === undefined) return "Null";
  const name = typeof kind === "string" ? kind : Object.keys(kind as object)[0]!;
  const inner = typeof kind === "object" && kind !== null ? (kind as Record<string, unknown>)[name] : undefined;
  switch (name) {
    case "Optional": return tagFor(inner, value);
    case "Id": case "Principal": case "Ref": return { Id: value };
    case "Text": case "Enum": return { String: value };
    case "Int": case "Time": case "Decimal": case "Money": return { Int: value };
    case "Bool": return { Bool: value };
    case "Bytes": return { Bytes: value };
    default: return typeof value === "string" ? { String: value } : typeof value === "number" ? { Int: value } : typeof value === "boolean" ? { Bool: value } : "Null";
  }
}

function untag(tagged: unknown): unknown {
  if (tagged === "Null") return null;
  if (typeof tagged !== "object" || tagged === null) return tagged;
  const [k, v] = Object.entries(tagged as Record<string, unknown>)[0]!;
  return k === "Array" ? (v as unknown[]).map(untag) : v;
}

const base64url = (bytes: Uint8Array): string => {
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
};

const fromBase64url = (text: string): Uint8Array | null => {
  try {
    const padded = text.replace(/-/g, "+").replace(/_/g, "/") + "=".repeat((4 - (text.length % 4)) % 4);
    const binary = atob(padded);
    return Uint8Array.from(binary, (c) => c.charCodeAt(0));
  } catch {
    return null;
  }
};

export function encodeCursor(position: CursorPosition): string {
  const tagged = [1, position.table, position.index, position.order, position.suffix, position.values.map((v, i) => tagFor(position.suffix[i]?.kind, v))];
  return base64url(new TextEncoder().encode(canonicalJson(tagged)));
}

export function decodeCursor(token: string): CursorPosition | null {
  const bytes = fromBase64url(token);
  if (!bytes) return null;
  try {
    const parsed = JSON.parse(new TextDecoder().decode(bytes)) as unknown[];
    if (!Array.isArray(parsed) || parsed[0] !== 1) return null;
    const suffix = parsed[4] as { name: string; kind: ColumnTypeJson }[];
    return { table: parsed[1] as string, index: parsed[2] as string, order: parsed[3] as "Asc" | "Desc", suffix, values: (parsed[5] as unknown[]).map(untag) };
  } catch {
    return null;
  }
}
