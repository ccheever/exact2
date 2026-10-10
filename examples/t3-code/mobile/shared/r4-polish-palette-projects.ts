// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r4-polish-palette-projects.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Lane r4-polish: the command palette's project lists in the sidebar's Project
// order (CommandPalette.tsx `pickerProjects`, MIT reference, see LICENSE-T3).
// The reference builds the sidebar's logical project groups
// (buildSidebarProjectSnapshots), sorts them by clientSettings
// .sidebarProjectSortOrder (sortLogicalProjectsForSidebar; Manual keeps the
// project order) and moves the contextual project's group first
// (buildSidebarProjectPickerEntries). Each entry is one group: its target is the
// contextual member when the group holds it, else the group's representative,
// and it is titled with the group's display name. Both the Projects search group
// and the New thread in... page read this list.
import type { T3Client } from './client';
import { str, type Obj } from './domain';
import { projectScopes } from './sidebar-view';

/** The palette's projects: one per sidebar group, in Project order, the current project's group first. */
export function pickerProjects(client: T3Client): Obj[] {
  const preferred = client.projectId;
  const entries = projectScopes(client).flatMap(scope => {
    const target = scope.members.find(member => str(member.id) === preferred) ?? scope.members[0];
    return target ? [{ project: { ...target, title: scope.name || str(target.title) }, preferred: !!preferred && scope.ids.has(preferred) }] : [];
  });
  const at = entries.findIndex(entry => entry.preferred);
  if (at > 0) entries.unshift(...entries.splice(at, 1));
  return entries.map(entry => entry.project);
}
