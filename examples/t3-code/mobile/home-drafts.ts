// Source365aa87982 pending-new-tasks-model, HomeScreen and ThreadListV2PendingRow.
// @ref llp/1109.004-home-projection.decision.md#decision
// @ref llp/1109.005-composer-and-transcript.decision.md#new-task-ownership
import { assistantCitationsToPlainText } from './shared/diff-citations';
import type { MobileNewTaskDraft } from './mobile-new-task-drafts';

/** A serializable presentation snapshot; its controller owns hydration and content. */
export type HomeDraftInput = Readonly<Pick<MobileNewTaskDraft, 'key' | 'environmentId' | 'projectId' | 'origin' | 'createdAt'> & {
  text: string; images: readonly object[]; files: readonly object[];
  workspace: Readonly<{ branch: string }> | null;
}>;
export interface HomeDraftProjectRef { readonly environmentId: string; readonly projectId: string }
export interface HomeDraftProject extends HomeDraftProjectRef { readonly title: string; readonly groupTitle?: string }
export interface HomeDraftEnvironment { readonly environmentId: string; readonly label: string; readonly machineSymbol: string }
export interface HomeDraftOptions {
  readonly environmentId?: string;
  /** Null/absent means all projects; an empty list means no matching projects. */
  readonly projectRefs?: readonly HomeDraftProjectRef[] | null;
  readonly query?: string;
  readonly projects?: readonly HomeDraftProject[];
  /** Saved connections, including offline ones; not the synchronized shell list. */
  readonly environments?: readonly HomeDraftEnvironment[];
}
export interface HomeDraftMenuItem {
  id: string; parentId: string; label: string; operation: string; value: string; symbol: string; subtitle: string;
  destructive: boolean; checked: boolean; disabled: boolean;
}
export interface HomeDraftRow {
  key: string; kind: 'draft'; draftKey: string; environmentId: string; projectId: string; origin: string; createdAt: string;
  title: string; branch: string; projectTitle: string; projectPresent: boolean; environmentLabel: string; machineSymbol: string;
  status: 'Draft'; statusSymbol: 'square.and.pencil'; accessibilityHint: string;
  showPendingDivider: boolean; trailingDivider: boolean; location: string; menuItems: HomeDraftMenuItem[];
}

/** Same title conversion as mobile projectThreadStartTurn, over the shared citation parser. */
export function homeDraftTitle(text: string, attachmentCount: number): string {
  if (!text.trim()) return attachmentCount === 1 ? '1 attachment' : `${attachmentCount} attachments`;
  const trimmed = assistantCitationsToPlainText(text).trim();
  if (!trimmed) return 'New thread';
  const compact = trimmed.replace(/\s+/g, ' ');
  return compact.length <= 72 ? compact : `${compact.slice(0, 69).trimEnd()}...`;
}

/** draftId is the full draft key. The receiving controller revalidates the stamped project. */
export function homeDraftLocation(draft: HomeDraftProjectRef & { readonly key: string }): string {
  return `/new/draft?environmentId=${encodeURIComponent(draft.environmentId)}&projectId=${encodeURIComponent(draft.projectId)}&draftId=${encodeURIComponent(draft.key)}`;
}

/** Draft rows are spliced after active threads, independently of transport readiness or shelves. */
export function projectHomeDrafts(drafts: readonly HomeDraftInput[], options: HomeDraftOptions = {}): HomeDraftRow[] {
  const query = options.query?.trim().toLocaleLowerCase() ?? '';
  const rows: HomeDraftRow[] = [];
  for (const draft of drafts) {
    const attachments = draft.images.length + draft.files.length;
    if (!draft.key.startsWith('new-task:') || !draft.environmentId || !draft.projectId || (!draft.text.trim() && !attachments)
      || options.environmentId && draft.environmentId !== options.environmentId
      || options.projectRefs != null && !options.projectRefs.some(ref => ref.environmentId === draft.environmentId && ref.projectId === draft.projectId)) continue;
    const title = homeDraftTitle(draft.text, attachments);
    if (query && !title.toLocaleLowerCase().includes(query)) continue;
    const project = options.projects?.find(ref => ref.environmentId === draft.environmentId && ref.projectId === draft.projectId);
    const environment = options.environments?.find(ref => ref.environmentId === draft.environmentId);
    const environmentLabel = (options.environments?.length ?? 0) > 1 ? environment?.label ?? '' : '';
    rows.push({ key: `draft-task:${draft.key}`, kind: 'draft', draftKey: draft.key, environmentId: draft.environmentId,
      projectId: draft.projectId, origin: draft.origin, createdAt: draft.createdAt, title, branch: draft.workspace?.branch ?? '',
      projectTitle: project?.groupTitle ?? project?.title ?? '', projectPresent: !!project,
      environmentLabel, machineSymbol: environmentLabel ? environment?.machineSymbol ?? '' : '',
      status: 'Draft', statusSymbol: 'square.and.pencil', accessibilityHint: 'Opens the draft in the new task composer',
      showPendingDivider: false, trailingDivider: false, location: homeDraftLocation(draft),
      menuItems: [{ id: 'discard', parentId: '', label: 'Discard', operation: 'draft-discard', value: draft.key,
        symbol: 'trash', subtitle: '', destructive: true, checked: false, disabled: false }] });
  }
  rows.sort((a, b) => b.createdAt.localeCompare(a.createdAt) || a.key.localeCompare(b.key));
  rows.forEach((row, index) => { row.showPendingDivider = index === 0; row.trailingDivider = index + 1 < rows.length; });
  return rows;
}
