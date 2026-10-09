// The Diff panel's comparison target, adapted from T3 Code 1e2ecbd975 (MIT; see LICENSE-T3):
// apps/web/src/lib/baseRefChoices.ts (buildBaseRefChoices, filterBaseRefChoices),
// diffPanelStore.ts (normalizeBaseRef) and components/DiffPanel.tsx (the Changes header's
// "<head> → <base>" label and its Combobox: two vcs.listRefs reads, local and remote, with
// includeMatchingRemoteRefs, the query and limit 100; the head left out of the local refs;
// Automatic first; a ref with both sides carries the "Use remote version of" switch, a remote
// ref alone the "Remote only" mark; "No matching refs." when the query matches nothing).
// The selection itself lives in diff.ts; the commands are client-ops-diff.ts.
import { arr, str, type Obj } from './domain';

export type VcsRef = { name: string; remoteName: string | null };
export type BaseRefChoice = { id: string; label: string; local: VcsRef | null; remote: VcsRef | null };
/** The picker's last answers: the query they were asked for and the preview cwd they came from. */
export type BaseRefPicker = { query: string; cwd: string; local: VcsRef[]; remote: VcsRef[]; loading: boolean };
export const AUTOMATIC_BASE_REF = '__automatic_base_ref__';

export function emptyBaseRefPicker(): BaseRefPicker { return { query: '', cwd: '', local: [], remote: [], loading: false }; }

/** diffPanelStore normalizeBaseRef: a blank ref is Automatic (null). */
export function normalizeBaseRef(baseRef: string | null | undefined): string | null {
  const normalized = baseRef?.trim();
  return normalized ? normalized : null;
}

function remoteBranchName(ref: VcsRef): string {
  if (ref.remoteName && ref.name.startsWith(`${ref.remoteName}/`)) return ref.name.slice(ref.remoteName.length + 1);
  return ref.name;
}

/** buildBaseRefChoices: each local ref with its remote of the same branch (origin first), then the remote refs left. */
export function buildBaseRefChoices(localRefs: readonly VcsRef[], remoteRefs: readonly VcsRef[]): BaseRefChoice[] {
  const unused = new Set(remoteRefs);
  const paired = localRefs.map(local => {
    const matches = remoteRefs.filter(remote => unused.has(remote) && remoteBranchName(remote) === local.name);
    const remote = matches.find(candidate => candidate.remoteName === 'origin') ?? matches[0] ?? null;
    if (remote) unused.delete(remote);
    return { id: `local:${local.name}`, label: local.name, local, remote };
  });
  const remoteOnly = remoteRefs.filter(remote => unused.has(remote)).map(remote => ({ id: `remote:${remote.name}`, label: remote.name, local: null, remote }));
  return [...paired, ...remoteOnly];
}

/** filterBaseRefChoices: case-insensitive substring over the label and both names. */
export function filterBaseRefChoices(choices: readonly BaseRefChoice[], query: string): BaseRefChoice[] {
  const normalized = query.trim().toLocaleLowerCase();
  if (!normalized) return [...choices];
  return choices.filter(choice => choice.label.toLocaleLowerCase().includes(normalized)
    || choice.local?.name.toLocaleLowerCase().includes(normalized) === true || choice.remote?.name.toLocaleLowerCase().includes(normalized) === true);
}

/** One vcs.listRefs read of the picker (DiffPanel localBranchRefs / remoteBranchRefs). */
export function listRefsPayload(cwd: string, refKind: 'local' | 'remote', query: string): Obj {
  return { cwd, includeMatchingRemoteRefs: true, refKind, ...(query.trim() ? { query: query.trim() } : {}), limit: 100 };
}
export function refsOf(result: Obj): VcsRef[] {
  return arr(result.refs).map(ref => ({ name: str(ref.name), remoteName: typeof ref.remoteName === 'string' ? ref.remoteName : null })).filter(ref => ref.name !== '');
}

/** Reads both lists for the picker's query; an answer for an older query or cwd is dropped, a failed read lists nothing. */
export async function loadBaseRefs(picker: BaseRefPicker, cwd: string, send: (method: string, payload: Obj) => Promise<Obj>): Promise<void> {
  const query = picker.query;
  picker.loading = true;
  const read = (kind: 'local' | 'remote') => send('vcs.listRefs', listRefsPayload(cwd, kind, query)).then(refsOf, () => [] as VcsRef[]);
  const [local, remote] = await Promise.all([read('local'), read('remote')]);
  if (picker.query !== query) return;
  Object.assign(picker, { cwd, local, remote, loading: false });
}

export type BaseRefRow = { id: string; label: string; value: string; remote: string; remoteOn: boolean; other: string; selected: boolean };
/**
 * The Combobox's rows: the choices that match the query (the server filtered its lists by it
 * too; the client's filter keeps the rows right while a newer query's answer is out), each
 * item's value (the remote name while the remote side is the selected base), the remote column
 * and the ref its switch picks; Automatic's selection (its row always shows, as the reference's
 * static item does); "No matching refs." when the query matches nothing (Automatic counts only
 * with no query).
 */
export function baseRefView(picker: BaseRefPicker, cwd: string, headRef: string | null, selectedBaseRef: string | null): { rows: BaseRefRow[]; automatic: boolean; empty: boolean } {
  const current = picker.cwd === cwd;
  const choices = current ? buildBaseRefChoices(picker.local.filter(ref => ref.name !== headRef), picker.remote) : [];
  const matching = filterBaseRefChoices(choices, picker.query);
  const valueFor = (choice: BaseRefChoice) => selectedBaseRef && selectedBaseRef === choice.remote?.name ? selectedBaseRef : choice.local?.name ?? choice.remote?.name ?? choice.id;
  const rows = matching.map(choice => {
    const value = valueFor(choice), both = choice.local !== null && choice.remote !== null, remoteOn = choice.remote?.name === value;
    return { id: choice.id, label: choice.label, value, remote: both ? 'switch' : choice.remote ? 'only' : '', remoteOn: both && remoteOn,
      other: both ? (remoteOn ? choice.local!.name : choice.remote!.name) : '', selected: value === selectedBaseRef };
  });
  const filtered = (picker.query.trim() ? 0 : 1) + matching.length;
  return { rows, automatic: selectedBaseRef === null, empty: filtered === 0 };
}
