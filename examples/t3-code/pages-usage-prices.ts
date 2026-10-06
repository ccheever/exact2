// The Usage page's Model prices dialog (lane "pages"): UsagePriceOverrides,
// "Custom model prices" with the Map to column (upstream b05adb70b0), opened
// from the environment menu or the model dialog's Set price. Sources: T3 Code
// (MIT, see LICENSE-T3) apps/web/src/components/usage/{UsagePriceOverrides.tsx,
// usagePriceTable,usagePriceTargets,usagePriceForm}.ts.
//
// Inputs keep the text the person types: each one's value is a seed that only
// changes when the input is (re)mounted, and every keystroke reaches the draft
// here (pages:usage-prices-edit), so validation and Save follow live.
import { obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import type { T3Client } from './client';
import { letGo } from './let-go';

export const USAGE_PRICE_FIELDS = [
  { key: 'inputCostPerMillionTokens', label: 'Input', optional: false },
  { key: 'outputCostPerMillionTokens', label: 'Output', optional: false },
  { key: 'cacheReadCostPerMillionTokens', label: 'Cache read', optional: true },
  { key: 'cacheWriteCostPerMillionTokens', label: 'Cache write', optional: true },
] as const;
export type PriceField = (typeof USAGE_PRICE_FIELDS)[number]['key'];
export type Price = { inputCostPerMillionTokens: number; outputCostPerMillionTokens: number; cacheReadCostPerMillionTokens?: number; cacheWriteCostPerMillionTokens?: number };
export type PriceForm = { model: string } & Record<PriceField, string>;

export function usagePriceForm(model = '', price?: Price): PriceForm {
  return {
    model,
    inputCostPerMillionTokens: price?.inputCostPerMillionTokens.toString() ?? '',
    outputCostPerMillionTokens: price?.outputCostPerMillionTokens.toString() ?? '',
    cacheReadCostPerMillionTokens: price?.cacheReadCostPerMillionTokens?.toString() ?? '',
    cacheWriteCostPerMillionTokens: price?.cacheWriteCostPerMillionTokens?.toString() ?? '',
  };
}
/** Blank cache prices use the input rate; explicit zero means free. */
export function parseUsagePriceForm(form: PriceForm): { model: string; price: Price } | null {
  const model = form.model.trim();
  if (!model) return null;
  const rates: Partial<Record<PriceField, number>> = {};
  for (const field of USAGE_PRICE_FIELDS) {
    const raw = form[field.key].trim();
    if (raw === '') { if (field.optional) continue; return null; }
    if (!/^(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?$/i.test(raw)) return null;
    const value = Number(raw);
    if (!Number.isFinite(value) || value < 0) return null;
    rates[field.key] = value;
  }
  if (rates.inputCostPerMillionTokens === undefined || rates.outputCostPerMillionTokens === undefined) return null;
  return { model, price: {
    inputCostPerMillionTokens: rates.inputCostPerMillionTokens, outputCostPerMillionTokens: rates.outputCostPerMillionTokens,
    ...(rates.cacheReadCostPerMillionTokens === undefined ? {} : { cacheReadCostPerMillionTokens: rates.cacheReadCostPerMillionTokens }),
    ...(rates.cacheWriteCostPerMillionTokens === undefined ? {} : { cacheWriteCostPerMillionTokens: rates.cacheWriteCostPerMillionTokens }),
  } };
}

export type Target = { environmentId: string; label: string; prices: Record<string, Price> | null; aliases: Record<string, string> | null; unavailable: string | null };
type Seed = { model: string; alias: string; values: Record<PriceField, string> };
export type Draft = { id: string; model: string; isNew: boolean; values: Partial<Record<PriceField, string>>; alias?: string; removed?: boolean; epoch: number; seed: Seed };
export type Change = { model: string; price: Price | null } | { model: string; alias: string | null };
type Result = { status: 'saved' } | { status: 'failed'; error: string } | { status: 'saving' };

export function isEmptyDraft(draft: Draft): boolean {
  return draft.isNew && draft.model.trim() === '' && (draft.alias ?? '').trim() === '' && Object.values(draft.values).every(value => value.trim() === '');
}
const hasOwn = (value: object, key: string) => Object.prototype.hasOwnProperty.call(value, key);
const own = <T>(record: Record<string, T> | null, key: string) => record && hasOwn(record, key) ? record[key] : undefined;

/** The "Map to" cell across the selected environments, read like a price cell. */
export function aliasCell(targets: Target[], model: string) {
  if (targets.some(target => target.aliases === null)) return { value: '', placeholder: 'Unavailable' };
  const values = targets.map(target => own(target.aliases, model) ?? '');
  if (values.some(value => value !== values[0])) return { value: '', placeholder: 'Mixed' };
  return { value: values[0] ?? '', placeholder: 'None' };
}
export function priceCell(targets: Target[], model: string, field: PriceField) {
  const optional = USAGE_PRICE_FIELDS.find(entry => entry.key === field)!.optional;
  const values = targets.map(target => own(target.prices, model) ? usagePriceForm(model, own(target.prices, model))[field] : null);
  if (targets.some(target => target.prices === null)) return { value: '', placeholder: 'Unavailable' };
  if (values.some(value => value !== values[0])) return { value: '', placeholder: 'Mixed' };
  return { value: values[0] ?? '', placeholder: values[0] === null ? 'Automatic' : optional ? 'Input rate' : '0.00' };
}

/** Only edited cells replace rates; untouched cells retain each environment's own values. */
export function tableChanges(target: Target, drafts: Draft[]): { changes: Change[]; errors: Map<string, string> } {
  const changes: Change[] = [], errors = new Map<string, string>();
  for (const draft of drafts) {
    if (isEmptyDraft(draft)) continue;
    const model = draft.model.trim(), currentAlias = own(target.aliases, model);
    if (draft.removed) {
      if (own(target.prices, model)) changes.push({ model, price: null });
      if (currentAlias !== undefined) changes.push({ model, alias: null });
      continue;
    }
    // A mapped model is priced as its target, so mapping it drops its own price.
    const alias = (draft.alias ?? currentAlias ?? '').trim();
    if (alias !== '') {
      if (model === '') errors.set(draft.id, 'Enter a model ID.');
      else if (alias === model) errors.set(draft.id, 'Map to a different model.');
      else {
        if (alias !== currentAlias) changes.push({ model, alias });
        if (own(target.prices, model)) changes.push({ model, price: null });
      }
      continue;
    }
    if (currentAlias !== undefined) {
      changes.push({ model, alias: null });
      // Clearing a mapping without entering prices returns to automatic pricing.
      if (!own(target.prices, model) && Object.values(draft.values).every(value => value.trim() === '')) continue;
    }
    const original = usagePriceForm(model, own(target.prices, model));
    const form = { ...original, ...draft.values };
    const parsed = parseUsagePriceForm(form);
    if (!parsed) {
      const missing = USAGE_PRICE_FIELDS.find(field => !field.optional && form[field.key].trim() === '');
      errors.set(draft.id, model === '' ? 'Enter a model ID.' : missing ? `${missing.label} is required on ${target.label}.` : 'Use non-negative numbers for prices.');
      continue;
    }
    const next = usagePriceForm(model, parsed.price);
    if (USAGE_PRICE_FIELDS.some(field => original[field.key] !== next[field.key])) changes.push(parsed);
  }
  return { changes, errors };
}
/** Unavailable destinations report a save failure without blocking writable environments. */
export function tableErrors(targets: Target[], drafts: Draft[]): Map<string, string> {
  return new Map(targets.filter(target => target.unavailable === null && target.prices !== null).flatMap(target => [...tableChanges(target, drafts).errors]));
}
/** One server.updateSettings patch per destination, with prices and/or mappings. */
export function settingsPatch(target: Target, changes: Change[]): { patch: Obj } | { error: string } | null {
  const prices = changes.flatMap(change => 'price' in change ? [[change.model, change.price] as const] : []);
  const aliases = changes.flatMap(change => 'alias' in change ? [[change.model, change.alias] as const] : []);
  if (target.unavailable !== null) return { error: target.unavailable };
  if (aliases.length > 0 && target.aliases === null) return { error: 'Update server to map models' };
  if (!changes.length) return null;
  return { patch: { ...(prices.length ? { usagePriceOverrides: Object.fromEntries(prices) } : {}), ...(aliases.length ? { usageModelAliases: Object.fromEntries(aliases) } : {}) } };
}

type State = {
  open: boolean; selectedOff: boolean; drafts: Draft[]; next: number; generation: number; pending: boolean;
  attempt: { drafts: Draft[]; destinations: { environmentId: string; label: string }[]; results: Map<string, Result> } | null;
};
const states = new WeakMap<object, State>();
const stateOf = (client: object): State => {
  let state = states.get(client);
  if (!state) { state = { open: false, selectedOff: false, drafts: [], next: 0, generation: 0, pending: false, attempt: null }; states.set(client, state); }
  return state;
};
export const pricesOpen = (client: object) => states.get(client)?.open === true;

/** The environments this client reads, as UsagePriceOverrides' priceTargetsAtom sees them. */
export function priceTargets(client: Pick<T3Client, 'environmentId' | 'config' | 'connection' | 'ready' | 'scopes'>): Target[] {
  if (!client.environmentId) return [];
  const environment = obj(client.config.environment), capabilities = obj(environment.capabilities);
  const settings = client.config.settings && typeof client.config.settings === 'object' ? obj(client.config.settings) : null;
  const record = <T>(value: unknown) => value && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, T> : {};
  return [{
    environmentId: client.environmentId, label: str(environment.label, 'This environment'),
    prices: settings ? record<Price>(settings.usagePriceOverrides) : null,
    aliases: capabilities.usageModelAliases === true && settings ? record<string>(settings.usageModelAliases) : null,
    unavailable: client.connection !== 'connected' || !client.ready ? 'Offline' : !settings ? 'Prices not loaded'
      : capabilities.usagePriceOverrides !== true ? 'Update server to edit prices'
      : client.scopes.length && !client.scopes.includes('orchestration:operate') ? 'Read-only access' : null,
  }];
}

const blankValues = (): Record<PriceField, string> => ({ inputCostPerMillionTokens: '', outputCostPerMillionTokens: '', cacheReadCostPerMillionTokens: '', cacheWriteCostPerMillionTokens: '' });
function savedSeed(selected: Target[], model: string): Seed {
  const values = blankValues();
  for (const field of USAGE_PRICE_FIELDS) values[field.key] = priceCell(selected, model, field.key).value;
  return { model, alias: aliasCell(selected, model).value, values };
}
const effective = (selected: Target[], draft: Draft, field: PriceField) => draft.values[field] ?? (draft.isNew ? '' : priceCell(selected, draft.model, field).value);
/** A seed follows the draft wherever its input is not on screen, so a remounted input shows the draft. */
function reseed(selected: Target[], draft: Draft): Draft {
  if (draft.removed) return draft;
  const alias = (draft.alias ?? aliasCell(selected, draft.model).value).trim();
  const values = { ...draft.seed.values };
  if (alias !== '') for (const field of USAGE_PRICE_FIELDS) values[field.key] = effective(selected, draft, field.key);
  return { ...draft, seed: { ...draft.seed, values } };
}

function selection(client: T3Client, state: State) {
  const environments = priceTargets(client);
  return { environments, selected: state.selectedOff ? [] : environments };
}
function customModels(selected: Target[]): string[] {
  return [...new Set(selected.flatMap(target => [...Object.keys(target.prices ?? {}), ...Object.keys(target.aliases ?? {})]))].sort();
}
function rowsOf(selected: Target[], state: State): Draft[] {
  const custom = customModels(selected), newModels = new Set(state.drafts.filter(draft => draft.isNew).map(draft => draft.model.trim()));
  return [
    // Keep new rows in place while successful environments publish their updated settings.
    ...custom.filter(model => state.attempt === null || !newModels.has(model)).map(model =>
      state.drafts.find(draft => draft.id === `model:${model}`) ?? { id: `model:${model}`, model, isNew: false, values: {}, epoch: 0, seed: savedSeed(selected, model) }),
    ...state.drafts.filter(draft => draft.isNew),
  ];
}
function staged(state: State) { return state.drafts.filter(draft => !isEmptyDraft(draft)); }
function errorsOf(selected: Target[], state: State): Map<string, string> {
  const drafts = staged(state), errors = tableErrors(selected, drafts), custom = customModels(selected);
  for (const draft of drafts) {
    if (!draft.isNew) continue;
    if (draft.model.trim() === '') errors.set(draft.id, 'Enter a model ID.');
    else if (state.attempt === null && (custom.includes(draft.model.trim()) || state.drafts.some(other => other.id !== draft.id && other.model.trim() === draft.model.trim())))
      errors.set(draft.id, 'This model already has a row. Edit its prices there.');
  }
  return errors;
}
const failedOf = (state: State) => state.attempt?.destinations.filter(destination => state.attempt!.results.get(destination.environmentId)?.status === 'failed') ?? [];

export function emptyPrices() {
  return {
    open: false, generation: 0, menuWidth: 160, allChecked: true, selectionLabel: 'All environments', targets: [] as { key: string; label: string; checked: boolean; unavailable: string }[],
    selectionNotice: '', rows: [] as PriceRowView[], emptyRows: '', footnote: '', statuses: [] as { key: string; label: string; status: string; failed: boolean }[],
    changesLabel: '', hasChanges: false, discardLabel: 'Discard changes', saveLabel: 'Save changes', saveDisabled: true, locked: false, pending: false,
  };
}
export type PriceRowView = {
  id: string; key: string; model: string; isNew: boolean; removed: boolean; modelValue: string; invalid: boolean; aliasValue: string; aliasPlaceholder: string; aliasLabel: string;
  countedAs: string; cells: { key: string; value: string; placeholder: string; ariaLabel: string }[]; error: string; resetLabel: string; resetTip: string; resetIcon: string;
};
export type PricesView = ReturnType<typeof emptyPrices>;

/** Everything the dialog renders for this client's drafts and the selected environments. */
export function presentPrices(client: T3Client): PricesView {
  const view = emptyPrices(), state = states.get(client);
  if (!state?.open) return view;
  const { environments, selected } = selection(client, state);
  const errors = errorsOf(selected, state), failed = failedOf(state), hasChanges = staged(state).length > 0;
  const destinationLabel = selected.length === 1 ? selected[0]!.label : `${selected.length} environments`;
  Object.assign(view, {
    open: true, generation: state.generation, allChecked: !state.selectedOff, selectionLabel: state.selectedOff ? destinationLabel : 'All environments',
    targets: environments.map(target => ({ key: target.environmentId, label: target.label, checked: !state.selectedOff, unavailable: target.unavailable ?? '' })),
    selectionNotice: selected.length ? '' : environments.length ? 'Select an environment to see and change its model prices.' : 'Connect an environment to set model prices.',
    footnote: `Blank cache rates use the input price. Enter 0 for free tokens. A mapped model’s usage counts as the model it maps to.${selected.length > 1 ? ' Mixed cells keep each environment’s rate until you edit them.' : ''}`,
    hasChanges, changesLabel: hasChanges ? `Changes apply to ${destinationLabel}` : '', pending: state.pending, locked: state.pending || failed.length > 0,
    discardLabel: failed.length ? 'Discard pending changes' : 'Discard changes',
    saveLabel: state.pending ? 'Saving…' : failed.length ? 'Retry failed saves' : 'Save changes',
    saveDisabled: state.pending || (!hasChanges && !failed.length) || (!failed.length && (errors.size > 0 || selected.length === 0)),
    statuses: (state.attempt?.destinations ?? []).map(destination => {
      const result = state.attempt!.results.get(destination.environmentId);
      return { key: destination.environmentId, label: destination.label, failed: result?.status === 'failed',
        status: result?.status === 'failed' ? `Not saved · ${result.error}` : result?.status === 'saved' ? 'Saved' : 'Saving…' };
    }),
  });
  const rows = rowsOf(selected, state);
  view.emptyRows = rows.length ? '' : selected.some(target => target.prices === null) ? 'Some environment prices are unavailable.' : 'No custom prices or mappings. Add a row to set one.';
  view.rows = rows.map(row => {
    const cell = aliasCell(selected, row.model), alias = (row.alias ?? cell.value).trim(), name = row.model || 'new model';
    const shown = errors.has(row.id) && (row.model.trim() !== '' || Object.values(row.values).some(value => value !== ''));
    return {
      id: row.id, key: `${row.id}:${state.generation}:${row.epoch}`, model: row.model, isNew: row.isNew, removed: row.removed === true,
      modelValue: row.seed.model, invalid: row.isNew && row.model.trim() !== '' && errors.has(row.id),
      aliasValue: row.seed.alias, aliasPlaceholder: row.isNew ? 'Optional' : cell.placeholder, aliasLabel: `Map ${name} to model`, countedAs: alias,
      cells: USAGE_PRICE_FIELDS.map(field => {
        const price = priceCell(selected, row.model, field.key);
        return { key: field.key, value: row.seed.values[field.key], placeholder: row.isNew ? (field.optional ? 'Input rate' : '0.00') : price.placeholder, ariaLabel: `${field.label} price for ${name}` };
      }),
      error: shown ? errors.get(row.id)! : '',
      resetLabel: row.removed ? `Undo reset for ${row.model}` : row.isNew ? 'Remove new model' : `Reset price for ${row.model} to automatic`,
      resetTip: row.removed ? 'Undo reset' : row.isNew ? 'Remove row' : 'Reset to automatic', resetIcon: row.isNew ? 'x' : 'rotate-ccw',
    };
  });
  return view;
}

const FIELDS = new Set<string>(['model', 'alias', ...USAGE_PRICE_FIELDS.map(field => field.key)]);
/**
 * pages:usage-prices-* — open (value: a model to prefill), close, env, add,
 * edit (id `row:field`), reset, discard. save runs from usagePricesSave.
 */
export function usagePricesLocal(client: T3Client, op: string, id: string, value: string): string {
  const state = stateOf(client);
  if (op === 'open') {
    const { selected } = selection(client, state);
    Object.assign(state, { open: true, drafts: [], attempt: null, pending: false, generation: state.generation + 1 });
    // A model that already has a custom price or mapping is edited in its existing row.
    const model = value.trim();
    if (model && !selected.some(target => hasOwn(target.prices ?? {}, model) || hasOwn(target.aliases ?? {}, model)))
      state.drafts = [{ id: 'new:initial', model, isNew: true, values: {}, epoch: 0, seed: { model, alias: '', values: blankValues() } }];
    return '';
  }
  if (!state.open) return '';
  if (op === 'close') { if (!state.pending) Object.assign(state, { open: false, drafts: [], attempt: null }); return ''; }
  const { selected } = selection(client, state), failed = failedOf(state), locked = state.pending || failed.length > 0;
  if (op === 'env') {
    if (state.pending || staged(state).length) return '';
    state.selectedOff = !state.selectedOff; state.attempt = null; return '';
  }
  if (op === 'discard') { if (!state.pending) Object.assign(state, { drafts: [], attempt: null, generation: state.generation + 1 }); return ''; }
  if (locked) return '';
  const rows = rowsOf(selected, state);
  const update = (draft: Draft) => {
    state.drafts = state.drafts.some(entry => entry.id === draft.id) ? state.drafts.map(entry => entry.id === draft.id ? draft : entry) : [...state.drafts, draft];
    state.attempt = null;
  };
  // An existing row edited back to its saved values has nothing left to save.
  const updateOrDrop = (draft: Draft) => {
    if (!draft.isNew && Object.keys(draft.values).length === 0 && draft.alias === undefined && !draft.removed) { state.drafts = state.drafts.filter(entry => entry.id !== draft.id); state.attempt = null; }
    else update(reseed(selected, draft));
  };
  if (op === 'add') {
    update({ id: `new:${state.next++}`, model: '', isNew: true, values: {}, epoch: 0, seed: { model: '', alias: '', values: blankValues() } });
    return '';
  }
  // edit ids are `row:field`; field names hold no colon, row ids (and model ids) may.
  const rowId = op === 'edit' ? id.slice(0, Math.max(0, id.lastIndexOf(':'))) : id;
  const row = rows.find(entry => entry.id === rowId);
  if (!row) throw new ClientError('That price row is no longer available.');
  if (op === 'reset') {
    if (row.isNew) { state.drafts = state.drafts.filter(entry => entry.id !== row.id); state.attempt = null; }
    else if (row.removed) {
      const values = { ...row.seed.values };
      for (const field of USAGE_PRICE_FIELDS) values[field.key] = effective(selected, row, field.key);
      updateOrDrop({ ...row, removed: false, epoch: row.epoch + 1, seed: { ...row.seed, alias: row.alias ?? aliasCell(selected, row.model).value, values } });
    } else update({ ...row, removed: true });
    return '';
  }
  if (op === 'edit') {
    const field = id.slice(id.lastIndexOf(':') + 1);
    if (!FIELDS.has(field)) throw new ClientError('Unknown price field.');
    const matches = (cell: { value: string; placeholder: string }) => !row.isNew && cell.placeholder !== 'Mixed' && cell.placeholder !== 'Unavailable' && value === cell.value;
    if (field === 'model') { if (row.isNew) update(reseed(selected, { ...row, model: value })); return ''; }
    if (field === 'alias') {
      const { alias: _alias, ...rest } = row;
      updateOrDrop(matches(aliasCell(selected, row.model)) ? rest : { ...rest, alias: value });
      return '';
    }
    const values = { ...row.values, [field]: value } as Draft['values'];
    if (matches(priceCell(selected, row.model, field as PriceField))) delete values[field as PriceField];
    updateOrDrop({ ...row, values });
    return '';
  }
  throw new ClientError(`Unknown price action: ${op}`);
}

/** Save changes (or Retry failed saves): each destination settles on its own. */
export async function usagePricesSave(client: T3Client, native: Native, forget: () => void): Promise<string> {
  const state = stateOf(client);
  if (!state.open || state.pending) return '';
  const { environments, selected } = selection(client, state), failed = failedOf(state);
  const retry = failed.length > 0;
  const destinations = retry ? failed : selected, edits = retry && state.attempt ? state.attempt.drafts : staged(state);
  if (!destinations.length || !edits.length) return '';
  const targets: Target[] = destinations.map(destination => environments.find(target => target.environmentId === destination.environmentId)
    ?? { environmentId: destination.environmentId, label: destination.label, prices: null, aliases: null, unavailable: 'Environment removed' });
  const previous = state.attempt;
  state.pending = true;
  state.attempt = { drafts: edits, destinations: retry && previous ? previous.destinations : targets.map(({ environmentId, label }) => ({ environmentId, label })),
    results: new Map([...(retry && previous ? previous.results : []), ...targets.map(target => [target.environmentId, { status: 'saving' } as Result] as const)]) };
  let anyFailed = false;
  try {
    for (const target of targets) {
      const plan = tableChanges(target, edits);
      const firstError = [...plan.errors.values()][0];
      const patch = settingsPatch({ ...target, unavailable: target.unavailable ?? firstError ?? null }, plan.changes);
      let result: Result = { status: 'saved' };
      if (patch && 'error' in patch) result = { status: 'failed', error: patch.error };
      else if (patch) {
        try {
          const saved = await client.rpc(native, 'server.updateSettings', patch, true);
          // The reply is the updated ServerSettings; settingsUpdated follows on the config stream.
          const current = obj(client.config.settings);
          client.config = { ...client.config, settings: hasOwn(saved, 'usagePriceOverrides') ? saved : { ...current, ...applyPatch(current, patch.patch) } };
        } catch (error) { if (letGo(error)) throw error; result = { status: 'failed', error: 'Could not save. Try again.' }; }
      }
      if (result.status === 'failed') anyFailed = true;
      state.attempt.results.set(target.environmentId, result);
    }
  } finally { state.pending = false; }
  if (!anyFailed) { state.drafts = []; state.generation++; }
  forget();
  return '';
}
/** ServerSettingsPatch semantics for the two records: null removes a key. */
export function applyPatch(settings: Obj, patch: Obj): Obj {
  const out: Obj = {};
  for (const key of ['usagePriceOverrides', 'usageModelAliases']) {
    if (!patch[key]) continue;
    const next: Obj = { ...obj(settings[key]) };
    for (const [model, value] of Object.entries(obj(patch[key]))) { if (value === null) delete next[model]; else next[model] = value; }
    out[key] = next;
  }
  return out;
}
