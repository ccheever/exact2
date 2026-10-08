// @ref llp/1109.009-mobile-settings.decision.md#scoped-server-settings
// Pinned365aa87982 SettingsProjectOverviewRouteScreen; existing shared shell/group-label/transport owners.
import { mobileClient } from './client';
import { mobileHomeSources } from './home';
import { obj, str, applyShell, initialShell, type Obj } from './shared/domain';
import { fleet } from './shared/settings-b-fleet';
import { groupLabel } from './shared/r6-polish-groups';
import { projectIdentity, ICON_COLORS } from './shared/settings-b-icons';
import { faviconFromAnswer } from './shared/r3-sidebar-glyph';
import { ClientError, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';
import { decodeMobileServerScope, type MobileServerScope } from './settings-server';
import { scheduledGrant } from './settings-scheduled';
import { settingsSources, settingsNative, settingsEndpoint, settingsCall, settingsEndpointCurrent, type MobileSettingsSource, type MobileSettingsEndpoint } from './settings-server-source';

export interface MobileProjectMember { environmentId: string; project: Obj }
export function mobileProjectMembers(scope: MobileServerScope, sources: MobileSettingsSource[], shells: { environmentId: string; shell: { projects: Obj[] } }[]): MobileProjectMember[] {
  if (scope.members === null) return [];
  const allowed = new Set(sources.filter(source => scope.environmentIds.includes(source.environmentId) && source.enabled && source.phase === 'connected').map(source => source.environmentId));
  return scope.members.flatMap(member => {
    const project = allowed.has(member.environmentId) ? shells.find(shell => shell.environmentId === member.environmentId)?.shell.projects.find(project => project.id === member.id) : undefined;
    return project ? [{environmentId: member.environmentId, project}] : [];
  });
}
export function mobileProjectIdentity(members: MobileProjectMember[]) { return JSON.stringify(members.map(member => [member.environmentId, member.project.id, member.project.workspaceRoot])); }
export function mobileProjectGlyph(project: Obj | undefined, favicon: string) {
  const icon = obj(project?.projectIcon), title = str(project?.title), kind = str(icon.kind);
  const text = kind === 'lucide' ? projectIdentity(title).monogram : str(icon.text);
  const ink = ICON_COLORS.find(color => color.value === icon.color)?.swatch ?? '#6a7282';
  return {kind: kind === 'emoji' ? 'emoji' : kind === 'monogram' || kind === 'lucide' ? 'monogram' : favicon ? 'image' : 'folder',
    text: kind === 'emoji' ? str(icon.emoji) : text, color: ink, background: `${ink}26`, src: kind ? '' : favicon,
    fontSize: kind === 'emoji' ? 38.4 : 48 * (Array.from(text.replace(/\p{M}/gu, '')).length === 1 ? 0.6 : 0.515625)};
}
export function mobileProjectProjection(scope: MobileServerScope, sources: MobileSettingsSource[], members: MobileProjectMember[], writable: Set<string>, favicon = '', error = '') {
  const name = groupLabel(members.map(member => member.project));
  return {title: 'Project overview', scopeJSON: JSON.stringify(scope), identity: mobileProjectIdentity(members), error, name,
    empty: members.length === 0, emptyMessage: 'This project has no checkout on the selected connected environments. Change the filter above.',
    checkoutsLabel: members.length === 1 ? '1 checkout' : `${members.length} checkouts`,
    writable: members.length > 0 && members.every(member => writable.has(member.environmentId)), icon: mobileProjectGlyph(members[0]?.project, favicon),
    checkouts: members.map((member, index) => { const source = sources.find(source => source.environmentId === member.environmentId); return {
      id: `${member.environmentId}:${str(member.project.id)}`, separated: index > 0, label: source?.label || 'Environment', displayURL: source?.origin ? `${source.origin.replace(/\/+$/, '')}/` : '', workspaceRoot: str(member.project.workspaceRoot)}; })};
}
async function prepare(scope: MobileServerScope, native: Native, fresh: boolean) {
  const sources = (await settingsSources(native)).filter(source => scope.environmentIds.includes(source.environmentId) && source.enabled && source.phase === 'connected');
  const endpoints = sources.map(source => settingsEndpoint(source, native)), writable = new Set<string>();
  const shells = fresh ? await Promise.all(endpoints.map(async endpoint => ({environmentId: endpoint.source.environmentId,
    shell: applyShell(initialShell(), await settingsCall(endpoint, {op: 'http', path: '/api/orchestration/shell'}))}))) : mobileHomeSources();
  const members = mobileProjectMembers(scope, sources, shells);
  await Promise.all(endpoints.filter(endpoint => members.some(member => member.environmentId === endpoint.source.environmentId)).map(async endpoint => {
    if (scheduledGrant(await settingsCall(endpoint, {op: 'http', path: '/api/auth/session'}))) writable.add(endpoint.source.environmentId);
  }));
  return {sources, endpoints, shells, members, writable};
}
export async function mobileProjectOverview(scopeJSON: string, nativeInput?: Native | null) {
  const scope = decodeMobileServerScope(scopeJSON);
  if (!nativeInput?.available) return mobileProjectProjection(scope, [], [], new Set(), '', 'Open T3 Code on your iPhone or iPad to manage projects.');
  try {
    const native = settingsNative(nativeInput), {sources, endpoints, members, writable} = await prepare(scope, native, false), first = members[0];
    let favicon = '';
    if (first && !obj(first.project.projectIcon).kind && str(first.project.workspaceRoot)) {
      const endpoint = endpoints.find(endpoint => endpoint.source.environmentId === first.environmentId)!;
      try { favicon = faviconFromAnswer(endpoint.source.origin, await settingsCall(endpoint, {op: 'request', method: 'assets.createUrl', payload: {resource: {_tag: 'project-favicon', cwd: first.project.workspaceRoot, ...(first.project.faviconPath ? {path: first.project.faviconPath} : {})}}})); }
      catch (error) { if (letGo(error)) throw error; /* Source shows folder fallback for failed favicons. */ }
    }
    return mobileProjectProjection(scope, sources, members, writable, favicon);
  } catch (error) { if (letGo(error)) throw error; return mobileProjectProjection(scope, [], [], new Set(), '', error instanceof Error ? error.message : 'Could not load project overview.'); }
}
let saving = false;
function adoptShell(endpoint: MobileSettingsEndpoint, shell: ReturnType<typeof initialShell>) {
  if (!settingsEndpointCurrent(endpoint)) return;
  if (endpoint.source.focused) mobileClient.shell = shell;
  else { const entry = fleet.entries.get(endpoint.source.key); if (entry) {entry.shell = shell; fleet.revision++;} }
}
/** Captured members are revalidated before any write; a narrower scope needs a new save. */
export async function mobileProjectRename(scopeJSON: string, identity: string, raw: string, nativeInput?: Native | null) {
  if (saving) return {revision: mobileClient.revision, message: 'A project name update is already in progress.', saved: false};
  saving = true;
  try {
    const name = raw.trim(); if (!name) throw new ClientError('Project title cannot be empty.');
    if (!nativeInput?.available) throw new ClientError('Open T3 Code on your iPhone or iPad to manage projects.');
    const scope = decodeMobileServerScope(scopeJSON), native = settingsNative(nativeInput);
    const {sources, endpoints, members, writable} = await prepare(scope, native, true);
    if (!members.length || mobileProjectIdentity(members) !== identity) throw new ClientError('The selected project checkouts changed. Reopen this page.');
    if (members.some(member => !writable.has(member.environmentId))) throw new ClientError('The selected environments do not allow project changes.');
    if (name === groupLabel(members.map(member => member.project))) return {revision: mobileClient.revision, message: '', saved: false};
    const results = await Promise.allSettled(members.map(async member => {
      const endpoint = endpoints.find(endpoint => endpoint.source.environmentId === member.environmentId)!;
      const [commandId] = await mobileClient.ids(endpoint.remote, 1);
      await settingsCall(endpoint, {op: 'request', method: 'projects.mutate', payload: {type: 'project.update', commandId, projectId: member.project.id, title: name}});
    }));
    for (const result of results) if (result.status === 'rejected' && letGo(result.reason)) throw result.reason;
    const failures = results.flatMap((result, index) => result.status === 'rejected' ? [`${sources.find(source => source.environmentId === members[index]!.environmentId)?.label}: ${result.reason instanceof Error ? result.reason.message : 'Not saved.'}`] : []);
    // Refresh actual shells after acknowledgments, including partial failures; never synthesize titles.
    const refreshed = await Promise.allSettled(endpoints.map(async endpoint => adoptShell(endpoint, applyShell(initialShell(), await settingsCall(endpoint, {op: 'http', path: '/api/orchestration/shell'})))));
    for (const result of refreshed) if (result.status === 'rejected') { if (letGo(result.reason)) throw result.reason; failures.push(`Could not refresh project names: ${result.reason instanceof Error ? result.reason.message : 'Connection unavailable.'}`); }
    if (failures.length) throw new ClientError(`${results.some(result => result.status === 'fulfilled') ? 'Some checkouts may have been renamed. ' : ''}${failures.join('\n')}`);
    return {revision: ++mobileClient.revision, message: '', saved: true};
  } catch (error) { if (letGo(error)) throw error; return {revision: ++mobileClient.revision, message: error instanceof Error ? error.message : 'Could not rename project.', saved: false}; }
  finally {saving = false;}
}
