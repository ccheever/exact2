// The composer's suggestions and what a send does to its text, as Discord's clients have them: `@`
// suggests people, `#` the server's channels and `:` emoji; picking one writes its plain name
// (`@ada`, `#general`, `:ferris:`), and the send turns a name the store knows into Discord's mention
// (`<@id>`, `<#id>`, `<:ferris:id>`), outside code.
import * as S from './discord/store';
import { SHORTCODES } from './discord/shortcodes';
import { avatarUrl } from './render';
import type { Result } from './app.contract.d.ts';

export type Suggestion = Result<'suggest'>[number];
const LIMIT = 8;
// The token being written: a trigger and up to 32 name characters at the end of the draft, after
// the start or a space.
const TOKEN = /(^|\s)([@#:])([^\s@#:]{0,32})$/u;

const handle = (u: S.User): string => String(u.username ?? u.id);
const shown = (u: S.User): string => String(u.global_name || u.username || u.id);
function rank(name: string, query: string): number {
  const n = name.toLowerCase();
  return n.startsWith(query) ? 0 : n.includes(query) ? 1 : -1;
}

/** The people a mention in this channel can name, those who wrote in it most recently first. */
function people(store: S.Store, channelId: string): S.User[] {
  const seen = new Map<string, S.User>();
  const channel = S.channel(store, channelId);
  for (const r of (channel?.recipients ?? []) as S.User[]) { const u = S.user(store, r.id) ?? r; seen.set(u.id, u); }
  const rows = S.latestWindow(store, channelId, 200).rows;
  for (let i = rows.length - 1; i >= 0; i--) {
    const id = String(rows[i].author?.id ?? '');
    if (id && !seen.has(id)) { const u = S.user(store, id); if (u) seen.set(id, u); }
  }
  if (channel?.guild_id) for (const u of store.users.values()) if (!seen.has(u.id)) seen.set(u.id, u);
  seen.delete(store.meId ?? '');
  return [...seen.values()];
}

export function suggest(store: S.Store, cdn: string, channelId: string, draft: string): Suggestion[] {
  const m = TOKEN.exec(draft);
  if (!m) return [];
  const [, lead, trigger, raw] = m, query = raw.toLowerCase(), head = draft.slice(0, m.index + lead.length);
  const guildId = S.channel(store, channelId)?.guild_id ?? '';
  const out: (Suggestion & { order: number })[] = [];
  if (trigger === '@') {
    for (const u of people(store, channelId)) {
      const order = Math.min(...[handle(u), shown(u)].map(n => rank(n, query)).filter(r => r >= 0));
      if (!Number.isFinite(order)) continue;
      out.push({ id: `u${u.id}`, label: shown(u), detail: handle(u), avatar: avatarUrl(cdn, u), glyph: '',
        text: `${head}@${handle(u)} `, order });
      if (out.length >= 50) break;
    }
  } else if (trigger === '#') {
    if (!guildId) return [];
    for (const c of S.guildChannelList(store, guildId)) {
      if (![0, 5].includes(c.type ?? 0) || !S.canView(store, c)) continue;
      const order = rank(String(c.name ?? ''), query);
      if (order >= 0) out.push({ id: `c${c.id}`, label: String(c.name), detail: '', avatar: '', glyph: '#',
        text: `${head}#${c.name} `, order });
    }
  } else {
    if (query.length < 2) return [];
    for (const e of guildId ? store.emojis.get(guildId) ?? [] : []) {
      const order = e.available ? rank(e.name, query) : -1;
      if (order >= 0) out.push({ id: `e${e.id}`, label: `:${e.name}:`, detail: '', glyph: '',
        avatar: `${cdn}/emojis/${e.id}.${e.animated ? 'gif' : 'png'}`, text: `${head}:${e.name}: `, order });
    }
    for (const [name, char] of SHORTCODES) {
      const order = rank(name, query);
      if (order >= 0) out.push({ id: `s${name}`, label: `:${name}:`, detail: '', avatar: '', glyph: char, text: `${head}${char} `, order });
    }
  }
  return out.sort((a, b) => a.order - b.order).slice(0, LIMIT).map(({ order: _order, ...s }) => s);
}

/** The text Discord is sent for a draft: names the store knows become mentions, outside code. */
export function resolveMentions(store: S.Store, channelId: string, text: string): string {
  const guildId = S.channel(store, channelId)?.guild_id ?? '';
  // A handle names one person; those who wrote here win a clash with a legacy name.
  const byHandle = new Map<string, string>();
  for (const u of [...store.users.values(), ...people(store, channelId).reverse()]) byHandle.set(handle(u).toLowerCase(), u.id);
  const channels = new Map<string, string>();
  if (guildId) for (const c of S.guildChannelList(store, guildId)) if (c.name) channels.set(String(c.name).toLowerCase(), c.id);
  const emojis = new Map((guildId ? store.emojis.get(guildId) ?? [] : []).filter(e => e.available).map(e => [e.name, e]));
  const plain = (s: string) => s
    .replace(/(^|[\s(])@([a-z0-9_.]{2,32})\b/giu, (whole, lead, name) => {
      const id = byHandle.get(name.toLowerCase());
      return id ? `${lead}<@${id}>` : whole;
    })
    .replace(/(^|[\s(])#([^\s#<>.,!?;:'")\]]{1,100})/gu, (whole, lead, name) => {
      const id = channels.get(name.toLowerCase());
      return id ? `${lead}<#${id}>` : whole;
    })
    .replace(/(^|[^<a-z0-9_]):([a-z0-9_]{2,32}):/giu, (whole, lead, name) => {
      const e = emojis.get(name);
      return e ? `${lead}<${e.animated ? 'a' : ''}:${e.name}:${e.id}>` : whole;
    });
  // Code blocks and spans are sent as written.
  return text.split(/(```[\s\S]*?```|`[^`\n]*`)/).map((part, i) => (i % 2 ? part : plain(part))).join('');
}
