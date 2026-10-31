import { mobileThreadMarkdownBlocks, type ThreadMarkdownBlock } from './thread-markdown';
import type { T3Client } from './shared/client';

interface Entry { body: string; blocks: ThreadMarkdownBlock[]; textUnits: number; objects: number }
interface Snapshot { scope: string; entries: Map<string, Entry> }
const snapshots = new WeakMap<T3Client, Snapshot>();
const maxEntries = 128, maxTextUnits = 512 * 1024, maxObjects = 32_768;

export function mobileThreadMarkdownCache(client: T3Client, dark: boolean, route: string) {
  const scope = JSON.stringify([client.origin, client.environmentId, client.generation, client.projectId,
    client.threadId, client.threadEpoch, route, dark]);
  const snapshot = snapshots.get(client);
  const previous = snapshot?.scope === scope ? snapshot.entries : undefined;
  const entries = new Map<string, Entry>();
  let textUnits = 0, objects = 0;
  const remove = (key: string) => {
    const entry = entries.get(key)!;
    textUnits -= entry.textUnits; objects -= entry.objects; entries.delete(key);
  };
  return {
    blocks(key: string, body: string): ThreadMarkdownBlock[] {
      let entry = previous?.get(key);
      if (!entry || entry.body !== body) {
        const blocks = mobileThreadMarkdownBlocks(body, dark);
        entry = { body, blocks, textUnits: key.length + body.length, objects: 2 + blocks.length };
        for (const block of blocks) {
          entry.textUnits += block.id.length + block.kind.length + block.text.length + block.language.length;
          entry.objects += 3 + block.tokens.length;
          for (const token of block.tokens) entry.textUnits += token.id.length + token.text.length + token.color.length + token.slant.length;
          if (block.kind === 'table') {
            entry.objects += block.rows.length;
            for (const alignment of block.alignments) entry.textUnits += alignment.length;
            for (const row of block.rows) {
              entry.textUnits += row.id.length; entry.objects += 1 + row.cells.length;
              for (const cell of row.cells) {
                entry.textUnits += cell.id.length; entry.objects += 1 + cell.runs.length;
                for (const run of cell.runs) entry.textUnits += run.id.length + run.kind.length + run.text.length;
              }
            }
          }
        }
      }
      if (entries.has(key)) remove(key);
      if (entry.blocks.length && entry.textUnits <= maxTextUnits && entry.objects <= maxObjects) {
        entries.set(key, entry); textUnits += entry.textUnits; objects += entry.objects;
        while (entries.size > maxEntries || textUnits > maxTextUnits || objects > maxObjects) {
          remove(entries.keys().next().value!);
        }
      }
      return entry.blocks;
    },
    commit() { snapshots.set(client, { scope, entries }); },
  };
}
