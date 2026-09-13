// The interpreter on the device: the same IR the server runs, over the
// replica store, with the same meter and the same cursors, so a query is
// `complete` inside the horizon and a predictable mutation runs locally
// first (LLP 2000.000 §7.2, §7.3). Values are plain JSON here; the server's
// tagged literals are untagged as they are read.

import { canonicalJson, decodeCursor, encodeCursor, type ColumnTypeJson } from "./cursor";
import type { Bounds, Row, SchemaJson, StoreTx } from "./store";

export type Json = unknown;

export interface ProgramJson {
  name: string;
  kind: "Query" | "Mutation" | "job";
  args: Record<string, unknown>;
  body: unknown[];
}

export interface Refusal {
  code: string;
  family: string;
  message: string;
  site?: string;
  rule?: string;
  retryable?: boolean;
}

export class Refused extends Error {
  constructor(readonly refusal: Refusal) {
    super(`${refusal.code}: ${refusal.message}`);
  }
}

/** A query's outcome on the device. */
export interface LocalRead {
  data: unknown;
  /** False beyond a horizon, at an online-only site, or past the meter. */
  complete: boolean;
  next: string | null;
  tables: Set<string>;
}

export interface Ceilings { examined: number; ruleProbes: number; steps: number; rowsWritten: number }
export const CEILINGS: Ceilings = { examined: 8192, ruleProbes: 8192, steps: 1_000_000, rowsWritten: 1024 };

const untagged = (v: unknown): unknown => {
  if (v === null || typeof v !== "object") return v;
  if (Array.isArray(v)) return v.map(untagged);
  const entries = Object.entries(v as Record<string, unknown>);
  if (entries.length === 1) {
    const [tag, inner] = entries[0]!;
    switch (tag) {
      case "Null": return null;
      case "Bool": case "Int": case "String": case "Id": case "Bytes": return inner;
      case "Array": return (inner as unknown[]).map(untagged);
      case "Row": case "Map": return Object.fromEntries(Object.entries(inner as Record<string, unknown>).map(([k, x]) => [k, untagged(x)]));
    }
  }
  return v;
};
/** A tagged literal from the IR as a plain value. */
/** Every `.field` a rule reads, on any probe key or comparison. */
function ruleFields(rule: unknown, out: Set<string>) {
  if (typeof rule !== "object" || rule === null) return;
  const [kind, body] = Object.entries(rule as Record<string, unknown>)[0] as [string, unknown];
  const term = (t: unknown) => { if (typeof t === "object" && t !== null && "Field" in t) out.add((t as { Field: string }).Field); };
  switch (kind) {
    case "Eq": case "Ne": (body as unknown[]).forEach(term); break;
    case "And": case "Or": (body as unknown[]).forEach((r) => ruleFields(r, out)); break;
    case "Not": ruleFields(body, out); break;
    case "Exists": case "Created": (body as { key: unknown[] }).key.forEach(term); break;
  }
}

/** Whether a rule term is pinned by a scan's prefix expression. */
function pinnedTerm(term: unknown, expression: unknown): boolean {
  if (expression === undefined) return false;
  if (term === "Viewer") return typeof expression === "object" && expression !== null && (expression as { Input?: string }).Input === "Viewer";
  if (typeof term === "object" && term !== null && "Literal" in term) return typeof expression === "object" && expression !== null && "Literal" in expression && canonicalJson((term as { Literal: unknown }).Literal) === canonicalJson((expression as { Literal: unknown }).Literal);
  return false;
}

export function literal(v: unknown): unknown {
  if (v === "Null") return null;
  return untagged(v);
}

const tagOf = (v: unknown): number => v === null || v === undefined ? 0 : typeof v === "boolean" ? 1 : typeof v === "number" ? 2 : typeof v === "string" ? 3 : Array.isArray(v) ? 7 : 6;

/** The value model's total order over plain values. */
export function totalCompare(a: unknown, b: unknown): number {
  const ta = tagOf(a), tb = tagOf(b);
  if (ta !== tb) return ta < tb ? -1 : 1;
  switch (ta) {
    case 0: return 0;
    case 1: return a === b ? 0 : a ? 1 : -1;
    case 2: return (a as number) < (b as number) ? -1 : a === b ? 0 : 1;
    case 3: return (a as string) < (b as string) ? -1 : a === b ? 0 : 1;
    case 7: {
      const x = a as unknown[], y = b as unknown[];
      for (let i = 0; i < Math.min(x.length, y.length); i++) {
        const c = totalCompare(x[i], y[i]);
        if (c !== 0) return c;
      }
      return x.length - y.length;
    }
    default: {
      const x = Object.entries(a as Record<string, unknown>).sort(), y = Object.entries(b as Record<string, unknown>).sort();
      if (x.length !== y.length) return x.length - y.length;
      for (let i = 0; i < x.length; i++) {
        if (x[i]![0] !== y[i]![0]) return x[i]![0] < y[i]![0] ? -1 : 1;
        const c = totalCompare(x[i]![1], y[i]![1]);
        if (c !== 0) return c;
      }
      return 0;
    }
  }
}

export function equal(a: unknown, b: unknown): boolean {
  return totalCompare(a, b) === 0;
}

class Flow {
  constructor(readonly kind: "return" | "break" | "continue", readonly value?: unknown) {}
}

export interface Context {
  viewer: string;
  args: Record<string, unknown>;
  now: number;
  newIds: string[];
  mint: () => string;
}

/** Runs programs over one store transaction. */
export class Interpreter {
  private env = new Map<string, unknown>();
  private examined = 0;
  private probes = 0;
  private steps = 0;
  private written = 0;
  private capped = false;
  private beyondHorizon = false;
  /** The program's page had more rows and no cursor to continue on. */
  private pageOverflow = false;
  /** A field of a pending row was read while building an object. */
  private pendingTouched = false;
  private page: { table: string; index: string; order: "Asc" | "Desc"; prefixLen: number; next: unknown[] | null } | null = null;
  private pageDepth = 0;
  private created = new Set<string>();
  readonly tables = new Set<string>();
  readonly writes: { table: string; row: Row; old?: Row }[] = [];
  private site = "";

  constructor(private tx: StoreTx, private schema: SchemaJson, private ctx: Context, private mode: "query" | "mutation", private ceilings: Ceilings = CEILINGS) {
    for (const [k, v] of Object.entries(ctx.args)) this.env.set(k, v);
  }

  private charge(kind: "examined" | "probes", n = 1): boolean {
    if (kind === "examined") this.examined += n; else this.probes += n;
    const over = kind === "examined" ? this.examined > this.ceilings.examined : this.probes > this.ceilings.ruleProbes;
    if (!over) return true;
    if (this.mode === "mutation") throw new Refused({ code: "E_BOUND", family: "bound", message: `${kind === "examined" ? "examined rows" : "rule probes"} reached the ceiling at ${this.site}`, site: this.site });
    this.capped = true;
    return false;
  }

  private step(n = 1) {
    this.steps += n;
    if (this.steps > this.ceilings.steps) throw new Refused({ code: "E_BOUND", family: "bound", message: `steps reached ${this.steps} against the ceiling ${this.ceilings.steps}`, site: this.site });
  }

  async run(program: ProgramJson): Promise<LocalRead> {
    // An omitted optional argument (or cursor) reads as null, as on the server.
    for (const [name, kind] of Object.entries(program.args)) {
      if (!this.env.has(name) && (kind === "Cursor" || (typeof kind === "object" && kind !== null && "Optional" in kind))) this.env.set(name, null);
    }
    const flow = await this.statements(program.body, program.name);
    const data = flow instanceof Flow && flow.kind === "return" ? flow.value : null;
    return { data, complete: !this.capped && !this.beyondHorizon && !this.pageOverflow, next: this.capped ? null : this.nextCursor(), tables: this.tables };
  }

  private nextCursor(): string | null {
    if (!this.page || !this.page.next) return null;
    const table = this.schema.tables[this.page.table]!;
    const columns = table.indexes[this.page.index]!.components.map((c) => ("Column" in c ? c.Column : "")).slice(this.page.prefixLen);
    const suffix = columns.map((name) => ({ name, kind: (name === "id" ? "Id" : table.columns[name]) as ColumnTypeJson }));
    return encodeCursor({ table: this.page.table, index: this.page.index, order: this.page.order, suffix, values: this.page.next });
  }

  private async statements(stmts: unknown[], prefix: string): Promise<Flow | undefined> {
    for (let i = 0; i < stmts.length; i++) {
      this.site = `${prefix}:${i + 1}`;
      this.step();
      const raw: unknown = stmts[i];
      if (raw === "Break") return new Flow("break");
      if (raw === "Continue") return new Flow("continue");
      const [kind, body] = Object.entries(raw as Record<string, unknown>)[0] as [string, Record<string, unknown>];
      switch (kind) {
        case "Const": case "Let": case "Assign": this.env.set(body.name as string, await this.expr(body.value)); break;
        case "Push": {
          const value = await this.expr(body.value);
          const target = this.env.get(body.target as string);
          if (!Array.isArray(target)) throw new Refused({ code: "E_TYPE", family: "input", message: `push into ${body.target}` });
          target.push(value);
          break;
        }
        case "If": {
          const c = await this.expr(body.condition);
          const flow = await this.statements((c ? body.then_body : body.else_body) as unknown[], `${this.site}:${c ? "then" : "else"}`);
          if (flow) return flow;
          break;
        }
        case "For": {
          const items = await this.domain(body.domain);
          const isPage = typeof body.domain === "object" && body.domain !== null && ("Page" in (body.domain as object) || "Recurse" in (body.domain as object));
          if (isPage) this.pageDepth++;
          let result: Flow | undefined;
          for (const item of items) {
            this.env.set(body.binding as string, item);
            const flow = await this.statements(body.body as unknown[], this.site);
            if (flow?.kind === "break") break;
            if (flow?.kind === "return") { result = flow; break; }
          }
          if (isPage) this.pageDepth--;
          if (result) return result;
          break;
        }
        case "Require": {
          if (!(await this.expr(body.condition))) throw new Refused({ code: body.code as string, family: "rule", message: `the program refused with ${body.code}`, rule: body.code as string, site: (body.site as string) ?? this.site });
          break;
        }
        case "Check": {
          if (!(await this.expr(body.condition))) throw new Refused({ code: "E_CONFLICT", family: "constraint", message: "the row no longer matches the update condition", retryable: true });
          break;
        }
        case "InsertIfAbsent": {
          const row = (await this.expr(body.row)) as Row;
          const inserted = await this.insert(body.table as string, row, true);
          this.env.set(body.result as string, inserted ? row : null);
          break;
        }
        case "Insert": await this.insert(body.table as string, (await this.expr(body.row)) as Row, false); break;
        case "Update": await this.update(body.table as string, (await this.expr(body.key)) as string, (await this.expr(body.patch)) as Record<string, unknown>); break;
        case "Upsert": await this.upsert(body.table as string, (await this.expr(body.key)) as string, (await this.expr(body.row)) as Row); break;
        case "Delete": await this.delete(body.table as string, (await this.expr(body.key)) as string); break;
        case "Emit": throw new Refused({ code: "E_PREDICT", family: "predict", message: "effects run on the server; this write is not predicted", retryable: true });
        case "Return": return new Flow("return", await this.expr(body));
        default: throw new Refused({ code: "E_SCHEMA", family: "schema", message: `unknown statement ${kind}` });
      }
    }
    return undefined;
  }

  private async domain(domain: unknown): Promise<unknown[]> {
    const [kind, body] = Object.entries(domain as Record<string, unknown>)[0] as [string, Record<string, unknown>];
    switch (kind) {
      case "Page": return this.scan(body);
      case "LiteralRange": { const out = []; for (let i = body.start as number; i < (body.end_exclusive as number); i++) out.push(i); return out; }
      case "ArgList": { const v = this.env.get(body.name as string); if (!Array.isArray(v)) return []; if (v.length > (body.max as number)) throw new Refused({ code: "E_FANOUT", family: "bound", message: "more items than the bound" }); return v; }
      case "DerivedArray": { const v = await this.expr(body.value); if (!Array.isArray(v)) return []; if (v.length > (body.max as number)) throw new Refused({ code: "E_FANOUT", family: "bound", message: "more items than the bound" }); return v; }
      case "Recurse": return this.recurse(body);
      default: throw new Refused({ code: "E_SCHEMA", family: "schema", message: `unknown loop domain ${kind}` });
    }
  }

  private async recurse(r: Record<string, unknown>): Promise<unknown[]> {
    const seed = await this.expr(r.seed);
    const queue: [unknown, number][] = [[seed, 0]];
    const visited = new Set<string>();
    const rows: unknown[] = [];
    while (queue.length) {
      const [key, depth] = queue.shift()!;
      if (depth > (r.depth as number) || visited.has(canonicalJson(key))) continue;
      visited.add(canonicalJson(key));
      const row = await this.get({ site: r.site as string, table: r.table as string, index: r.index as string, key: [{ Literal: tag(key) }] });
      if (row === null) continue;
      rows.push(row);
      const next = (row as Row)[r.next_field as string];
      if (next === null || next === undefined) continue;
      if (Array.isArray(next)) { if (next.length > (r.fanout as number)) throw new Refused({ code: "E_FANOUT", family: "bound", message: "walk fan-out exceeds the bound" }); for (const n of next) queue.push([n, depth + 1]); }
      else queue.push([next, depth + 1]);
    }
    return rows;
  }

  async expr(e: unknown): Promise<unknown> {
    this.step();
    if (typeof e === "string") {
      // Unit variants appear as bare strings.
      throw new Refused({ code: "E_SCHEMA", family: "schema", message: `bare expression ${e}` });
    }
    const [kind, body] = Object.entries(e as Record<string, unknown>)[0] as [string, unknown];
    switch (kind) {
      case "Literal": return literal(body);
      case "Variable": { if (!this.env.has(body as string)) throw new Refused({ code: "E_INPUT", family: "input", message: `unknown variable ${body}` }); return this.env.get(body as string); }
      case "Input": switch (body as string) {
        case "Viewer": return this.ctx.viewer;
        case "Now": return this.ctx.now;
        case "NewId": return this.ctx.newIds.shift() ?? this.ctx.mint();
        default: return this.ctx.now;
      }
      case "Field": { const b = body as { base: unknown; name: string }; const base = await this.expr(b.base); if (base === null || base === undefined) return null; if (typeof base !== "object") throw new Refused({ code: "E_FIELD", family: "input", message: `no field ${b.name}` }); if ((base as Record<string, unknown>).pending === true) this.pendingTouched = true; return (base as Record<string, unknown>)[b.name] ?? null; }
      case "Array": { const out = []; for (const item of body as unknown[]) out.push(await this.expr(item)); return out; }
      case "Object": {
        // A shape over a pending row is pending: the card's word survives
        // the field selection, so a screen shows "Sending…" on shaped rows too.
        const outer = this.pendingTouched;
        this.pendingTouched = false;
        const out: Record<string, unknown> = {};
        for (const [k, v] of Object.entries(body as Record<string, unknown>)) out[k] = await this.expr(v);
        if (this.pendingTouched && !("pending" in out)) out.pending = true;
        this.pendingTouched = outer || this.pendingTouched;
        return out;
      }
      case "Binary": {
        const b = body as { op: string; left: unknown; right: unknown };
        const left = await this.expr(b.left);
        if (b.op === "And" && !left) return false;
        if (b.op === "Or" && left) return true;
        const right = await this.expr(b.right);
        switch (b.op) {
          case "Eq": return equal(left, right);
          case "Ne": return !equal(left, right);
          case "Lt": return totalCompare(left, right) < 0;
          case "Lte": return totalCompare(left, right) <= 0;
          case "Gt": return totalCompare(left, right) > 0;
          case "Gte": return totalCompare(left, right) >= 0;
          case "Add": return num(left) + num(right);
          case "Sub": return num(left) - num(right);
          case "Mul": return num(left) * num(right);
          case "And": return Boolean(left) && Boolean(right);
          case "Or": return Boolean(left) || Boolean(right);
          default: return equal(left, right);
        }
      }
      case "Unary": { const b = body as { op: string; value: unknown }; const v = await this.expr(b.value); return b.op === "Not" ? !v : -num(v); }
      case "Builtin": return this.builtin(body as { builtin: unknown; args: unknown[] });
      case "Get": return this.get(body as ReadJson);
      case "Exists": return (await this.get(body as ReadJson)) !== null;
      case "Scan": return this.scan(body as Record<string, unknown>);
      case "Merge": return this.merge(body as Record<string, unknown>);
      default: throw new Refused({ code: "E_SCHEMA", family: "schema", message: `unknown expression ${kind}` });
    }
  }

  private async builtin(b: { builtin: unknown; args: unknown[] }): Promise<unknown> {
    const args: unknown[] = [];
    for (const a of b.args) args.push(await this.expr(a));
    const name = typeof b.builtin === "string" ? b.builtin : Object.keys(b.builtin as object)[0]!;
    switch (name) {
      case "Len": case "Count": case "CountCapped": { const v = args[0]; return typeof v === "string" ? [...v].length : Array.isArray(v) ? v.length : v && typeof v === "object" ? Object.keys(v).length : 0; }
      case "Sum": return (args[0] as number[]).reduce((s, x) => s + x, 0);
      case "Slice": { const [v, s, e] = args as [unknown, number, number]; const start = Math.max(0, s), end = Math.max(0, e); return typeof v === "string" ? [...v].slice(start, end).join("") : (v as unknown[]).slice(start, end); }
      case "Concat": { const [l, r] = args; return Array.isArray(l) && Array.isArray(r) ? [...l, ...r] : `${String(l ?? "")}${String(r ?? "")}`; }
      case "Min": return totalCompare(args[0], args[1]) <= 0 ? args[0] : args[1];
      case "Max": return totalCompare(args[0], args[1]) >= 0 ? args[0] : args[1];
      case "Divmod": { const [n, d] = args as [number, number]; if (d === 0) throw new Refused({ code: "E_DIV_ZERO", family: "input", message: "division by zero" }); const q = Math.floor(n / d); return { quotient: q, remainder: n - q * d, sink: "lexicographic-id" }; }
      case "Sort": { const keys = (b.builtin as { Sort: { keys: { field: string; order: "Asc" | "Desc" }[] } }).Sort.keys; return [...(args[0] as Record<string, unknown>[])].sort((x, y) => { for (const k of keys) { const c = totalCompare(x[k.field], y[k.field]); if (c !== 0) return k.order === "Desc" ? -c : c; } return 0; }); }
      case "MentionTokens": { const seen = new Set<string>(); const out: string[] = []; for (const w of String(args[0]).split(/[\s,;()[\]{}<>"']+/)) { if (!w.startsWith("@")) continue; const t = w.slice(1).replace(/^[^\p{L}\p{N}_]+|[^\p{L}\p{N}_]+$/gu, ""); if (t && !seen.has(t)) { seen.add(t); out.push(t); } } return out; }
      case "DeterministicShare": { const [amount, list, who] = args as [number, unknown[], unknown]; const sorted = [...list].sort(totalCompare); const at = sorted.findIndex((x) => equal(x, who)); if (at < 0) throw new Refused({ code: "E_INPUT", family: "input", message: "recipient missing from shares" }); const n = sorted.length; const base = Math.floor(amount / n); return base + (at < amount - base * n ? 1 : 0); }
      default: throw new Refused({ code: "E_SCHEMA", family: "schema", message: `unknown builtin ${name}` });
    }
  }

  // ----- reads -----

  /** Whether a held row is past its table's `expire` column on this
   * device's clock: absent to every read, as on the server, until the
   * server's sweep deletes it and the deletion arrives in the log. */
  private expired(table: string, row: Row): boolean {
    const column = this.schema.tables[table]?.expire;
    if (!column) return false;
    const at = row[column];
    return typeof at === "number" && at <= this.ctx.now;
  }

  private async get(read: ReadJson): Promise<unknown> {
    const key: unknown[] = [];
    for (const k of read.key) key.push(await this.expr(k));
    this.site = read.site;
    this.tables.add(read.table);
    const table = this.schema.tables[read.table];
    if (!table) throw new Refused({ code: "E_SCHEMA", family: "schema", message: `unknown table ${read.table}` });
    if (!table.sync) this.beyondHorizon = true;
    // Absent, hidden or visible: one examined row; the probes that decided
    // a hidden row are rolled back, as on the server.
    const mark = { probes: this.probes, capped: this.capped };
    const row = await this.tx.lookup(read.table, read.index, key);
    if (!this.charge("examined")) return null;
    if (!row || this.expired(read.table, row)) return null;
    if (await this.readable(read.table, row)) return row;
    this.probes = mark.probes; this.capped = mark.capped;
    return null;
  }

  private async prepareScan(scan: Record<string, unknown>) {
    const table = scan.table as string, index = scan.index as string;
    const prefix: unknown[] = [];
    for (const p of scan.prefix as { Scalar?: unknown }[]) prefix.push(p.Scalar === undefined ? null : await this.expr(p.Scalar));
    const definition = this.schema.tables[table];
    if (!definition) throw new Refused({ code: "E_SCHEMA", family: "schema", message: `unknown table ${table}` });
    const take = typeof scan.take === "object" && scan.take !== null && "Literal" in (scan.take as object) ? (scan.take as { Literal: number }).Literal : num(this.env.get((scan.take as { Arg: { name: string } }).Arg.name));
    const order = scan.order as "Asc" | "Desc";
    const bounds: Bounds = {};
    if (scan.after) {
      const after = await this.expr(scan.after);
      if (after !== null && after !== undefined) {
        if (scan.opaque_cursor) {
          const position = decodeCursor(String(after));
          if (!position || position.table !== table || position.index !== index || position.order !== order) throw new Refused({ code: "E_INPUT", family: "input", message: "cursor: the token names another scope" });
          bounds.after = position.values;
        } else bounds.after = Array.isArray(after) ? after : [after];
      }
    }
    if (scan.before) { const before = await this.expr(scan.before); if (before !== null && before !== undefined) bounds.before = Array.isArray(before) ? before : [before]; }
    if (scan.range) {
      const r = scan.range as Record<string, unknown>;
      for (const k of ["gt", "gte", "lt", "lte"] as const) if (r[k]) bounds[k] = await this.expr(r[k]);
    }
    return { table, index, prefix, definition, take, order, bounds };
  }

  private async scan(scan: Record<string, unknown>, prepared?: Awaited<ReturnType<Interpreter["prepareScan"]>>): Promise<unknown[]> {
    const { table, index, prefix, definition, take, order, bounds } = prepared ?? await this.prepareScan(scan);
    this.tables.add(table);
    this.site = scan.site as string;
    if (!definition.sync) this.beyondHorizon = true;
    const dir = order === "Asc" ? "asc" : "desc";
    const visible: unknown[] = [];
    let lastKey: Uint8Array | null = null;
    let hasMore = false;
    let cursor: Bounds = { ...bounds };
    const batch = take + 1;
    const { uniform, decidedTrue } = this.visibility(table, index, scan.prefix as { Scalar?: unknown }[]);
    let verdict: boolean | null = decidedTrue ? true : null;
    const mark = { examined: this.examined, probes: this.probes, capped: this.capped };
    outer: for (;;) {
      const fetched = await this.tx.scan(table, index, prefix, cursor, dir, batch);
      if (fetched.length === 0) break;
      for (const { row, key } of fetched) {
        if (!this.charge("examined")) break outer;
        cursor = { ...cursor, after: suffixOf(this.schema, table, index, row, prefix.length) };
        // Gone, whatever the rule says; it cost one examined row.
        if (this.expired(table, row)) continue;
        let allowed: boolean;
        if (verdict !== null) allowed = verdict;
        else {
          allowed = await this.readable(table, row);
          if (uniform) verdict = allowed;
        }
        if (this.capped) break outer;
        if (allowed) {
          if (visible.length === take) { hasMore = true; break outer; }
          visible.push(row);
          lastKey = key;
        } else if (uniform) {
          // One hidden row hides the prefix: stop without reading it, at
          // the cost of an empty range.
          this.examined = mark.examined; this.probes = mark.probes; this.capped = mark.capped;
          break outer;
        }
      }
      if (fetched.length < batch) break;
    }
    // Inside the horizon, a short page is the whole group; at the horizon
    // it may continue on the server.
    const horizon = definition.sync?.horizon;
    if (horizon && !hasMore && !this.capped && horizon.by === index) {
      const held = await this.tx.count(table, index, prefix, horizon.last);
      if (held >= horizon.last) this.beyondHorizon = true;
    }
    if (this.pageDepth === 0 && !scan.opaque_cursor && hasMore) this.pageOverflow = true;
    if (this.pageDepth === 0 && scan.opaque_cursor) {
      const columns = definition.indexes[index]!.components.length;
      this.page = { table, index, order, prefixLen: prefix.length, next: hasMore && lastKey && visible.length ? suffixOf(this.schema, table, index, visible[visible.length - 1] as Row, prefix.length).slice(0, columns - prefix.length) : null };
    }
    return visible;
  }

  private async merge(merge: Record<string, unknown>): Promise<unknown[]> {
    const lanes = merge.lanes as Record<string, unknown>[];
    const leading = lanes[0]?.scan as Record<string, unknown> | undefined;
    // @ref LLP 3000#7-decisions-during-execution — lazy uniform merge lanes
    if (lanes.every((lane) => {
      const scan = lane.scan as Record<string, unknown>;
      const index = this.schema.tables[scan.table as string]?.indexes[scan.index as string];
      const totalUnique = index?.unique && index.components.every((c) => {
        const kind = "Column" in c ? this.schema.tables[scan.table as string]?.columns[c.Column] : undefined;
        return kind !== undefined && !(kind && typeof kind === "object" && "Optional" in kind);
      });
      return !(lane.setup as unknown[] | undefined)?.length && scan.table === leading?.table && scan.index === leading?.index && scan.order === leading?.order
        && (scan.prefix as unknown[]).length === (leading?.prefix as unknown[]).length
        && index && (scan.prefix as unknown[]).length < index.components.length
        && (totalUnique || (index.components[index.components.length-1] as { Column?: string })?.Column === "id")
        && this.visibility(scan.table as string, scan.index as string, scan.prefix as { Scalar?: unknown }[]).uniform;
    })) return this.mergeLazy(merge);
    const first = lanes[0]?.scan as Record<string, unknown> | undefined;
    if (!first) return [];
    const table = first.table as string, index = first.index as string, order = first.order as "Asc" | "Desc";
    const take = (first.take as { Literal: number }).Literal;
    const columns = this.schema.tables[table]!.indexes[index]!.components.map((c) => ("Column" in c ? c.Column : ""));
    const prefixLen = (first.prefix as unknown[]).length;
    const suffixColumns = columns.slice(prefixLen);
    const candidates: { key: unknown[]; row: Row }[] = [];
    const seen = new Set<string>();
    const depth = this.pageDepth;
    for (const lane of lanes) {
      const sources = lane.source ? await (async () => { this.pageDepth++; const r = await this.scan(lane.source as Record<string, unknown>); this.pageDepth--; return r; })() : [null];
      for (const source of sources) {
        if (lane.source) this.env.set(lane.binding as string, source);
        await this.statements((lane.setup as unknown[]) ?? [], merge.site as string);
        this.pageDepth++;
        // One row past the page per lane, so a lane that alone fills the
        // page still shows there is more (as on the server).
        const rows = await this.scan({ ...(lane.scan as Record<string, unknown>), take: { Literal: take + 1 } });
        this.pageDepth = depth;
        for (const row of rows as Row[]) {
          if (seen.has(row.id)) continue;
          seen.add(row.id);
          candidates.push({ key: suffixColumns.map((c) => row[c] ?? null), row });
        }
      }
    }
    candidates.sort((a, b) => { const c = totalCompare(a.key, b.key); return order === "Asc" ? c : -c; });
    const hasMore = candidates.length > take;
    const page = candidates.slice(0, take);
    if (this.pageDepth === 0 && !first.opaque_cursor && hasMore) this.pageOverflow = true;
    if (this.pageDepth === 0 && first.opaque_cursor) this.page = { table, index, order, prefixLen, next: hasMore && page.length ? page[page.length - 1]!.key : null };
    return page.map((c) => c.row);
  }

  private async mergeLazy(merge: Record<string, unknown>): Promise<unknown[]> {
    const lanes = merge.lanes as Record<string, unknown>[];
    const first = lanes[0]?.scan as Record<string, unknown> | undefined;
    if (!first) return [];
    const table = first.table as string, index = first.index as string, order = first.order as "Asc" | "Desc";
    const take = "Literal" in (first.take as object) ? (first.take as { Literal: number }).Literal : num(this.env.get((first.take as { Arg: { name: string } }).Arg.name));
    const prefixLen = (first.prefix as unknown[]).length;
    const columns = this.schema.tables[table]!.indexes[index]!.components.slice(prefixLen).map((c) => "Column" in c ? c.Column : "");
    // Validate even when lane discovery is empty, as the server does.
    if (first.opaque_cursor && first.after) {
      const after = await this.expr(first.after);
      if (after !== null && after !== undefined) {
        const c = decodeCursor(String(after));
        if (!c || c.table !== table || c.index !== index || c.order !== order) throw new Refused({ code: "E_INPUT", family: "input", message: "cursor: the token names another scope" });
      }
    }
    type Head = { key: unknown[]; row: Row; lane: number };
    const heap: Head[] = [];
    const cmp = (a: Head, b: Head) => (order === "Asc" ? 1 : -1) * totalCompare(a.key, b.key) || a.lane - b.lane;
    const push = (head: Head) => {
      let i = heap.length;
      heap.push(head);
      while (i > 0) {
        const parent = (i - 1) >> 1;
        if (cmp(heap[parent]!, head) <= 0) break;
        heap[i] = heap[parent]!; i = parent;
      }
      heap[i] = head;
    };
    const pop = () => {
      const first = heap[0]!, last = heap.pop()!;
      if (heap.length) {
        let i = 0;
        for (;;) {
          let child = i * 2 + 1;
          if (child >= heap.length) break;
          if (child + 1 < heap.length && cmp(heap[child + 1]!, heap[child]!) < 0) child++;
          if (cmp(last, heap[child]!) <= 0) break;
          heap[i] = heap[child]!; i = child;
        }
        heap[i] = last;
      }
      return first;
    };
    const states: { scan: Record<string, unknown>; prepared: Awaited<ReturnType<Interpreter["prepareScan"]>>; rows: Row[]; done: boolean }[] = [];
    const depth = this.pageDepth;
    this.pageDepth++;
    const advance = async (lane: number) => {
      const state = states[lane]!;
      this.step();
      if (!state.rows.length && !state.done && !this.capped) {
        state.rows = await this.scan(state.scan, state.prepared) as Row[];
        state.done = state.rows.length < state.prepared.take;
      }
      const row = state.rows.shift();
      if (row) push({ key: columns.map((c) => row[c] ?? null), row, lane });
    };
    for (const lane of lanes) {
      const sources = lane.source ? await this.scan(lane.source as Record<string, unknown>) : [null];
      for (const source of sources) {
        if (lane.source) this.env.set(lane.binding as string, source);
        const scan = lane.scan as Record<string, unknown>;
        const prepared = await this.prepareScan(scan);
        states.push({ scan, prepared, rows: [], done: false });
      }
    }
    // Batch the few-lane case so it does not pay one SQLite call per row.
    for (let lane = 0; lane < states.length; lane++) {
      states[lane]!.prepared.take = states.length <= 8 ? take + 1 : 1;
      await advance(lane);
    }
    const seen = new Set<string>(), page: Head[] = [];
    let hasMore = false;
    while (heap.length) {
      this.step();
      const head = pop();
      if (!seen.has(head.row.id)) {
        seen.add(head.row.id);
        if (page.length === take) { hasMore = true; break; }
        page.push(head);
      }
      states[head.lane]!.prepared.bounds.after = head.key;
      await advance(head.lane);
    }
    this.pageDepth = depth;
    if (depth === 0) {
      if (first.opaque_cursor) this.page = { table, index, order, prefixLen, next: (hasMore || this.capped) && page.length ? page[page.length - 1]!.key : null };
      else if (hasMore) this.pageOverflow = true;
    }
    return page.map((c) => c.row);
  }

  /** The server's `scan_visibility`, on the device: a read rule whose every
   * field the prefix pins is decided once for the whole range (`uniform`);
   * one the prefix proves is `decidedTrue` before any row is read. A
   * uniform-false range halts after one row at the cost of an empty one,
   * so the rows the viewer may no longer read (delivered history) are never
   * counted. */
  private visibility(table: string, index: string, prefix: { Scalar?: unknown }[]): { uniform: boolean; decidedTrue: boolean } {
    const definition = this.schema.tables[table]!;
    const rule = (definition as unknown as { rules: { read: unknown } }).rules.read;
    const pinned = new Map<string, unknown>();
    definition.indexes[index]!.components.forEach((component, n) => {
      if ("Column" in component && n < prefix.length && prefix[n]!.Scalar !== undefined) pinned.set(component.Column, prefix[n]!.Scalar);
    });
    const decidedTrue = this.ruleTrue(rule, table, pinned);
    const used = new Set<string>();
    ruleFields(rule, used);
    return { uniform: decidedTrue || [...used].every((field) => pinned.has(field)), decidedTrue };
  }

  private ruleTrue(rule: unknown, table: string, pinned: Map<string, unknown>): boolean {
    if (rule === "Allow") return true;
    if (typeof rule !== "object" || rule === null) return false;
    const [kind, body] = Object.entries(rule as Record<string, unknown>)[0] as [string, unknown];
    switch (kind) {
      case "Public": return true;
      case "Eq": {
        const [a, b] = body as [unknown, unknown];
        return [[a, b], [b, a]].some(([field, value]) => typeof field === "object" && field !== null && "Field" in field && pinnedTerm(value, pinned.get((field as { Field: string }).Field)));
      }
      case "Or": return (body as unknown[]).some((r) => this.ruleTrue(r, table, pinned));
      case "And": return (body as unknown[]).length > 0 && (body as unknown[]).every((r) => this.ruleTrue(r, table, pinned));
      case "Exists": {
        // A self-probe on a total unique key whose every component is the
        // row's own column or pinned to the probing value holds for every
        // row the prefix selects.
        const b = body as { table: string; index: string; key: unknown[] };
        if (b.table !== table) return false;
        const definition = this.schema.tables[table]!;
        const index = definition.indexes[b.index];
        if (!index || !index.unique || b.key.length === 0 || b.key.length !== index.components.length) return false;
        return index.components.every((component, n) => {
          if (!("Column" in component)) return false;
          const type = definition.columns[component.Column];
          if (typeof type === "object" && type !== null && "Optional" in type) return false;
          const term = b.key[n];
          if (typeof term === "object" && term !== null && "Field" in term) return (term as { Field: string }).Field === component.Column;
          return pinnedTerm(term, pinned.get(component.Column));
        });
      }
      default: return false;
    }
  }

  /** Whether the viewer may read a held row. A synced table's rows are
   * the partition: the server delivered each under the read rule and
   * withdrew what it revoked, so what the device holds it may read — and
   * what `retain delivered-history` kept after a leave stays readable here
   * even though the server now refuses it. Only an online-only table's
   * rows (none, unless predicted) meet the rule on the device. */
  async readable(table: string, row: Row): Promise<boolean> {
    const definition = this.schema.tables[table];
    if (definition?.sync) return true;
    const rule = (definition as unknown as { rules: { read: unknown } }).rules.read;
    return this.rule(rule, undefined, row, false);
  }

  private async rule(rule: unknown, old: Row | undefined, next: Row, insert: boolean): Promise<boolean> {
    if (rule === "Allow") return true;
    if (rule === "Deny") return false;
    const [kind, body] = Object.entries(rule as Record<string, unknown>)[0] as [string, unknown];
    const term = (t: unknown): unknown => {
      if (t === "Viewer") return this.ctx.viewer;
      const [k, v] = Object.entries(t as Record<string, unknown>)[0] as [string, unknown];
      if (k === "Field" || k === "NextField") return next[v as string] ?? null;
      if (k === "OldField") return old?.[v as string] ?? null;
      return literal(v);
    };
    switch (kind) {
      case "Public": return true;
      case "Eq": return equal(term((body as unknown[])[0]), term((body as unknown[])[1]));
      case "Ne": return !equal(term((body as unknown[])[0]), term((body as unknown[])[1]));
      case "And": for (const r of body as unknown[]) if (!(await this.rule(r, old, next, insert))) return false; return true;
      case "Or": for (const r of body as unknown[]) if (await this.rule(r, old, next, insert)) return true; return false;
      case "Not": return !(await this.rule(body, old, next, insert));
      case "Exists": {
        const b = body as { table: string; index: string; key: unknown[] };
        if (!this.charge("probes")) return false;
        this.tables.add(b.table);
        const found = await this.tx.lookup(b.table, b.index, b.key.map(term));
        return found !== undefined && !this.expired(b.table, found);
      }
      case "Created": {
        const b = body as { table: string; index: string; key: unknown[] };
        return this.created.has(`${b.table}\u0000${b.index}\u0000${canonicalJson(b.key.map(term))}`);
      }
      default: return false;
    }
  }

  // ----- writes (predictions) -----

  private async permitted(table: string, effect: "insert" | "update" | "delete", old: Row | undefined, next: Row) {
    const rules = (this.schema.tables[table] as unknown as { rules: Record<string, unknown> }).rules;
    if (!(await this.rule(rules[effect], old, next, effect === "insert"))) throw new Refused({ code: "E_RULE", family: "auth", message: `${table}'s ${effect} rule does not admit this viewer` });
  }

  private async constraints(table: string, old: Row | undefined, row: Row, effect: "insert" | "update") {
    const definition = this.schema.tables[table]! as TableWithConstraints;
    for (const [name, index] of Object.entries(definition.indexes)) {
      if (!index.unique) continue;
      const columns = index.components.map((c) => ("Column" in c ? c.Column : ""));
      const key = columns.map((c) => row[c] ?? null);
      if (key.some((v) => v === null)) continue;
      const hit = await this.tx.lookup(table, name, key);
      if (hit && hit.id !== row.id) throw new Refused({ code: "E_CONSTRAINT", family: "constraint", message: `${table} already has a row where ${columns.join(", ")} = ${key.map(String).join(", ")}` });
    }
    for (const constraint of definition.constraints ?? []) {
      if (typeof constraint !== "object" || constraint === null) continue;
      if ("Cap" in constraint) {
        const { by, max } = (constraint as { Cap: { by: string[]; max: number } }).Cap;
        const group = by.map((c) => row[c] ?? null);
        const moved = !old || by.some((c) => !equal(old[c] ?? null, row[c] ?? null));
        if (!moved) continue;
        const index = Object.entries(definition.indexes).find(([, i]) => by.every((c, n) => (i.components[n] as { Column?: string })?.Column === c))?.[0];
        if (!index) continue;
        if ((await this.tx.count(table, index, group, max + 1)) >= max) throw new Refused({ code: "E_CONSTRAINT", family: "constraint", message: `${table} holds at most ${max} rows where ${by.join(", ")} = ${group.map(String).join(", ")}` });
      }
      if ("Immutable" in constraint && old && effect === "update") {
        for (const field of (constraint as { Immutable: { fields: string[] } }).Immutable.fields) if (!equal(old[field] ?? null, row[field] ?? null)) throw new Refused({ code: "E_CONSTRAINT", family: "constraint", message: `${table}.${field} is immutable` });
      }
    }
  }

  private async insert(table: string, row: Row, ifAbsent: boolean): Promise<boolean> {
    this.tables.add(table);
    await this.permitted(table, "insert", undefined, row);
    if (ifAbsent) {
      for (const [name, index] of Object.entries(this.schema.tables[table]!.indexes)) {
        if (!index.unique) continue;
        const key = index.components.map((c) => ("Column" in c ? row[c.Column] ?? null : null));
        if (await this.tx.lookup(table, name, key)) return false;
      }
    }
    if (await this.tx.get(table, row.id)) throw new Refused({ code: "E_CONSTRAINT", family: "constraint", message: `${table} already has a row with id ${row.id}` });
    await this.constraints(table, undefined, row, "insert");
    await this.put(table, { ...row, pending: true });
    for (const [name, index] of Object.entries(this.schema.tables[table]!.indexes)) this.created.add(`${table}\u0000${name}\u0000${canonicalJson(index.components.map((c) => ("Column" in c ? row[c.Column] ?? null : null)))}`);
    return true;
  }

  private async update(table: string, id: string, patch: Record<string, unknown>) {
    this.tables.add(table);
    const old = await this.tx.get(table, id);
    if (!old || this.expired(table, old)) throw new Refused({ code: "NOT_FOUND", family: "rule", message: "the row does not exist on this device", rule: "NOT_FOUND" });
    const next = { ...old, ...patch, pending: true } as Row;
    await this.permitted(table, "update", old, next);
    await this.constraints(table, old, next, "update");
    await this.put(table, next, old);
  }

  private async upsert(table: string, id: string, row: Row) {
    const old = await this.tx.get(table, id);
    if (old) return this.update(table, id, row);
    await this.insert(table, row, false);
  }

  private async delete(table: string, id: string) {
    this.tables.add(table);
    const old = await this.tx.get(table, id);
    if (!old || this.expired(table, old)) throw new Refused({ code: "NOT_FOUND", family: "rule", message: "the row does not exist on this device", rule: "NOT_FOUND" });
    await this.permitted(table, "delete", undefined, old);
    this.written++;
    if (this.written > this.ceilings.rowsWritten) throw new Refused({ code: "E_BOUND", family: "bound", message: "rows written reached the ceiling", site: this.site });
    await this.tx.delete(table, id);
    this.writes.push({ table, row: old, old });
    await this.maintain(table, old, undefined);
  }

  private async put(table: string, row: Row, old?: Row) {
    this.written++;
    if (this.written > this.ceilings.rowsWritten) throw new Refused({ code: "E_BOUND", family: "bound", message: "rows written reached the ceiling", site: this.site });
    await this.tx.put(table, row);
    this.writes.push(old ? { table, row, old } : { table, row });
    await this.maintain(table, old, row);
  }

  /** A device keeps its counts honest while a prediction stands. */
  private async maintain(source: string, old: Row | undefined, next: Row | undefined) {
    for (const maintain of (this.schema.maintains ?? []) as MaintainJson[]) {
      if (maintain.source !== source) continue;
      const delta = (row: Row | undefined, sign: number) => (row ? (maintain.kind === "Count" ? sign : sign * num(row[(maintain.kind as { Sum: { of: string } }).Sum.of])) : 0);
      const groups = new Map<string, { key: unknown[]; change: number }>();
      for (const [row, sign] of [[old, -1], [next, 1]] as [Row | undefined, number][]) {
        if (!row) continue;
        const key = maintain.by.map((c) => row[c] ?? null);
        const id = canonicalJson(key.map(tag));
        const g = groups.get(id) ?? { key, change: 0 };
        g.change += delta(row, sign);
        groups.set(id, g);
      }
      for (const [id, g] of groups) {
        if (g.change === 0) continue;
        const existing = await this.tx.get(maintain.target, id);
        const value = num(existing?.value ?? 0) + g.change;
        if (value === 0) { if (existing) await this.tx.delete(maintain.target, id); continue; }
        const row: Row = { id, value, pending: true };
        maintain.by.forEach((c, i) => { row[c] = g.key[i]; });
        await this.tx.put(maintain.target, row);
      }
    }
  }
}

interface ReadJson { site: string; table: string; index: string; key: unknown[] }
interface MaintainJson { kind: "Count" | { Sum: { of: string } }; source: string; by: string[]; target: string }
type TableWithConstraints = SchemaJson["tables"][string] & { constraints?: unknown[] };

function num(v: unknown): number {
  if (typeof v !== "number") throw new Refused({ code: "E_TYPE", family: "input", message: `expected a number, got ${typeof v}` });
  return v;
}

/** A plain value as the server's tagged literal (for canonical ids). */
export function tag(v: unknown): unknown {
  if (v === null || v === undefined) return "Null";
  if (typeof v === "boolean") return { Bool: v };
  if (typeof v === "number") return { Int: v };
  if (typeof v === "string") return { Id: v };
  if (Array.isArray(v)) return { Array: v.map(tag) };
  return { Row: Object.fromEntries(Object.entries(v as Record<string, unknown>).map(([k, x]) => [k, tag(x)])) };
}

function suffixOf(schema: SchemaJson, table: string, index: string, row: Row, prefixLen: number): unknown[] {
  const columns = schema.tables[table]!.indexes[index]!.components.map((c) => ("Column" in c ? c.Column : ""));
  const suffix = columns.slice(prefixLen).map((c) => row[c] ?? null);
  if (columns[columns.length - 1] !== "id") suffix.push(row.id);
  return suffix;
}
