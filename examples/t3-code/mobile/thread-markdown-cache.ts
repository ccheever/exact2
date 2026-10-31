import { mobileMarkdownBlocks, type MarkdownBlock } from './markdown-blocks';
import type { T3Client } from './shared/client';

interface Entry { body: string; blocks: MarkdownBlock[]; textUnits: number; objects: number }
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
    blocks(key: string, body: string): MarkdownBlock[] {
      // Keep the previous lookup intact: a forward scan longer than the cache must not evict its own hits.
      let entry = previous?.get(key);
      if (!entry || entry.body !== body) {
        const blocks = mobileMarkdownBlocks(body, dark);
        entry = { body, blocks, textUnits: key.length + body.length, objects: blocks.length };
        for (const block of blocks) {
          entry.textUnits += block.text.length + block.language.length;
          entry.objects += block.tokens.length;
          for (const token of block.tokens) entry.textUnits += token.text.length;
        }
      }
      if (entries.has(key)) remove(key);
      // Large messages still render in full; they must not flush the reusable smaller entries.
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
