// Settings › Devices › Device sources (MIT T3 Code 5d3d78ab93, see LICENSE-T3:
// components/settings/DeviceSourcesSettings.tsx). Every source the selected environments can open,
// one row each: this machine's simulators, each SSH host, each EAS host. A source configured on
// several environments is one row, and its statuses and platforms say how it stands on each.
// Simlock pools show only with Labs' `simlockDeviceHostsEnabled` on, which this clone leaves off.
// exact2: the rows' commands are the hosts modules' (`hosts-*`, `eas-*`), each naming the
// environment it acts on when the row belongs to one.
import { arr, obj, str, type Obj } from '../client/domain';
import { connectionKey, groupDeviceHosts, hostsOf, hostTesting, type HostConfig, type HostTarget } from './settings-a-hosts';
import { easHostsOf, easSessionLimits, type EasHost } from './settings-eas-hosts';

type Tone = 'pending' | 'warning' | 'error';
type Status = { tone: Tone; label: string; detail: string; action: { label: string; op: string; params: string } | null };
type Member = { environmentId: string; label: string; settings: Obj; state: Obj | null };
type SourceEnvironment = { environmentId: string; label: string; added: boolean; status: Status | null; platforms: Obj[] | null };

export type SourceLine = { id: string; tone: string; text: string; action: string; op: string; params: string };
export type SourceEntry = { id: string; label: string; detail: string };
export type SourceGroup = { heading: string; entries: SourceEntry[] };
export type SourceCell = { id: string; label: string; value: string; muted: boolean };
export type SourcePlatform = { id: string; aria: string; available: boolean; partial: boolean; title: string; reason: string; groups: SourceGroup[]; grid: SourceCell[] };
export type SourceRow = { key: string; title: string; kind: string; description: string; problems: SourceLine[]; pending: string; platforms: SourcePlatform[];
  actions: boolean; edit: string; editParams: string; remove: string; removeParams: string; test: boolean; menuId: string };
/** `disabled` turns off every row's Edit and More (a project scope, or no connected environment); `canAdd` also waits for an open editor. */
export type DeviceSourcesView = { canAdd: boolean; disabled: boolean; eas: boolean; rows: SourceRow[] };

const status = (tone: Tone, label: string, detail = ''): Status => ({ tone, label, detail, action: null });
/** What a configured host is doing on one environment, when it is not simply working. */
function hostStatus(state: Obj | null, hostId: string): Status | null {
  if (!state) return status('pending', 'Loading');
  const entry = obj(obj(state.hostStatuses)[hostId]);
  switch (str(entry.status)) {
    case 'ready': return entry.androidDiscoveryIncomplete === true ? status('warning', 'Android list may be incomplete', str(entry.detail)) : null;
    case 'installing': case 'starting': return status('pending', 'Starting', str(entry.detail));
    case 'failed': return status('error', 'Failed', str(entry.detail));
    default: return null;
  }
}
const hostPlatforms = (state: Obj | null, hostId: string): Obj[] | null => {
  const host = arr(state?.hosts).find(entry => str(entry.id) === hostId);
  return host && Array.isArray(host.platforms) ? arr(host.platforms) : null;
};
const params = (entries: Record<string, string>) => Object.entries(entries).filter(([, value]) => value !== '').map(([key, value]) => `${key}=${encodeURIComponent(value)}`).join('&');

const PLATFORMS = [{ platform: 'ios', label: 'iOS' }, { platform: 'android', label: 'Android' }] as const;
/** The platforms a source offers: one icon each, solid where available, muted where not, and its popover's lines. */
function sourcePlatforms(title: string, shared: boolean, environments: SourceEnvironment[]): SourcePlatform[] {
  const known = environments.filter(environment => environment.added && environment.platforms !== null);
  const several = environments.length > 1;
  return PLATFORMS.filter(({ platform }) => known.some(environment => environment.platforms!.some(entry => str(entry.platform) === platform))).map(({ platform, label }) => {
    const availability = (environment: SourceEnvironment) => environment.platforms?.find(entry => str(entry.platform) === platform);
    const availableOn = known.filter(environment => availability(environment)?.available === true);
    const available = availableOn.length > 0, partial = available && availableOn.length < known.length;
    // A shared source usually answers the same everywhere, so its answer is given once.
    const sharedAnswer = known.map(availability).find(Boolean);
    const uniform = known.every(environment => (availability(environment)?.available === true) === (sharedAnswer?.available === true) && str(availability(environment)?.reason) === str(sharedAnswer?.reason));
    const entries = (list: SourceEnvironment[], reasons: boolean) => list.map(environment => ({ id: environment.environmentId, label: environment.label, detail: reasons ? str(availability(environment)?.reason) : '' }));
    const groups: SourceGroup[] = shared && several ? [
      ...(uniform ? [{ heading: 'Connected', entries: entries(environments.filter(environment => environment.added), false) }]
        : [{ heading: 'Available', entries: entries(availableOn, false) }, { heading: 'Not available', entries: entries(known.filter(environment => availability(environment)?.available !== true), true) }]),
      { heading: 'Not added', entries: entries(environments.filter(environment => !environment.added), false) },
    ].filter(group => group.entries.length > 0) : [];
    const grid: SourceCell[] = !shared && several ? known.map(environment => {
      const entry = availability(environment);
      return { id: environment.environmentId, label: environment.label, value: entry?.available === true ? 'Available' : str(entry?.reason) || 'Not available', muted: entry?.available !== true };
    }) : [];
    const reason = shared ? (uniform && !available ? str(sharedAnswer?.reason) : '')
      : !several && !available ? str(availability(known[0]!)?.reason) || 'Not set up on this machine.' : '';
    return { id: platform, aria: `${title}: ${label} ${available ? 'available' : 'not available'}`, available, partial,
      title: `${label} ${available ? (partial ? 'available on some' : 'available') : 'not available'}`, reason, groups, grid };
  });
}

function row(key: string, title: string, kind: string, description: string, shared: boolean, environments: SourceEnvironment[], actions: Partial<SourceRow> = {}): SourceRow {
  const several = environments.length > 1;
  const issues = environments.filter(environment => environment.status !== null);
  const problems = issues.filter(environment => environment.status!.tone !== 'pending').map(environment => {
    const { tone, label, detail, action } = environment.status!;
    return { id: environment.environmentId, tone, text: `${several ? `${environment.label}: ` : ''}${!several && detail ? detail : label}`,
      action: action?.label ?? '', op: action?.op ?? '', params: action?.params ?? '' };
  });
  return { key, title, kind, description, problems, pending: issues.find(environment => environment.status!.tone === 'pending')?.status!.label ?? '',
    platforms: sourcePlatforms(title, shared, environments), actions: false, edit: '', editParams: '', remove: '', removeParams: '', test: false,
    menuId: `device-source-menu-${key.replace(/[^a-zA-Z0-9_-]/g, '-')}`, ...actions };
}

/**
 * The section's rows over the scope's connected targets. `targets` are the hosts module's (its
 * syncHostTargets), `primary` the environment Edit checks an EAS account on when it holds the host,
 * `supportsEas` the primary's device state.
 */
export function deviceSourcesView(targets: HostTarget[], primary: string, supportsEas: boolean, projectScope: boolean, editing: boolean): DeviceSourcesView {
  const labelOf = (target: HostTarget) => target.secondary ? `${target.label} · ${target.secondary}` : target.label;
  const members: Member[] = targets.map(target => ({ environmentId: target.client.environmentId, label: labelOf(target), settings: obj(target.client.config.settings), state: target.state }));
  // Environments with devices off are left out: the switch above already says so.
  const active = members.filter(member => member.settings.enableDeviceSupport === true);
  const disabled = projectScope || members.length === 0;
  const rows: SourceRow[] = [];

  rows.push(row('local', 'Local simulators', '', members.length > 1 ? "Simulators and emulators installed on each environment's own machine." : "Simulators and emulators installed on this environment's machine.", false,
    active.map(member => {
      const local = arr(member.state?.hosts).find(host => str(host.kind) === 'local');
      const found = hostStatus(member.state, local ? str(local.id) : '');
      return { environmentId: member.environmentId, label: member.label, added: true, platforms: local && Array.isArray(local.platforms) ? arr(local.platforms) : null,
        status: found?.tone === 'error' ? { ...found, action: { label: 'Refresh', op: 'hosts-refresh', params: params({ environment: member.environmentId }) } } : found };
    })));

  const hostsByMember = members.map(member => ({ environmentId: member.environmentId, label: member.label, hosts: hostsOf(member.settings) }));
  const grouped = groupDeviceHosts(hostsByMember);
  // Hosts that cannot be matched across environments stay one row per environment.
  const sshGroups = grouped ?? hostsByMember.flatMap(member => member.hosts.map(host => ({ host, members: [{ environmentId: member.environmentId, label: member.label, host }] })));
  for (const group of sshGroups) {
    const host: HostConfig = group.host;
    const only = grouped ? '' : group.members[0]!.environmentId;
    const testing = targets.some(target => hostTesting(target.client, connectionKey(host)));
    const hostParams = params({ id: host.id, environment: only });
    rows.push(row(`ssh:${host.id}:${group.members.map(member => member.environmentId).join(',')}`, host.label, 'SSH', host.target, true, active.map(member => {
      const held = group.members.find(candidate => candidate.environmentId === member.environmentId);
      if (!held) return { environmentId: member.environmentId, label: member.label, added: false, status: null, platforms: null };
      const found = testing ? status('pending', 'Testing connection') : hostStatus(member.state, held.host.id);
      return { environmentId: member.environmentId, label: member.label, added: true, platforms: hostPlatforms(member.state, held.host.id),
        status: found?.tone === 'error' && member.state?.supportsHostRetry === true
          ? { ...found, label: "Can't connect", action: { label: 'Retry', op: 'hosts-retry', params: params({ id: held.host.id, environment: member.environmentId }) } } : found };
    }), { actions: true, edit: 'hosts-edit', editParams: hostParams, remove: 'hosts-remove', removeParams: hostParams, test: true }));
  }

  if (supportsEas) {
    const easHosts = new Map<string, EasHost>();
    for (const member of members) for (const host of easHostsOf(member.settings)) if (!easHosts.has(host.id)) easHosts.set(host.id, host);
    for (const host of easHosts.values()) {
      const holds = (member: Member) => easHostsOf(member.settings).some(candidate => candidate.id === host.id);
      const holders = members.filter(holds);
      const checkOn = (holders.find(member => member.environmentId === primary) ?? holders[0]!).environmentId;
      rows.push(row(`eas:${host.id}`, host.label, 'EAS', easSessionLimits(host), true, active.map(member => {
        const added = holds(member);
        return { environmentId: member.environmentId, label: member.label, added, status: added ? hostStatus(member.state, host.id) : null, platforms: added ? hostPlatforms(member.state, host.id) : null };
      }), { actions: true, edit: 'eas-edit', editParams: params({ id: host.id, environment: checkOn }), remove: 'eas-remove', removeParams: params({ id: host.id }) }));
    }
  }
  return { canAdd: !disabled && !editing, disabled, eas: supportsEas, rows };
}
