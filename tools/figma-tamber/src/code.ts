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

/** A variable's value in a mode, an alias followed to a value: in the
 * target's collection, the mode of the same name, else its default. None
 * for an alias that goes nowhere or round in a cycle. */
async function valueOf(v: any, modeId: string, collections: any[]): Promise<FigmaValue | undefined> {
  const mode = collections.find(c => c.id === v.variableCollectionId)?.modes.find((m: any) => m.modeId === modeId)?.name;
  const seen = new Set([v.id]);
  let value = v.valuesByMode[modeId];
  while (value && typeof value === 'object' && value.type === 'VARIABLE_ALIAS') {
    if (seen.has(value.id)) return undefined;
    seen.add(value.id);
    const target = await figma.variables.getVariableByIdAsync(value.id);
    const col = target && collections.find(c => c.id === target.variableCollectionId);
    if (!col) return undefined;
    value = target.valuesByMode[col.modes.find((m: any) => m.name === mode)?.modeId ?? col.defaultModeId];
  }
  return value ?? undefined;
}

/** A plan's variable in a collection: by the token it is (the plugin's mark
 * on it, so a variable renamed in Figma is still the token's), else by name. */
const variableOf = (variables: any[], col: any, v: { name: string; token: string }) => {
  const mine = variables.filter(x => x.variableCollectionId === col.id);
  return mine.find(x => x.getSharedPluginData?.(KEY, 'token') === v.token) ?? mine.find(x => x.name === v.name && !x.getSharedPluginData?.(KEY, 'token'));
};

/** The plan's collections as the document holds them now. */
async function holding(plan: Plan): Promise<Held> {
  const collections = await figma.variables.getLocalVariableCollectionsAsync();
  const variables = await figma.variables.getLocalVariablesAsync();
  const out: Held = {};
  for (const c of plan.collections) {
    const col = collections.find((x: any) => x.name === c.name);
    if (!col) continue;
    out[c.name] = {};
    for (const v of c.variables) {
      const fv = variableOf(variables, col, v);
      if (!fv) continue;
      out[c.name][v.name] = {};
      for (const mode of c.modes) {
        const m = col.modes.find((x: any) => x.name === mode);
        const value = m && (await valueOf(fv, m.modeId, collections));
        if (value !== undefined) out[c.name][v.name][mode] = value;
      }
    }
  }
  return out;
}

/** What a write made, for a refused one to take away. */
interface Made { collections: any[]; modes: [any, string][]; variables: any[] }
const undo = (made: Made) => {
  for (const v of made.variables) v.remove();
  for (const [col, id] of made.modes) if (!made.collections.includes(col)) col.removeMode(id);
  for (const col of made.collections) col.remove();
};
const tooManyModes = (e: any) => /limit/i.test(String(e?.message ?? e)) && /mode/i.test(String(e?.message ?? e));

/** Write a plan's collections, their modes and variables, reconciled with
 * what an earlier pull wrote: its modes and tokens the design no longer has
 * go, a variable whose type changed is made again. Undone if refused. */
async function write(plan: Plan) {
  const collections = await figma.variables.getLocalVariableCollectionsAsync();
  const variables = await figma.variables.getLocalVariablesAsync();
  const made: Made = { collections: [], modes: [], variables: [] };
  try {
    for (const c of plan.collections) {
      let col = collections.find((x: any) => x.name === c.name);
      if (!col) made.collections.push((col = figma.variables.createVariableCollection(c.name)));
      col.setSharedPluginData(KEY, 'owned', '1');
      // Modes the plan does not name: the first renamed to its first (a new
      // collection's `Mode 1` too) when the plan's first is not there, the
      // rest removed before any is added, so a full collection takes the new.
      const ids: Record<string, string> = {};
      for (const m of col.modes) if (c.modes.includes(m.name)) ids[m.name] = m.modeId;
      const stale = col.modes.filter((m: any) => !c.modes.includes(m.name));
      if (stale.length && !ids[c.modes[0]]) { col.renameMode(stale[0].modeId, c.modes[0]); ids[c.modes[0]] = stale.shift().modeId; }
      for (const m of stale) col.removeMode(m.modeId);
      for (const mode of c.modes) if (!ids[mode]) { ids[mode] = col.addMode(mode); made.modes.push([col, ids[mode]]); }
      const tokens = new Set(c.variables.map(v => v.token));
      for (const x of variables) if (x.variableCollectionId === col.id && x.getSharedPluginData?.(KEY, 'token') && !tokens.has(x.getSharedPluginData(KEY, 'token'))) x.remove();
      for (const v of c.variables) {
        let fv = variableOf(variables, col, v);
        if (fv && fv.resolvedType !== v.type) { fv.remove(); fv = undefined; }
        if (!fv) made.variables.push((fv = figma.variables.createVariable(v.name, col, v.type)));
        fv.setSharedPluginData(KEY, 'token', v.token);
        for (const mode of c.modes) fv.setValueForMode(ids[mode], v.values[mode]);
      }
    }
  } catch (e) {
    undo(made);
    throw e;
  }
}

figma.ui.onmessage = async (msg: any) => {
  const say = (reply: object) => figma.ui.postMessage({ ...reply, id: msg.id });
  try {
    switch (msg.type) {
      case 'load': {
        const { baseline, plan } = stored();
        say({ type: 'loaded', server: await figma.clientStorage.getAsync('server'), token: await figma.clientStorage.getAsync('token'), digest: baseline?.match(/"digest":\s*"([0-9a-f]+)"/)?.[1] ?? null, pulled: !!plan });
        break;
      }
      case 'save': await figma.clientStorage.setAsync('server', msg.server); await figma.clientStorage.setAsync('token', msg.token ?? null); say({ type: 'saved' }); break;
      case 'check': {
        // Before a pull: what both Figma and the design moved since the last
        // one, by token and context; and the contexts the design added or dropped.
        const { plan } = stored(), doc = JSON.parse(msg.text);
        say({ type: 'checked', conflicts: plan ? conflicts(plan, doc, await holding(plan)) : [], contexts: plan ? contextsMoved(plan, doc) : { added: [], removed: [] } });
        break;
      }
      case 'apply': {
        // One collection a mode per context, or split per app tuple where the
        // file's plan refuses as many modes; another error is refused as it is.
        const doc = JSON.parse(msg.text), before = stored().plan;
        let plan = pull(doc, Infinity);
        try { await write(plan); } catch (e) {
          if (!tooManyModes(e)) throw e;
          plan = pull(doc, 2);
          await write(plan);
        }
        // A layout the design outgrew (one collection to one a tuple, or
        // back): the earlier pull's collections go.
        const now = new Set(plan.collections.map(c => c.name)), collections = await figma.variables.getLocalVariableCollectionsAsync();
        for (const c of before?.collections ?? []) if (!now.has(c.name)) collections.find((x: any) => x.name === c.name && x.getSharedPluginData?.(KEY, 'owned'))?.remove();
        keep('baseline', msg.text);
        keep('plan', JSON.stringify(plan));
        say({ type: 'applied', collections: plan.collections.map(c => c.name), skipped: plan.skipped });
        break;
      }
      case 'collect': {
        const { baseline, plan } = stored();
        if (!baseline || !plan) { say({ type: 'error', message: 'pull first: this file has no Tamber variables from a pull' }); break; }
        say({ type: 'collected', source: push(baseline, plan, await holding(plan)), baseline });
        break;
      }
    }
  } catch (e: any) {
    say({ type: 'error', message: String(e?.message ?? e) });
  }
};
