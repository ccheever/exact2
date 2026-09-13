// The replica: the viewer's partitions on the device as one store with one
// watermark (LLP 2000.000 §7.2). A fresh device takes a snapshot, then
// follows the merged stream; every batch applies in one transaction with
// its watermark; a scope tombstone applies the table's leave policy; a
// generation change re-bootstraps. Predictions live beside the rows and are
// replaced by the server's images as they arrive.

import type { Row, SchemaJson, Store, StoreTx } from "./store";

export interface StreamEvent {
  seq?: number;
  table: string;
  kind: "put" | "del" | "scope";
  id: string;
  data?: unknown;
}

export interface StreamPage {
  snapshot: boolean;
  events: StreamEvent[];
  watermark: number;
  more: boolean;
  /** A snapshot's next page, when `more`. */
  next?: string | null;
  generation?: number;
}

export interface Transport {
  /** `GET /sync?from=W`; `from: 0` is a snapshot, paged by `after`. */
  sync(from: number, limit: number, after?: string): Promise<StreamPage | { denied: unknown }>;
}

export interface ReplicaState {
  watermark: number;
  generation: number;
}

/** The meta keys the replica keeps. */
const WATERMARK = "watermark";
const GENERATION = "generation";
const PAGE = 4000;

export class Replica {
  private listeners = new Set<(touched: Set<string>) => void>();
  private caughtUp = false;
  private following: Promise<void> | null = null;
  private closed = false;

  constructor(readonly store: Store, public schema: SchemaJson, private transport: Transport, public generation: number) {}

  /** A new generation from the server: the next sync re-derives the partition. */
  adopt(schema: SchemaJson, generation: number) {
    this.schema = schema;
    this.generation = generation;
  }

  /** Whether the device has applied everything the server had at the last sync. */
  get isCaughtUp(): boolean {
    return this.caughtUp;
  }

  onChange(listener: (touched: Set<string>) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  async state(): Promise<ReplicaState> {
    return this.store.read(async (tx) => ({ watermark: Number((await tx.getMeta(WATERMARK)) ?? 0), generation: Number((await tx.getMeta(GENERATION)) ?? 0) }));
  }

  /** One round with the server: a snapshot when the device has none or the
   * generation changed, else the stream from the watermark; every page
   * applies as one transaction. Returns the tables that changed. */
  async syncOnce(): Promise<{ touched: Set<string>; ok: boolean }> {
    const touched = new Set<string>();
    if (this.closed) return { touched, ok: false };
    let { watermark, generation } = await this.state();
    if (generation !== this.generation) {
      // A new generation re-derives the partition: drop rows, keep the outbox.
      await this.store.clearRows();
      await this.store.write(async (tx) => { await tx.setMeta(WATERMARK, 0); await tx.setMeta(GENERATION, this.generation); });
      watermark = 0;
    }
    // A snapshot arrives in pages too: the first clears the rows, each
    // applies in its own transaction, and the watermark the first pinned
    // is written with the last, so a device never holds more than a page
    // in memory and a cold start interrupted midway starts over.
    // The next page is fetched while this one applies, so the server's
    // walk and the device's writes overlap.
    let after: string | undefined;
    let inFlight = this.transport.sync(watermark, PAGE, after);
    for (let pages = 0; pages < 100_000; pages++) {
      const page = await inFlight;
      if (this.closed) return { touched, ok: false };
      if ("denied" in page) return { touched, ok: false };
      if (page.watermark < watermark) return { touched, ok: false }; // never move backwards
      const first = after === undefined;
      const continues = page.snapshot && page.more && page.next ? page.next : undefined;
      if (continues) inFlight = this.transport.sync(watermark, PAGE, continues);
      await this.apply(page, touched, first);
      if (continues) {
        after = continues;
        continue;
      }
      after = undefined;
      watermark = page.watermark;
      if (!page.more) break;
      inFlight = this.transport.sync(watermark, PAGE, undefined);
    }
    this.caughtUp = true;
    if (touched.size > 0) for (const listener of this.listeners) listener(touched);
    return { touched, ok: true };
  }

  /** Apply one page atomically: rows, tombstones and the watermark together.
   * A snapshot's first page clears the rows; its watermark lands with its
   * last page, so an interrupted snapshot is taken again from the start. */
  async apply(page: StreamPage, touched: Set<string>, first = true): Promise<void> {
    if (page.snapshot && first) {
      // A snapshot is the whole partition: clear, then load. Predicted rows
      // are re-applied from the outbox by the client, not kept here.
      await this.store.clearRows();
      for (const table of Object.keys(this.schema.tables)) touched.add(table);
    }
    await this.store.write(async (tx) => {
      if (page.snapshot && first) await tx.setMeta(WATERMARK, 0);
      for (const event of page.events) {
        touched.add(event.table);
        switch (event.kind) {
          case "put": {
            const row = event.data as Row;
            await tx.put(event.table, row);
            break;
          }
          case "del": await tx.delete(event.table, event.id); break;
          case "scope": await this.leave(tx, event.table, event.data as unknown[]); break;
        }
      }
      if (!(page.snapshot && page.more)) await tx.setMeta(WATERMARK, page.watermark);
      await tx.setMeta(GENERATION, this.generation);
    });
  }

  /** A group the device left: drop its rows (`until-revoked`) or keep what
   * was delivered (`delivered-history`). */
  private async leave(tx: StoreTx, table: string, group: unknown[]) {
    const definition = this.schema.tables[table];
    if (!definition || definition.leave === "delivered-history") return;
    const audience = definition.sync?.audience as Record<string, unknown> | undefined;
    const prefix: string[] = audience && typeof audience === "object" && "Target" in audience ? (audience.Target as { prefix: string[] }).prefix : audience && "Column" in audience ? [audience.Column as string] : [];
    // The group's rows are the prefix range of any index leading with the
    // audience columns; the horizon index does when there is one.
    const index = definition.sync?.horizon?.by ?? Object.entries(definition.indexes).find(([, i]) => prefix.every((c, n) => (i.components[n] as { Column?: string })?.Column === c))?.[0];
    if (!index) return;
    for (;;) {
      const rows = await tx.scan(table, index, group, {}, "asc", 500);
      if (rows.length === 0) break;
      for (const { row } of rows) await tx.delete(table, row.id);
      if (rows.length < 500) break;
    }
  }

  /** Follow the stream until closed: sync, wait for the transport's next
   * change signal, sync again. */
  follow(wait: () => Promise<void>): void {
    if (this.following) return;
    this.following = (async () => {
      while (!this.closed) {
        let ok = false;
        try {
          ({ ok } = await this.syncOnce());
        } catch {
          // A store that closed under a sync, or a transport that threw:
          // nothing escapes the loop; the next round decides.
          if (this.closed) break;
        }
        if (this.closed) break;
        if (!ok) await new Promise((r) => setTimeout(r, 1000));
        else await wait();
      }
    })();
  }

  /** Stop following, let a sync in flight finish on the open store, then
   * close it: nothing touches the store after this returns. */
  async close() {
    this.closed = true;
    await this.following?.catch(() => {});
    await this.store.close();
  }
}
