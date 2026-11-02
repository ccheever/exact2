// SettingsScopeNotice's choices (MIT T3 Code 9c161462c9, the Archived mark at 2359643cff; LICENSE-T3:
// apps/web/src/components/settings/SettingsScopeNotice.tsx). A category with no settings at the
// selected scope lists the targets that own it as rows: an icon, the name, the line that tells it
// apart and a chevron. Changes: the clone has no checkout picker and no search target owned by a
// checkout, so a choice sets the project and environment axes only (`project`, `machine`).
import { projectGroupOrganization } from '../client/domain';
import { projectIdentity } from '../client/presentation';
import { settingsScopeEnvironmentAddress, settingsScopeEnvironmentLabels, settingsScopeProjectMachines, type ScopeGroup } from './settings-scope';

type NoticeEnvironment = { environmentId: string; label: string; displayUrl?: string | null; kind: string };
export type ScopeNoticeTarget = 'environment' | 'project' | 'all';
/** One row: `icon` is "environment" (the machine `kind`), "project" (its mark) or "layers"; a press selects `project` and `machine`. */
export type ScopeNoticeChoice = { key: string; label: string; secondary: string; icon: string; kind: string; mark: string; ink: string; surface: string; project: string; machine: string; member: string };
export type ScopeNoticeGroup = { key: string; title: string; choices: ScopeNoticeChoice[] };

const choice = (key: string, label: string, secondary: string, extra: Partial<ScopeNoticeChoice>): ScopeNoticeChoice =>
  ({ key, label, secondary, icon: '', kind: '', mark: '', ink: '', surface: '', project: '', machine: '', member: '', ...extra });

/** The choices for the notice's target, in one untitled group. */
export function scopeNoticeGroups(target: ScopeNoticeTarget, groups: readonly ScopeGroup[], environments: readonly NoticeEnvironment[], eligibleEnvironmentIds?: readonly string[]): ScopeNoticeGroup[] {
  const choices = target === 'project'
    ? groups.map(group => {
      const identity = projectIdentity(group.displayName);
      const sameName = groups.some(other => other.projectKey !== group.projectKey && other.displayName === group.displayName);
      const archived = projectGroupOrganization(group.memberProjects).archived;
      return choice(`project:${group.projectKey}`, group.displayName, [...(archived ? ['Archived'] : []), ...(sameName ? settingsScopeProjectMachines(group, environments) : []), String(group.memberProjects[0]?.workspaceRoot ?? '')].join(' · '),
        { icon: 'project', mark: identity.projectMark, ink: identity.projectInk, surface: identity.projectSurface, project: group.projectKey, member: String(group.memberProjects[0]?.id ?? '') });
    })
    : target === 'environment'
      ? environments.filter(entry => eligibleEnvironmentIds === undefined || eligibleEnvironmentIds.includes(entry.environmentId)).map(entry => {
        const { primary, secondary } = settingsScopeEnvironmentLabels(entry, environments);
        return choice(`machine:${entry.environmentId}`, primary, secondary || settingsScopeEnvironmentAddress(entry), { icon: 'environment', kind: entry.kind, machine: entry.environmentId });
      })
      : [choice('all', 'Open all environments', '', { icon: 'layers' })];
  return [{ key: target, title: '', choices }];
}
