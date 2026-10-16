// Same file in both isolated adapter mounts; neither mount can import its sibling.
import type { ReleaseSnapshot, ServedRelease } from 'snapback4/contract';
export type { ReleaseSnapshot } from 'snapback4/contract';
const object = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v);
/** No timers: Contract refreshes release() even while every operation gate is closed. */
export class ReleaseTracker {
  private generation = -1;
  private credential = '';
  private listeners = new Set<(generation: number) => void>();
  identity(headers: Record<string, string>): string {
    const key = JSON.stringify(Object.entries(headers).sort(([a], [b]) => a.localeCompare(b)));
    if (key !== this.credential) { this.credential = key; this.generation = -1; }
    return key;
  }
  subscribe(listener: (generation: number) => void): () => void {
    this.listeners.add(listener);
    return () => { this.listeners.delete(listener); };
  }
  observe(response: Response, key: string): void {
    const value = response.headers.get('x-snapback-generation');
    if (value !== null) this.adopt(Number(value), key);
  }
  private adopt(generation: number, key: string): boolean {
    if (key !== this.credential || !Number.isSafeInteger(generation) || generation < 0 || generation < this.generation) return false;
    if (generation !== this.generation) {
      const previous = this.generation;
      this.generation = generation;
      if (previous !== -1) for (const listener of this.listeners) listener(generation);
    }
    return true;
  }
  decode(value: unknown, key: string): ReleaseSnapshot {
    if (!object(value)) return {loading: true};
    if (object(value.denied) && typeof value.denied.code === 'string' && typeof value.denied.message === 'string')
      return {denied: value.denied as unknown as Extract<ReleaseSnapshot, {denied: unknown}>['denied']};
    if (!Array.isArray(value.ops) || !Array.isArray(value.tables) || !Number.isSafeInteger(value.generation)) return {loading: true};
    const operations: Record<string, ServedRelease['operations'][string]> = Object.create(null);
    const tables: Record<string, ServedRelease['tables'][string]> = Object.create(null);
    for (const op of value.ops) {
      if (!object(op) || typeof op.name !== 'string' || !['query', 'mutation', 'action'].includes(String(op.kind)) || !object(op.args)) return {loading: true};
      operations[op.name] = {kind: op.kind, args: op.args} as ServedRelease['operations'][string];
    }
    for (const table of value.tables) {
      if (!object(table) || typeof table.name !== 'string' || !object(table.fields)) return {loading: true};
      tables[table.name] = {fields: table.fields} as ServedRelease['tables'][string];
    }
    if (!this.adopt(value.generation as number, key)) return {loading: true};
    return {generation: value.generation as number, operations, tables};
  }
}
