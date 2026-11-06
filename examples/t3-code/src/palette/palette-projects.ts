// The command palette's project lists in the sidebar's Project
// order (CommandPalette.tsx `pickerProjects`, MIT reference, see LICENSE-T3).
// The reference builds the sidebar's logical project groups
// (buildSidebarProjectSnapshots), sorts them by clientSettings
// .sidebarProjectSortOrder (sortLogicalProjectsForSidebar; Manual keeps the
// project order) and moves the contextual project's group first
// (buildSidebarProjectPickerEntries) after the pinned ones; archived groups are left out
// (pickableProjectGroups). Each entry is one group: its target is the
// contextual member when the group holds it, else the group's representative,
// and it is titled with the group's display name; `memberTitle` keeps the target's
// own title (its ProjectFavicon's name) and `memberProjects` the group's members.
// Both the Projects search group and the New thread in... page read this list.
import type { T3Client } from '../client/client';
import { str, type Obj } from '../client/domain';
import { pickableScopes } from '../sidebar/sidebar-view';

/**
 * The palette's projects: one per open sidebar group, pinned ones first, then the current project's group, then Project
 * order. Each carries its group's key (`scopeKey`), whether the group is pinned (`scopePinned`) and whether it is the
 * current project's (`preferred`), for its pin and for where "No project" goes.
 */
export function pickerProjects(client: T3Client): Obj[] {
  const preferred = client.projectId;
  const entries = pickableScopes(client).flatMap(scope => {
    const target = scope.members.find(member => str(member.id) === preferred) ?? scope.members[0];
    const isPreferred = !!preferred && scope.ids.has(preferred);
    return target ? [{ project: { ...target, title: scope.name || str(target.title), memberTitle: str(target.title), memberProjects: scope.members, scopeKey: scope.key,
      scopePinned: scope.pinned, preferred: isPreferred }, preferred: isPreferred, pinned: scope.pinned }] : [];
  });
  // The preferred project leads the unpinned ones; pinned projects stay first.
  const lead = entries.find(entry => entry.preferred);
  if (!lead || lead.pinned) return entries.map(entry => entry.project);
  const rest = entries.filter(entry => entry !== lead), pinnedCount = rest.filter(entry => entry.pinned).length;
  return [...rest.slice(0, pinnedCount), lead, ...rest.slice(pinnedCount)].map(entry => entry.project);
}
