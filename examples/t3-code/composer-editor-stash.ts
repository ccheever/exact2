// T3's prompt stash (promptStashStore.ts, ChatComposer stashCurrentPrompt /
// restoreStashEntry, ComposerStashMenu.tsx): ⌘S parks the composer's prompt in
// a device-local list of up to 20 entries and clears the composer; the badge
// and menu restore or delete them. Entries persist in the app's preference
// file beside the drafts. SnapShot images stay with their draft (the stash
// carries prompt text and context links only).
import { arr, obj, str, type Obj } from './domain';

export const MAX_STASH_ENTRIES = 20;
export type StashEntry = { id: string; createdAt: string; prompt: string; environmentId: string };
export type StashRow = { id: string; index: number; snippet: string; age: string; label: string };
export type StashView = { count: number; pulse: number; rows: StashRow[] };

type Holder = { promptStash?: StashEntry[] };
export function stashEntries(local: object): StashEntry[] { return (local as Holder).promptStash ?? []; }
function setEntries(local: object, entries: StashEntry[]) { (local as Holder).promptStash = entries; }

/** Decode the saved list (newest first), dropping malformed rows. */
export function adoptStash(next: object, saved: Obj): void {
  setEntries(next, arr(saved.promptStash).filter(entry => str(entry.id) && typeof entry.prompt === 'string' && str(entry.createdAt))
    .slice(0, MAX_STASH_ENTRIES).map(entry => ({ id: str(entry.id), createdAt: str(entry.createdAt), prompt: str(entry.prompt), environmentId: str(entry.environmentId) })));
}

const CITATION = /\[Assistant quote\]\((t3-citation:\/\/v1\/[^\s)]+)\)/g;
/** stashEntrySnippet: one line, citations as their quote, 90 characters. */
export function stashSnippet(prompt: string): string {
  const plain = prompt.replace(CITATION, (source, href: string) => {
    try { return new URL(href).searchParams.get('text') ?? source; } catch { return source; }
  }).trim().replace(/\s+/g, ' ');
  if (!plain) return '(empty)';
  return plain.length > 90 ? `${plain.slice(0, 90)}…` : plain;
}

/** formatRelativeTimeLabel. */
export function relativeTime(createdAt: string, now: number): string {
  const at = Date.parse(createdAt);
  if (!Number.isFinite(at) || !Number.isFinite(now)) return '';
  const seconds = Math.max(0, Math.floor((now - at) / 1000));
  if (seconds < 60) return 'just now';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

const pulses = new WeakMap<object, number>();
export function stashView(local: object, now: number): StashView {
  const entries = stashEntries(local);
  return { count: entries.length, pulse: pulses.get(local) ?? 0,
    rows: entries.map((entry, index) => ({ id: entry.id, index, snippet: stashSnippet(entry.prompt), age: relativeTime(entry.createdAt, now),
      label: `Restore stashed prompt: ${stashSnippet(entry.prompt)}` })) };
}

export type StashHost = { local: object; draft: string; environmentId: string };

/**
 * Save the prompt (newest first). Returns the action the caller still owes:
 * clear the composer, restore the single entry, or toggle the menu.
 */
export function stashPrompt(host: StashHost, id: string, now: Date): { clear: boolean; restore: StashEntry | null; toggleMenu: boolean; evicted: boolean } {
  const prompt = host.draft.trim();
  const entries = stashEntries(host.local);
  if (!prompt) {
    if (entries.length === 1) return { clear: false, restore: entries[0]!, toggleMenu: false, evicted: false };
    return { clear: false, restore: null, toggleMenu: true, evicted: false };
  }
  const next = [{ id, createdAt: now.toISOString(), prompt: host.draft, environmentId: host.environmentId }, ...entries];
  const evicted = next.length > MAX_STASH_ENTRIES;
  setEntries(host.local, next.slice(0, MAX_STASH_ENTRIES));
  pulses.set(host.local, (pulses.get(host.local) ?? 0) + 1);
  return { clear: true, restore: null, toggleMenu: false, evicted };
}

/** Take an entry out of the stash; the restored prompt follows the current one after a blank line. */
export function takeStashEntry(host: StashHost, id: string): { prompt: string } | null {
  const entries = stashEntries(host.local);
  const entry = entries.find(candidate => candidate.id === id);
  if (!entry) return null;
  setEntries(host.local, entries.filter(candidate => candidate.id !== id));
  const current = host.draft;
  const prompt = !entry.prompt ? current : current.trim() ? `${current.replace(/\s+$/, '')}\n\n${entry.prompt}` : entry.prompt;
  return { prompt };
}

export function deleteStashEntry(local: object, id: string): boolean {
  const entries = stashEntries(local);
  const next = entries.filter(entry => entry.id !== id);
  setEntries(local, next);
  return next.length !== entries.length;
}
export { obj };
