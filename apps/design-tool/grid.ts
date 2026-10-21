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
type Build = { id: string; page?: string; status: any; pin: any; planDigest: string | null; unsupported: { what: string; detail: string | null } | null; holding: { can: boolean; hold: any; held: any } };
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
 * `large` px, and at 6 px larger again where `cap` asks for more; the
 * first `cap`. */
export function modeTiles(axis: { name: string; values: string[] } | undefined, large: number, cap: number): Tile[] {
  const modes: { label: string; pin: Pin }[] = [{ label: 'Light', pin: { scheme: 'light' } }, ...(axis?.values.length ? axis.values.map(v => ({ label: title(v), pin: { scheme: 'dark', mode: { [axis.name]: v } } })) : [{ label: 'Dark', pin: { scheme: 'dark' } }])];
  const big = large >= 4 && large <= 64 ? large : 0, sizes = [0, big, big && big + 6 <= 64 ? big + 6 : 0].filter((s, i) => i === 0 || s > 0);
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

// What the grid set on each build and still holds there: a pin with the
// revision the server gave it (a later revision is another client's, which
// the grid leaves), a hold with its state and instance (the server may move
// its site to the one that corresponds in a new plan); kept until given
// back, a server run's alone. Each hold is asked once a selection (each
// choice of a view counts again) and state; a send that fails asks again.
// The location each copy last said; where each was sent and has not since
// said anywhere; the leader's place; the copies reloaded at a place (their
// build can't navigate) and each build's navigate refusals seen. The
// selection States shows, as it was when chosen.
const owned = new Map<string, { pin?: { json: string; revision: number }; hold?: { state: string; instance: string } }>();
const asked = new Map<string, string>();
const seen = new Map<string, string>(), sentTo = new Map<string, string>(), sentAt = new Map<string, number>(), went = new Map<string, string>(), reloaded = new Map<string, string>(), refusals = new Map<string, number>();
let run = '', place = '', handle = '', chosen = 0, policy = '', passNo = 0, framed = '';
let shown: { sel: object; e: any; facts: Pin; location: string; words: string; chosen: number } | null = null;
// The passes asked: each runs in turn, and one a newer ask has superseded
// sends nothing more (the newer pass reconciles from what is then).
let passes: Promise<unknown> = Promise.resolve(), asks = 0;

/** The grid's source, over the tool's one client and its builds and selection as the poll keeps them. */
export function gridSources(call: Call, builds: () => Map<string, Build & { pinRevision?: number; navRefusals?: number }>, selection: () => Selection, axes: () => { name: string; values: string[] }[]): Pick<Sources, 'loadGrid'> {
  /** One command, bound to the server run the grid's state is of (a restarted server's token is not taken
   * for it); its refusal, `unsupported:` before one the build can't do, `superseded` when a newer pass is
   * asked, or '' with the reply. */
  const send = async (mine: number, body: Record<string, unknown>) => {
    if (mine !== asks) return { why: 'superseded', ok: null as any };
    const { error, reply, used } = await call('/__tamber', body, undefined, handle || undefined);
    if (!error && !handle) handle = used;
    return { why: error ? (reply.error?.unsupported ? `unsupported:${error}` : error) : '', ok: reply.ok };
  };
  /** What the grid set on build `id` and no longer wants, given back where it is still the grid's, read from the builds as they are now. */
  const giveBack = async (mine: number, client: string, id: string, keepPin: boolean, keepHold: boolean) => {
    const duty = owned.get(id), b = builds().get(id);
    // (A build away keeps the duty: the server replays its pin when it comes back, and a later pass gives it back.)
    if (!duty || !b) return;
    if (duty.pin && !keepPin) {
      if (Number(b.pinRevision ?? 0) > duty.pin.revision || (Number(b.pinRevision ?? 0) === duty.pin.revision && JSON.stringify(b.pin ?? null) !== duty.pin.json)) delete duty.pin;
      else if (!(await send(mine, { method: 'pin', client, build: id, pin: null })).why) delete duty.pin;
    }
    if (duty.hold && !keepHold) {
      const h = b.holding.hold;
      if (!h || h.state !== duty.hold.state || JSON.stringify(h.instance ?? []) !== duty.hold.instance) delete duty.hold;
      else if (!(await send(mine, { method: 'hold', client, build: id, state: null })).why) delete duty.hold;
    }
    if (!duty.pin && !duty.hold) owned.delete(id);
  };
  const pass = async (mine: number, [client, view, axisName, large, cap, at, mirror, canWrite, epoch, local]: any[]) => {
    // Another server run (an answer that says one, not a failed hello's none): nothing the grid set survives it.
    if (epoch && epoch !== run) { run = epoch; handle = ''; owned.clear(); asked.clear(); seen.clear(); sentTo.clear(); went.clear(); reloaded.clear(); refusals.clear(); }
    // (Where copies were sent, and what they said since, is the navigation policy's: another view, or Mirror turned off or on, starts again.)
    if (`${view} ${mirror}` !== policy) { policy = `${view} ${mirror}`; sentTo.clear(); went.clear(); seen.clear(); }
    const moving = (view === 'grid' && mirror) || view === 'states';
    passNo += 1;
    const now = builds(), sel = selection();
    let tiles: Tile[] = [], note = '';
    if (view === 'states') {
      // The selection as it was when chosen (a copy's pin moves its build's
      // facts, which makes it stale), until the person selects again.
      const b = sel && !sel.stale ? now.get(String(sel.e.build)) : undefined;
      // (Chosen again is a new selection, not the same one's report moved by a hold.)
      if (b && sel !== shown?.sel) shown = { sel: sel!, e: sel!.e, facts: factsOf(b.status), location: String(b.status?.location ?? ''), words: factsText({ ...b.status, location: undefined }), chosen: ++chosen };
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
    // Another page framed (the variant gallery's, or the app's again): where copies were sent or reloaded was the old page's.
    if (at !== framed) { framed = at; reloaded.clear(); sentTo.clear(); went.clear(); seen.clear(); place = ''; }
    // Each tile's page: the dev loop's, or the place it was reloaded at; a new page waits for its own word.
    const shownNames = new Set(tiles.map(t => t.name));
    for (const n of NAMES) if (!shownNames.has(n)) { srcs.delete(n); copies.delete(n); reloaded.delete(n); }
    for (const t of tiles) {
      const own = tileSrc(src!, t.name, !!local), want = reloaded.has(t.name) ? `${new URL(own).origin}${reloaded.get(t.name)}` : own;
      if (srcs.get(t.name) !== want) { srcs.set(t.name, want); copies.delete(t.name); }
    }
    // A tile's build: the one its frame said under the page it shows, while the server lists it.
    const of = (t: Tile) => { const c = copies.get(t.name); return c && c.src === srcs.get(t.name) ? now.get(c.id) : undefined; };
    // A States copy's hold is wanted only on the selection's own plan.
    const planned = (b: Build) => !!shown?.e.planDigest && b.planDigest === shown.e.planDigest;
    const wanted = new Map<string, Tile>();
    for (const t of tiles) { const b = of(t); if (b) wanted.set(b.id, t); }
    // What the grid set and no longer wants given back: copies not shown, holds off the selection's plan or on the grid.
    if (canWrite) for (const id of [...owned.keys()]) { const t = wanted.get(id), b = now.get(id); await giveBack(mine, client, id, !!t, !!t?.state && !!b && planned(b)); }
    // Where the copies go: the selection's screen in States; with Mirror navigation, a copy that moved
    // (not one sent somewhere, which has gone once it says any new place or five passes on, and isn't sent there
    // again, a router's rewrite or a Back included; nor one saying its first).
    for (const t of tiles) {
      const b = of(t), loc = String(b?.status?.location ?? '');
      // (A variant gallery's copy leads nowhere and is sent nowhere: its page is the gallery's, not a route.)
      if (!b || !loc || b.page === 'gallery') continue;
      const was = seen.get(b.id);
      if (sentTo.has(b.id) && (loc !== was || passNo - (sentAt.get(b.id) ?? 0) > 5)) { went.set(b.id, sentTo.get(b.id)!); sentTo.delete(b.id); }
      else if (view === 'grid' && mirror && was !== undefined && loc !== was && !sentTo.has(b.id)) place = loc;
      seen.set(b.id, loc);
    }
    if (view === 'states' && shown) place = shown.location;
    else if (!moving) place = '';
    const views = [];
    for (const t of tiles) {
      const b = of(t);
      let words = '';
      if (b && canWrite) {
        // Its own mode, as a pin on its own build only.
        if (!same(b.pin, t.pin)) {
          const { why, ok } = await send(mine, { method: 'pin', client, build: b.id, pin: t.pin });
          if (why && why !== 'superseded') words = `Not pinned: ${why.replace(/^unsupported:/, '')}`;
          else if (!why) owned.set(b.id, { ...owned.get(b.id), pin: { json: JSON.stringify(t.pin), revision: Number(ok?.revision ?? 0) } });
        }
        // Its state, on the selection's site in the selection's plan, once the copy shows the selection's screen.
        if (view === 'states' && t.state) {
          const e = shown!.e, want: Hold = { site: Number(e.site), instance: e.instance ?? [], state: t.state };
          const key = JSON.stringify([shown!.chosen, e.site, e.instance ?? [], e.planDigest ?? null, t.state]);
          if (!planned(b)) words = 'This copy runs another plan than the selection (or says none): reload it.';
          else if (!b.holding.can) words = 'This copy\'s build cannot hold states.';
          else if (shown!.location && String(b.status?.location ?? '') !== shown!.location && went.get(b.id) !== shown!.location) words = 'Going to the selection\'s screen.';
          else if (!same(b.holding.hold, want) && asked.get(b.id) !== key) {
            asked.set(b.id, key);
            const { why } = await send(mine, { method: 'hold', client, build: b.id, site: want.site, instance: want.instance, state: want.state, plan: e.planDigest });
            if (why) { asked.delete(b.id); if (why !== 'superseded') words = `Not held: ${why.replace(/^unsupported:/, '')}`; }
            else owned.set(b.id, { ...owned.get(b.id), hold: { state: want.state, instance: JSON.stringify(want.instance) } });
          } else if (!same(b.holding.hold, want) && b.holding.hold?.state !== want.state) words = `Let go${b.holding.held?.reason ? ` (${b.holding.held.reason})` : ''}: select the view again to hold it again.`;
        }
        // The leader's place, sent once; a build that can't navigate (refused at once, or by its page since) is reloaded there.
        const loc = String(b.status?.location ?? ''), refused = Number(b.navRefusals ?? 0);
        if (sentTo.has(b.id) && refused > (refusals.get(b.id) ?? 0)) { if (moving) reloaded.set(t.name, sentTo.get(b.id)!); sentTo.delete(b.id); }
        refusals.set(b.id, refused);
        // (A copy reloaded once, its build unable to navigate, is reloaded again at each new place.)
        if (b.page === 'gallery') { /* never navigated, never reloaded at the app's place */ } else if (place && reloaded.has(t.name) && reloaded.get(t.name) !== place) reloaded.set(t.name, place);
        else if (place && loc && loc !== place && !sentTo.has(b.id) && went.get(b.id) !== place && !reloaded.has(t.name)) {
          sentTo.set(b.id, place);
          sentAt.set(b.id, passNo);
          const { why } = await send(mine, { method: 'navigate', client, build: b.id, location: place });
          if (why.startsWith('unsupported:')) { sentTo.delete(b.id); reloaded.set(t.name, place); }
          else if (why) { sentTo.delete(b.id); if (why !== 'superseded') words ||= `Not moved: ${why}`; }
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
          : words || (reason ? `It said: ${reason}` : ''),
        // (theme2's F2 condition 5: nothing the app asked is hidden by the pin.)
        app: st?.app && b?.pin ? `The app set ${appText(st.app)}; pinned to ${pinWords(b.pin)}.` : '',
      });
    }
    return { ok: true, error: '', note, tiles: views };
  };
  return {
    loadGrid: (args) => {
      const ask = ++asks;
      const answer = passes.then(() => pass(ask, args as unknown as any[]));
      passes = answer.catch(() => {});
      return answer;
    },
  };
}
