// Several live copies of the app (LLP 1122 T7, tamber/1.14): a grid of the
// dev loop's page, each copy pinned to one mode and text size; or the
// selected view's states side by side, each copy holding one, in the
// selection's mode. Each copy is a frame this page names `exact-copy-<n>`;
// its page posts `{type: 'exact-build', id}` here on registering, and the
// copy is the build whose word came from that frame (the message's source,
// from the copies' origin): no URL marker (theme2's 18:11Z/18:14Z rulings).
// A pin or hold goes only to a copy's own build. With Mirror navigation, a
// copy the person moves leads: the others are sent `navigate` to its
// location, or reloaded there when their build can't. Copies of one origin
// share the app's data.
import type { Sources } from './app.contract.d.ts';

type Call = (path: string, body: unknown, timeout?: number, bound?: string) => Promise<{ error: string; reply: { ok?: any; error?: any }; used: string }>;
type Build = { id: string; status: any; pin: any; planDigest: string | null; unsupported: { what: string; detail: string | null } | null; holding: { can: boolean; hold: any; held: any } };
type Selection = { e: any; stale: string } | null;
type Pin = { scheme?: string; mode?: Record<string, string>; rootFontSize?: number };
type Tile = { name: string; label: string; pin: Pin | null; state: string };

/** The most copies a grid shows (Q5's measured cap is the default's). */
export const MOST = 8;
const NAMES = Array.from({ length: MOST }, (_, i) => `exact-copy-${i + 1}`);
const ID = /^[\w.:-]{1,64}$/;
const STATES: [string, string][] = [['', 'At rest'], ['active', 'Pressed'], ['hover', 'Hovered'], ['focus-visible', 'Focused']];
const WORDS: Record<string, string> = { active: 'pressed', hover: 'hovered', 'focus-visible': 'focused' };
/** A copy's hold in words: held once its build says so. */
const heldText = (h: Build['holding']) => !h.hold ? '' : `${h.held?.state === h.hold.state ? 'held' : 'holding'} ${WORDS[h.hold.state] ?? h.hold.state}`;
const same = (a: unknown, b: unknown) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);
const title = (v: string) => v.charAt(0).toUpperCase() + v.slice(1);

// Each frame's build, as its page said it, and the origin the copies are
// on ("" while there is no grid): a word from any other origin, or from a
// window that is not one of this page's copy frames, is not a copy's.
const copies = new Map<string, string>();
let origin = '';
type Message = { origin?: string; source?: unknown; currentTarget?: { frames?: Record<string, unknown> } | null; data?: { type?: unknown; id?: unknown } };
const page = globalThis as { addEventListener?: (type: string, listener: (e: Message) => void) => void };
/** The page's `message` listener for copies (exported for its test). */
export function onCopyWord(e: Message) {
  const id = e?.data?.id;
  if (e?.data?.type !== 'exact-build' || typeof id !== 'string' || !ID.test(id) || !origin || e.origin !== origin) return;
  // (Named access gives this page's own child frame of that name, never a frame inside one, nor a popup.)
  const frames = e.currentTarget?.frames, name = frames ? NAMES.find(n => { try { return frames[n] === e.source && !!e.source; } catch { return false; } }) : undefined;
  if (name) copies.set(name, id);
}
page.addEventListener?.('message', onCopyWord);

/** The grid's tiles: Light, then the dark scheme at each value of `axis`
 * (Dark alone with no axis), at the build's own text size, then again at
 * `large` px; the first `cap`. */
export function modeTiles(axis: { name: string; values: string[] } | undefined, large: number, cap: number): Tile[] {
  const modes: { label: string; pin: Pin }[] = [{ label: 'Light', pin: { scheme: 'light' } }, ...(axis?.values.length ? axis.values.map(v => ({ label: title(v), pin: { scheme: 'dark', mode: { [axis.name]: v } } })) : [{ label: 'Dark', pin: { scheme: 'dark' } }])];
  const sizes = [0, large >= 4 && large <= 64 ? large : 0].filter((s, i) => i === 0 || s > 0);
  return sizes.flatMap(size => modes.map(m => ({ label: size ? `${m.label} · ${size}px` : m.label, pin: size ? { ...m.pin, rootFontSize: size } : m.pin }))).slice(0, Math.max(1, Math.min(MOST, cap))).map((t, i) => ({ ...t, name: NAMES[i], state: '' }));
}

/** The states side by side: the selected view at rest, pressed, hovered and
 * focused, every copy in the selection's own mode (its build's facts). */
export function stateTiles(facts: Pin): Tile[] {
  return STATES.map(([state, label], i) => ({ name: NAMES[i], label, pin: facts, state }));
}

const factsOf = (st: any): Pin => ({ ...(st?.scheme ? { scheme: String(st.scheme) } : {}), ...(st?.mode && Object.keys(st.mode).length ? { mode: st.mode } : {}), ...(Number.isFinite(st?.rootFontSize) && st.rootFontSize >= 4 && st.rootFontSize <= 64 ? { rootFontSize: st.rootFontSize } : {}) });
const factsText = (st: any) => !st ? '' : [st.scheme, ...Object.entries(st.mode ?? {}).map(([a, v]) => `${a}=${v}`), st.rootFontSize && `${st.rootFontSize}px`, st.location].filter(Boolean).join(' · ');

// What the grid sent each build (a pin, a hold) and whether one is on its
// way; the location each copy last said, the leader's place the others were
// sent to, and the copies reloaded at a location (their build can't navigate).
const pinned = new Map<string, string>(), holding = new Map<string, string>(), sending = new Set<string>();
const seen = new Map<string, string>(), sentTo = new Map<string, string>(), reloaded = new Map<string, string>();
let place = '';
// The selection States shows, as it was when it opened.
let shown: { key: string; e: any; facts: Pin; location: string; words: string } | null = null;

/** The grid's source, over the tool's one client and its builds and selection as the poll keeps them. */
export function gridSources(call: Call, builds: () => Map<string, Build>, selection: () => Selection, axes: () => { name: string; values: string[] }[]): Pick<Sources, 'loadGrid'> {
  /** One command for one build, once at a time; its refusal, or ''. */
  const send = async (id: string, body: Record<string, unknown>) => {
    if (sending.has(id)) return '';
    sending.add(id);
    try { const { error, reply } = await call('/__tamber', body); return error ? (reply.error?.unsupported ? `unsupported:${error}` : error) : ''; } finally { sending.delete(id); }
  };
  /** Every pin and hold the grid made, handed back. */
  const handBack = async (client: string) => {
    for (const id of [...pinned.keys()]) { pinned.delete(id); if (builds().has(id)) await send(id, { method: 'pin', client, build: id, pin: null }); }
    for (const id of [...holding.keys()]) { holding.delete(id); if (builds().has(id)) await send(id, { method: 'hold', client, build: id, state: null }); }
  };
  return {
    loadGrid: async ([client, view, axisName, large, cap, at, mirror, canWrite]) => {
      if (view === 'one' || !at) {
        origin = '';
        seen.clear(); sentTo.clear(); reloaded.clear(); place = ''; shown = null;
        if (canWrite) await handBack(client);
        return { ok: true, error: '', note: '', tiles: [] };
      }
      let src: URL;
      try { src = new URL(at); } catch { return { ok: false, error: 'No page to frame the copies at.', note: '', tiles: [] }; }
      origin = src.origin;
      const now = builds(), sel = selection();
      let tiles: Tile[], target = '', note = '';
      if (view === 'states') {
        // The selection as it was when States opened (a copy's pin moves its
        // build's facts, which makes the selection stale), until another.
        const b = sel && !sel.stale ? now.get(String(sel.e.build)) : undefined;
        if (b) {
          const key = JSON.stringify([b.id, sel!.e.site, sel!.e.instance ?? [], sel!.e.planDigest ?? null]);
          if (shown?.key !== key) shown = { key, e: sel!.e, facts: factsOf(b.status), location: String(b.status?.location ?? ''), words: factsText({ ...b.status, location: undefined }) };
        }
        if (!shown) return { ok: true, error: '', note: 'Select a view (Inspect, then click it in a copy) to see its states side by side.', tiles: [] };
        tiles = stateTiles(shown.facts);
        // (Every copy at the selected copy's screen.)
        target = shown.location;
        note = `${shown.e.type ?? 'The view'} #${shown.e.node ?? shown.e.site}, in ${shown.words || 'its build\'s mode'}.`;
      } else {
        shown = null;
        tiles = modeTiles(axes().find(a => a.name === axisName) ?? axes()[0], large, cap);
      }
      // A tile's build: the one its frame said, while the server lists it.
      const of = (t: Tile) => { const id = copies.get(t.name); return id ? now.get(id) : undefined; };
      const mine = new Set(tiles.map(of).filter(Boolean).map(b => b!.id));
      // Copies the grid no longer shows (a smaller cap, States closed): theirs handed back.
      if (canWrite) for (const id of [...pinned.keys(), ...holding.keys()]) if (!mine.has(id) && now.has(id)) {
        if (pinned.delete(id)) await send(id, { method: 'pin', client, build: id, pin: null });
        if (holding.delete(id)) await send(id, { method: 'hold', client, build: id, state: null });
      }
      // Mirror navigation: a copy whose location moved (not to where the grid sent it) leads.
      if (mirror && view === 'grid') {
        for (const t of tiles) {
          const b = of(t), loc = String(b?.status?.location ?? '');
          if (!b || !loc) continue;
          if (seen.get(b.id) !== loc && sentTo.get(b.id) !== loc) target = loc;
          seen.set(b.id, loc);
        }
        target ||= place;
      }
      if (target) place = target;
      const views = [];
      for (const t of tiles) {
        const b = of(t);
        let words = '';
        if (b && canWrite) {
          // Its own mode, as a pin on its own build only.
          if (!same(b.pin, t.pin)) {
            const why = await send(b.id, { method: 'pin', client, build: b.id, pin: t.pin });
            if (why) words = `Not pinned: ${why.replace(/^unsupported:/, '')}`; else pinned.set(b.id, JSON.stringify(t.pin));
          }
          // Its state, on the selected view's site in the same plan.
          if (view === 'states') {
            const e = shown!.e, want = t.state ? { site: Number(e.site), instance: e.instance ?? [], state: t.state } : null;
            if (t.state && b.planDigest && e.planDigest && b.planDigest !== e.planDigest) words = 'This copy runs another plan than the selection: reload it.';
            else if (t.state && !b.holding.can) words = 'This copy\'s build cannot hold states.';
            else if (!same(b.holding.hold, want)) {
              const why = await send(b.id, { method: 'hold', client, build: b.id, ...(want ?? { state: null }) });
              if (why) words = `Not held: ${why.replace(/^unsupported:/, '')}`; else if (want) holding.set(b.id, JSON.stringify(want)); else holding.delete(b.id);
            }
          }
          // The leader's place, sent once a move; a build that can't navigate is reloaded there.
          const loc = String(b.status?.location ?? '');
          if (place && loc && loc !== place && sentTo.get(b.id) !== place) {
            sentTo.set(b.id, place);
            const why = await send(b.id, { method: 'navigate', client, build: b.id, location: place });
            if (why.startsWith('unsupported:')) reloaded.set(t.name, place);
            else if (why) words ||= `Not moved: ${why}`;
          }
        }
        const st = b?.status;
        const reason = b?.unsupported ? `${b.unsupported.what}${b.unsupported.detail ? `: ${b.unsupported.detail}` : ''}` : '';
        const back = reloaded.get(t.name);
        views.push({
          name: t.name, label: t.label, build: b?.id ?? '', ready: !!b,
          src: back ? `${origin}${back}` : src.href,
          facts: b ? [factsText(st), heldText(b.holding)].filter(Boolean).join(' · ') : '',
          note: !b ? 'Waiting for this copy\'s build to say which it is.'
            : !canWrite ? 'Pins need write scope: this copy shows the app\'s own mode.'
            : words || (reason ? `It said: ${reason}` : back ? 'Reloaded at the leader\'s place: this build cannot navigate.' : ''),
        });
      }
      return { ok: true, error: '', note, tiles: views };
    },
  };
}
