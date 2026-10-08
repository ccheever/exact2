// @ref llp/1109.005-composer-and-transcript.decision.md#work-log-detail-rows
// Pinned365aa87982 thread-work-log.tsx ThreadWorkLogRow; shared state/read owners remain unchanged.
import { mobileAnswerFile } from './thread-answer-files';
import type { ThreadBlock } from './thread';
import { questionAnswerText, questionAnswerPreview, hasQuestionAnswer } from './shared/timeline-inspect';
import type { T3Client } from './shared/client';
import { arr, obj, str, type Activity, type Obj } from './shared/domain';
import { toolCallLines, turnItemNeedsDetailFetch, turnItemOutputText } from './shared/timeline-item-detail';
import { turnItemDetailView } from './shared/timeline-item-fetch';
import { projectedWorkEntry, groupAction, collectToolFilePaths } from './shared/timeline-worklog';
import { preparationFailureRunId, workspacePreparationRetryRunIds } from './shared/r11-upstream-retry';

export interface ThreadAnswerFile { id: string; name: string; image: boolean; url: string }
export interface ThreadAnswerHistory { id: string; question: string; answer: string; files: ThreadAnswerFile[] }
export interface ThreadActivity { id: string; label: string; body: string; output: string; result: string; detail: string;
  failed: boolean; expandable: boolean; expanded: boolean; reasoning: boolean; loading: boolean; symbol: string; timestamp: string;
  prominentError: boolean; warning: boolean; call: boolean; retryRunId: string; retryDisabled: boolean; iconURL: string; reasoningBlocks: ThreadBlock[]; answerPreview: string; hasAnswer: boolean; answerHistory: ThreadAnswerHistory[] }
const toolSymbols: Record<string, string> = { terminal: 'terminal', 'file-text': 'doc.text', 'file-code': 'doc.text', search: 'magnifyingglass',
  brain: 'brain', 'circle-alert': 'exclamationmark.circle', 'file-pen': 'square.and.pencil', 'folder-open': 'folder', globe: 'globe', 'git-branch': 'arrow.triangle.branch' };
const errorTime = new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' });
function dateLabel(value: unknown) { const stamp = Date.parse(str(value)); return Number.isFinite(stamp) ? errorTime.format(stamp) : ''; }

/** QuestionAnswerHistory365aa87982 preserves the source union order. */
function answerHistory(answer: Obj, client: T3Client, row: Obj, now: number): ThreadAnswerHistory[] {
  const questions = obj(answer.questionTextById), answers = obj(answer.answers), attachments = obj(answer.attachmentsByQuestionId);
  return [...new Set([...Object.keys(questions), ...Object.keys(answers), ...Object.keys(attachments)])].map(id => ({
    id, question: str(questions[id]), answer: questionAnswerText(answers[id]),
    files: arr(attachments[id]).map(file => mobileAnswerFile(client, row, id, file, now)),
  }));
}

/** Scalar projection only. Disclosure/fetched output/retry remain on the adopted shared owners. */
export function mobileThreadActivity(activity: Activity, row: Obj | undefined, client: T3Client, now: number, dark: boolean, timestamp: string): ThreadActivity {
  const original = obj(row?.item), expanded = activity.detailOpen === true;
  const detail = row ? turnItemDetailView(client, original, now, activity.id) : null;
  const shown = detail?.item ?? original;
  const isRead = row ? groupAction(projectedWorkEntry(row)) === 'read' : false;
  const call = expanded && !isRead && shown.type === 'command_execution' ? toolCallLines({ command: str(shown.input) })
    : expanded && !isRead && shown.type === 'dynamic_tool' ? toolCallLines({ args: shown.input })
    : expanded && shown.type === 'file_search' ? toolCallLines({ args: { pattern: shown.pattern } })
    : expanded && shown.type === 'web_search' ? toolCallLines({ args: { query: Array.isArray(shown.patterns) ? shown.patterns.map(value => str(value)).join(', ') : undefined } }) : null;
  const callBody = call ? [call.command, ...(call.args ?? []).map(([key, value]) => `${key} ${value}`), call.argsText].filter(Boolean).join('\n') : '';
  const prominentError = original.type === 'error' && original.status === 'failed';
  const failure = obj(original.failure), warning = prominentError && failure.class === 'usage_limit';
  const reset = warning ? dateLabel(failure.resetAt) : '';
  const retryCandidate = preparationFailureRunId(original);
  const retryRunId = retryCandidate && workspacePreparationRetryRunIds(arr(client.projection.runs), arr(client.projection.turnItems)).has(retryCandidate) ? retryCandidate : '';
  const readPaths = isRead && original.type === 'dynamic_tool' ? collectToolFilePaths({ input: original.input }).join('\n') : '';
  const output = turnItemNeedsDetailFetch(original)
    ? detail?.state === 'missing' ? 'Output is no longer available.' : detail?.text ?? '' : detail?.text || activity.output;
  const fetched = detail !== null && detail.item !== original;
  const answer = original.type === 'user_input_request' && original.questionAnswer ? obj(original.questionAnswer) : null;
  return {
    id: activity.id, reasoningBlocks: [], answerPreview: answer ? questionAnswerPreview(answer) : '',
    hasAnswer: answer !== null && hasQuestionAnswer(answer), answerHistory: expanded && answer && row ? answerHistory(answer, client, row, now) : [], label: warning ? `Usage limit reached.${reset ? ` Retry after ${reset}.` : ''}`
      : activity.reasoning && expanded ? activity.status ?? 'Thought' : activity.label,
    body: activity.reasoning ? '' : call ? callBody : readPaths || activity.body,
    output: !row ? activity.output : expanded ? ['file_search', 'web_search'].includes(str(shown.type)) ? turnItemOutputText(shown) ?? ''
      : activity.reasoning ? activity.output : fetched ? turnItemOutputText(shown) ?? 'No output.' : output : '',
    result: call && shown.type === 'command_execution' && typeof shown.exitCode === 'number' && shown.exitCode !== 0 ? `exit ${shown.exitCode}` : call ? '' : activity.result,
    detail: prominentError && !warning ? str(failure.message) : activity.detail ?? '',
    failed: activity.failed, expandable: !prominentError && activity.expandable === true, expanded,
    reasoning: activity.reasoning === true, loading: detail?.state === 'loading',
    symbol: toolSymbols[activity.icon] ?? 'wrench', timestamp: prominentError ? dateLabel(activity.timestamp) : timestamp,
    prominentError, warning, call: call !== null, retryRunId,
    retryDisabled: !client.writable || client.busy || !!client.pending,
    iconURL: dark ? activity.iconDark ?? activity.iconLight ?? '' : activity.iconLight ?? activity.iconDark ?? '',
  };
}
