// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/timeline-inspect.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// What an expanded work-log entry shows, adapted from T3 Code (MIT, see LICENSE-T3):
// apps/web/src/components/chat/MessagesTimeline.tsx (WorkEntryLogRow: reads and
// skills expand to plain text, everything else to the item inspector),
// MessagesTimeline.logic.ts (workEntryReadOutput), V2ItemInspector.tsx
// (StructuredValue highlightJson, the dynamic tool's "Input" heading) and
// V2LifecycleRow.tsx (SubagentNotificationLink).
import { arr, obj, str, type Activity, type Obj } from './domain';
import { claudeSkillInvocation, collectToolFilePaths, groupAction, type WorkEntry } from './timeline-worklog';
import { subagent } from './timeline-events';

const isWindowsAbsolute = (path: string) => /^[A-Za-z]:[\\/]/.test(path);

/** workEntryReadOutput: a read's paths, absolute where the workspace root is known, one per line. */
export function readOutput(entry: WorkEntry, root: string): string | null {
  const raw = entry.changedFiles?.length ? entry.changedFiles
    : entry.itemType === 'dynamic_tool' && collectToolFilePaths({ input: entry.item.input }).length ? collectToolFilePaths({ input: entry.item.input })
      : collectToolFilePaths(entry.toolData);
  const paths = [...new Set(raw.map(path => {
    const trimmed = path.trim().replace(/\\/g, '/');
    if (!root || trimmed.startsWith('/') || isWindowsAbsolute(trimmed)) return trimmed;
    return `${root.replace(/\\/g, '/').replace(/\/+$/, '')}/${trimmed.replace(/^\.\//, '').replace(/^\/+/, '')}`;
  }).filter(path => path.length > 0))];
  return paths.length ? paths.join('\n') : null;
}

/** Reads and skills expand to plain text (null: nothing to show); undefined leaves the inspector. */
export function plainOutput(entry: WorkEntry, root: string): string | null | undefined {
  if (groupAction(entry) === 'read') return readOutput(entry, root);
  const skill = entry.itemType === 'dynamic_tool' ? claudeSkillInvocation(entry.item.toolName, entry.item.input) : undefined;
  return skill ? skill.args ?? null : undefined;
}

export interface InspectorCode { id: string; code: string; icon: string; tokens: { id: string; text: string; cls: string }[] }
/** notificationChildThreadId: a subagent or delegated task's notification names its child thread. */
export function notificationChildThreadId(item: Obj): string {
  const source = obj(item.source);
  return item.type === 'notification' && (source.kind === 'subagent' || source.kind === 'delegated_task') ? str(source.childThreadId) : '';
}
const OUTCOME_STATUS: Record<string, string> = { completed: 'completed', failed: 'failed', cancelled: 'cancelled' };
const OUTCOME_LABEL: Record<string, string> = { completed: 'Finished', failed: 'Failed', cancelled: 'Stopped', updated: 'Updated', unknown: 'Finished' };

/**
 * SubagentNotificationLink: a notification about a subagent the parent knows
 * drawn as that subagent's card, with the status it reported then (no dot for
 * an update) and the time it arrived in place of the elapsed time.
 */
export function notificationSubagent(item: Obj, subagents: Obj[], createdAt: string, time: string): Activity | null {
  const childThreadId = notificationChildThreadId(item);
  const agent = childThreadId ? subagents.find(candidate => candidate.childThreadId === childThreadId) : undefined;
  if (!agent) return null;
  const outcome = str(item.outcome, 'unknown'), status = OUTCOME_STATUS[outcome] ?? '';
  const card = subagent({ ...agent, status: status || str(agent.status) });
  return { ...card, id: str(item.id, card.id), result: OUTCOME_LABEL[outcome] ?? 'Finished', output: time, failed: status === 'failed',
    tone: status === 'failed' ? 'error' : status === 'completed' ? 'success' : status === 'cancelled' ? 'muted' : 'none',
    status: 'event', targetId: childThreadId, timestamp: createdAt };
}

/** deriveRunlessWorkStartedAt: a provider-native subagent thread's live runless root turn, or ''. */
export function runlessWorkStartedAt(projection: Obj): string {
  const thread = obj(projection.thread);
  if (obj(thread.lineage).relationshipToParent !== 'subagent' || thread.creationSource !== 'provider') return '';
  const node = [...arr(projection.nodes)].reverse().find(candidate => candidate.kind === 'root_turn' && (candidate.runId === null || candidate.runId === undefined));
  return node && ['pending', 'running', 'waiting'].includes(str(node.status)) ? str(node.startedAt) : '';
}

// ---- answered questions (client-runtime/work-log/userInput.ts, MessagesTimeline QuestionAnswerHistory) ----
export function questionAnswerText(value: unknown): string {
  if (typeof value === 'string') return value;
  if (Array.isArray(value)) return value.map(questionAnswerText).filter(Boolean).join(', ');
  return value !== null && typeof value === 'object' ? questionAnswerText((value as Obj).answers) : '';
}
const texts = (answer: Obj) => Object.values(obj(answer.questionTextById)).map(text => str(text));
const attachmentNames = (answer: Obj) => Object.values(obj(answer.attachmentsByQuestionId)).flatMap(list => Array.isArray(list) ? list : []).map(item => str(obj(item).name));
/** getQuestionTextPreview: the questions asked, one line, joined by " · ". */
export function questionTextPreview(answer: Obj): string {
  return texts(answer).map(text => text.replace(/\s+/g, ' ').trim()).filter(Boolean).join(' · ');
}
/** getQuestionAnswerPreview: the answers, else the attached files, else the questions. */
export function questionAnswerPreview(answer: Obj): string {
  const answers = Object.values(obj(answer.answers)).map(questionAnswerText).filter(Boolean), files = attachmentNames(answer);
  return (answers.length ? answers.join(' · ') : files.length ? files.join(', ') : texts(answer).join(' · ')).replace(/\s+/g, ' ').trim();
}
export function hasQuestionAnswer(answer: Obj): boolean {
  return Object.values(obj(answer.answers)).some(value => !!questionAnswerText(value))
    || Object.values(obj(answer.attachmentsByQuestionId)).some(list => Array.isArray(list) && list.length > 0);
}
/** QuestionAnswerHistory as lines: each question (q), its answer indented (a), and attachment names (f). */
export function questionHistory(id: string, answer: Obj): InspectorCode {
  const ids = [...new Set([...Object.keys(obj(answer.questionTextById)), ...Object.keys(obj(answer.answers)), ...Object.keys(obj(answer.attachmentsByQuestionId))])];
  const lines: { text: string; cls: string }[] = [];
  for (const questionId of ids) {
    const question = str(obj(answer.questionTextById)[questionId]), reply = questionAnswerText(obj(answer.answers)[questionId]);
    const files = (Array.isArray(obj(answer.attachmentsByQuestionId)[questionId]) ? obj(answer.attachmentsByQuestionId)[questionId] as unknown[] : []).map(item => str(obj(item).name)).filter(Boolean);
    if (question) lines.push({ text: question, cls: 'q' });
    if (reply) lines.push({ text: reply, cls: 'a' });
    if (files.length) lines.push({ text: files.join('  '), cls: 'f' });
  }
  return { id, code: '', icon: 'QA', tokens: lines.map((line, index) => ({ id: String(index), ...line })) };
}
