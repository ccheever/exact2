// Several live copies of the app (LLP 1122 T7, tamber/1.14): a grid of the
// dev loop's page, each copy pinned to one mode and text size; or the
// selected view's states side by side, each copy holding one, in the
// selection's mode. Each copy is a frame this page names `exact-copy-<n>`;
// its page posts `{type: 'exact-build', id}` here on registering, and the
// copy is the build whose word came from that frame (the message's source,
// from that frame's origin) since the frame last loaded its page: no URL
// marker (theme2's 18:11Z/18:14Z rulings). A pin or hold goes only to a
// copy's own build, and the grid gives back only what it set and still
// holds. With Mirror navigation, a copy the person moves leads: the others
// are sent `navigate` to its location, or reloaded there when their build
// can't. Each local copy is at a loopback name of its own,
// `exact-copy-<n>.localhost` (theme2's F1 ruling: a browser carries six
// connections a host, and each copy holds a stream), so each has its own
// storage; through a paired server's gateway the copies share its one origin,
// and at most 4 show. A pinned copy holds its pin over the app's own choice,
// and says when the app asked otherwise (F2).
import type { Sources } from './app.contract.d.ts';

type Call = (path: string, body: unknown, timeout?: number, bound?: string) => Promise<{ error: string; reply: { ok?: any; error?: any }; used: string }>;
type Build = { id: string; status: any; pin: any; planDigest: string | null; unsupported: { what: string; detail: string | null } | null; holding: { can: boolean; hold: any; held: any } };
type Selection = { e: any; stale: string } | null;
type Pin = { scheme?: string; mode?: Record<string, string>; rootFontSize?: number };
type Hold = { site: number; instance: unknown[]; state: string };
type Tile = { name: string; label: string; pin: Pin; state: string };

/** The most copies a grid shows (Q5's measured cap is the default's). */
export const MOST = 8;
const NAMES = Array.from({ length: MOST }, (_, i) => `exact-copy-${i + 1}`);
const ID = /^[\w.:-]{1,64}$/;
const STATES: [string, string][] = [['', 'At rest'], ['active', 'Pressed'], ['hover', 'Hovered'], ['focus-visible', 'Focused']];
const WORDS: Record<string, string> = { active: 'pressed', hover: 'hovered', 'focus-visible': 'focused' };
const same = (a: unknown, b: unknown) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);
const title = (v: string) => v.charAt(0).toUpperCase() + v.slice(1);

// Each frame's page now (the src the grid gave it), and the build its page
// said it is, with the page it said it under: a word is its frame's only
// while that frame still shows that page, and from that page's origin. A
// word from any other window (a popup, a frame inside a copy, the one
// canvas frame) is no copy's.
const srcs = new Map<string, string>();
const copies = new Map<string, { id: string; src: string }>();
type Message = { origin?: string; source?: unknown; currentTarget?: { frames?: Record<string, unknown> } | null; data?: { type?: unknown; id?: unknown } };
const page = globalThis as { addEventListener?: (type: string, listener: (e: Message) => void) => void };
/** The page's `message` listener for copies (exported for its test). */
export function onCopyWord(e: Message) {
  const id = e?.data?.id, frames = e?.currentTarget?.frames;
  if (e?.data?.type !== 'exact-build' || typeof id !== 'string' || !ID.test(id) || !frames || !e.source) return;
  // (Named access gives this page's own child frame of that name, never a frame inside one, nor a popup.)
  const name = NAMES.find(n => { try { return frames[n] === e.source; } catch { return false; } });
  const src = name ? srcs.get(name) : undefined;
  if (!name || !src) return;
  let origin = '';
  try { origin = new URL(src).origin; } catch { return; }
  if (e.origin === origin) copies.set(name, { id, src });
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
/** The most copies through a gateway: its one origin's connections (six) less room for reports. */
export const GATEWAY_MOST = 4;
/** A tile's page: its own loopback name on the dev loop's port; through a gateway, the gateway's page. */
export const tileSrc = (src: URL, name: string, local: boolean) => local ? `${src.protocol}//${name}.localhost:${src.port}${src.pathname}` : src.href;
const appText = (app: any) => [app?.scheme, ...Object.entries(app?.mode ?? {}).map(([a, v]) => `${a}=${v}`), app?.rootFontSize && `root ${app.rootFontSize}`].filter(Boolean).join(', ');
const pinWords = (pin: Pin) => [pin.scheme, ...Object.entries(pin.mode ?? {}).map(([a, v]) => `${a}=${v}`), pin.rootFontSize && `${pin.rootFontSize}px`].filter(Boolean).join(', ');
const heldText = (h: Build['holding']) => !h.hold ? '' : `${h.held?.state === h.hold.state ? 'held' : 'holding'} ${WORDS[h.hold.state] ?? h.hold.state}`;

// What the grid set on each build and still holds there (a pin, a hold),
// kept until it is given back or another client's replaces it; the server
// run they were set under. Each hold asked once a selection and state (a
// build that lets it go is not asked again until either moves). The
// location each copy last said; where each was sent and has not yet said
// it is; the leader's place; the copies reloaded at a place (their build
// can't navigate). The selection States shows, as it was when chosen.
const owned = new Map<string, { pin?: string; hold?: string }>();
const asked = new Map<string, string>();
const seen = new Map<string, string>(), sentTo = new Map<string, string>(), reloaded = new Map<string, string>();
let run = '', place = '';
let shown: { e: any; facts: Pin; location: string; words: string } | null = null;
// One pass at a time: a pass reconciles from what is now, so the next sees
// what the last one set.
let passes: Promise<unknown> = Promise.resolve();

/** The grid's source, over the tool's one client and its builds and selection as the poll keeps them. */
export function gridSources(call: Call, builds: () => Map<string, Build>, selection: () => Selection, axes: () => { name: string; values: string[] }[]): Pick<Sources, 'loadGrid'> {
  /** One command; its refusal, `unsupported:` before one the build can't do, or ''. */
  const send = async (body: Record<string, unknown>) => {
    const { error, reply } = await call('/__tamber', body);
    return error ? (reply.error?.unsupported ? `unsupported:${error}` : error) : '';
  };
  /** What the grid set on build `b` and no longer wants, given back where it is still the grid's. */
  const giveBack = async (client: string, b: Build | undefined, id: string, keepPin: boolean, keepHold: boolean) => {
    const mine = owned.get(id);
    // (A build away keeps the duty: the server replays its pin when it comes back, and the next pass gives it back.)
    if (!mine || !b) return;
    if (mine.pin && !keepPin) {
      if (JSON.stringify(b.pin ?? null) !== mine.pin) delete mine.pin;
      else if (!(await send({ method: 'pin', client, build: id, pin: null }))) delete mine.pin;
    }
    if (mine.hold && !keepHold) {
      if (JSON.stringify(b.holding.hold ?? null) !== mine.hold) delete mine.hold;
      else if (!(await send({ method: 'hold', client, build: id, state: null }))) delete mine.hold;
    }
    if (!mine.pin && !mine.hold) owned.delete(id);
  };
  const pass = async ([client, view, axisName, large, cap, at, mirror, canWrite, epoch, local]: any[]) => {
    // Another server run: nothing the grid set survives it.
    if (epoch !== run) { run = epoch; owned.clear(); asked.clear(); seen.clear(); sentTo.clear(); }
    const now = builds(), sel = selection();
    let tiles: Tile[] = [], note = '';
    if (view === 'states') {
      // The selection as it was when chosen (a copy's pin moves its build's
      // facts, which makes it stale), until the person selects again.
      const b = sel && !sel.stale ? now.get(String(sel.e.build)) : undefined;
      if (b && sel!.e !== shown?.e) shown = { e: sel!.e, facts: factsOf(b.status), location: String(b.status?.location ?? ''), words: factsText({ ...b.status, location: undefined }) };
      if (shown) {
        tiles = stateTiles(shown.facts);
        note = `${shown.e.type ?? 'The view'} #${shown.e.node ?? shown.e.site}, in ${shown.words || 'its build\'s mode'}.`;
      } else note = 'Select a view (Inspect, then click it in a copy) to see its states side by side.';
    } else {
      shown = null;
      if (view === 'grid') tiles = modeTiles(axes().find(a => a.name === axisName) ?? axes()[0], large, local ? cap : Math.min(cap, GATEWAY_MOST));
      if (view === 'grid' && !local && cap > GATEWAY_MOST) note = `Through the gateway at most ${GATEWAY_MOST} copies show: they share its one origin, and a browser carries six connections to one host.`;
    }
    let src: URL | null = null;
    try { src = at ? new URL(at) : null; } catch { src = null; }
    if (!src) tiles = [];
    // Each tile's page: the dev loop's, or the place it was reloaded at; a new page waits for its own word.
    const shownNames = new Set(tiles.map(t => t.name));
    for (const n of NAMES) if (!shownNames.has(n)) { srcs.delete(n); copies.delete(n); reloaded.delete(n); }
    for (const t of tiles) {
      const own = tileSrc(src!, t.name, !!local), want = reloaded.has(t.name) ? `${new URL(own).origin}${reloaded.get(t.name)}` : own;
      if (srcs.get(t.name) !== want) { srcs.set(t.name, want); copies.delete(t.name); }
    }
    // A tile's build: the one its frame said under the page it shows, while the server lists it.
    const of = (t: Tile) => { const c = copies.get(t.name); return c && c.src === srcs.get(t.name) ? now.get(c.id) : undefined; };
    const wanted = new Map<string, Tile>();
    for (const t of tiles) { const b = of(t); if (b) wanted.set(b.id, t); }
    // What the grid set and no longer wants given back: copies not shown, States' holds once on the grid.
    if (canWrite) for (const id of [...owned.keys()]) { const t = wanted.get(id); await giveBack(client, now.get(id), id, !!t, !!t?.state); }
    // Where the copies go: the selection's screen in States; with Mirror navigation, the copy that moved (not where it was sent).
    if (view === 'states' && shown) place = shown.location;
    else if (view === 'grid' && mirror) {
      for (const t of tiles) {
        const b = of(t), loc = String(b?.status?.location ?? '');
        if (!b || !loc) continue;
        if (sentTo.get(b.id) === loc) sentTo.delete(b.id);
        else if (seen.get(b.id) !== loc && !sentTo.has(b.id)) place = loc;
        seen.set(b.id, loc);
      }
    } else place = '';
    const views = [];
    for (const t of tiles) {
      const b = of(t);
      let words = '';
      if (b && canWrite) {
        // Its own mode, as a pin on its own build only.
        if (!same(b.pin, t.pin)) {
          const why = await send({ method: 'pin', client, build: b.id, pin: t.pin });
          if (why) words = `Not pinned: ${why.replace(/^unsupported:/, '')}`; else owned.set(b.id, { ...owned.get(b.id), pin: JSON.stringify(t.pin) });
        }
        // Its state, on the selection's site in the selection's plan.
        if (view === 'states' && t.state) {
          const e = shown!.e, want: Hold = { site: Number(e.site), instance: e.instance ?? [], state: t.state };
          const key = JSON.stringify([e.build, e.site, e.instance ?? [], e.planDigest ?? null, t.state]);
          if (!e.planDigest || b.planDigest !== e.planDigest) words = 'This copy runs another plan than the selection (or says none): reload it.';
          else if (!b.holding.can) words = 'This copy\'s build cannot hold states.';
          else if (!same(b.holding.hold, want) && asked.get(b.id) !== key) {
            asked.set(b.id, key);
            const why = await send({ method: 'hold', client, build: b.id, site: want.site, instance: want.instance, state: want.state, plan: e.planDigest });
            if (why) words = `Not held: ${why.replace(/^unsupported:/, '')}`; else owned.set(b.id, { ...owned.get(b.id), hold: JSON.stringify(want) });
          } else if (!same(b.holding.hold, want)) words = `Let go${b.holding.held?.reason ? ` (${b.holding.held.reason})` : ''}: select the view again to hold it again.`;
        }
        // The leader's place, sent once; a build that can't navigate is reloaded there.
        const loc = String(b.status?.location ?? '');
        if (place && loc && loc !== place && sentTo.get(b.id) !== place) {
          sentTo.set(b.id, place);
          const why = await send({ method: 'navigate', client, build: b.id, location: place });
          if (why.startsWith('unsupported:')) { sentTo.delete(b.id); reloaded.set(t.name, place); }
          else if (why) { sentTo.delete(b.id); words ||= `Not moved: ${why}`; }
        }
      }
      const st = b?.status, back = reloaded.get(t.name);
      const reason = b?.unsupported ? `${b.unsupported.what}${b.unsupported.detail ? `: ${b.unsupported.detail}` : ''}` : '';
      views.push({
        name: t.name, label: t.label, build: b?.id ?? '', ready: !!b,
        src: srcs.get(t.name) ?? '',
        facts: b ? [factsText(st), heldText(b.holding), back ? 'reloaded there: its build can\'t navigate' : ''].filter(Boolean).join(' · ') : '',
        note: !b ? 'Waiting for this copy\'s build to say which it is.'
          : !canWrite ? 'Pins need write scope: this copy shows the app\'s own mode.'
          : words || (st?.app && b.pin ? `The app set ${appText(st.app)}; pinned to ${pinWords(b.pin)}.` : '') || (reason ? `It said: ${reason}` : ''),
      });
    }
    return { ok: true, error: '', note, tiles: views };
  };
  return {
    loadGrid: (args) => {
      const mine = passes.then(() => pass(args as unknown as any[]));
      passes = mine.catch(() => {});
      return mine;
    },
  };
}
