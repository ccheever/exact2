// The Figma plugin's main thread (LLP 1121 §5 stage 10b, rt1's plan E4): it
// writes the Tamber collections a pull plans (mapping.ts) and reads them back
// for a push. The UI (ui.ts) holds the server, the paired token and every
// request; this thread only touches the document, never the network. Each
// reply carries the request's `id`, an error too.
import { conflicts, contextsMoved, pull, push, type FigmaValue, type Plan } from './mapping';

declare const figma: any;
declare const __html__: string;

figma.showUI(__html__, { width: 380, height: 520 });

const KEY = 'exact';

// The pulled export and its plan, on the file: an entry holds at most 100 KB,
// so each is kept in parts (`name#0`, `name#1`, …, their count in `name#n`).
const PART = 30_000; // characters: 100 KB at three bytes a character
// Each part written only when it differs, the step marked dirty once one is.
const put = (key: string, value: string) => { if (figma.root.getSharedPluginData(KEY, key) !== value) { figma.root.setSharedPluginData(KEY, key, value); dirty = true; } };
const keep = (name: string, text: string) => {
  const n = Math.ceil(text.length / PART);
  for (let i = 0; i < n; i++) put(`${name}#${i}`, text.slice(i * PART, (i + 1) * PART));
  const was = Number(figma.root.getSharedPluginData(KEY, `${name}#n`) || 0);
  for (let i = n; i < was; i++) put(`${name}#${i}`, '');
  put(`${name}#n`, String(n));
};
const kept = (name: string) => {
  const n = Number(figma.root.getSharedPluginData(KEY, `${name}#n`) || 0);
  return n ? Array.from({ length: n }, (_, i) => figma.root.getSharedPluginData(KEY, `${name}#${i}`)).join('') : null;
};
const stored = () => ({ baseline: kept('baseline'), plan: JSON.parse(kept('plan') ?? 'null') as Plan | null });

type Held = Record<string, Record<string, Record<string, FigmaValue>>>;

/** A collection by id: the file's own, else a library's. */
async function collectionOf(id: string, local: any[]): Promise<any> {
  return local.find(c => c.id === id) ?? (await figma.variables.getVariableCollectionByIdAsync?.(id)) ?? null;
}

/** A variable's value in a mode, an alias followed to a value: in the
 * target's collection (the file's or a library's), the mode of the same
 * name, else its default. Why not, for an alias that goes nowhere or round
 * in a cycle. */
async function valueOf(v: any, modeId: string, collections: any[]): Promise<{ value: FigmaValue } | { why: string }> {
  const mode = (await collectionOf(v.variableCollectionId, collections))?.modes.find((m: any) => m.modeId === modeId)?.name;
  const seen = new Set([v.id]);
  let value = v.valuesByMode[modeId];
  while (value && typeof value === 'object' && value.type === 'VARIABLE_ALIAS') {
    if (seen.has(value.id)) return { why: 'its alias goes round in a cycle' };
    seen.add(value.id);
    const target = await figma.variables.getVariableByIdAsync(value.id);
    const col = target && (await collectionOf(target.variableCollectionId, collections));
    if (!col) return { why: 'its alias names a variable this file cannot read' };
    value = target.valuesByMode[col.modes.find((m: any) => m.name === mode)?.modeId ?? col.defaultModeId];
  }
  return value === undefined || value === null ? { why: 'no value in this mode' } : { value };
}

/** A plan's variable in a collection: by the token it is (the plugin's mark
 * on it, so a variable renamed in Figma is still the token's), never by a
 * stranger's name. */
const variableOf = (variables: any[], col: any, v: { token: string }) =>
  variables.find(x => x.variableCollectionId === col.id && x.getSharedPluginData?.(KEY, 'token') === v.token);
/** A plan's collection by the id it was written with: that one or none. */
const collectionIn = (collections: any[], c: Plan['collections'][number]) => collections.find(x => x.id === c.id) ?? null;
/** A plan's mode by the id its context was written with: that one or none. */
const modeIn = (col: any, c: Plan['collections'][number], mode: string) => col.modes.find((m: any) => m.modeId === c.contextIds?.[c.contexts[mode]]) ?? null;

/** The plan's collections as the document holds them now, and the values it
 * could not read, and why. */
async function holding(plan: Plan): Promise<{ now: Held; unread: { variable: string; mode: string; why: string }[] }> {
  const collections = await figma.variables.getLocalVariableCollectionsAsync();
  const variables = await figma.variables.getLocalVariablesAsync();
  const now: Held = {}, unread: { variable: string; mode: string; why: string }[] = [];
  for (const c of plan.collections) {
    const col = collectionIn(collections, c);
    if (!col) { unread.push({ variable: c.name, mode: '', why: 'the collection is gone from this file' }); continue; }
    now[c.name] = {};
    for (const v of c.variables) {
      const fv = variableOf(variables, col, v);
      if (!fv) { unread.push({ variable: `${c.name}: ${v.name}`, mode: '', why: 'the variable is gone from this file' }); continue; }
      now[c.name][v.name] = {};
      for (const mode of c.modes) {
        const m = modeIn(col, c, mode);
        if (!m) { unread.push({ variable: `${c.name}: ${fv.name}`, mode, why: 'its mode is gone from this file' }); continue; }
        const got = await valueOf(fv, m.modeId, collections);
        if ('value' in got) now[c.name][v.name][mode] = got.value;
        else unread.push({ variable: `${c.name}: ${fv.name}`, mode, why: got.why });
      }
    }
  }
  return { now, unread };
}

/** Write a plan's collections, their modes and variables, reconciled with
 * what the last pull wrote (`before`): its modes and tokens the design no
 * longer has go, a variable whose type changed is made again. A collection
 * of the plan's name this plugin did not make is refused, never taken
 * over. Each collection's id and its modes' are written into the plan. */
async function write(plan: Plan, before: Plan | null, fresh = false) {
  const collections = await figma.variables.getLocalVariableCollectionsAsync();
  const variables = await figma.variables.getLocalVariablesAsync();
  // Each collection first, refused before anything is touched: the one the
  // last pull wrote (by id), else one of the plan's name this plugin made
  // that no other of the last pull's collections is; one each.
  const reserved = new Set((before?.collections ?? []).map(c => collectionIn(collections, c)?.id).filter(Boolean));
  const taken = new Set<string>();
  const cols = plan.collections.map(c => {
    // (fresh: every collection new, the last pull's left for the migration to take away)
    const was = fresh ? undefined : before?.collections.find(x => x.name === c.name);
    const col = fresh ? null : (was && collectionIn(collections, was)) || collections.find((x: any) => x.name === c.name && !reserved.has(x.id) && !taken.has(x.id)) || null;
    if (col && !col.getSharedPluginData?.(KEY, 'owned')) throw new Error(`this file has a collection \`${col.name}\` that Tamber did not make: rename it, then pull`);
    // (a variable of a token's name this plugin did not make: refused, not taken over)
    const stranger = col && c.variables.find(v => !variableOf(variables, col, v) && variables.some((x: any) => x.variableCollectionId === col.id && x.name === v.name && !x.getSharedPluginData?.(KEY, 'token')));
    if (stranger) throw new Error(`the collection \`${col.name}\` has a variable \`${stranger.name}\` that Tamber did not make: rename it, then pull`);
    if (col) taken.add(col.id);
    return { c, was, col };
  });
  // Each change marks the step dirty once it has happened (a refusal before
  // any change undoes nothing, never the person's own last edit).
  const did = <T>(r: T): T => { dirty = true; return r; };
  for (let { c, was, col } of cols) {
    const made = !col;
    if (!col) col = did(figma.variables.createVariableCollection(c.name));
    if (col.getSharedPluginData?.(KEY, 'owned') !== '1') did(col.setSharedPluginData(KEY, 'owned', '1'));
    if (col.name !== c.name) did((col.name = c.name));
    // Each context's mode by the id the last pull wrote it with (renamed in
    // Figma, it is still that context; a mode's shown name is only that).
    const prior: Record<string, string> = { ...was?.parked, ...was?.contextIds };
    const ids: Record<string, string> = {}, used = new Set<string>();
    for (const mode of c.modes) {
      const m = col.modes.find((x: any) => x.modeId === prior[c.contexts[mode]]);
      if (m && !used.has(m.modeId)) { ids[mode] = m.modeId; used.add(m.modeId); }
    }
    // A default mode can't be removed. One that was a context's stays that
    // context's, parked (`unused: <context>`) until the context comes back;
    // a new collection's own first mode takes the first context. The others
    // no context keeps go before any is added, so a full collection takes
    // the new ones.
    // (a default that is neither a pull's nor a new collection's own is the
    // person's: left as it is, its name and values theirs)
    let parked: [string, string] | null = null;
    if (!used.has(col.defaultModeId)) {
      const own = Object.keys(prior).find(ctx => prior[ctx] === col.defaultModeId);
      const take = own === undefined && made ? c.modes.find(mode => !ids[mode]) : undefined;
      if (take) { ids[take] = col.defaultModeId; used.add(col.defaultModeId); } else if (own !== undefined) parked = [own, col.defaultModeId];
    }
    // Only modes a pull made (the ids it kept for contexts) go; one the person added stays.
    const ours = new Set(Object.values(prior));
    for (const m of [...col.modes]) if (!used.has(m.modeId) && m.modeId !== parked?.[1] && ours.has(m.modeId)) did(col.removeMode(m.modeId));
    // Names in two steps, each first to a name no mode has or will have, so
    // two modes trading names never ask for one another still has.
    const final: [string, string][] = [...c.modes.map(mode => [mode, mode] as [string, string]), ...(parked ? [[`unused: ${parked[0]}`.slice(0, 40), ''] as [string, string]] : [])];
    const names = new Set([...col.modes.map((x: any) => x.name), ...final.map(([n]) => n)]);
    let n = 0;
    const temp = () => { while (names.has(`~tamber ${n}`)) n++; names.add(`~tamber ${n}`); return `~tamber ${n}`; };
    // Figma's refusal of a mode too many, told from every other error here and only here.
    for (const mode of c.modes) {
      if (ids[mode]) continue;
      try { ids[mode] = did(col.addMode(temp())); } catch (e: any) {
        const limit = /\blimited to (\d+) modes?\b/i.exec(String(e?.message ?? e));
        // (with the collection, and the modes in it a person added, for the refusal to name)
        const personal = col.modes.filter((m: any) => !ours.has(m.modeId) && !Object.values(ids).includes(m.modeId)).map((m: any) => m.name);
        throw limit ? Object.assign(new Error(String(e?.message ?? e)), { capacity: Number(limit[1]), collection: col.name, personal }) : e;
      }
    }
    const idOf = (mode: string, slot: string) => (slot === '' && parked ? parked[1] : ids[mode]);
    const touched = final.filter(([name, slot]) => col.modes.find((x: any) => x.modeId === idOf(name, slot))?.name !== name);
    for (const [name, slot] of touched) did(col.renameMode(idOf(name, slot), temp()));
    for (const [name, slot] of touched) did(col.renameMode(idOf(name, slot), name));
    const tokens = new Set(c.variables.map(v => v.token));
    for (const x of variables) if (x.variableCollectionId === col.id && x.getSharedPluginData?.(KEY, 'token') && !tokens.has(x.getSharedPluginData(KEY, 'token'))) did(x.remove());
    for (const v of c.variables) {
      let fv = variableOf(variables, col, v);
      if (fv && fv.resolvedType !== v.type) { did(fv.remove()); fv = undefined; }
      if (!fv) fv = did(figma.variables.createVariable(v.name, col, v.type));
      if (fv.getSharedPluginData?.(KEY, 'token') !== v.token) did(fv.setSharedPluginData(KEY, 'token', v.token));
      for (const mode of c.modes) did(fv.setValueForMode(ids[mode], v.values[mode]));
    }
    c.id = col.id;
    c.contextIds = Object.fromEntries(c.modes.map(mode => [c.contexts[mode], ids[mode]]));
    if (parked?.[0]) c.parked = { [parked[0]]: parked[1] };
  }
}
/** Whether the document changed since the last undo step was committed. */
let dirty = false;
/** Figma's refusal of one mode too many, as `write` marks it at `addMode`, and nothing else. */
const tooManyModes = (e: any) => typeof e?.capacity === 'number';
/** The modes a collection may have, from Figma's refusal (`Limited to N modes`). */
const limitOf = (e: any) => e.capacity as number;

figma.ui.onmessage = async (msg: any) => {
  const say = (reply: object) => figma.ui.postMessage({ ...reply, id: msg.id });
  try {
    switch (msg.type) {
      case 'load': {
        // The server and its token in one entry: never one without the other.
        const { baseline, plan } = stored(), paired = (await figma.clientStorage.getAsync('paired')) ?? {};
        say({ type: 'loaded', server: paired.server ?? '', token: paired.token ?? null, digest: baseline?.match(/"digest":\s*"([0-9a-f]+)"/)?.[1] ?? null, pulled: !!plan });
        break;
      }
      case 'save': await figma.clientStorage.setAsync('paired', { server: msg.server, token: msg.token ?? null }); say({ type: 'saved' }); break;
      case 'check': {
        // Before a pull: what both Figma and the design moved since the last
        // one, by token and context; and the contexts the design added or dropped.
        // And what it could not read there (an alias to nowhere, a mode gone), which a pull overwrites.
        // And the variables a pull removes or makes again (a token the design dropped, a type changed): their bindings go.
        const { plan } = stored(), doc = JSON.parse(msg.text), held = plan ? await holding(plan) : { now: {}, unread: [] };
        const fresh = new Map(pull(doc, Infinity).collections[0].variables.map(v => [v.token, v.type])), drops = (plan?.collections ?? []).flatMap(c => c.variables).filter((v, i, all) => all.findIndex(w => w.token === v.token) === i && fresh.get(v.token) !== v.type).map(v => ({ token: v.token, variable: v.name, why: fresh.has(v.token) ? 'its type changed: it is made again' : 'the design dropped it' }));
        say({ type: 'checked', conflicts: plan ? conflicts(plan, doc, held.now) : [], unread: held.unread, drops, contexts: plan ? contextsMoved(plan, doc) : { added: [], removed: [] } });
        break;
      }
      case 'apply': {
        // One collection a mode per context, or split per app tuple where the
        // file's plan refuses as many modes; another error is refused as it
        // is. The whole apply is one undo step: refused, Figma's undo takes
        // back everything it did, the earlier pull's collections, modes,
        // variables, values and the kept export as they were.
        const doc = JSON.parse(msg.text), before = stored().plan;
        // (undone only when something changed since the step began: never an undo of the person's own last edit)
        const undo = () => { if (dirty) figma.triggerUndo(); dirty = false; };
        figma.commitUndo();
        dirty = false;
        try {
          // The last pull's layout while the contexts are the same: one collection a tuple stays so, a plan's limit raised or not.
          const moved = before && contextsMoved(before, doc), split = !!before?.collections.some(c => c.name !== 'Tamber');
          let plan = pull(doc, split && !moved?.added.length && !moved?.removed.length ? 2 : Infinity);
          try { await write(plan, before); } catch (e) {
            if (!tooManyModes(e)) throw e;
            undo();
            figma.commitUndo();
            plan = pull(doc, Math.min(2, limitOf(e))); // a 1-mode file: Tamber needs Light and Dark
            try { await write(plan, before); } catch (e2) {
              // Still too many: a parked default mode holds a place. Only when
              // every context the last pull's collections served is gone (each
              // listed, the person said yes): new collections, the old taken
              // away; a surviving context keeps its mode, and the pull is refused.
              if (!tooManyModes(e2)) throw e2;
              const kept = (before?.collections ?? []).flatMap(c => Object.keys(c.contextIds ?? {})).filter(ctx => !moved?.removed.includes(ctx));
              if (kept.length) throw new Error(e2.personal?.length
                ? `this file allows ${limitOf(e2)} modes a collection, and \`${e2.collection}\` has modes Tamber did not make (${e2.personal.join(', ')}) in the places its contexts need: remove one, then pull`
                : `this file allows ${limitOf(e2)} modes a collection, and a default mode Figma can't remove holds one in \`${e2.collection}\`: to pull, remove that collection (what is bound to it, and any variables you added there, go with it)`);
              undo();
              figma.commitUndo();
              await write(plan, before, true);
            }
          }
          // A layout the design outgrew (one collection to one a tuple, or
          // back): the earlier pull's collections go.
          const now = new Set(plan.collections.map(c => c.id)), collections = await figma.variables.getLocalVariableCollectionsAsync();
          // One holding a variable or a mode Tamber did not make is refused (the whole pull undone), never removed with it.
          const variables = await figma.variables.getLocalVariablesAsync();
          for (const c of before?.collections ?? []) {
            const old = collectionIn(collections, c);
            if (!old || now.has(old.id) || !old.getSharedPluginData?.(KEY, 'owned')) continue;
            const theirs = variables.filter((v: any) => v.variableCollectionId === old.id && !v.getSharedPluginData?.(KEY, 'token')).map((v: any) => v.name);
            const ours = new Set([...Object.values(c.contextIds ?? {}), ...Object.values(c.parked ?? {})]);
            theirs.push(...old.modes.filter((m: any) => !ours.has(m.modeId)).map((m: any) => `the mode ${m.name}`));
            if (theirs.length) throw new Error(`the collection \`${old.name}\` this pull would replace has what Tamber did not make (${theirs.join(', ')}): move it, then pull`);
            old.remove();
            dirty = true;
          }
          keep('baseline', msg.text);
          keep('plan', JSON.stringify(plan));
          figma.commitUndo();
          dirty = false;
          say({ type: 'applied', collections: plan.collections.map(c => c.name), skipped: plan.skipped });
        } catch (e) {
          undo();
          throw e;
        }
        break;
      }
      case 'collect': {
        const { baseline, plan } = stored();
        if (!baseline || !plan) { say({ type: 'error', message: 'pull first: this file has no Tamber variables from a pull' }); break; }
        const { now, unread } = await holding(plan);
        say({ type: 'collected', source: push(baseline, plan, now), baseline, unread });
        break;
      }
    }
  } catch (e: any) {
    say({ type: 'error', message: String(e?.message ?? e) });
  }
};
