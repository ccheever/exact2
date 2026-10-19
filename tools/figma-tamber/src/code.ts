// The Figma plugin's main thread (LLP 1121 §5 stage 10b, rt1's plan E4): it
// writes the Tamber collections a pull plans (mapping.ts) and reads them back
// for a push. The UI (ui.ts) holds the server, the paired token and every
// request; this thread only touches the document, never the network.
import { conflicts, pull, push, type FigmaValue, type Plan } from './mapping';

declare const figma: any;
declare const __html__: string;

figma.showUI(__html__, { width: 380, height: 520 });

const KEY = 'exact';
const say = (msg: object) => figma.ui.postMessage(msg);
const stored = () => ({ baseline: figma.root.getSharedPluginData(KEY, 'baseline') || null, plan: JSON.parse(figma.root.getSharedPluginData(KEY, 'plan') || 'null') as Plan | null });

/** A variable's value in a mode, an alias followed to its value there. */
async function valueOf(v: any, modeId: string): Promise<FigmaValue | undefined> {
  let value = v.valuesByMode[modeId];
  for (let hops = 0; value && typeof value === 'object' && value.type === 'VARIABLE_ALIAS' && hops < 8; hops++) {
    const target = await figma.variables.getVariableByIdAsync(value.id);
    if (!target) return undefined;
    value = target.valuesByMode[modeId] ?? Object.values(target.valuesByMode)[0];
  }
  return value;
}

/** The plan's collections as the document holds them now. */
async function holding(plan: Plan): Promise<Record<string, Record<string, Record<string, FigmaValue>>>> {
  const collections = await figma.variables.getLocalVariableCollectionsAsync();
  const variables = await figma.variables.getLocalVariablesAsync();
  const out: Record<string, Record<string, Record<string, FigmaValue>>> = {};
  for (const c of plan.collections) {
    const col = collections.find((x: any) => x.name === c.name);
    if (!col) continue;
    out[c.name] = {};
    for (const v of c.variables) {
      const fv = variables.find((x: any) => x.variableCollectionId === col.id && x.name === v.name);
      if (!fv) continue;
      out[c.name][v.name] = {};
      for (const mode of c.modes) {
        const m = col.modes.find((x: any) => x.name === mode);
        const value = m && (await valueOf(fv, m.modeId));
        if (value !== undefined) out[c.name][v.name][mode] = value;
      }
    }
  }
  return out;
}

/** Write a plan's collections, their modes and variables; each collection it
 * makes is in `made`, for a refused write to take away. */
async function write(plan: Plan, made: any[] = []) {
  const collections = await figma.variables.getLocalVariableCollectionsAsync();
  const variables = await figma.variables.getLocalVariablesAsync();
  for (const c of plan.collections) {
    let col = collections.find((x: any) => x.name === c.name);
    if (!col) made.push((col = figma.variables.createVariableCollection(c.name)));
    const ids: Record<string, string> = {};
    c.modes.forEach((mode, i) => {
      const have = col.modes.find((x: any) => x.name === mode);
      if (have) ids[mode] = have.modeId;
      else if (i === 0 && col.modes.length === 1 && !c.modes.includes(col.modes[0].name)) { col.renameMode(col.modes[0].modeId, mode); ids[mode] = col.modes[0].modeId; }
      else ids[mode] = col.addMode(mode);
    });
    for (const v of c.variables) {
      const fv = variables.find((x: any) => x.variableCollectionId === col.id && x.name === v.name) ?? figma.variables.createVariable(v.name, col, v.type);
      fv.setSharedPluginData(KEY, 'token', v.token);
      for (const mode of c.modes) fv.setValueForMode(ids[mode], v.values[mode]);
    }
  }
}

figma.ui.onmessage = async (msg: any) => {
  try {
    switch (msg.type) {
      case 'load': {
        const { baseline, plan } = stored();
        say({ type: 'loaded', server: await figma.clientStorage.getAsync('server'), token: await figma.clientStorage.getAsync('token'), digest: baseline?.match(/"digest":\s*"([0-9a-f]+)"/)?.[1] ?? null, pulled: !!plan });
        break;
      }
      case 'save': await figma.clientStorage.setAsync('server', msg.server); await figma.clientStorage.setAsync('token', msg.token ?? null); say({ type: 'saved' }); break;
      case 'check': {
        // Before a pull: what both Figma and the design moved since the last one.
        const { plan } = stored();
        say({ type: 'checked', conflicts: plan ? conflicts(plan, pull(JSON.parse(msg.text), plan.collections[0].modes.length), await holding(plan)) : [] });
        break;
      }
      case 'apply': {
        // One collection a mode per context, or split per app tuple where the plan refuses as many modes.
        const doc = JSON.parse(msg.text);
        let plan = pull(doc, Infinity);
        const made: any[] = [];
        try { await write(plan, made); } catch {
          // The file's plan takes fewer modes: what the first try made goes.
          for (const col of made) col.remove();
          plan = pull(doc, 2);
          await write(plan);
        }
        figma.root.setSharedPluginData(KEY, 'baseline', msg.text);
        figma.root.setSharedPluginData(KEY, 'plan', JSON.stringify(plan));
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
