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
const keep = (name: string, text: string) => {
  const n = Math.ceil(text.length / PART);
  for (let i = 0; i < n; i++) figma.root.setSharedPluginData(KEY, `${name}#${i}`, text.slice(i * PART, (i + 1) * PART));
  const was = Number(figma.root.getSharedPluginData(KEY, `${name}#n`) || 0);
  for (let i = n; i < was; i++) figma.root.setSharedPluginData(KEY, `${name}#${i}`, '');
  figma.root.setSharedPluginData(KEY, `${name}#n`, String(n));
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
 * on it, so a variable renamed in Figma is still the token's), else by name. */
const variableOf = (variables: any[], col: any, v: { name: string; token: string }) => {
  const mine = variables.filter(x => x.variableCollectionId === col.id);
  return mine.find(x => x.getSharedPluginData?.(KEY, 'token') === v.token) ?? mine.find(x => x.name === v.name && !x.getSharedPluginData?.(KEY, 'token'));
};
/** A plan's collection: by the id it was written with, else by name. */
const collectionIn = (collections: any[], c: Plan['collections'][number]) => (c.id && collections.find(x => x.id === c.id)) || collections.find(x => x.name === c.name);
/** A plan's mode in its collection: by the id it was written with, else by name. */
const modeIn = (col: any, c: Plan['collections'][number], mode: string) => (c.modeIds?.[mode] && col.modes.find((m: any) => m.modeId === c.modeIds![mode])) || col.modes.find((m: any) => m.name === mode);

/** The plan's collections as the document holds them now, and the values it
 * could not read, and why. */
async function holding(plan: Plan): Promise<{ now: Held; unread: { variable: string; mode: string; why: string }[] }> {
  const collections = await figma.variables.getLocalVariableCollectionsAsync();
  const variables = await figma.variables.getLocalVariablesAsync();
  const now: Held = {}, unread: { variable: string; mode: string; why: string }[] = [];
  for (const c of plan.collections) {
    const col = collectionIn(collections, c);
    if (!col) continue;
    now[c.name] = {};
    for (const v of c.variables) {
      const fv = variableOf(variables, col, v);
      if (!fv) continue;
      now[c.name][v.name] = {};
      for (const mode of c.modes) {
        const m = modeIn(col, c, mode);
        if (!m) continue;
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
async function write(plan: Plan, before: Plan | null) {
  const collections = await figma.variables.getLocalVariableCollectionsAsync();
  const variables = await figma.variables.getLocalVariablesAsync();
  for (const c of plan.collections) {
    const was = before?.collections.find(x => x.name === c.name);
    let col = was ? collectionIn(collections, was) : collections.find((x: any) => x.name === c.name);
    if (col && !col.getSharedPluginData?.(KEY, 'owned')) throw new Error(`this file has a collection \`${col.name}\` that Tamber did not make: rename it, then pull`);
    if (!col) col = figma.variables.createVariableCollection(c.name);
    col.setSharedPluginData(KEY, 'owned', '1');
    if (col.name !== c.name) col.name = c.name;
    // Each mode by the id the last pull wrote (a mode renamed in Figma is
    // still its context), else by name. Modes the plan does not name are
    // removed before any is added, so a full collection takes the new ones.
    const ids: Record<string, string> = {};
    for (const mode of c.modes) {
      const m = (was && modeIn(col, was, mode)) || col.modes.find((x: any) => x.name === mode);
      if (m && !Object.values(ids).includes(m.modeId)) ids[mode] = m.modeId;
    }
    // A default mode can't be removed: one the plan does not name becomes
    // the plan's first missing mode, else its first, whose mode then goes.
    if (!Object.values(ids).includes(col.defaultModeId)) {
      const take = c.modes.find(mode => !ids[mode]) ?? c.modes[0];
      ids[take] = col.defaultModeId;
    }
    for (const m of [...col.modes]) if (!Object.values(ids).includes(m.modeId)) col.removeMode(m.modeId);
    for (const mode of c.modes) {
      if (!ids[mode]) ids[mode] = col.addMode(mode);
      const m = col.modes.find((x: any) => x.modeId === ids[mode]);
      if (m && m.name !== mode) col.renameMode(ids[mode], mode);
    }
    const tokens = new Set(c.variables.map(v => v.token));
    for (const x of variables) if (x.variableCollectionId === col.id && x.getSharedPluginData?.(KEY, 'token') && !tokens.has(x.getSharedPluginData(KEY, 'token'))) x.remove();
    for (const v of c.variables) {
      let fv = variableOf(variables, col, v);
      if (fv && fv.resolvedType !== v.type) { fv.remove(); fv = undefined; }
      if (!fv) fv = figma.variables.createVariable(v.name, col, v.type);
      fv.setSharedPluginData(KEY, 'token', v.token);
      for (const mode of c.modes) fv.setValueForMode(ids[mode], v.values[mode]);
    }
    c.id = col.id;
    c.modeIds = ids;
  }
}
const tooManyModes = (e: any) => /limit/i.test(String(e?.message ?? e)) && /mode/i.test(String(e?.message ?? e));
/** The modes a collection may have, from Figma's refusal (`Limited to N modes`). */
const limitOf = (e: any) => Number(/limited to (\d+)/i.exec(String(e?.message ?? e))?.[1] ?? 2);

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
        const { plan } = stored(), doc = JSON.parse(msg.text);
        say({ type: 'checked', conflicts: plan ? conflicts(plan, doc, (await holding(plan)).now) : [], contexts: plan ? contextsMoved(plan, doc) : { added: [], removed: [] } });
        break;
      }
      case 'apply': {
        // One collection a mode per context, or split per app tuple where the
        // file's plan refuses as many modes; another error is refused as it
        // is. The whole apply is one undo step: refused, Figma's undo takes
        // back everything it did, the earlier pull's collections, modes,
        // variables, values and the kept export as they were.
        const doc = JSON.parse(msg.text), before = stored().plan;
        figma.commitUndo();
        try {
          let plan = pull(doc, Infinity);
          try { await write(plan, before); } catch (e) {
            if (!tooManyModes(e)) throw e;
            figma.triggerUndo();
            figma.commitUndo();
            plan = pull(doc, Math.min(2, limitOf(e))); // a 1-mode file: Tamber needs Light and Dark
            await write(plan, before);
          }
          // A layout the design outgrew (one collection to one a tuple, or
          // back): the earlier pull's collections go.
          const now = new Set(plan.collections.map(c => c.id)), collections = await figma.variables.getLocalVariableCollectionsAsync();
          for (const c of before?.collections ?? []) { const old = collectionIn(collections, c); if (old && !now.has(old.id) && old.getSharedPluginData?.(KEY, 'owned')) old.remove(); }
          keep('baseline', msg.text);
          keep('plan', JSON.stringify(plan));
          figma.commitUndo();
          say({ type: 'applied', collections: plan.collections.map(c => c.name), skipped: plan.skipped });
        } catch (e) {
          figma.triggerUndo();
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
