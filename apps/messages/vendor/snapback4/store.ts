// The replica store: rows by (table, id) and one index entry per (index,
// key bytes), the server's layout on the device. Two stores share one
// interface — in memory for tests and the simulator's twin, IndexedDB on
// the web. Every read runs inside one transaction; every write commits as
// one batch, so a partition batch applies atomically and a watermark never
// gets ahead of its rows.

import { compareKeys, encodeKey, kindOf, upperBound, type Kind } from "./key";

export type Row = Record<string, unknown> & { id: string };

/** The server's schema, as `GET /schema` sends it. */
export interface SchemaJson {
  tables: Record<string, TableJson>;
  maintains?: unknown[];
}
export interface TableJson {
  columns: Record<string, unknown>;
  indexes: Record<string, { components: ({ Column: string } | { WordPrefixes: unknown })[]; unique: boolean }>;
  sync?: { audience: unknown; horizon?: { last: number; by: string } | null } | null;
  leave?: "until-revoked" | "delivered-history";
  /** The time column that ends a row's life; a row past it is absent here too. */
  expire?: string | null;
}

export interface Bounds {
  after?: unknown[];
  before?: unknown[];
  gt?: unknown;
  gte?: unknown;
  lt?: unknown;
  lte?: unknown;
}

export interface StoreTx {
  get(table: string, id: string): Promise<Row | undefined>;
  lookup(table: string, index: string, key: readonly unknown[]): Promise<Row | undefined>;
  scan(table: string, index: string, prefix: readonly unknown[], bounds: Bounds, dir: "asc" | "desc", limit: number): Promise<{ row: Row; key: Uint8Array }[]>;
  count(table: string, index: string, prefix: readonly unknown[], limit: number): Promise<number>;
  put(table: string, row: Row): Promise<void>;
  delete(table: string, id: string): Promise<void>;
  getMeta(key: string): Promise<unknown>;
  setMeta(key: string, value: unknown): Promise<void>;
  /** The outbox and other durable side tables share the transaction. */
  side(name: string): SideTx;
}

export interface SideTx {
  get(key: string): Promise<unknown>;
  put(key: string, value: unknown): Promise<void>;
  delete(key: string): Promise<void>;
  all(): Promise<{ key: string; value: unknown }[]>;
  clear(): Promise<void>;
}

export interface Store {
  /** The compiled backend's schema, for key encoding; set once it is known
   * (a device that starts offline reads it from its own store first). */
  readonly schema: SchemaJson;
  setSchema(schema: SchemaJson): void;
  read<T>(body: (tx: StoreTx) => Promise<T>): Promise<T>;
  write<T>(body: (tx: StoreTx) => Promise<T>): Promise<T>;
  /** Drop every row and index entry (a re-bootstrap); meta and side tables stay. */
  clearRows(): Promise<void>;
  close(): Promise<void>;
}

/** The index components' kinds for a table, from the schema. */
export function indexKinds(schema: SchemaJson, table: string, index: string): { columns: string[]; kinds: Kind[] } {
  const definition = schema.tables[table]?.indexes[index];
  if (!definition) throw new Error(`no index ${table}.${index}`);
  const columns = definition.components.map((c) => ("Column" in c ? c.Column : "")).filter(Boolean);
  const kinds = columns.map((column) => (column === "id" ? "id" : kindOf(schema.tables[table]!.columns[column])));
  return { columns, kinds };
}

/** The stored key of one index entry: the components, then the id. */
export function indexKey(schema: SchemaJson, table: string, index: string, row: Row): Uint8Array {
  const { columns, kinds } = indexKinds(schema, table, index);
  const components: [Kind, unknown][] = columns.map((column, i) => [kinds[i]!, row[column] ?? null]);
  components.push(["id", row.id]);
  return encodeKey(components);
}

/** The byte range of a scan: `[lo, hi)`, or null when empty. */
export function scanRange(schema: SchemaJson, table: string, index: string, prefix: readonly unknown[], bounds: Bounds, dir: "asc" | "desc"): [Uint8Array, Uint8Array] | null {
  const { kinds } = indexKinds(schema, table, index);
  const enc = (values: readonly unknown[]) => encodeKey(values.map((v, i) => [kinds[i] ?? "string", v] as [Kind, unknown]));
  const base = enc(prefix);
  let lo = base;
  let hi = upperBound(base);
  const withNext = (value: unknown) => enc([...prefix, value]);
  const max = (a: Uint8Array, b: Uint8Array) => (compareKeys(a, b) >= 0 ? a : b);
  const min = (a: Uint8Array, b: Uint8Array) => (compareKeys(a, b) <= 0 ? a : b);
  if (bounds.gte !== undefined) lo = max(lo, withNext(bounds.gte));
  if (bounds.gt !== undefined) lo = max(lo, upperBound(withNext(bounds.gt)));
  if (bounds.lte !== undefined) hi = min(hi, upperBound(withNext(bounds.lte)));
  if (bounds.lt !== undefined) hi = min(hi, withNext(bounds.lt));
  if (bounds.after) {
    const position = enc([...prefix, ...bounds.after]);
    if (dir === "asc") lo = max(lo, upperBound(position));
    else hi = min(hi, position);
  }
  if (bounds.before) {
    const position = enc([...prefix, ...bounds.before]);
    if (dir === "asc") hi = min(hi, position);
    else lo = max(lo, upperBound(position));
  }
  return compareKeys(lo, hi) < 0 ? [lo, hi] : null;
}

// ---------------------------------------------------------------------------
// In memory.

function lowerBoundIndex(entries: { key: Uint8Array }[], key: Uint8Array): number {
  let lo = 0, hi = entries.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (compareKeys(entries[mid]!.key, key) < 0) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

export class MemoryStore implements Store {
  private rows = new Map<string, Map<string, Row>>();
  private indexes = new Map<string, { key: Uint8Array; id: string }[]>();
  private meta = new Map<string, unknown>();
  private sides = new Map<string, Map<string, unknown>>();
  constructor(public schema: SchemaJson) {}
  setSchema(schema: SchemaJson) { this.schema = schema; }

  private tx(): StoreTx {
    const self = this;
    const entries = (table: string, index: string) => {
      const name = `${table}.${index}`;
      let list = self.indexes.get(name);
      if (!list) { list = []; self.indexes.set(name, list); }
      return list;
    };
    const side = (name: string): SideTx => {
      let map = self.sides.get(name);
      if (!map) { map = new Map(); self.sides.set(name, map); }
      const m = map;
      return {
        async get(key) { return m.get(key); },
        async put(key, value) { m.set(key, structuredClone(value)); },
        async delete(key) { m.delete(key); },
        async all() { return [...m.entries()].map(([key, value]) => ({ key, value })); },
        async clear() { m.clear(); },
      };
    };
    return {
      async get(table, id) { return self.rows.get(table)?.get(id); },
      async lookup(table, index, key) {
        const { kinds } = indexKinds(self.schema, table, index);
        const prefix = encodeKey(key.map((v, i) => [kinds[i] ?? "string", v] as [Kind, unknown]));
        const list = entries(table, index);
        const at = lowerBoundIndex(list, prefix);
        const entry = list[at];
        if (!entry || compareKeys(entry.key, upperBound(prefix)) >= 0) return undefined;
        return self.rows.get(table)?.get(entry.id);
      },
      async scan(table, index, prefix, bounds, dir, limit) {
        const range = scanRange(self.schema, table, index, prefix, bounds, dir);
        if (!range) return [];
        const [lo, hi] = range;
        const list = entries(table, index);
        const start = lowerBoundIndex(list, lo);
        const end = lowerBoundIndex(list, hi);
        const out: { row: Row; key: Uint8Array }[] = [];
        if (dir === "asc") {
          for (let i = start; i < end && out.length < limit; i++) {
            const row = self.rows.get(table)?.get(list[i]!.id);
            if (row) out.push({ row, key: list[i]!.key });
          }
        } else {
          for (let i = end - 1; i >= start && out.length < limit; i--) {
            const row = self.rows.get(table)?.get(list[i]!.id);
            if (row) out.push({ row, key: list[i]!.key });
          }
        }
        return out;
      },
      async count(table, index, prefix, limit) {
        const range = scanRange(self.schema, table, index, prefix, {}, "asc");
        if (!range) return 0;
        const list = entries(table, index);
        return Math.min(limit, lowerBoundIndex(list, range[1]) - lowerBoundIndex(list, range[0]));
      },
      async put(table, row) {
        let rows = self.rows.get(table);
        if (!rows) { rows = new Map(); self.rows.set(table, rows); }
        const old = rows.get(row.id);
        const definition = self.schema.tables[table];
        if (!definition) throw new Error(`unknown table ${table}`);
        for (const index of Object.keys(definition.indexes)) {
          const list = entries(table, index);
          if (old) {
            const before = indexKey(self.schema, table, index, old);
            const at = lowerBoundIndex(list, before);
            if (list[at] && compareKeys(list[at]!.key, before) === 0) list.splice(at, 1);
          }
          const key = indexKey(self.schema, table, index, row);
          list.splice(lowerBoundIndex(list, key), 0, { key, id: row.id });
        }
        rows.set(row.id, structuredClone(row));
      },
      async delete(table, id) {
        const rows = self.rows.get(table);
        const old = rows?.get(id);
        if (!old) return;
        for (const index of Object.keys(self.schema.tables[table]?.indexes ?? {})) {
          const list = entries(table, index);
          const before = indexKey(self.schema, table, index, old);
          const at = lowerBoundIndex(list, before);
          if (list[at] && compareKeys(list[at]!.key, before) === 0) list.splice(at, 1);
        }
        rows!.delete(id);
      },
      async getMeta(key) { return self.meta.get(key); },
      async setMeta(key, value) { self.meta.set(key, value); },
      side,
    };
  }

  async read<T>(body: (tx: StoreTx) => Promise<T>): Promise<T> { return body(this.tx()); }
  async write<T>(body: (tx: StoreTx) => Promise<T>): Promise<T> { return body(this.tx()); }
  async clearRows() { this.rows.clear(); this.indexes.clear(); }
  async close() {}
}

// ---------------------------------------------------------------------------
// IndexedDB.

const STORES = ["rows", "idx", "meta", "outbox", "predicted", "log"] as const;

/** IndexedDB compares binary keys byte-wise; hand it a plain ArrayBuffer. */
function idbKey(bytes: Uint8Array): ArrayBuffer {
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
}

function request<T>(req: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

export async function openIndexedDb(name: string, schema: SchemaJson): Promise<Store> {
  const db = await new Promise<IDBDatabase>((resolve, reject) => {
    const open = indexedDB.open(name, 1);
    open.onupgradeneeded = () => {
      const database = open.result;
      for (const store of STORES) if (!database.objectStoreNames.contains(store)) database.createObjectStore(store);
    };
    open.onsuccess = () => resolve(open.result);
    open.onerror = () => reject(open.error);
  });
  return new IndexedDbStore(db, schema);
}

class IndexedDbStore implements Store {
  constructor(private db: IDBDatabase, public schema: SchemaJson) {}
  setSchema(schema: SchemaJson) { this.schema = schema; }

  private tx(mode: IDBTransactionMode): { transaction: IDBTransaction; api: StoreTx; done: Promise<void> } {
    const transaction = this.db.transaction([...STORES], mode);
    const done = new Promise<void>((resolve, reject) => {
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error);
      transaction.onabort = () => reject(transaction.error ?? new Error("aborted"));
    });
    // A body that throws (a refused prediction) aborts the transaction on
    // purpose; the abort must not surface a second time as an unhandled
    // rejection in the page. The caller still sees the body's own error.
    done.catch(() => {});
    const rows = transaction.objectStore("rows");
    const idx = transaction.objectStore("idx");
    const meta = transaction.objectStore("meta");
    const schema = this.schema;
    const side = (name: string): SideTx => {
      const store = transaction.objectStore(name);
      return {
        get: (key) => request(store.get(key)),
        put: async (key, value) => { await request(store.put(value, key)); },
        delete: async (key) => { await request(store.delete(key)); },
        all: async () => {
          const keys = await request(store.getAllKeys());
          const values = await request(store.getAll());
          return keys.map((key, i) => ({ key: String(key), value: values[i] }));
        },
        clear: async () => { await request(store.clear()); },
      };
    };
    const api: StoreTx = {
      get: (table, id) => request(rows.get([table, id])) as Promise<Row | undefined>,
      async lookup(table, index, key) {
        const { kinds } = indexKinds(schema, table, index);
        const prefix = encodeKey(key.map((v, i) => [kinds[i] ?? "string", v] as [Kind, unknown]));
        const range = IDBKeyRange.bound([`${table}.${index}`, idbKey(prefix)], [`${table}.${index}`, idbKey(upperBound(prefix))], false, true);
        const id = await request(idx.get(range)) as string | undefined;
        return id === undefined ? undefined : (request(rows.get([table, id])) as Promise<Row | undefined>);
      },
      async scan(table, index, prefix, bounds, dir, limit) {
        const range = scanRange(schema, table, index, prefix, bounds, dir);
        if (!range || limit <= 0) return [];
        const name = `${table}.${index}`;
        const keyRange = IDBKeyRange.bound([name, idbKey(range[0])], [name, idbKey(range[1])], false, true);
        const out: { row: Row; key: Uint8Array }[] = [];
        await new Promise<void>((resolve, reject) => {
          const cursor = idx.openCursor(keyRange, dir === "asc" ? "next" : "prev");
          cursor.onerror = () => reject(cursor.error);
          cursor.onsuccess = () => {
            const c = cursor.result;
            if (!c || out.length >= limit) { resolve(); return; }
            const id = c.value as string;
            const key = new Uint8Array((c.key as [string, ArrayBuffer])[1]);
            const get = rows.get([table, id]);
            get.onsuccess = () => {
              if (get.result) out.push({ row: get.result as Row, key });
              if (out.length >= limit) resolve();
              else c.continue();
            };
            get.onerror = () => reject(get.error);
          };
        });
        return out;
      },
      async count(table, index, prefix, limit) {
        const range = scanRange(schema, table, index, prefix, {}, "asc");
        if (!range) return 0;
        const name = `${table}.${index}`;
        const n = await request(idx.count(IDBKeyRange.bound([name, idbKey(range[0])], [name, idbKey(range[1])], false, true)));
        return Math.min(n, limit);
      },
      async put(table, row) {
        const old = (await request(rows.get([table, row.id]))) as Row | undefined;
        const definition = schema.tables[table];
        if (!definition) throw new Error(`unknown table ${table}`);
        for (const index of Object.keys(definition.indexes)) {
          const name = `${table}.${index}`;
          if (old) await request(idx.delete([name, idbKey(indexKey(schema, table, index, old))]));
          await request(idx.put(row.id, [name, idbKey(indexKey(schema, table, index, row))]));
        }
        await request(rows.put(row, [table, row.id]));
      },
      async delete(table, id) {
        const old = (await request(rows.get([table, id]))) as Row | undefined;
        if (!old) return;
        for (const index of Object.keys(schema.tables[table]?.indexes ?? {})) {
          await request(idx.delete([`${table}.${index}`, idbKey(indexKey(schema, table, index, old))]));
        }
        await request(rows.delete([table, id]));
      },
      getMeta: (key) => request(meta.get(key)),
      setMeta: async (key, value) => { await request(meta.put(value, key)); },
      side,
    };
    return { transaction, api, done };
  }

  async read<T>(body: (tx: StoreTx) => Promise<T>): Promise<T> {
    const { transaction, api, done } = this.tx("readonly");
    try {
      const result = await body(api);
      await done;
      return result;
    } catch (error) {
      try { transaction.abort(); } catch { /* already done */ }
      throw error;
    }
  }

  async write<T>(body: (tx: StoreTx) => Promise<T>): Promise<T> {
    const { transaction, api, done } = this.tx("readwrite");
    try {
      const result = await body(api);
      await done;
      return result;
    } catch (error) {
      try { transaction.abort(); } catch { /* already done */ }
      throw error;
    }
  }

  async clearRows() {
    const transaction = this.db.transaction(["rows", "idx", "predicted"], "readwrite");
    await Promise.all([request(transaction.objectStore("rows").clear()), request(transaction.objectStore("idx").clear()), request(transaction.objectStore("predicted").clear())]);
  }

  async close() { this.db.close(); }
}
