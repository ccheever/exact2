// The design tool's protocol client (LLP 1122 D2): `tamber/1` over HTTP to the
// app's Tamber server, which serves this page from the same local origin
// (`bun exact.mjs tamber tool`). The token comes once from `GET
// /__tamber/token`, which answers only this machine's loopback, and rides
// every POST as `x-exact-tamber-token`, never in a URL. Changes are followed
// by the long poll, `POST /__tamber/events`: a data module's `fetch` reads a
// body whole, so it cannot follow the server-sent stream. A failure is
// words in the answer, never a throw.
import type { Answer, Result, Sources } from './app.contract.d.ts';
import { buildKey, placeOf, schemeOf, selectionView, staleness, visitOf, type Selected } from './inspector';
// The panel's controls and the grid's cells by kind.
import { cellText, colourLiteral, panelOf, sides } from './dials';
import { componentsOf, levelsOf, lookOf } from './components';
import { baseColumns, groupOf, shippedOf, sourceOf, writable } from './base';
import { pageOf } from './origins';
import { pairingSources } from './pairing';
import { gridSources } from './grid'; import { canvasRefusal } from './canvas-check'; import { refusalOf, refusalRows, refuse, restoreRefusals, stepping, stepRefused, takeRefusals } from './refusals';
import { GONE, wire, type Reply } from './wire';
import { heldEvent, holdingOf, holdNote, letGo, type Holding } from './holds';
import { paramInfoOf, rescopeOf, tokenOfIn, type ParamInfo, type Rescope } from './rescope';
import { moveLabel, moveOps, NUMBER, unread } from './moves';
export { cellText, colourLiteral, controlOf, folded, panelOf } from './dials';
import { partsOf } from './parts';
export { joined, missing, partsOf } from './parts';

export const appId = 'com.exact.design-tool';
// The tool's own local URL: `tamber tool` serves it here, and a grant names
// one origin and port.
export const API = 'http://127.0.0.1:7740';
// Another machine's design server, paired with (T6): an https door, or a
// tunnel to this machine's loopback (wire.ts `doorOf` refuses any other
// before a request). And the files the person picks to import, which the
// host copies under app:/tmp/picked/, and nothing else of app:/tmp.
export const grants = `net.fetch ${API}\nnet.fetch https:\nnet.fetch http:\nfs.read app:/tmp/picked`;

// A write's wait: longer than the server's own for a write (two minutes),
// so the tool never gives up on one that then lands.
const WRITE_WAIT = 130_000;
// A design read's requests' (its describe, resolves and hello), queued at
// the one server behind each other: as long as a write's.
const READ_WAIT = WRITE_WAIT;
// The resolves a read has in flight at once (a browser opens six
// connections to one origin).
const RESOLVES_AT_ONCE = 4, LEVEL_WAIT = 5000;
// The one `tamber/1` client (wire.ts): this machine's server at API, or
// another machine's the tool paired with.
const { call, RESTARTED, pairWith, remote, scope: scopeNow } = wire(API, id => (heldRaw.find(p => String(p.proposal) === id)?.client ?? null) as string | null);

const files = (digests: Record<string, string> = {}) => Object.entries(digests).map(([path, digest]) => ({ path, digest }));

type Change = Result<'waitChanges'>['recent'][number];
// The session's changes, newest first, as the long poll brought them.
let recent: Change[] = [];
let answers = 0;
// The place read to, and the run (its token) that answered there.
let place = { seq: 0, epoch: '' }, placeRun = '';

/** An event from the long poll as the view lists it. */
function change(e: any): Change | null {
  const who = typeof e.client === 'string' ? e.client : 'changed on disk';
  switch (e.event) {
    case 'changed': return { seq: e.seq, kind: 'changed', who, files: e.edited ?? [], tokens: (e.changed ?? []).map((c: { token: string }) => c.token) };
    case 'committed': return { seq: e.seq, kind: 'committed', who, files: e.files ?? [], tokens: [] };
    case 'proposal': return { seq: e.seq, kind: 'proposal', who, files: [], tokens: (e.changed ?? []).map((c: { token: string }) => c.token) };
    case 'proposal-dropped': return { seq: e.seq, kind: `proposal ${e.reason ?? 'dropped'}`, who: typeof e.client === 'string' ? e.client : 'the server', files: [], tokens: [] };
    case 'down': return { seq: e.seq, kind: 'down', who: 'the server', files: [], tokens: [] };
    default: return null;
  }
}

type Design = Result<'loadDesign'>;
const noDesign = (error: string): Design => ({ ok: false, error, file: '', answer: ++designs, groups: [], columns: [], firstColumn: '', tokenGroups: [], names: [], axes: [], icons: [], components: [] });
let designs = 0;


import { branchOf, columnsOf, inputsOf, type Axis, type Column } from './branches';
export { branchOf, columnsOf, inputsOf } from './branches';

type When = { axis: string; value: string };
const whenOf = (text: string): When[] => text.split(' ').filter(Boolean).map(w => { const [axis, value] = w.split('='); return { axis, value }; });
// A param's name as its `params` line spells it: its short name where one
// param answers it (else the full one), from the newest design read.
let paramNames = new Map<string, string>();
// The params as the newest settled read of the app's design has them.
let appParams = new Map<string, ParamInfo>(), appParamsKey = '', appKinds = new Map<string, string>();
const lineName = (param: string) => paramNames.get(param) ?? param;
const whenText = (w: When[]) => w.map(c => `${c.axis}=${c.value}`).sort().join(' ');

// Each design read, by its number (given as the read starts): its axes,
// its columns, its dials' groups and each token's stored values per tuple.
// A read that a newer one has overtaken is never shown (the runner drops
// its answer), so it keeps nothing; the shown one is the newest kept, and
// what follows it (a cell's branch, contrast) names it.
type Stored = { css: string; srgb: string; residual: boolean; note: string; value: string };
type Snapshot = { file: string; axes: Axis[]; columns: Column[]; stored: Map<string, Stored[][]>; dials: Map<string, { group: string; more: boolean }>; digests: string; kinds: Map<string, string>; params: Map<string, ParamInfo>; tokenOf: (v: string) => string | null; levelParams: string[] };
const snapshots = new Map<number, Snapshot>();
// Design reads, and reads of the app design's axes (either kind): only the
// newest of the second sets the axes a selection is placed by.
let issued = 0, axesReads = 0;
// Whether running apps take a value edit with no reload: a server says so
// in its hello once it has the live table (LLP 1121 stage 4). Until then
// every edit reaches them through the dev loop's reload.
let liveTable = false;

/** One consistent read of a design: `describe`, each token's `resolve`, and
 * the files' digests again after them; if a write landed between, the
 * replies may be of two revisions, so it reads again. `wanted`: false once
 * a newer read overtakes it, which then asks for nothing more. */
async function readDesign(file: string, wanted: () => boolean): Promise<{ error: string; ir?: any; resolved?: { tuples: any[] }[]; files?: Record<string, string>; pairs?: unknown; base?: any[]; targets?: unknown; params?: string[] }> {
  const which = file ? { file } : {};
  for (let attempt = 0; attempt < 3 && wanted(); attempt++) {
    const described = await call('/__tamber', { method: 'describe', ...which }, READ_WAIT);
    if (described.error) return { error: described.error };
    const ir = described.reply.ok?.ir, pairs = described.reply.ok?.pairs;
    if (!ir || !Array.isArray(ir.tokens) || !Array.isArray(ir.axes) || ir.axes.some((a: Axis) => !Array.isArray(a.values))) return { error: 'The design server answered describe with no design in it.' };
    // A few at a time, so the browser keeps connections for the long poll
    // and the other reads; each waits behind the others at the one design
    // server, so the wait is the whole read's, not one request's.
    // And, in the app's design, each package component's param (an icon's has no level), whose levels `loadLevels` reads after.
    const owned = new Set(Object.keys(described.reply.ok?.files ?? {})), listed = (!file || file === designFile) && [...owned].some(f => f.endsWith('.contract'));
    const params: string[] = listed ? (ir.components ?? []).filter((c: { identity: string }) => { const home = c.identity.split('#')[0]; return !owned.has(home) && home !== designFile; }).flatMap((c: { params?: { name: string; kind: string }[] }) => (c.params ?? []).filter(p => p.kind !== 'icon').map(p => p.name)) : [];
    const asks = ir.tokens.map((t: { name: string }) => t.name);
    const resolved: Awaited<ReturnType<typeof call>>[] = [];
    let next = 0;
    await Promise.all(Array.from({ length: Math.min(RESOLVES_AT_ONCE, asks.length) }, async () => {
      while (next < asks.length && wanted()) {
        const i = next++;
        resolved[i] = await call('/__tamber', { method: 'resolve', token: asks[i], ...which }, READ_WAIT);
      }
    }));
    if (!wanted()) return { error: 'overtaken by a newer read' };
    const failed = resolved.find(r => r.error);
    if (failed) return { error: failed.error };
    const after = await call('/__tamber', { method: 'hello' }, READ_WAIT);
    if (after.error) return { error: after.error };
    const before = described.reply.ok.files ?? {};
    const now = after.reply.ok?.files ?? {};
    const same = Object.keys(before).length === Object.keys(now).length && Object.entries(before).every(([f, d]) => now[f] === d);
    // And each tuple is the one its index says, by its modes.
    const tuples = columnsOf(ir.axes);
    // Each resolve read the revision describe did (an edit and its undo
    // between leave the same digests at the end, not in between).
    const one = resolved.every(r => digestKey(r.reply.ok?.files) === digestKey(before));
    const placed = one && resolved.every(r => (r.reply.ok?.tuples ?? []).every((t: { modes?: Record<string, string> }, i: number) => {
      const col = tuples.find(c => c.tuple === i);
      return !col || ir.axes.every((a: Axis, k: number) => t.modes?.[a.name] === col.values[k]);
    }));
    if (same && placed) {
      return { error: '', ir, resolved: resolved.map(r => r.reply.ok), files: before, pairs, base: Array.isArray(described.reply.ok?.base) ? described.reply.ok.base : [], targets: described.reply.ok?.targets, params };
    }
  }
  return { error: wanted() ? 'The design kept changing while it was read. Try again.' : 'overtaken by a newer read' };
}

/** An edit's operation in `tamber/1`'s shape, on `file` (the app's design
 * when ""). */
/** An edit's operations: its one, and before a param's `when` line where
 * the app has no line for the param, that line (its default as written),
 * which a `when` line needs first (LLP 1121 D15). */
function opsOf(file: string, kind: string, target: string, when: string, value: string): Record<string, unknown>[] {
  if (kind === 'promote-param' || kind === 'promote-token' || kind === 'detach-param') return moveOps(file, kind, JSON.parse(target), value.trim(), appParams, appKinds);
  const op = opOf(file, kind, target, when, value);
  const p = kind === 'set-param' && when ? appParams.get(target) : undefined;
  // A default that reads another param (named in full) is no value a line can
  // take: none, said by the proposal.
  if (p && p.set === null && p.default.includes('#')) return [];
  return p && p.set === null ? [{ op: 'set-param', name: lineName(target), when: [], value: p.default }, op] : [op];
}
// Whether `opsOf` read the params of the design these digests are of.
const paramsOf = (kind: string, when: string, files: Record<string, string>, key = appParamsKey) => !((kind === 'set-param' && when) || kind === 'detach-param') || key === digestKey(files);
function opOf(file: string, kind: string, target: string, when: string, value: string): Record<string, unknown> {
  const on = file ? { file } : {};
  switch (kind) {
    case 'set-dial': return { op: kind, name: target, value, ...on };
    // A param's line is the app design's (LLP 1121 D15), whichever file is
    // open, written by its short name where that names one param.
    case 'set-param': return { op: kind, name: lineName(target), when: whenOf(when), value };
    case 'unset-param': return { op: kind, name: lineName(target), when: whenOf(when) };
    // A style's state part row (T4): its value a number where it is one, else a Contract string.
    case 'set-state': { const [style, state, row, f] = target.split('|'); return { op: kind, style, state, row, value: NUMBER.test(value) ? Number(value) : value, ...(f ? { file: f } : {}) }; }
    // A row as its author wrote it (T5): `set-row` at its place or on its style, the address the join gave.
    case 'set-row': return { op: kind, ...JSON.parse(target), value: NUMBER.test(value) ? Number(value) : value };
    case 'set-token': return { op: kind, name: target, when: whenOf(when), value, ...on };
    case 'add-token': return { op: kind, name: target, value, ...on };
    case 'rename-token': return { op: kind, from: target, to: value, ...on };
    default: return { op: 'remove-token', name: target, ...on };
  }
}
const label = (kind: string, target: string, when: string, value: string) =>
  moveLabel(kind, target, value) ?? (kind === 'set-state' ? `${target.split('|').slice(0, 3).join(' ')} = ${value}` : kind === 'rename-token' ? `${target} → ${value}` : kind === 'remove-token' ? `removed ${target}` : kind === 'unset-param' ? `reset ${target}${when ? ` when ${when}` : ''}` : `${target}${when ? ` when ${when}` : ''} = ${value}`);
// The scrub (LLP 1122 D6), one session at a time, from Scrub live to Keep,
// Cancel or an end: the edit it is of (file, kind, target, branch), the
// digests and server run it opened on, its id once the server answers, the
// op the server last previewed (`shown`, what Keep writes) and the latest
// the draft wants (`want`). One frame is in flight at a time, each the
// latest wanted; an answer for a session that has ended changes nothing
// (and a scrub the server opened for one already cancelled is cancelled).
type Scrubbing = { ask: number; client: string; of: string; token: string; files: Record<string, string>; id: string; shown: Record<string, unknown> | null; want: Record<string, unknown> | null; sending: Promise<void> | null; error: string; errorOp: Record<string, unknown> | null; releasing: boolean };
let scrubbing: Scrubbing | null = null;
// The Scrub live press last seen, and how the last session ended (said
// once it is not live). `closing`: a session ending at the server (its
// opening answered and cancelled); the next one opens after it, so it
// never joins the one being cancelled. `endedIds`: this client's scrubs
// the server has ended, by id, for an opening whose answer comes after its
// end.
let scrubAsked = 0, scrubEnded = '', closing: Promise<void> | null = null;
const endedIds = new Map<string, string>();
const opKey = (op: Record<string, unknown> | null) => JSON.stringify(op);
const endScrub = (s: Scrubbing, why: string) => { if (scrubbing === s) { scrubbing = null; scrubEnded = why; } };
const scrubEndText = (reason: string) => `The scrub ended: ${SCRUB_ENDS[reason] ?? reason}. Nothing was written, and the running copies have the committed value back.`;
// An answer that says the server no longer has this scrub.
const gone = (error: string) => /the scrub ended|is not the scrub open|no such scrub/.test(error);
/** Sends the session's latest wanted op until the server previews it (not
 * while Keep is writing). An opening waits for any closing scrub first. */
function pump(s: Scrubbing): Promise<void> {
  if (s.sending) return s.sending;
  s.sending = (async () => {
    while (scrubbing === s && !s.releasing && s.want && opKey(s.want) !== opKey(s.shown)) {
      if (!s.id && closing) { await closing; continue; }
      const op = s.want;
      const { error, reply } = await call('/__tamber', { method: 'scrub', client: s.client, ops: [op], ...(s.id ? { scrub: s.id } : { files: s.files }) }, WRITE_WAIT, s.token);
      const ok = (reply.ok ?? {}) as { scrub?: string; ended?: string; superseded?: boolean };
      if (scrubbing !== s) {
        // Cancelled while it opened: the scrub the server opened goes too.
        if (!error && !s.id && ok.scrub && !ok.ended) await call('/__tamber', { method: 'scrub-cancel', client: s.client, scrub: ok.scrub }, 15000, s.token);
        return;
      }
      if (error) {
        // Refused before it opened: said, as the server put it. Gone at the
        // server: ended. A value refused that the draft has moved past: the
        // next one is sent.
        s.error = error; s.errorOp = op;
        if (!s.id) { endScrub(s, `No scrub: ${error}`); return; }
        if (gone(error)) { endScrub(s, scrubEndText('the server no longer has it')); return; }
        if (opKey(s.want) !== opKey(op)) continue;
        return;
      }
      if (ok.ended) { endScrub(s, scrubEndText(ok.ended)); return; }
      // Ended at the server before this answer came (its event first).
      if (ok.scrub && endedIds.has(ok.scrub)) { endScrub(s, scrubEndText(endedIds.get(ok.scrub)!)); void call('/__tamber', { method: 'scrub-cancel', client: s.client, scrub: ok.scrub }, 15000, s.token); return; }
      s.error = '';
      s.id = ok.scrub ?? s.id;
      if (!ok.superseded) s.shown = op;
    }
  })().finally(() => { s.sending = null; });
  return s.sending;
}
const scrubState = (draft: Record<string, unknown> | null) => {
  const s = scrubbing;
  if (!s) return { ok: true, error: '', live: false, open: '', ready: false, sending: false, ended: scrubEnded };
  if (!draft) return { ok: false, error: 'Give it a value: an empty one is not previewed.', live: true, open: s.id, ready: false, sending: !!s.sending, ended: '' };
  // A refusal is its frame's: the draft back on a value the server
  // previews is fine again.
  const refused = !!s.error && opKey(s.errorOp) === opKey(draft);
  return { ok: !refused, error: refused ? s.error : '', live: true, open: s.id, ready: !!s.id && !s.sending && opKey(s.shown) === opKey(draft), sending: !!s.sending, ended: '' };
};
/** The session ended here: its scrub at the server cancelled (once its
 * opening has answered), the next opening held until that is done. The
 * cancel's error, when it did not land (the server no longer having it is
 * fine). */
function closeScrub(s: Scrubbing): Promise<string> {
  const done = (async () => {
    if (s.sending) await s.sending;
    if (!s.id) return '';
    const { error } = await call('/__tamber', { method: 'scrub-cancel', client: s.client, scrub: s.id }, 15000, s.token);
    return error && !gone(error) ? error : '';
  })();
  const fence: Promise<void> = done.then(() => { if (closing === fence) closing = null; }, () => { if (closing === fence) closing = null; });
  closing = fence;
  return done;
}
const SCRUB_ENDS: Record<string, string> = { 'competing write': 'another write came first', disconnected: 'this tool stopped being present', idle: 'nothing moved for 30 s', 'server lost': 'the design server stopped' };
/** An edit's identity, as the view spells its draft (`draftKey`). */
const keyOf = (file: string, kind: string, target: string, when: string, value: string) => [file, kind, target, when, value].join('␟');

// The session's undo and redo, for one run of one server: another run (or
// another app's server on the port) has none of these inverses, and a redo
// there would write this app's edit into that one, so a new epoch empties
// them, and `session` counts those.
// Each with the token of the server run that issued its inverse.
// An accepted proposal's entry keeps its proposer: its redo is theirs.
type Entry = { inverse: string; ops: Record<string, unknown>[]; label: string; token: string; client?: string; rescope?: Rescope | null };
const undoStack: Entry[] = [], redoStack: Entry[] = [];
let session = 0;
/** The stacks emptied: another run of the server has none of their
 * inverses. `session` counts these, and an operation that began before one
 * changes neither stack when it ends. Once per run gone (by the token it
 * gave): a restart found by a refused write, then by the poll's new epoch,
 * forgets once, so the refusal's answer stays; unless (`still`) the stacks
 * hold another run's entries meanwhile. */
const forgotRuns = new Set<string>();
function forget(run: string, still = false) {
  if (run && forgotRuns.has(run) && !still) return;
  undoStack.length = 0; redoStack.length = 0; mine.length = 0; session += 1;
  if (run) forgotRuns.add(run);
}
const stacks = (ok: boolean, error: string, last: string, key = '') => ({ ok, error, undo: undoStack.length, redo: redoStack.length, last, key, session, note: '', carried: [] as string[], rescope: undoStack.at(-1)?.rescope?.text ?? '' });
/** Who moved a file a refused write or undo found moved: its client, or
 * "changed on disk". */
const byWhom = (reply: Reply) => {
  const by = (reply.error as { by?: Record<string, string> } | undefined)?.by ?? {};
  const said = Object.entries(by).map(([file, who]) => `${file} was ${who === 'changed on disk' ? 'changed on disk' : `changed by ${who}`}`);
  return said.length ? said.join('; ') : '';
};
const conflict = (reply: Reply) => (reply.error as { conflict?: boolean } | undefined)?.conflict === true;


// The session changeset (LLP 1122 D9): every write since the tool opened,
// its own and others', newest first. Each says its files, who, and the
// token values before and after; under the tool's own (an undo and a redo
// among them), the diff its reply gave. The tool's own write and its event
// are matched by the server's number for the write, which both carry,
// whichever comes first.
//
// Before and after are each file's digest either side of the event (each
// event carries the digests after it; the ones before are the last
// event's, or the poll's when it starts again: the first answer, a gap, a
// new run). A design read is kept by its file's digest, so a change is
// measured, file by file, between two reads of exactly those states,
// whenever each was read; a state no read saw leaves its tokens named only.
type Noted = { seq: number; who: string; acceptedBy: string; write: number; changed: { file: string; token: string }[]; files: string[]; before: Record<string, string>; after: Record<string, string>; mine: boolean; diff: string; values: Map<string, string[]>; committed: boolean; gap?: boolean; redoneBy?: string };
let changeset: Noted[] = [];
// The tool's own writes not yet matched to their events, by write number.
const mine: { write: number; diff: string; redoneBy: string }[] = [];
/** The tool's own write, as its reply says it: matched to its event, now
 * or when the event comes. */
function wroteMine(ok: { write?: number; diff?: string }, redoneBy = '') {
  if (!ok.write) return;
  const n = changeset.find(c => c.write === ok.write);
  if (n) { n.mine = true; n.diff = ok.diff ?? ''; n.redoneBy = redoneBy; } else mine.push({ write: ok.write, diff: ok.diff ?? '', redoneBy });
}
// The dev loop's running builds (LLP 1122 D4), by id, only as the server
// shows them: hello's list when the poll starts again (the first answer, a
// gap, a new run), then the build events. Each with the revision of the
// last pin frame it was sent (`pinRevision`, whoever sent it), and the last
// command it could not apply, kept while that pin is its pin.
type BuildState = { inspectRefused?: string | null; refusalSeq?: number; id: string; host: string; page: string; status: any; plan: string | null; planDigest: string | null; design: string | null; pin: any; pinRevision: number; unsupported: { what: string; detail: string | null } | null; holding: Holding; navRefusals?: number };
let buildsNow = new Map<string, BuildState>();
// The latest selection (the server's join, LLP 1122 D5) with what it was
// made under; a stale one stays stale until a fresh selection (a reload back
// to the same facts is another document). And the builds this tool turned
// inspect mode on, as the server and the pages tell it.
let selectionNow: Selected | null = null;
// Each build's last word on a hold that went, by build id.
const holdNotes = new Map<string, string>();
// The hold each build's page was last asked to report the selection under.
const asked = new Map<string, string>();
const sameHold = (a: unknown, b: unknown) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);
/** A build's hold moved (another, or none): the selection's held report
 * was of the last one, so it is asked for again under this one. */
const forgetHeld = (id: string) => { if (selectionNow?.e.held && String(selectionNow.e.build) === id) selectionNow.e = { ...selectionNow.e, held: undefined }; };
let designAxes: Axis[] = [];
// Each library component's parts, by its declaring identity, as the app design's newest describe lists them (stage
// 12 D3, LLP 1139 D5): the inspector's part outline. Empty until the server says them.
let libraryParts = new Map<string, { name: string; slot: boolean }[]>();
const partsFrom = (ir: { components?: { identity: string; parts?: { name: unknown; slot?: unknown }[] }[] } | undefined) =>
  new Map((ir?.components ?? []).filter(c => Array.isArray(c.parts)).map(c => [c.identity, c.parts!.map(p => ({ name: String(p.name), slot: p.slot === true }))]));
// Each app describe's read while another file is open, numbered: the inspector's outline follows it. And every app
// describe that gives the parts (the app design's read, or its read while another file is open), numbered: only the
// newest sets them, so a late one never puts back what a newer one dropped.
let appReads = 0, partsReads = 0;
const inspectOn = new Set<string>();
const latch = () => { if (selectionNow && !selectionNow.stale) selectionNow.stale = staleness(selectionNow, buildsNow.get(String(selectionNow.e.build)), designAxes); };
// The poll now (a newer one replaces it), and whether the build list must
// be read again from hello (the poll started again, and no read has
// succeeded since).
let polls = 0, resync = true, polled = '', seeded = '';
const pinText = (pin: any) => pin ? [pin.scheme, pin.contrast && `contrast ${pin.contrast}`, ...Object.entries(pin.mode ?? {}).map(([a, v]) => `${a}=${v}`), pin.rootFontSize && `${pin.rootFontSize}px`].filter(Boolean).join(', ') : '';
/** Builds as the panel lists them: each one's effective scheme, contrast,
 * mode and root font size from its status (an app's own setScheme beats a
 * pin, D3), its pin, whether its status has caught up with the last pin
 * frame it was sent (a pin, or a hand-back; none before it reports), and
 * the last command it could not apply, as the server keeps it. */
const buildRows = () => [...buildsNow.values()].map(b => {
  const st = b.status ?? {};
  const mode = Object.entries((st.mode ?? {}) as Record<string, string>).map(([a, v]) => `${a}=${v}`).join(' ');
  return {
    id: b.id, host: b.host, gallery: b.page === 'gallery', reported: b.status != null, scheme: String(st.scheme ?? ''), contrast: String(st.contrast ?? ''), mode, rootFontSize: Number(st.rootFontSize ?? 0),
    pin: pinText(b.pin), caughtUp: Number.isSafeInteger(st.revision) && st.revision >= b.pinRevision,
    behind: Number.isSafeInteger(st.revision) && st.revision < b.pinRevision,
    unsupported: b.unsupported ? `${b.unsupported.what}${b.unsupported.detail ? `: ${b.unsupported.detail}` : ''}` : '',
  };
});
const buildOf = (b: any, was?: BuildState): BuildState => {
  // Its hold's last word as the listing moves it, and none asked of a hold gone.
  const id = String(b.id), holding = holdingOf(b, was?.holding), note = holdNote(holding, was?.holding, holdNotes.get(id) ?? '');
  if (note) holdNotes.set(id, note); else holdNotes.delete(id);
  // Another hold, none, or a new frame of the same one (its report forgotten at the server): asked afresh.
  if (!sameHold(holding.hold, was?.holding.hold) || (holding.hold && was?.holding.held && !holding.held)) { forgetHeld(id); asked.delete(id); }
  return { id, host: String(b.host ?? was?.host ?? ''), page: b.page === 'gallery' ? 'gallery' : '', status: b.status ?? null, plan: b.plan ?? null, planDigest: 'planDigest' in b ? (typeof b.planDigest === 'string' ? b.planDigest : null) : was?.planDigest ?? null, design: b.design ?? null, pin: b.pin ?? null, pinRevision: Number(b.pinRevision ?? 0), unsupported: b.unsupported ?? null, holding, navRefusals: was?.navRefusals, ...refusalOf(b, was) };
};
/** The server's list, whole: a build it no longer lists is gone. */
const takeBuilds = (list: any[], client = '') => {
  buildsNow = new Map((list ?? []).map(b => [String(b.id), buildOf(b, buildsNow.get(String(b.id)))]));
  for (const b of list ?? []) owned(b, client);
};
// This tool's inspect mode on a build, as its record says (tamber/1.6).
const owned = (b: any, client: string) => { if (client && b && 'inspector' in b) { if (b.inspector === client) inspectOn.add(String(b.id)); else { inspectOn.delete(String(b.id)); const s = buildsNow.get(String(b.id)); if (s?.inspectRefused != null) refuse(s, null); } } };
// The app's design file, as hello names it: what "" reads.
let designFile = '';
let lastFiles: Record<string, string> = {};
// Each file change the poll has seen, and each fresh start of it: what a
// design read follows (with the digests), not every event's `seq`.
let fileChanges = 0;
// Commits told (with fileChanges, what moves while a scrub is open).
let commitsTold = 0;
// The scrubs open on the server, any client's, as its events tell them:
// while one is, a build's status or a frame re-reads nothing.
const scrubsOpen = new Set<string>();
// Each scrub opened or ended (two in one answer still move it); each client paired or gone (the list read again).
let scrubMoves = 0, clientMoves = 0;
// The scale columns the app ships, as the last design read named them (stage 11).
let shippedNow: string[] = [];
const digestKey = (files: Record<string, string> = {}) => JSON.stringify(Object.keys(files).sort().map(f => [f, files[f]]));
// Each read's stored values, axes and columns, by `<file>|<its digest>`; a few.
const reads = new Map<string, { stored: Map<string, Stored[][]>; axes: Axis[]; columns: Column[] }>();
const named = (file: string) => file || designFile;
/** An entry's tokens in one file. */
const tokensIn = (n: Noted, file: string) => [...new Set(n.changed.filter(c => c.file === file).map(c => c.token))];

/** A client's colour, the same each time (LLP 1122 D10: each client's
 * edits in its own colour). */
const TINTS = ['-exact-system-blue', '-exact-system-green', '-exact-system-orange', '-exact-system-purple', '-exact-system-pink', '-exact-system-teal', '-exact-system-indigo', '-exact-system-brown'];
// Given in the order clients first appear, so the first eight differ.
const tinted = new Map<string, string>();
export const tintOf = (who: string) => {
  if (!tinted.has(who)) tinted.set(who, TINTS[tinted.size % TINTS.length]);
  return tinted.get(who)!;
};

// Held proposals, as the last poll answer listed them (LLP 1122 D10).
type Held = Result<'waitChanges'>['held'][number];
let held: Held[] = [];
// Each held proposal's operations (for redoing one accepted, and editing an
// import's before accepting it) and its review.
let heldOps = new Map<string, any[]>();
let heldReview = new Map<string, any>();
// The token of the server run whose poll listed them.
let heldToken = '';
// The list's revision in that run (tamber/1.12): a poll that names it is
// answered without the list while it is unchanged.
let heldAt = { epoch: '', revision: -1 };
// The list as last sent, and the design read its swatches were drawn from.
let heldRaw: any[] = [], heldDesign = -1;
/** An import's review (LLP 1122 D11) as the view lists it: the mapped
 * tokens with their values per mode, the role overrides it offers, the
 * values it could not map and why, and the count check. */
function reviewOf(p: any) {
  const r = p.review, none = { importer: '', mapped: [], offers: [], unmapped: [], counted: '', countOk: true, from: '', kept: false };
  if (!r?.importer) return none;
  const ops: any[] = (p.ops ?? []).map((o: { op: unknown }) => o.op);
  // A swatch for a colour a page can paint: a reference to a token the
  // import adds, or the design has, is followed to its colour; one to
  // anything else is said as written, unpainted.
  // Each token the import adds or sets, by mode: its default and each
  // `when`, so a reference is followed in the same mode.
  const proposed = new Map<string, Map<string, string>>();
  for (const o of ops) {
    if (o?.op !== 'add-token' && o?.op !== 'set-token') continue;
    const m = proposed.get(String(o.name)) ?? new Map<string, string>();
    if (o.op === 'add-token') { m.set('default', String(o.value ?? '')); for (const w of o.whens ?? []) m.set(whenText(w.when ?? []), String(w.value ?? '')); }
    else m.set(o.when?.length ? whenText(o.when) : 'default', String(o.value ?? ''));
    proposed.set(String(o.name), m);
  }
  const design = snapshots.get(Math.max(0, ...snapshots.keys()));
  const swatch = (v: string, side: 0 | 1, label: string, depth = 0): string => {
    const t = sides(v)[side];
    if (colourLiteral(t, new Set())) return t;
    if (depth > 8) return '';
    const m = proposed.get(t);
    if (m) return swatch(m.get(label) ?? m.get('default') ?? '', side, label, depth + 1);
    // A design token: its first tuple, a residual at its fallback.
    // (A colour's: another kind's cell is its text, no colour to paint.)
    const cell = design?.stored.get(t)?.[0]?.[side];
    return cell && (cell.residual || cell.srgb) ? (cell.residual ? cell.srgb : cell.css) : '';
  };
  const mode = (label: string, mode: number, value: string) => ({ label, mode, value, light: swatch(value, 0, label), dark: swatch(value, 1, label) });
  // What the import does before its role overrides, each by its kind: a
  // token added or set (each mode's value, editable), a mode's value
  // dropped, an axis added.
  const mapped = ops.slice(0, Number(r.tokens ?? 0)).map((op, index) => {
    const name = String(op.name ?? op.axis ?? '');
    switch (op.op) {
      case 'add-token':
        return { index, kind: 'token', name, text: '', modes: [mode('default', 0, String(op.value ?? '')), ...(op.whens ?? []).map((w: any, k: number) => mode(whenText(w.when ?? []), k + 1, String(w.value ?? '')))] };
      case 'set-token':
        return { index, kind: 'token', name, text: '', modes: [mode(op.when?.length ? whenText(op.when) : 'default', 0, String(op.value ?? ''))] };
      case 'add-axis':
        return { index, kind: 'axis', name, text: `a new axis: ${[].concat(op.values ?? []).join(' | ')}`, modes: [] };
      case 'unset-token-when':
        return { index, kind: 'drop', name, text: `its value when ${whenText(op.when ?? [])} is dropped`, modes: [] };
      default:
        return { index, kind: 'other', name, text: opText(op), modes: [] };
    }
  });
  const c = r.counted ?? {}, recomputed = !!p.replaces || !!r.kept;
  return {
    importer: String(r.importer), mapped, from: String(r.from ?? ''), kept: recomputed,
    // Off until ticked; in a recomputed (or edited) import, the ones it
    // kept were ticked already, so they are on. One with no items is one
    // the importer could not make, its note saying why; in a recomputed
    // import, one with neither was not chosen, and is left out.
    offers: (r.offers ?? []).filter((o: any) => (o.items ?? []).length > 0 || o.note).map((o: any) => ({ role: String(o.role), token: String(o.token ?? ''), items: (o.items ?? []).join(','), note: String(o.note ?? ''), on: recomputed && (o.items ?? []).length > 0 })),
    // Each with where it came from (a file, a CSS block), so two of one
    // name in two modes are told apart.
    unmapped: (r.unmapped ?? []).map((u: any, i: number) => ({ name: String(u.name), value: String(u.value ?? ''), why: String(u.why ?? '') + (u.block === '(derived)' ? ' (derived: outside the count)' : ''), derived: u.block === '(derived)', block: String(u.block ?? ''), key: `${i}` })),
    counted: `${recomputed ? 'the source import: ' : ''}${c.source} source values = ${c.mapped} mapped + ${c.reported} reported`, countOk: c.source === c.mapped + c.reported,
  };
}
/** An import's items its card has chosen: the mapped tokens unless
 * unticked, the role overrides only if ticked (LLP 1121 D1), or, on a
 * recomputed import or an edit of one (`kept`), on unless unticked.
 * `picked`: the view's flipped items, `<proposal>:<i[,j]>` joined by `;`. */
function chosen(proposal: string, picked: string): number[] {
  const r = heldReview.get(proposal), ops = heldOps.get(proposal) ?? [];
  const recomputed = !!held.find(h => h.proposal === proposal)?.replaces || !!r?.kept;
  const flipped = new Set(picked.split(';').filter(x => x.startsWith(`${proposal}:`)).flatMap(x => x.slice(proposal.length + 1).split(',').map(Number)));
  const tokens = Number(r?.tokens ?? 0);
  return ops.map((_, i) => i).filter(i => (i < tokens || recomputed ? !flipped.has(i) : flipped.has(i)));
}
/** An operation in plain words, as a held proposal lists its items. */
const opText = (o: any): string => {
  const what =
    o?.op === 'set-token' ? `${o.name}${o.when?.length ? ` when ${whenText(o.when)}` : ''} = ${o.value}`
    : o?.op === 'unset-token-when' ? `${o.name}: drop when ${whenText(o.when ?? [])}`
    : o?.op === 'set-dial' ? `${o.name} = ${o.value}`
    : o?.op === 'set-param' ? `${o.name}${o.when?.length ? ` when ${whenText(o.when)}` : ''} = ${o.value}`
    : o?.op === 'unset-param' ? `reset ${o.name}${o.when?.length ? ` when ${whenText(o.when)}` : ''}`
    : o?.op === 'add-token' ? `add ${o.name} = ${o.value}`
    : o?.op === 'rename-token' ? `rename ${o.from} → ${o.to}`
    : o?.op === 'remove-token' ? `remove ${o.name}`
    : o?.op === 'add-axis' ? `add axis ${o.name ?? o.axis} = ${[].concat(o.values ?? []).join(' | ')}`
    : o?.op === 'add-axis-value' ? `add ${o.value} to axis ${o.axis ?? o.name}`
    : o?.op === 'remove-axis-value' ? `remove ${o.value} from axis ${o.axis ?? o.name}`
    : o?.op === 'set-state' ? `set ${o.style} :${String(o.state).replace(/^:/, '')} ${o.row} = ${typeof o.value === 'string' ? JSON.stringify(o.value) : o.value}`
    : o?.op === 'set-row' ? `set ${o.style ? `${o.style} ` : ''}${o.state ? `:${o.state} ` : ''}${o.row}${o.style ? '' : ` at ${o.file}:${o.at?.line}`} = ${typeof o.value === 'string' ? JSON.stringify(o.value) : o.value}`
    : o?.op === 'unset-state' ? `unset ${o.style} :${String(o.state).replace(/^:/, '')} ${o.row}`
    : typeof o?.op === 'string' && /axis/.test(o.op) ? `${o.op.replace(/-/g, ' ')} ${o.name ?? o.axis ?? ''}${o.value ? ` ${o.value}` : ''}${o.values ? ` = ${[].concat(o.values).join(' | ')}` : ''}`.trim()
    : JSON.stringify(o);
  // The file it edits (the design's when it names none): two items may
  // differ only there.
  return `${what} (${o?.file || designFile || 'the design'})`;
};

/** An event as the changeset keeps it. */
function noted(e: any) {
  if (e.event === 'committed') {
    commitsTold += 1;
    // What was written before the commit, in its files.
    for (const n of changeset) if (n.seq < e.seq && n.files.some(f => (e.files ?? []).includes(f))) n.committed = true;
    return;
  }
  if (e.event !== 'changed') return;
  fileChanges += 1;
  const who = typeof e.client === 'string' ? e.client : 'changed on disk';
  const before = lastFiles, after: Record<string, string> = e.files ?? {};
  lastFiles = after;
  const at = e.write ? mine.findIndex(m => m.write === e.write) : -1;
  const own = at >= 0 ? mine.splice(at, 1)[0] : null;
  changeset.unshift({
    seq: e.seq, who, acceptedBy: typeof e.acceptedBy === 'string' && e.acceptedBy !== who ? e.acceptedBy : '', write: Number(e.write ?? 0),
    changed: (e.changed ?? []).map((c: { file: string; token: string }) => ({ file: String(c.file), token: String(c.token) })), files: e.edited ?? [],
    before, after, mine: !!own, diff: own?.diff ?? '', redoneBy: own?.redoneBy ?? '', values: new Map(), committed: false,
  });
  // Only what is committed is let go of, past 200.
  while (changeset.length > 200 && changeset.findLastIndex(n => n.committed) >= 0) changeset.splice(changeset.findLastIndex(n => n.committed), 1);
}

/** Each tuple's modes, as a key that names it whatever the axes' order of
 * values: a tuple is compared with the one of the same modes. */
const tupleKeys = (axes: Axis[]) => columnsOf(axes).filter(c => c.dark).map(c => ({ tuple: c.tuple, key: axes.map((a, i) => `${a.name}=${c.values[i]}`).join(' '), label: c.label }));
/** A token's value change between two reads, in plain words: its first
 * tuple, by modes, that moved (named when not the first), or added or
 * removed; a tuple only one side has is said so. */
function moved(token: string, a: { stored: Map<string, Stored[][]>; axes: Axis[] }, b: { stored: Map<string, Stored[][]>; axes: Axis[] }): string | null {
  const before = a.stored.get(token), after = b.stored.get(token);
  if (!before && !after) return null;
  if (!before) return `added ${token} = ${after![0]?.[0]?.value ?? ''}`;
  if (!after) return `removed ${token} (was ${before[0]?.[0]?.value ?? ''})`;
  const was = new Map(tupleKeys(a.axes).map(t => [t.key, before[t.tuple]?.[0]?.value]));
  const keys = tupleKeys(b.axes);
  for (const [i, t] of keys.entries()) {
    const now = after[t.tuple]?.[0]?.value, then = was.get(t.key);
    if (now === then) continue;
    const where = i === 0 ? '' : ` (${t.label.replace(/^Dark · /, '')})`;
    return then === undefined ? `${token} = ${now}${where}, a mode it had not` : `${token} ${then} → ${now}${where}`;
  }
  // A mode the design no longer has.
  const gone = tupleKeys(a.axes).find(t => !keys.some(k => k.key === t.key));
  return gone ? `${token} (${gone.label.replace(/^Dark · /, '')}) removed, was ${before[gone.tuple]?.[0]?.value ?? ''}` : null;
}

/** Each entry's files not yet measured whose states either side have been
 * read: that file's tokens' value changes; a hand edit, which names no
 * tokens, every token of the file that moved. */
function measure() {
  for (const n of changeset) {
    for (const f of n.files) {
      if (n.values.has(f)) continue;
      const a = reads.get(`${f}|${n.before[f]}`), b = reads.get(`${f}|${n.after[f]}`);
      if (!a || !b) continue;
      let tokens = tokensIn(n, f);
      if (!tokens.length) {
        tokens = [...new Set([...a.stored.keys(), ...b.stored.keys()])].filter(t => moved(t, a, b) !== null);
        n.changed.push(...tokens.map(token => ({ file: f, token })));
      }
      n.values.set(f, tokens.map(t => moved(t, a, b)).filter((v): v is string => v !== null));
    }
  }
}
/** One file of an entry in plain words: what moved in it (its tokens
 * named, until measured), who. */
const fileWords = (n: Noted, f: string) => `${f}: ${n.values.get(f)?.join('; ') || tokensIn(n, f).join(', ') || 'edited'} · ${n.who}${n.acceptedBy ? `, accepted by ${n.acceptedBy}` : ''}${n.redoneBy ? `, redone by ${n.redoneBy}` : ''}`;
/** An entry in plain words: each file and what moved in it (its tokens
 * named, until measured), who. */
const plain = (n: Noted) => n.gap ? 'Some changes were missed here (the server keeps the last 256), so this list may be incomplete.' : `${n.files.map(f => `${f}: ${n.values.get(f)?.join('; ') || tokensIn(n, f).join(', ') || 'edited'}`).join('; ') || 'the design: edited'} · ${n.who}${n.acceptedBy ? `, accepted by ${n.acceptedBy}` : ''}${n.redoneBy ? `, redone by ${n.redoneBy}` : ''}`;

/** The re-scope an edit of the app's design would offer once written (D6),
 * read from the design as it is before the write. */
async function rescopeFor(file: string, op: Record<string, unknown>, token: string, files: Record<string, string>): Promise<Rescope | null> {
  // The design read and every explain of the revision the edit was reviewed on.
  const key = digestKey(files);
  const app = (f: string) => f === '' || f === designFile;
  const snap = [...snapshots.values()].reverse().find(x => app(x.file) && x.digests === key);
  if (!app(file) || !snap || (op.op !== 'set-param' && op.op !== 'set-token')) return null;
  return rescopeOf(op as { op: string; name: string; when: When[]; value: string }, snap.params, snap.tokenOf, async name => {
    const { error, reply } = await call('/__tamber', { method: 'explain', token: name }, 15000, token);
    // (A base name's explain, stage 11, has no branches to re-scope: none offered.)
    return error || !Array.isArray(reply.ok?.whens) ? null : (reply.ok as { value: string; whens: { when: When[]; value: string }[]; files?: Record<string, string> });
  }, key);
}

const sources: Sources = {
  ...pairingSources(call, pairWith, remote), sendSelectUp: async ([client, build, up]) => { stepping.add(String(build)); const r = await call('/__tamber', { method: 'inspect', client, build, select: { up } }); if (r.error) stepping.delete(String(build)); return { ok: !r.error, error: r.error }; },
  ...gridSources(call, () => buildsNow, () => selectionNow, () => designAxes), loadCanvasCheck: () => canvasRefusal(API),
  loadHello: async () => {
    const { error, reply } = await call('/__tamber', { method: 'hello' });
    if (error) return { ok: false, error, protocol: '', format: 0, app: '', files: [], page: '', frame: '', live: false, scope: '', local: false, client: '', epoch: '' };
    const ok = reply.ok ?? {};
    // Paired, the server is another machine's whatever its hello says: never this machine's pages.
    const local = remote() === '' && ok.local !== false;
    liveTable = ok.live === true;
    designFile = String(ok.design ?? '');
    // `page`: the dev loop's own origin, which the canvas frames; "" with no
    // dev loop (tamber/1.3).
    return { ok: true, error: '', protocol: String(ok.protocol ?? ''), format: Number(ok.format ?? 0), app: String(ok.app ?? ''), files: files(ok.files), ...(local ? pageOf(ok.page, Number(new URL(API).port)) : { page: '', frame: '' }), live: liveTable,
      // Its scope, and whether it is the local tool's token (tamber/1.13): a server before 1.13 has the local one alone.
      scope: String(ok.scope ?? 'write'), local, client: String(ok.client ?? ''), epoch: String(ok.epoch ?? '') };
  },
  loadDesign: async ([file]) => {
    const number = ++issued, axesAt = !file || file === designFile ? ++axesReads : -1, partsAt = !file || file === designFile ? ++partsReads : -1;
    try {
      const read = await readDesign(file, () => number === issued);
      if (read.error) return noDesign(read.error);
      const ir = read.ir, resolved = read.resolved!;
      const axes: Axis[] = ir.axes;
      const columns = columnsOf(axes);
      // The first read after changes arrived highlights the tokens they
      // changed, in their client's colour; a read again at the same place
      // does not.
      // Each token's stored value per tuple and scheme, a residual as its
      // fallback (no running app reports it until T2).
      const stored = new Map<string, Stored[][]>();
      const baseRows = new Map<string, any>((read.base ?? []).map((r: any) => [String(r.name), r])), targets = shippedOf(read.targets);
      shippedNow = targets;
      const tokens = ir.tokens.map((t: { name: string; kind: string; dials: string[]; role: string | null }, i: number) => {
        const tuples = (resolved[i].tuples ?? []) as any[];
        const sides = tuples.map(r => [false, true].map(dark => {
          const side = dark ? 'dark' : 'light';
          // A token of another kind than colour (LLP 1121 stage 6): its value
          // as `resolve` gives it (`cellText`); it has no sides and draws no
          // swatch.
          if (t.kind !== 'color') return { css: cellText(t.kind, r), srgb: '', residual: false, note: '', value: r.value };
          if (r.residual) return { css: r.value, srgb: r.fallback?.[side] ?? '', residual: true, note: r.value, value: r.value };
          return { css: r[side], srgb: r.srgb?.[side] ?? r[side], residual: false, note: '', value: r.value };
        }));
        stored.set(t.name, sides);
        const cells = columns.map(c => {
          const v = sides[c.tuple]?.[c.dark ? 1 : 0] ?? { css: '', srgb: '', residual: false, note: '', value: '' };
          // A shadow is previewed on a small box (a text shadow on a word).
          const shadow = t.kind === 'box-shadow' || t.kind === 'text-shadow' ? String(v.value ?? '') : '';
          return { scheme: c.dark ? 'dark' : 'light', column: c.id, label: `${t.name}, ${c.label}, ${v.residual ? `${v.note}, fallback ${v.srgb}` : v.css}`, css: v.css, swatch: t.kind !== 'color' ? 'transparent' : v.residual ? v.srgb : v.css, residual: v.residual, note: v.note, kind: t.kind, shadow, pairs: [] as { name: string; ratio: string; low: boolean }[] };
        });
        // (Set for this app starts from the value as written, never the grid's words for it.)
        return { name: t.name, kind: t.kind, follows: (t.dials ?? []).join(', '), cells, start: String(tuples[0]?.value ?? ''), ...sourceOf(t, baseRows.get(t.name), targets, designFile) };
      });
      // Exact's base names the app does not override (stage 11): per platform, no fold, so no cells.
      for (const row of baseRows.values()) if (!row.override && !tokens.some((t: { name: string }) => t.name === row.name)) { const src = sourceOf(null, row, targets, designFile); tokens.push({ name: row.name, kind: 'length', follows: '', cells: [], start: writable(src.columns[0]?.value ?? ''), ...src }); }
      // Each declared pair's contrast per cell (LLP 1121 D3, `pair=`): the
      // token over what it is drawn on, a token's stored value or a role's
      // fallback; below 4.5:1, WCAG's for text, it is marked low.
      // A Map, so a token named `constructor` is no inherited property.
      const declared = new Map(Object.entries((read.pairs ?? {}) as Record<string, { name: string; token: boolean; fallback: { light: string; dark: string } | null }[]>));
      for (const row of tokens) {
        for (const pair of declared.get(row.name) ?? []) {
          const under = stored.get(pair.name), mine = stored.get(row.name)!;
          for (const [k, c] of columns.entries()) {
            const fg = mine[c.tuple]?.[c.dark ? 1 : 0];
            const bg = under ? under[c.tuple]?.[c.dark ? 1 : 0]?.srgb : pair.fallback?.[c.dark ? 'dark' : 'light'];
            const r = fg && bg ? contrast(fg.srgb, bg) : 'not a colour';
            const residual = fg?.residual || (under ? under[c.tuple]?.[c.dark ? 1 : 0]?.residual : true);
            // Two places cut, never rounded up past 4.5, as the server's warning.
            const cut = typeof r === 'string' ? r : Math.floor(r * 100) / 100;
            const badge = typeof cut === 'string'
              ? { name: pair.name, ratio: cut, low: false }
              : { name: pair.name, ratio: `${cut.toFixed(2)}:1${residual ? ' (fallback)' : ''}`, low: cut < 4.5 };
            row.cells[k].pairs.push(badge);
            // Said with the cell, not only drawn under it.
            row.cells[k].label += `, ${badge.ratio} on ${pair.name}${badge.low ? ', low contrast' : ''}`;
          }
        }
      }
      // Rows grouped by their dotted prefix (an undotted name's before its
      // first `_`, as `contrast_0`'s); a role override among the roles.
      const groups = new Map<string, typeof tokens>();
      // A role override, as the IR marks it (`role`), whatever its name.
      const roles = new Set(ir.tokens.filter((t: { role: string | null }) => t.role).map((t: { name: string }) => t.name));
      for (const t of tokens) {
        const g = roles.has(t.name) ? 'roles' : groupOf(t, t.name.split(t.name.includes('.') ? '.' : '_')[0]);
        if (!groups.has(g)) groups.set(g, []);
        groups.get(g)!.push(t);
      }
      const names = new Set<string>(tokens.map((t: { name: string }) => t.name));
      const panel = panelOf(ir.dials ?? [], names);
      const dials = new Map<string, { group: string; more: boolean }>();
      for (const g of panel) for (const [list, more] of [[g.dials, false], [g.more, true]] as const) for (const c of list) dials.set(c.name, { group: g.name, more });
      // Overtaken: never shown, so nothing to keep.
      if (number === issued) {
        // The app design's axes, of the newest read only.
        if (axesAt === axesReads) designAxes = axes;
        if (partsAt === partsReads) libraryParts = partsFrom(ir);
        snapshots.set(number, { file, axes, columns, stored, dials, digests: digestKey(read.files), kinds: new Map([...(ir.dials ?? []), ...ir.tokens, ...(ir.components ?? []).flatMap((c: { params: unknown[] }) => c.params)].map((t: { name: string; kind: string }) => [t.name, t.kind])), params: paramInfoOf(ir), tokenOf: tokenOfIn(ir), levelParams: read.params ?? [] });
        if (!file || file === designFile) { appParams = snapshots.get(number)!.params; appParamsKey = snapshots.get(number)!.digests; appKinds = snapshots.get(number)!.kinds; paramNames = new Map([...appParams].map(([name, p]) => [name, p.short])); }
        for (const old of snapshots.keys()) if (old < number - 4) snapshots.delete(old);
        reads.set(`${named(file)}|${read.files?.[named(file)] ?? ''}`, { stored, axes, columns });
        while (reads.size > 24) reads.delete(reads.keys().next().value!);
      }
      return {
        ok: true, error: '', file: file || '', answer: number, groups: panel, axes: axes.map(a => ({ name: a.name, values: a.values })),
        columns: columns.map(({ id, label }) => ({ id, label })), firstColumn: columns[0]?.id ?? '',
        tokenGroups: [...groups].map(([name, rows]) => ({ name, tokens: rows })),
        names: [...names],
        icons: (ir.icons ?? []).map((i: { name: string; sf: string | null; material: string | null; svg: string | null }) => ({ name: i.name, sf: i.sf ?? '', material: i.material ?? '', svg: i.svg ?? '' })),
        // A param's line is the app design's: the panel only while that is open.
        components: file && file !== designFile ? [] : componentsOf(ir.components, designFile, names, new Set(Object.keys(read.files ?? {})), (v => { const t = tokenOfIn(ir)(v); return t ?? (names.has(v.trim()) ? v.trim() : null); }), new Map(ir.tokens.filter((t: { package?: string | null }) => t.package).map((t: { name: string; package: string }) => [t.name, t.package])), new Set([...baseRows.values()].filter((r: { name: string; override?: unknown }) => !r.override && !ir.tokens.some((t: { name: string }) => t.name === r.name)).map((r: { name: string }) => String(r.name))), lookOf(columns, stored, tokens)),
      };
    } catch (e) {
      if ((e as { code?: string }).code === 'bake') throw e;
      return noDesign(`The design could not be read: ${(e as Error).message ?? e}`);
    }
  },
  loadBranch: async ([file, token, column, answer]) => {
    const none = { ok: true, error: '', token: '', kind: 'color', column: '', place: '', label: '', when: '', value: '', expression: false, inputs: [], dials: [], roles: [], exact: true, own: '', param: false, readOnly: '', answer };
    const snap = snapshots.get(answer);
    const col = snap?.columns.find(c => c.id === column);
    if (!token || !col || !snap || snap.file !== file) return none;
    const { error, reply } = await call('/__tamber', { method: 'explain', token, ...(file ? { file } : {}) }, READ_WAIT);
    if (error) return { ...none, ok: false, error };
    const raw = reply.ok as { value?: string; whens?: { when: When[]; value: string }[]; dials?: string[]; names?: string[]; bases?: string[]; files?: Record<string, string>; kind?: string; package?: string; file?: string; base?: { override?: { text?: string } | null; columns?: Record<string, { value?: string }> } };
    // A base name (stage 11, `kind: 'base'`): its override is the app's line, edited as one; with none, the
    // base's value, read only (no per-tuple fold: each platform's, the first the app ships said).
    // An override with its branches (`whens`, as an app token's explain gives them) is edited as one; a base
    // reply that names the override's text alone shows it read only, since its branches can't be read back.
    const based = raw.kind === 'base', overridden = based && !!raw.base?.override, branched = Array.isArray(raw.whens);
    const first = baseColumns(raw.base, shippedNow)[0]?.value ?? '';
    const x = { value: based && !branched ? String(raw.base?.override?.text ?? first) : String(raw.value ?? ''), whens: branched ? raw.whens! : [], dials: raw.dials ?? [], names: raw.names ?? [], files: raw.files };
    // Of the revision the grid shows, or it is read again with the grid.
    if (x.files && digestKey(x.files) !== snap.digests) return { ...none, ok: false, error: 'The design changed while this cell was read; it is read again with the grid.' };
    const kind = snap.kinds.get(token) ?? (based ? 'length' : 'color');
    const b = branchOf(snap.axes, col, x.whens, kind);
    if (b.tie) return { ...none, ok: false, error: `${token} has two branches for ${col.label}, ${b.tie}; the checker refuses that, so fix the file by hand.` };
    const value = b.index >= 0 ? x.whens[b.index].value : x.value;
    // An expression's inputs: the tokens and dials it names; a dial's
    // control is where it is edited.
    // (A base name a value names, stage 11, is an input too: explain lists those apart, as `bases`.)
    const inputs = inputsOf(value, [...x.names, ...(Array.isArray(raw.bases) ? raw.bases : [])], x.dials);
    // A dial with no control (`control=none`) is said, not offered.
    const dials = inputs.dials.map(name => ({ name, group: snap.dials.get(name)?.group ?? '', more: snap.dials.get(name)?.more ?? false, hidden: !snap.dials.has(name) }));
    return {
      ok: true, error: '', token, kind, column, place: placeOf(snap.axes, col), label: `${token} · ${b.when || 'default'} (${col.label})`, when: b.when, value,
      expression: inputs.tokens.length + inputs.dials.length + inputs.roles.length > 0, inputs: inputs.tokens, dials, roles: inputs.roles, exact: b.own === '' || b.when === b.own, own: b.own, param: snap.params.has(token), answer,
      // (Stage 11: the base's or a package's value is theirs; the app sets its own, from the grid.)
      readOnly: based && !overridden ? 'Exact\'s base, read only: Set for this app (in the grid) to change it here.' : based && !branched ? 'Set for this app: the server names its line without its branches, so edit it in the app\'s design here once it does.' : raw.package && raw.file !== designFile ? `The ${raw.package} package's, read only: Set for this app (in the grid) to change it here.` : '',
    };
  },
  // The branch editor's typed fields for a value (parts.ts), the names in
  // it read by their kinds in the design read (a token's or a dial's).
  loadParts: async ([kind, value, answer]) => {
    const kinds = snapshots.get(answer)?.kinds;
    const found = partsOf(kind, value, name => kinds?.get(name));
    return found ? { ok: true, of: value, ...found } : { ok: false, of: value, parts: [], tail: '' };
  },
  proposeEdit: async ([file, kind, target, when, value, , , live]) => {
    const key = keyOf(file, kind, target, when, value);
    const idle = { ok: true, error: '', active: false, chip: '', diff: '', tokens: [], key, scrubbable: false, review: '' };
    if (!kind || !target) return idle;
    if (kind !== 'remove-token' && kind !== 'unset-param' && kind !== 'detach-param' && !value) return { ...idle, ok: false, error: kind.startsWith('promote') ? 'Give it a name.' : 'Give it a value.' };
    let ops = opsOf(file, kind, target, when, value), paramsAt = appParamsKey; // (the params read these ops were made from)
    if (!ops.length && kind === 'detach-param') return { ...idle, ok: false, error: 'This param is set by the app\'s design, or its default reads another param: unset it, or give it a value of its own, before detaching it.' };
    if (!ops.length) return { ...idle, ok: false, error: `${lineName(target)}'s default reads another param, which its line can't: give it a value of its own first, then Only this mode.` };
    // (A proposal waits as long as a read: the server plans it behind the reads it serializes.)
    let { error, reply, used } = await call('/__tamber', { method: 'propose', ops }, READ_WAIT);
    for (let n = 0, t: any = null, u: { more: string[]; keep: boolean }; kind === 'detach-param' && error && n < 4 && ((u = unread(error, target, ops)).more.length || (u.keep && !t?.keep)); n++)
      ({ error, reply, used } = await call('/__tamber', { method: 'propose', ops: ops = moveOps(file, kind, (t = { ...(t ?? JSON.parse(target)), keep: t?.keep || u.keep, extra: [...(t?.extra ?? []), ...u.more] }), '', appParams, appKinds) })); // (a detach's closure, as the server names it, D6)
    if (error) return { ...idle, ok: false, error };
    const ok = reply.ok as { diff: string; class: string; files: Record<string, string>; changed: { token: string }[] };
    // A param's first `when` line was planned from the params of another revision than this proposal's: asked again once the design is read.
    if (!paramsOf(kind, when, ok.files, paramsAt)) return { ...idle, ok: false, error: kind === 'detach-param' && file && file !== designFile ? 'The app\'s params are from an older read: open the app\'s design, which reads them again, then detach.' : 'The design changed: reading it again before this is proposed.' };
    if (/^(set-state|set-row|promote-|detach-)/.test(kind) && selectionNow?.files !== digestKey(ok.files)) return { ...idle, ok: false, error: 'The files moved since this copy was selected: select it again.' }; // (the selected copy's source edited as reported, T4/T5)
    // What Apply writes: this edit, against the digests this proposal read,
    // to the server run that read them (its token), carried on the proposal
    // itself, so what is shown is what is written, and where.
    return { ok: true, error: '', active: true, chip: ok.class === 'live' && live === true ? 'live' : 'rebuild', diff: ok.diff, tokens: ok.changed.map(c => c.token).filter(t => typeof t === 'string'), key, scrubbable: ops.length === 1, review: JSON.stringify({ key, files: ok.files, diff: ok.diff, token: used, ops }) };
  },
  writeEdit: async ([file, kind, target, when, value, client, review]) => {
    const at = session;
    const key = keyOf(file, kind, target, when, value);
    let read: { key?: string; files?: Record<string, string>; diff?: string; token?: string; ops?: Record<string, unknown>[] } = {};
    try { read = JSON.parse(review || '{}'); } catch { /* none */ }
    if (read.key !== key || !read.files || !read.token || !read.ops?.length) return stacks(false, 'Review the change before applying it.', '');
    // The operations reviewed, as reviewed.
    const ops = read.ops;
    const op = ops.at(-1)!;
    const move = ops.length === 1 ? await rescopeFor(file, op, read.token, read.files) : null;
    // A client paired at `propose` (T6) holds it for someone with write to accept instead.
    if (scopeNow() === 'propose') { const held = await call('/__tamber', { method: 'propose', hold: true, client, ops }, WRITE_WAIT, read.token); return { ...stacks(!held.error, held.error, ''), note: held.error ? '' : `Held for review as ${held.reply.ok?.proposal}: someone with write accepts it.` }; }
    // Every file's digest as the proposal read it: the server checks the
    // ones the edit writes, whichever file the app's design is.
    const { error, reply, used } = await call('/__tamber', { method: 'transaction', client, files: read.files, ops }, WRITE_WAIT, read.token);
    if (error === RESTARTED) forget(used);
    if (session !== at) return stacks(false, RESTARTED, '');
    if (error) return stacks(false, byWhom(reply) ? `Not written: ${byWhom(reply)} since you reviewed it.` : error, '');
    const inverse = (reply.ok as { inverse: string | null }).inverse;
    // A write that changed no byte has no inverse, and leaves redo be.
    // A re-scope is checked against the files as this write left them: one
    // that finds them moved since (another write, a hand edit) is refused.
    const after = (reply.ok as { files?: Record<string, string> }).files;
    if (inverse) { undoStack.push({ inverse, ops, label: label(kind, target, when, value), token: read.token, rescope: move && after ? { ...move, files: after } : null }); redoStack.length = 0; wroteMine(reply.ok); }
    return stacks(true, '', kind.split('-')[0], key);
  },
  rescopeLast: async ([client]) => {
    // The last write moved to the other rung: its inverse and the new write,
    // one transaction, to the server run it was written to; undone as one.
    const at = session;
    const top = undoStack.at(-1);
    if (!top?.rescope) return stacks(false, 'The last write has no other rung to move to.', '');
    const { ops, files } = top.rescope;
    // Every file the move was worked out on, edited or not (a component's
    // default in its Contract source decides what the param shows): `requires`.
    const { error, reply, used } = await call('/__tamber', { method: 'transaction', client, files, requires: files, ops }, WRITE_WAIT, top.token);
    if (error === RESTARTED) forget(used);
    if (session !== at) return stacks(false, RESTARTED, '');
    // Refused by the server (a file moved, a loop through another token), it is not offered again.
    if (error) { if (reply.error) top.rescope = null; return stacks(false, byWhom(reply) ? `Not moved: ${byWhom(reply)} since the edit, so its inverse would undo theirs.` : error, ''); }
    const inverse = (reply.ok as { inverse: string | null }).inverse;
    if (inverse) { undoStack.push({ inverse, ops, label: top.rescope.text.replace(/^Apply to/, 'moved to').replace(/ instead$/, ''), token: top.token }); redoStack.length = 0; wroteMine(reply.ok); }
    return stacks(true, '', 'rescope');
  },
  scrubFrame: async ([ask, , file, kind, target, when, value, client, review]) => {
    // Scrub live pressed: a session for this edit, opened on the digests
    // and server run its proposal read. Then each value the draft takes is
    // wanted, if it is still this edit; the state as it stands is answered.
    // (Not a param's first `when` line, which needs its own line written first.)
    const scrubbable = (kind === 'set-dial' || kind === 'set-token' || kind === 'set-param') && !!value && opsOf(file, kind, target, when, value).length === 1;
    const op = scrubbable ? opOf(file, kind, target, when, value) : null;
    const of = [file, kind, target, when].join('␟');
    if (ask > scrubAsked) {
      scrubAsked = ask;
      let read: { files?: Record<string, string>; token?: string } = {};
      try { read = JSON.parse(review || '{}'); } catch { /* none */ }
      if (scrubbing) return scrubState(op);
      if (!op || !read.files || !read.token) { scrubEnded = 'Review the change before scrubbing it.'; return scrubState(op); }
      scrubEnded = '';
      const s = { ask, client, of, token: read.token, files: read.files, id: '', shown: null, want: op, sending: null, error: '', errorOp: null, releasing: false };
      scrubbing = s;
      // The last one gone at the server first.
      if (closing) await closing;
      if (scrubbing !== s) return scrubState(op);
    }
    const s = scrubbing;
    if (s && s.of !== of) return { ...scrubState(op), ok: false, error: 'This scrub is of another edit: Keep or cancel it first.' };
    if (s && op) { s.want = op; await pump(s); }
    return scrubState(op);
  },
  releaseScrub: async ([draftFile, draftKind, draftTarget, draftWhen, draftValue]) => {
    // The one write: this draft's value (fixed when Keep is pressed or the
    // slider let go) once the server previews it, no further frame sent
    // meanwhile; only for the edit the scrub is of. Checked against the
    // digests the scrub opened with: a file moved since is refused, naming
    // who moved it, and nothing is written. Undone as any write.
    const at = session;
    const s = scrubbing;
    if (!s) return stacks(false, scrubEnded || 'No scrub is open: it ended before Keep.', '');
    if (s.of !== [draftFile, draftKind, draftTarget, draftWhen].join('␟') || !draftValue) return stacks(false, 'This scrub is of another edit: Keep or cancel it there.', '');
    const kept = opOf(draftFile, draftKind, draftTarget, draftWhen, draftValue);
    s.want = kept;
    await pump(s);
    s.releasing = true;
    while (s.sending) await s.sending;
    if (scrubbing !== s || !s.id || !s.shown) { s.releasing = false; return stacks(false, scrubEnded || 'No scrub is open: it ended before Keep.', ''); }
    if (opKey(s.shown) !== opKey(kept)) { s.releasing = false; void pump(s); return stacks(false, s.error || 'That value is not previewed: Keep it once it is, or cancel.', ''); }
    scrubbing = null;
    scrubEnded = '';
    const op = kept, [file, kind, target, when] = s.of.split('␟');
    const value = draftValue;
    // Its re-scope, from the files the scrub opened on (it wrote nothing until now).
    const move = await rescopeFor(file, op, s.token, s.files);
    const { error, reply, used } = await call('/__tamber', { method: 'scrub-release', client: s.client, scrub: s.id }, WRITE_WAIT, s.token);
    // No answer from the server (not a refusal): the scrub may still be
    // open there, so it stays, to keep or cancel again.
    if (error && !reply.error && error !== RESTARTED && error !== GONE) { s.releasing = false; if (!scrubbing) scrubbing = s; return stacks(false, `Keep did not reach the server (${error}): keep or cancel again.`, ''); }
    if (error === RESTARTED) forget(used);
    if (session !== at) return stacks(false, RESTARTED, '');
    if (error) return stacks(false, byWhom(reply) ? `Not written: ${byWhom(reply)} since the scrub opened; the running copies have the committed value back.` : error, '');
    const inverse = (reply.ok as { inverse: string | null }).inverse;
    const after = (reply.ok as { files?: Record<string, string> }).files;
    if (inverse) { undoStack.push({ inverse, ops: [op], label: label(kind, target, when, value), token: s.token, rescope: move && after ? { ...move, files: after } : null }); redoStack.length = 0; wroteMine(reply.ok); }
    return stacks(true, '', kind.split('-')[0], keyOf(file, kind, target, when, value));
  },
  cancelScrub: async ([ask]) => {
    // Ended without a write: the server sends every copy the committed
    // design. One still opening is cancelled once its answer comes; this
    // answers when the server has let it go.
    const note = 'Cancelled: nothing was written, and the running copies have the committed value back.';
    // A Scrub live press not yet taken up is cancelled with it.
    scrubAsked = Math.max(scrubAsked, ask);
    const s = scrubbing;
    scrubbing = null;
    scrubEnded = '';
    const error = s ? await closeScrub(s) : '';
    // It did not land: the scrub stays, to cancel again.
    // (Revoked, the server ended it: said so, and nothing is sent again.)
    if (error === GONE) return stacks(false, GONE, '');
    if (error && s) { if (!scrubbing) scrubbing = s; return stacks(false, `Cancel did not reach the server (${error}): the running copies may still show the preview; cancel again.`, ''); }
    return { ...stacks(true, '', ''), note };
  },
  undoLast: async ([client]) => {
    const at = session;
    const top = undoStack.at(-1);
    if (!top) return stacks(false, 'Nothing to undo.', '');
    const { error, reply, used } = await call('/__tamber', { method: 'undo', client, inverse: top.inverse }, WRITE_WAIT, top.token);
    if (error === RESTARTED) forget(used);
    if (session !== at) return stacks(false, RESTARTED, '');
    if (error) {
      // Refused for a conflict, it can't apply while the file stays as it
      // is: say who moved it, and set it aside. An inverse the server no
      // longer has (used by an undo whose answer was lost, or dropped) is
      // set aside too, said so. Any other failure (no answer) keeps it to
      // try again.
      if (/^no inverse/.test(error)) {
        undoStack.pop();
        return stacks(false, `Can't undo ${top.label}: the server no longer has its inverse (it may be undone already). It is set aside.`, '');
      }
      if (!conflict(reply)) return stacks(false, error, '');
      undoStack.pop();
      return stacks(false, `Can't undo ${top.label}: ${byWhom(reply) || 'the file changed'} since. It is set aside.`, '');
    }
    undoStack.pop();
    redoStack.push(top);
    wroteMine(reply.ok);
    return stacks(true, '', 'undo');
  },
  redoLast: async ([client]) => {
    const at = session;
    const top = redoStack.at(-1);
    if (!top) return stacks(false, 'Nothing to redo.', '');
    // The original transaction again, against the files as they are now,
    // on the run of the server it was undone on (its token).
    const hello = await call('/__tamber', { method: 'hello' }, 15000, top.token);
    if (hello.error === RESTARTED) forget(hello.used);
    if (session !== at) return stacks(false, RESTARTED, '');
    if (hello.error) return stacks(false, hello.error, '');
    const { error, reply, used } = await call('/__tamber', { method: 'transaction', client: top.client ?? client, files: hello.reply.ok?.files ?? {}, ops: top.ops }, WRITE_WAIT, top.token);
    if (error === RESTARTED) forget(used);
    if (session !== at) return stacks(false, RESTARTED, '');
    if (error) {
      if (!conflict(reply)) return stacks(false, `Can't redo ${top.label}: ${error}`, '');
      return stacks(false, `Can't redo ${top.label}: ${byWhom(reply) || 'the file changed'} since. Try again.`, '');
    }
    redoStack.pop();
    const inverse = (reply.ok as { inverse: string | null }).inverse;
    // A redo of an accepted proposal is the proposer's write; its row says
    // who redid it.
    // (A redo offers no re-scope: its move was worked out from the files before the edit, not before the redo.)
    if (inverse) { undoStack.push({ ...top, inverse, rescope: null }); wroteMine(reply.ok, top.client && top.client !== client ? client : ''); }
    return stacks(true, '', 'redo');
  },
  loadLevels: async ([answer]) => { // (components.ts `levelsOf`: after the design shows, so a slow resolve delays only its words)
    const snap = snapshots.get(answer), which = snap?.file ? { file: snap.file } : {}, fits = (ok: any) => !!snap && digestKey(ok?.files) === snap.digests && (ok?.tuples ?? []).every((t: { modes?: Record<string, string> }, i: number) => { const col = snap.columns.find(c => c.tuple === i); return !col || snap.axes.every((a, k) => t.modes?.[a.name] === col.values[k]); });
    return { ok: true, answer, rows: !snap ? [] : await levelsOf(snap.levelParams, name => call('/__tamber', { method: 'resolve', token: name, ...which }, LEVEL_WAIT), () => answer === issued, fits) };
  },
  loadContrast: async ([a, b, answer]) => {
    if (!a || !b) return { ok: true, error: '', ratios: [] };
    const snap = snapshots.get(answer);
    if (!snap) return { ok: true, error: '', ratios: [] };
    const x = snap.stored.get(a), y = snap.stored.get(b);
    if (!x || !y) return { ok: false, error: `${!x ? a : b} is not in this design.`, ratios: [] };
    return { ok: true, error: '', ratios: snap.columns.map(c => {
      const p = x[c.tuple]?.[c.dark ? 1 : 0], q = y[c.tuple]?.[c.dark ? 1 : 0];
      const r = p && q ? contrast(p.srgb, q.srgb) : 'not a colour';
      return { column: c.id, label: c.label, ratio: typeof r === 'string' ? r : `${r.toFixed(2)}:1${p!.residual || q!.residual ? ' (fallback)' : ''}` };
    }) };
  },
  waitChanges: async ([since, epoch, , client]) => {
    // Up to 25 s with nothing new, and the server answers once there is.
    // The first asks, at once, from the start of what the server keeps. It
    // names the tool's client: a client is present while its poll is, and
    // its pins live only while it is (LLP 1122 D4).
    // A poll another has replaced (a new name starts one) changes nothing
    // when it answers: the newer one reads the same events.
    const gen = ++polls;
    // Another name is another client: what this tool inspects is read again.
    if (client !== polled) { polled = client; inspectOn.clear(); resync = true; }
    const { error, reply, used } = await call('/__tamber/events', epoch ? { after: since, epoch, client, ...(heldAt.epoch === epoch ? { held: heldAt.revision } : {}) } : { after: 0, wait: false, client }, 40000);
    answers += 1;
    const state = () => ({ answer: answers, seq: place.seq, epoch: place.epoch, files: `${fileChanges} ${digestKey(lastFiles)}`, paced: scrubsOpen.size ? `scrub ${scrubMoves} ${fileChanges} ${commitsTold}` : `answer ${answers}`, committable: `${session} ${fileChanges} ${commitsTold}`, clients: `${clientMoves}`, recent, undo: undoStack.length, redo: redoStack.length, session, held, builds: buildRows(), ...refusalRows(buildsNow.values()) });
    if (error) return { ok: false, error, missed: false, ...state() };
    if (gen !== polls) return { ok: true, error: '', missed: false, ...state() };
    const r = reply as { events?: unknown[]; seq?: number; epoch?: string; missed?: boolean; held?: number; proposals?: any[] };
    // The open proposals, each with its proposer, diff and tokens; a
    // recomputed one says which it replaces, and is reviewed afresh. None
    // sent: the list is the one this tool has (its revision named), shown
    // again only when a newer read of the design moves its swatches.
    const design = Math.max(0, ...snapshots.keys());
    if (r.proposals || design !== heldDesign) {
      if (r.proposals) {
        heldAt = { epoch: String(r.epoch ?? ''), revision: typeof r.held === 'number' ? r.held : -1 };
        heldRaw = r.proposals;
        heldToken = used;
      }
      heldDesign = design;
      heldOps = new Map(heldRaw.map(p => [String(p.proposal), (p.ops ?? []).map((o: { op: unknown }) => o.op)]));
      heldReview = new Map(heldRaw.map(p => [String(p.proposal), p.review]));
      held = heldRaw.map(p => ({
        proposal: String(p.proposal), client: String(p.client ?? ''), tint: tintOf(String(p.client ?? '')), diff: String(p.diff ?? ''),
        tokens: [...new Set<string>((p.changed ?? []).map((c: { token: string }) => c.token))], replaces: String(p.replaces ?? ''),
        items: (p.ops ?? []).map((o: { index: number; op: unknown }) => ({ index: Number(o.index), text: opText(o.op) })),
        ...reviewOf(p),
      }));
    }
    // Another run of the server (or another app's, on this port), or more
    // missed than it keeps: its sequence numbers start again, or the list
    // has a gap, so the list starts again. Another run has none of this
    // session's inverses: undo and redo start again too.
    // Another run's session has none of this one's writes to commit: the
    // changeset starts again with it.
    // Another run of the server has none of this run's scrub: it ended
    // there, and a new one is chosen again.
    if (place.epoch && r.epoch !== place.epoch) { holdNotes.clear(); asked.clear(); forget(placeRun, [...undoStack, ...redoStack].some(e => e.token !== used)); changeset = []; reads.clear(); buildsNow = new Map(); selectionNow = null; inspectOn.clear(); endedIds.clear(); if (scrubbing) endScrub(scrubbing, scrubEndText('server lost')); }
    // Events missed: an end among them would be lost, so a scrub open is
    // ended here (cancelled, if the server still has it).
    // Missed, inspect mode's state is not known either: Inspect sends it again.
    if (epoch && r.missed) { if (selectionNow) selectionNow.stale = 'some changes were missed'; inspectOn.clear(); seeded = ''; }
    if (epoch && r.missed && r.epoch === place.epoch && scrubbing) {
      const s = scrubbing;
      endScrub(s, 'The scrub was ended: some changes were missed, so its state was not known. Nothing was written.');
      void closeScrub(s);
    }
    if ((place.epoch && r.epoch !== place.epoch) || r.missed) recent = [];
    // The session's changeset: every write since the tool opened (not the
    // backlog the first answer carries).
    // Where the changes start from: the files as the poll has them when it
    // starts again (the first answer, a gap it says it missed, a new run).
    const anew = !epoch || r.missed || (place.epoch && r.epoch !== place.epoch);
    if (anew) { lastFiles = (r as { files?: Record<string, string> }).files ?? {}; fileChanges += 1; clientMoves += 1; if (r.missed) mine.length = 0; scrubsOpen.clear(); }
    // A gap in this run: said in the list, which is no longer complete.
    if (epoch && r.missed && r.epoch === place.epoch) {
      changeset.unshift({ seq: Number(r.seq ?? 0), who: '', acceptedBy: '', write: 0, changed: [], files: [], before: {}, after: {}, mine: false, diff: '', values: new Map(), committed: false, gap: true });
    }
    // (The files as each event came: a selection is of those, not of a write later in the answer. The
    // first answer's start is not known, so a selection before a write in it is of no files known.)
    let filesAt = lastFiles;
    const lastWrite = anew ? ((r.events ?? []) as any[]).findLastIndex(e => e.event === 'changed') : -1;
    if (epoch && !r.missed) for (const e of (r.events ?? []) as any[]) noted(e);
    // The builds, as their events tell them.
    for (const [i, e] of ((r.events ?? []) as any[]).entries()) {
      // (The design server gone: its scrubs with it.)
      if (e.event === 'down') { scrubMoves += 1; scrubsOpen.clear(); }
      if (e.event === 'paired' || e.event === 'revoked') clientMoves += 1;
      if (e.event === 'scrub') { scrubMoves += 1; if (e.reason === 'opened') scrubsOpen.add(String(e.scrub)); else scrubsOpen.delete(String(e.scrub)); if (scrubsOpen.size > 64) scrubsOpen.delete(scrubsOpen.values().next().value!); }
      if (e.event === 'changed' && e.files) filesAt = e.files;
      // Each as it comes, so a reload told in one answer still ends a
      // selection (a build that went keeps its inspect mode at the server,
      // which tells it again when it is back).
      if (e.event === 'builds') { takeBuilds(e.builds, client); latch(); }
      else if (e.event === 'build-status' && e.build?.id) { buildsNow.set(String(e.build.id), buildOf(e.build, buildsNow.get(String(e.build.id)))); owned(e.build, client); latch(); }
      // This tool's scrub, ended by the server (another write, idle, the
      // tool gone, the server lost): said, and a new one is chosen again.
      else if (e.event === 'scrub' && !['opened', 'released', 'cancelled'].includes(e.reason)) {
        if (e.client === client) { endedIds.set(String(e.scrub), String(e.reason)); if (endedIds.size > 32) endedIds.delete(endedIds.keys().next().value!); }
        if (scrubbing?.id && e.scrub === scrubbing.id) endScrub(scrubbing, scrubEndText(String(e.reason)));
      }
      // A hold's end, in words, for the inspector (LLP 1122 D8): a build's own
      // let-go, the server's clearing (no single site after a new plan).
      else if (e.event === 'held' || e.event === 'hold-cleared') {
        const b = buildsNow.get(String(e.build)); if (b) b.holding = heldEvent(b.holding, e);
        if (!e.state) { asked.delete(String(e.build)); forgetHeld(String(e.build)); }
        if (!e.state && e.reason) holdNotes.set(String(e.build), letGo(String(e.reason), e.detail));
      }
      else if (e.event === 'unsupported') {
        const b = buildsNow.get(String(e.build)), step = e.what === 'inspect' && stepRefused(String(e.build)); if (b && !step) b.unsupported = { what: String(e.what ?? ''), detail: e.detail ?? null };
        // (A navigation its page refused, counted: the grid reloads that copy at the place instead, LLP 1122 T7.)
        if (b && e.what === 'navigate') b.navRefusals = (b.navRefusals ?? 0) + 1;
        if (e.what === 'hold') holdNotes.set(String(e.build), `This build cannot hold that state${e.detail ? `: ${e.detail}` : ''}.`); else if (b && e.what === 'inspect' && !step && inspectOn.has(String(e.build))) refuse(b, String(e.detail ?? '')); // (refusals.ts)
        if (e.what === 'inspect' && !step && selectionNow && String(selectionNow.e.build) === String(e.build)) selectionNow.stale ||= `its page could not inspect it${e.detail ? ` (${e.detail})` : ''}`;
      }
      // A selection joined to source, with what it was made under.
      // A selection's held report counts only as the answer to the hold its page was asked under: one
      // from before the hold moved (or unasked) is asked for again.
      else if (e.event === 'selection') {
        const b = buildsNow.get(String(e.build)), held = e.held && asked.get(String(e.build)) === JSON.stringify(b?.holding.hold ?? null); if (b?.inspectRefused != null) refuse(b, null); stepping.delete(String(e.build));
        selectionNow = { e: held || !e.held ? e : { ...e, held: undefined }, at: buildKey(b), axes: JSON.stringify(designAxes), scheme: schemeOf(e, b), stale: '', files: i < lastWrite ? '' : digestKey(filesAt), visit: visitOf(e) };
      }
      // Inspect mode's owners as they move (tamber/1.6): this client's on or
      // off, or another's taking a build over. Inspect mode ending where the
      // selection is (an off taken by no one, `to: null`) ends it, whoever
      // had it; this client's own loss ends it too.
      else if (e.event === 'inspect' && Array.isArray(e.builds)) {
        for (const id of e.builds.map(String)) { if (e.client === client && e.on) inspectOn.add(id); else if (e.client === client || e.on) { inspectOn.delete(id); const b = buildsNow.get(id); if (b?.inspectRefused != null) refuse(b, null); } }
        if (!e.on && (e.client === client || !e.to) && selectionNow && e.builds.map(String).includes(String(selectionNow.e.build))) selectionNow.stale ||= 'inspect mode was turned off there';
      }
      // A page that left inspect mode itself (the server says so only for
      // its current command): its selection is no longer current.
      else if (e.event === 'selected' && e.selected === null) {
        if (selectionNow && String(selectionNow.e.build) === String(e.build)) selectionNow.stale = 'its page left inspect mode';
      }
    }
    // Starting again, the builds are the server's list now (its events
    // before this answer are not sent): read at once, not a poll later, and
    // again each poll until a read succeeds (the list kept meanwhile).
    if (anew) resync = true;
    if (resync) {
      const now = await call('/__tamber', { method: 'hello' });
      if (gen !== polls) return { ok: true, error: '', missed: false, ...state() };
      if (!now.error) { takeBuilds((now.reply.ok as { builds?: any[] })?.builds ?? [], client); latch(); resync = false; }
    }
    const fresh = (r.events ?? []).map(change).filter((c): c is Change => c !== null).reverse();
    recent = [...fresh, ...recent].slice(0, 50);
    place = { seq: Number(r.seq ?? 0), epoch: String(r.epoch ?? '') };
    placeRun = used;
    return { ok: true, error: '', missed: r.missed === true, ...state() };
  },
  loadLit: async ([file, answer, faded]) => {
    // The cells of this file changed since its highlights last faded (in
    // this run of the server), each by the latest client that changed that
    // cell, for a short highlight in its colour: the newest change a read
    // has shown, and each one before it. A change is measured between the
    // nearest reads around it (its own states, else the closest older and
    // newer ones read), cell by cell, matched by modes and scheme, so a
    // cell it did not move is never lit. A hand edit, which names no
    // tokens, is the tokens that moved there that no other change in that
    // stretch claims. They fade oldest first, one each 3 s, so a busy token
    // never keeps another lit.
    const f = named(file);
    let cursor: { epoch?: string; files?: Record<string, number> } = {};
    try { cursor = JSON.parse(faded || '{}'); } catch { /* none */ }
    const since = cursor.epoch === place.epoch ? cursor.files?.[f] ?? 0 : 0;
    measure();
    const snap = snapshots.get(answer);
    const of = changeset.filter(n => n.files.includes(f));
    const shown = of.findIndex(n => n.seq > since && reads.has(`${f}|${n.after[f]}`));
    const lit = shown < 0 ? [] : of.slice(shown).filter(n => n.seq > since);
    // Each read's cells by modes and scheme: `axis=value … |dark`.
    const cellsOf = (r: { stored: Map<string, Stored[][]>; axes: Axis[] }, t: string) => {
      const out = new Map<string, string>();
      for (const c of columnsOf(r.axes)) out.set(`${r.axes.map((x, i) => `${x.name}=${c.values[i]}`).join(' ')}|${c.dark}`, r.stored.get(t)?.[c.tuple]?.[c.dark ? 1 : 0]?.css ?? '');
      return out;
    };
    const around = (k: number) => {
      // The nearest read at or before the change, and at or after it.
      let a, b, lo = k, hi = k;
      for (let j = k; j < of.length && !a; j++) { a = reads.get(`${f}|${of[j].before[f]}`); lo = j; }
      for (let j = k; j >= 0 && !b; j--) { b = reads.get(`${f}|${of[j].after[f]}`); hi = j; }
      return { a, b, lo, hi };
    };
    const cells = new Map<string, string>(), rows = new Map<string, Set<string>>();
    const current = new Map((snap?.columns ?? []).map(c => [`${snap!.axes.map((x, i) => `${x.name}=${c.values[i]}`).join(' ')}|${c.dark}`, c.id]));
    for (const n of [...lit].reverse()) {
      const k = of.indexOf(n), { a, b, lo, hi } = around(k);
      let tokens = tokensIn(n, f);
      if (!tokens.length && a && b) {
        const claimed = new Set(of.slice(hi, lo + 1).filter(m => m !== n).flatMap(m => tokensIn(m, f)));
        tokens = [...new Set([...a.stored.keys(), ...b.stored.keys()])].filter(t => !claimed.has(t) && moved(t, a, b) !== null);
      }
      for (const t of tokens) {
        const before = a ? cellsOf(a, t) : null, after = b ? cellsOf(b, t) : null;
        let any = false;
        for (const [key, id] of current) {
          if (before && after && before.get(key) === after.get(key)) continue;
          cells.set(`${t}|${id}`, n.who);
          any = true;
        }
        if (any) rows.set(t, new Set([...(rows.get(t) ?? []), n.who]));
      }
    }
    const oldest = lit.at(-1)?.seq ?? since;
    const fadeTo = JSON.stringify({ epoch: place.epoch, files: { ...(cursor.epoch === place.epoch ? cursor.files : {}), [f]: oldest } });
    // Each row names every client still lit on it.
    const whos = [...rows.values()].map(w => [...w].join(', '));
    return {
      seq: oldest, epoch: place.epoch, file: f, fadeTo,
      names: [...rows.keys()], whos, tints: [...rows.values()].map(w => tintOf([...w].at(-1)!)),
      cells: [...cells.keys()], cellTints: [...cells.values()].map(tintOf),
    };
  },
  loadScope: async ([client]) => {
    // What a commit would take now (a dry run, of the run the poll is on), so Commit is on when there is something,
    // writes from before the tool opened among them, a refusal to say, or no answer (it says why when pressed).
    // Read again only when the files move, a commit is told or the server starts again; never for no one
    // (`design-tool:`, before the name).
    if (!client || client === 'design-tool:') return { open: 0, refused: false, unavailable: false };
    const dry = await call('/__tamber', { method: 'commit', client, dry: true }, 15000, heldToken || undefined);
    const refused = ((dry.reply.error as { refused?: unknown[] } | undefined)?.refused ?? []).length;
    const nothing = /nothing to commit|written nothing/.test(dry.error);
    return { open: dry.error ? 0 : ((dry.reply.ok as { edits?: unknown[] })?.edits ?? []).length, refused: refused > 0, unavailable: !!dry.error && !refused && !nothing };
  },
  loadChangeset: async () => (measure(), {
    ok: true, error: '',
    entries: changeset.map(n => ({ seq: n.seq, who: n.who, tint: tintOf(n.who), mine: n.mine, text: plain(n), diff: n.diff, committed: n.committed })),
  }),
  commitSession: async ([client]) => {
    // What the commit takes is the server's session, which can hold writes
    // from before the tool opened: a dry run, of the run the poll is on,
    // says it, and the message is composed from that, each edit by its
    // write's number in the words of that write's row (for its file), else
    // its tokens and client. The commit goes to the run the dry run read,
    // with its scope, so a write in between is refused, not committed under
    // a message that leaves it out. The server adds each edit's author and
    // file, and never pushes.
    const at = session;
    const refusedOf = (reply: Reply) => ((reply.error as { refused?: { file: string; why: string }[] } | undefined)?.refused ?? []).map(r => ({ file: String(r.file), why: String(r.why) }));
    const fail = (error: string, refused: { file: string; why: string }[]) => ({ ok: false, error: refused.length ? 'Not committed: these files are not only the session\'s. Commit them by hand.' : error, commit: '', files: [], refused, edits: [] });
    const dry = await call('/__tamber', { method: 'commit', client, dry: true }, 15000, heldToken || undefined);
    if (dry.error === RESTARTED) forget(dry.used);
    if (session !== at) return fail(RESTARTED, []);
    if (dry.error) return fail(dry.error, refusedOf(dry.reply));
    type Edit = { file: string; client: string; tokens: string[]; write?: number };
    const scope = dry.reply.ok as { files: string[]; edits: Edit[]; scope: string };
    const words = (e: Edit) => {
      const n = e.write ? changeset.find(m => m.write === e.write) : undefined;
      return n ? fileWords(n, e.file) : `${e.file}: ${e.tokens.join(', ') || 'edited'} · ${e.client}`;
    };
    const lines = scope.edits.map(words);
    const authors = [...new Set(scope.edits.map(e => e.client))];
    const message = `design: ${scope.edits.length} change${scope.edits.length === 1 ? '' : 's'} by ${authors.join(', ')}\n\n${lines.map(l => `- ${l}`).join('\n')}`;
    const sent = changeset[0]?.seq ?? 0;
    const { error, reply, used } = await call('/__tamber', { method: 'commit', client, message, scope: scope.scope }, WRITE_WAIT, dry.used);
    if (error === RESTARTED) forget(used);
    if (session !== at) return fail(RESTARTED, []);
    if (error) return fail((reply.error as { stale?: boolean } | undefined)?.stale ? 'Something was written while the commit was being composed: nothing was committed. Commit again.' : error, refusedOf(reply));
    const ok = reply.ok as { commit: string; files: string[] };
    // What was written before it was sent, in its files; a write since is
    // not in it. The edits it took are the scope's, as composed.
    for (const n of changeset) if (n.seq <= sent && n.files.some(f => ok.files.includes(f))) n.committed = true;
    return { ok: true, error: '', commit: ok.commit.slice(0, 12), files: ok.files, refused: [], edits: lines.map((text, n) => ({ n, text })) };
  },
  loadHistory: async ([shown, pages]) => {
    const none = { ok: true, error: '', commits: [], more: false };
    if (!shown) return none;
    // Ten a page, the newest first.
    const { error, reply } = await call('/__tamber', { method: 'history', limit: 10, skip: 10 * (Math.max(1, pages) - 1) }, 30000);
    if (error) return { ...none, ok: false, error };
    const ok = reply.ok as { commits: any[]; more: boolean };
    const when = (w: When[]) => (w?.length ? ` when ${whenText(w)}` : '');
    // A state part's row (a `.contract` file's, LLP 1121 stage 8 D) is its style's: `Post :active opacity`.
    const edit = (e: { what: string; name: string; state?: string; row?: string; when: When[]; before: string | null; after: string | null }) => {
      const name = e.what === 'state' ? `${e.name} :${e.state} ${e.row}` : e.name, what = e.what === 'state' ? 'row' : e.what;
      return e.before === null ? `added ${what} ${name}${when(e.when)} = ${e.after}`
        : e.after === null ? `removed ${what} ${name}${when(e.when)} (was ${e.before})`
        : `${name}${when(e.when)} ${e.before} → ${e.after}`;
    };
    return {
      ok: true, error: '', more: ok.more === true,
      commits: ok.commits.map(c => ({
        commit: String(c.commit), short: String(c.commit).slice(0, 12), author: String(c.author ?? ''), date: String(c.date ?? '').slice(0, 10),
        subject: String(c.message ?? '').split('\n')[0],
        files: (c.files ?? []).map((f: any) => ({ file: String(f.file), refused: f.refused ? String(f.refused) : '', edits: [...(f.edits ?? []).map(edit), ...(f.code ? ['and lines outside its state parts (the app\'s code)'] : [])] })),
      })),
    };
  },
  acceptHeld: async ([client, proposal, items]) => {
    // All, or the items picked (indices of its operations). Some, or a
    // file moved since it was held: the server recomputes it on today's
    // files and holds that instead, to be reviewed and accepted afresh.
    // Its operations, kept now: the poll that answers as it is accepted no
    // longer lists it.
    const at = session, some = items ? items.split(',').map(Number) : undefined, ops = (heldOps.get(proposal) ?? []) as Record<string, unknown>[];
    const proposer = held.find(h => h.proposal === proposal)?.client;
    // To the server run that listed it.
    const { error, reply, used } = await call('/__tamber', { method: 'accept', client, proposal, ...(some ? { items: some } : {}) }, WRITE_WAIT, heldToken || undefined);
    if (error === RESTARTED) forget(used);
    if (session !== at) return { ...stacks(false, RESTARTED, ''), note: '' };
    if (error) return { ...stacks(false, error, ''), note: '' };
    const ok = reply.ok as { recomputed?: boolean; proposal?: string; inverse?: string | null; changed?: { token: string }[] };
    if (ok.recomputed) return { ...stacks(true, '', 'recomputed'), note: `${proposal} was recomputed on the files as they are now, as ${ok.proposal}: review its diff and accept it again.` };
    // Accepted: written as its proposer's, and one step of this session's undo.
    if (ok.inverse) { undoStack.push({ inverse: ok.inverse, ops, label: `the proposal ${proposal}`, token: used, client: proposer ?? (reply.ok as { client?: string }).client }); redoStack.length = 0; }
    return { ...stacks(true, '', 'accepted'), note: `Accepted ${proposal}.` };
  },
  importFiles: async ([client, kind, paths, names], _store, storage) => {
    // The picked files' text, sent with `import`: the server runs the
    // importer and holds its proposal, with its review, for the inbox.
    const list = paths.split('\n').filter(Boolean), called = names.split('\n');
    const none = { counted: '', unmapped: [] };
    if (!list.length) return { ok: false, error: 'Choose a file to import.', proposal: '', note: '', ...none };
    // A stylesheet's importer reads one sheet; DTCG, a file a mode.
    if (list.length > 1 && kind !== 'dtcg') return { ok: false, error: `${kind} imports one stylesheet: choose one file (several files are DTCG's, a mode each).`, proposal: '', note: '', ...none };
    let texts: { name: string; text: string }[];
    try {
      texts = await Promise.all(list.map(async (path, i) => ({ name: called[i] || path.split('/').pop()!, text: new TextDecoder().decode(await storage.fs.readFile(path)) })));
    } catch (e) {
      if ((e as { code?: string }).code === 'bake') throw e;
      return { ok: false, error: `The file could not be read (${(e as { code?: string }).code ?? 'failed'}).`, proposal: '', note: '', ...none };
    }
    const body = texts.length === 1 ? { source: texts[0].text, name: texts[0].name } : { sources: texts };
    const { error, reply } = await call('/__tamber', { method: 'import', client, kind, ...body }, 60000);
    if (error) return { ok: false, error, proposal: '', note: '', ...none };
    const ok = reply.ok as { proposal: string | null; message?: string; review?: unknown };
    // Nothing to propose still has its report: what it could not map, and
    // the count.
    if (!ok.proposal) {
      const r = reviewOf({ review: ok.review, ops: [] });
      return { ok: true, error: '', proposal: '', note: ok.message ?? 'Nothing to propose.', counted: r.counted, unmapped: r.unmapped };
    }
    return { ok: true, error: '', proposal: ok.proposal, note: `Held for review as ${ok.proposal}.`, ...none };
  },
  acceptImport: async ([client, proposal, picked]) => {
    const r = heldReview.get(proposal), ops = heldOps.get(proposal) ?? [];
    if (!r) return { ...stacks(false, `No held import \`${proposal}\`.`, ''), note: '' };
    const items = chosen(proposal, picked);
    if (!items.length) return { ...stacks(false, 'Nothing is chosen to import.', ''), note: '' };
    return sources.acceptHeld([client, proposal, items.length === ops.length ? '' : items.join(',')], undefined as never, undefined as never, undefined as never);
  },
  editImport: async ([client, proposal, index, mode, value, picked]) => {
    // A mapped token's value in one mode changed before accepting: the
    // import held again (on the same server run) as what the card has
    // chosen, with the edit, its review renumbered to those items and its
    // overrides on, since they were ticked; the old one dropped. So its
    // diff is what Accept writes, and an override left unticked is not
    // checked along with an edit it has no part in.
    const at = session;
    const r = heldReview.get(proposal), ops = (heldOps.get(proposal) ?? []).map(o => ({ ...o, whens: o.whens?.map((w: any) => ({ ...w })) }));
    const op = ops[index];
    if (!r || !op || (op.op !== 'add-token' && op.op !== 'set-token')) return { ...stacks(false, `No value to edit in \`${proposal}\`.`, ''), note: '', carried: [] };
    if (!value.trim()) return { ...stacks(false, 'Give it a value.', ''), note: '', carried: [] };
    if (mode === 0) op.value = value.trim();
    else if (op.whens?.[mode - 1]) op.whens[mode - 1].value = value.trim();
    else return { ...stacks(false, `No such mode in \`${proposal}\`.`, ''), note: '', carried: [] };
    const keep = chosen(proposal, picked);
    if (!keep.includes(index)) return { ...stacks(false, `${op.name} is left out of the import: tick it to edit it.`, ''), note: '', carried: [] };
    const tokens = Number(r.tokens ?? 0);
    const review = {
      ...r, kept: true, from: proposal, tokens: keep.filter(i => i < tokens).length,
      offers: (r.offers ?? []).map((o: any) => ({ ...o, items: (o.items ?? []).map((i: number) => keep.indexOf(i)).filter((i: number) => i >= 0) })),
    };
    const made = await call('/__tamber', { method: 'propose', hold: true, client, ops: keep.map(i => ops[i]), review }, WRITE_WAIT, heldToken || undefined);
    if (made.error === RESTARTED) forget(made.used);
    if (session !== at) return { ...stacks(false, RESTARTED, ''), note: '', carried: [] };
    if (made.error) return { ...stacks(false, made.error, ''), note: '', carried: [] };
    const fresh = String(made.reply.ok.proposal);
    const dropped = await call('/__tamber', { method: 'drop', client, proposal }, 15000, heldToken || undefined);
    const carried: string[] = [];
    const said = `${op.name} is ${value.trim()}${mode ? ` when ${whenText(op.whens[mode - 1].when ?? [])}` : ''} in the import, held again as ${fresh} with what was chosen: review it and accept.`;
    return { ...stacks(true, '', 'edited'), note: dropped.error ? `${said} The old one, ${proposal}, could not be dropped (${dropped.error}): drop it.` : said, carried };
  },
  loadAppAxes: async ([file]) => {
    // The running app's axes while another file is open: its own design's,
    // read again as the changes move (the open file's read is the app's
    // when none is open).
    if (!file) return { axes: [], answer: 0 };
    const axesAt = ++axesReads, partsAt = ++partsReads, { error, reply } = await call('/__tamber', { method: 'describe' });
    const ir = (reply?.ok as { ir?: { axes?: Axis[] } } | undefined)?.ir, axes = error ? [] : (ir?.axes ?? []);
    // Also the axes a selection is placed by, while this file is open; and its library components' parts, of the
    // newest of these reads (the open file's own reads move `axesReads`, never the parts).
    if (!error && axesAt === axesReads) designAxes = axes;
    if (!error && partsAt === partsReads) libraryParts = partsFrom(ir as Parameters<typeof partsFrom>[0]);
    return { axes: axes.map(a => ({ name: a.name, values: a.values })), answer: ++appReads };
  },
  loadSelection: async () => {
    // Below write (paired, T6), once a server run, its builds and the design read: the selection each build
    // last reported, as the server keeps it (a read-only refresh); its events before the poll began are not
    // sent, and Inspect is not this client's.
    // It is shown as reported, not as current (the server keeps no cursor of when): it offers no edit.
    // Asked again after a failure, or once the poll missed events, until one answers.
    if (scopeNow() !== 'write' && place.epoch && seeded !== place.epoch && snapshots.size && (!selectionNow || selectionNow.stale)) {
      const sel = await call('/__tamber', { method: 'inspect', client: polled, refresh: true }), e = (sel.reply.ok?.selections ?? []).at(-1);
      if (!sel.error) seeded = place.epoch;
      if (!sel.error && e && (!selectionNow || selectionNow.stale)) { const b = buildsNow.get(String(e.build)); selectionNow = { e: { ...e, held: undefined }, at: buildKey(b), axes: JSON.stringify(designAxes), scheme: schemeOf(e, b), stale: 'the build reported it before this client joined', files: digestKey(lastFiles) }; }
    }
    latch();
    const b = selectionNow ? buildsNow.get(String(selectionNow.e.build)) : undefined;
    const view = () => selectionView(selectionNow, selectionNow?.stale ?? '', designAxes, columnsOf(designAxes), inspectOn.size > 0, b?.holding, holdNotes.get(String(selectionNow?.e.build)) ?? '', id => libraryParts.get(id) ?? null);
    const shown = view();
    // Held here and the selection not reporting that state yet (none, or an
    // earlier hold's): its page is asked to report it again, once a hold.
    // (Until a selection answers, its held rows are said to be asked for.)
    const id = String(selectionNow?.e.build), ask = selectionNow && shown.hold.held && selectionNow.e.held?.state !== b?.holding.hold?.state ? JSON.stringify(b?.holding.hold) : '';
    if (!ask || asked.get(id) === ask) return shown;
    const asking = selectionNow!;
    asked.set(id, ask); asking.asking = true;
    // (Refused or lost: asked again on a later read, and not said to be asked meanwhile.)
    void call('/__tamber', { method: 'inspect', client: polled, build: asking.e.build, refresh: true }).then(({ error }) => { if (error && asked.get(id) === ask) { asked.delete(id); asking.asking = false; } });
    return view();
  },
  sendHold: async ([client, state]) => {
    // A state held on the selected copy (LLP 1122 D8; `hold`, stage 8 E), or
    // let go (""): one a build, which replaces the last.
    const e = selectionNow?.e;
    if (!e || selectionNow?.stale) return { ok: false, error: 'Select a view again to hold a state on it.', note: '' };
    const { error } = await call('/__tamber', { method: 'hold', client, build: e.build, site: e.site, instance: e.instance ?? [], state: state || null });
    // (A refused hold keeps the last word: the refusal is beside it.)
    if (!error) { if (state) holdNotes.delete(String(e.build)); else holdNotes.set(String(e.build), 'Let go.'); }
    return error ? { ok: false, error, note: '' } : { ok: true, error: '', note: '' };
  },
  sendInspect: async ([client, on]) => {
    // Inspect mode on every build that can (LLP 1122 D4), or off; asked again, the refusals said before go (back on a failure).
    const refused = takeRefusals(buildsNow), { error, reply } = await call('/__tamber', { method: 'inspect', client, on });
    if (error) { restoreRefusals(buildsNow, refused); return { ok: false, error, note: '' }; }
    const ok = reply.ok as { builds: string[]; unsupported: string[] };
    // The toggle follows the server's records and `inspect` events, not this
    // answer, which a newer event (another client taking over) may follow.
    const none = ok.unsupported?.length ? ` ${ok.unsupported.join(', ')} cannot inspect.` : '';
    return { ok: true, error: '', note: on ? `Inspecting on ${ok.builds.length} build(s): click a view in one.${none}` : `Inspect mode off on ${ok.builds.length} build(s).` };
  },
  loadUses: async ([token, answer]) => {
    // Where the app reads a token, statically (the server's `uses`): the
    // impact of an edit to it, as source sites, not mounted copies.
    if (!token) return { ok: true, error: '', token: '', sites: [] };
    const { error, reply } = await call('/__tamber', { method: 'uses', token });
    if (error) return { ok: false, error, token, sites: [] };
    const via = (v: any) => !v ? '' : v.token ? ` · through ${v.token}` : v.synthetic ? ` · through ${v.synthetic}` : v.slot ? ` · through the slot ${v.slot}` : v.derive ? ` · through the derive ${v.derive}` : '';
    const sites = ((reply.ok as { sites?: any[] }).sites ?? []).map(x => ({ file: String(x.file), line: Number(x.line), col: Number(x.col), text: `${String(x.file).split('/').pop()}:${x.line} · ${x.component}${via(x.via)}` }));
    return { ok: true, error: '', token, sites };
  },
  outlineUses: async ([client, token]) => {
    // A token's sites outlined on every build in inspect mode (`outline`).
    const { error, reply } = await call('/__tamber', { method: 'uses', token });
    if (error) return { ok: false, error, note: '' };
    const at = ((reply.ok as { sites?: any[] }).sites ?? []).map(x => ({ file: x.file, line: x.line, col: x.col }));
    const out = await call('/__tamber', { method: 'outline', client, at });
    if (out.error) return { ok: false, error: out.error, note: '' };
    const done = out.reply.ok as { sites: number[]; builds: string[] };
    return { ok: true, error: '', note: done.builds?.length ? `Sent ${token}'s ${done.sites.length} site(s) to ${done.builds.join(', ')}; a build in inspect mode on that plan outlines them.` : `No build can inspect: ${token}'s sites were outlined nowhere.` };
  },
  sendPin: async ([client, build, scheme, contrast, mode, rootFontSize, preview, handBack]) => {
    // A pin on one build ("" every build): the facts `prefer` pins, each
    // left out when ""; null to hand them back, only when asked (Hand back);
    // or `preview`, one axis on one build, which replaces its pin. The rows
    // follow the server's build events, which say when a status catches up.
    const facts: Record<string, unknown> = {};
    if (scheme) facts.scheme = scheme;
    if (contrast) facts.contrast = contrast;
    const modes = Object.fromEntries(mode.split(' ').filter(Boolean).map(w => w.split('=') as [string, string]));
    if (Object.keys(modes).length) facts.mode = modes;
    if (rootFontSize >= 4 && rootFontSize <= 64) facts.rootFontSize = rootFontSize;
    const clear = !Object.keys(facts).length;
    if (clear && !handBack && !preview) return { ok: false, error: 'Choose what to pin: a scheme, a contrast, a mode or a root font size.', note: '' };
    const body = preview
      ? { method: 'preview', client, build, axis: Object.keys(modes)[0], value: Object.values(modes)[0] }
      : { method: 'pin', client, ...(build ? { build } : {}), pin: clear ? null : facts };
    const { error, reply } = await call('/__tamber', body);
    if (error) return { ok: false, error, note: '' };
    const ok = reply.ok as { builds?: string[]; revision?: number };
    return { ok: true, error: '', note: clear ? `Pins handed back on ${(ok.builds ?? []).length} build(s).` : `Pinned on ${(ok.builds ?? []).length} build(s).` };
  },
  dropHeld: async ([client, proposal]) => {
    const { error } = await call('/__tamber', { method: 'drop', client, proposal });
    return { ...stacks(!error, error, error ? '' : 'dropped'), note: error ? '' : `Dropped ${proposal}.` };
  },
  revertCommit: async ([client, commit]) => {
    // Held, never written: reviewed and accepted like any held proposal.
    const { error, reply } = await call('/__tamber', { method: 'revert', client, commit }, 30000);
    const refused = ((reply.error as { refused?: any[] } | undefined)?.refused ?? []).map(r => `${r.file}: ${r.edit ? `${r.edit.name}${r.edit.what === 'state' ? ` :${r.edit.state} ${r.edit.row}` : ''} ${r.edit.before ?? '(absent)'} → ${r.edit.after ?? '(absent)'}` : 'the file'}, ${r.why}${r.now != null ? ` (now ${r.now})` : ''}`);
    if (error) return { ok: false, error, commit, proposal: '', refused };
    return { ok: true, error: '', commit, proposal: String((reply.ok as { proposal: string }).proposal), refused: [] };
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);

/** WCAG 2's contrast ratio of a text colour `fg` over a background `bg`,
 * each `#rrggbb[aa]`: the text composited over the background first. A
 * background that is not opaque has no one ratio until what is behind it
 * is known: said, as is anything that is not such a colour. */
export function contrast(fg: string, bg: string): number | string {
  const rgba = (hex: string) => {
    const m = /^#([0-9a-f]{6})([0-9a-f]{2})?$/i.exec(hex.trim());
    return m ? [0, 2, 4].map(i => parseInt(m[1].slice(i, i + 2), 16) / 255).concat(m[2] ? parseInt(m[2], 16) / 255 : 1) : null;
  };
  const [f, b] = [rgba(fg), rgba(bg)];
  if (!f || !b) return 'not a colour';
  if (b[3] < 1) return 'translucent background';
  const over = f.slice(0, 3).map((c, i) => c * f[3] + b[i] * (1 - f[3]));
  const lum = (c: number[]) => {
    const [r, g, bl] = c.map(v => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
    return 0.2126 * r + 0.7152 * g + 0.0722 * bl;
  };
  const [x, y] = [lum(over), lum(b.slice(0, 3))];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}
