// Pending approvals on their timeline row, adapted from T3 Code v0.1.5 (MIT, see LICENSE-T3):
// apps/web/src/components/chat/TimelinePendingApprovalRow.tsx, ComposerPendingApprovalActions.tsx (its inline
// presentation) and MessagesTimeline.tsx (pendingApprovalsByRequestId, unmatchedPendingApprovals).
import { str } from '../client/domain';
import type { T3Client } from '../client/client';
import { composerRequests, fullAccessOverrideExplanation, pendingRequests } from '../composer/requests';

const APPROVAL_PRESENTATION: Record<string, { icon: string; label: string; detailLabel: string }> = {
  command: { icon: 'terminal', label: 'Command approval', detailLabel: 'Command' },
  'file-change': { icon: 'square-pen', label: 'File change approval', detailLabel: 'File change' },
  'file-read': { icon: 'eye', label: 'File read approval', detailLabel: 'File to read' },
  permission: { icon: 'wrench', label: 'App permission approval', detailLabel: 'Permission request' },
  'mcp-elicitation': { icon: 'hammer', label: 'App access approval', detailLabel: 'App access request' },
};

export interface ApprovalChoice { id: string; label: string; warning: string; primary: boolean }
/** A pending approval as its timeline row draws it (`TimelineApproval`, timeline-approval.contract). */
export interface TimelineApprovalView {
  id: string; icon: string;
  /** The row's group name ("Command approval") and its detail's ("Command"). */
  label: string; detailLabel: string;
  /** The command, file or request asked about, else the group name. */
  detail: string; appName: string; reason: string;
  /** The provider can still take an answer (responseCapability "live"). */
  live: boolean;
  /** The session may operate the thread (canOperateThread). */
  operable: boolean;
  /** An answer to it is on its way. */
  responding: boolean;
  /** "Why am I being asked this?": why full access still asked, or "". */
  whyAsked: string;
  /** Approve and Decline stay on the row; every other advertised decision is under More. */
  options: ApprovalChoice[]; more: ApprovalChoice[];
}

/**
 * The approvals the timeline draws as answerable, by request id. One the composer holds while its user types is
 * left out, so the row never paints as answerable then (composerTypingGuard): it reads as its ordinary work row.
 */
export function timelinePendingApprovals(client: T3Client): Map<string, TimelineApprovalView> {
  if (!pendingRequests(client.projection).approvals.length) return new Map();
  const operable = client.scopes.includes('orchestration:operate');
  const responding = str(client.pending?.payload.requestId);
  return new Map(composerRequests(client).approvals.map(approval => {
    const presentation = APPROVAL_PRESENTATION[approval.kind] ?? APPROVAL_PRESENTATION['file-change']!;
    const choices = approval.options.map(option => ({ id: str(option.decision), label: str(option.label), warning: str(option.warning),
      primary: option.decision === 'accept' || option.decision === 'decline' }));
    return [approval.requestId, { id: approval.requestId, icon: presentation.icon, label: presentation.label, detailLabel: presentation.detailLabel,
      detail: approval.detail || presentation.label, appName: approval.appName, reason: approval.reason, live: approval.live, operable,
      responding: responding === approval.requestId, whyAsked: approval.fullAccessOverride ? fullAccessOverrideExplanation(approval.fullAccessOverride) : '',
      options: choices.filter(choice => choice.primary), more: choices.filter(choice => !choice.primary) }];
  }));
}
