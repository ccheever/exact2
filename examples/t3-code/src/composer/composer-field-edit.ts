import type { ComposerFieldEdit, ComposerFieldEffects, ComposerFieldSnapshot } from './composer-field-commands';
import { composerTrigger, type ComposerChip } from './composer-source-chips';
import type { ComposerMode } from './composer-source-session';
import type { ComposerBlockOrigin } from './composer-markdown';

export type ComposerFieldReplacement = {
  kind: 'edit'; edit: ComposerFieldEdit;
} & ComposerFieldEffects | { kind: 'refused'; reason: string };
type Context = { snapshot: ComposerFieldSnapshot; draftOwner: string; mode: ComposerMode; chips: readonly ComposerChip[]; blockOrigins?: readonly ComposerBlockOrigin[] };
const blank = (character: string) => /^[ \t\r\n]$/.test(character);
const integer = (value: unknown): value is number => typeof value === 'number' && Number.isSafeInteger(value);
const record = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value);

/** Normalize the two app commands at the shared source boundary. Selection,
 *  history and source mutation remain the field host's responsibility. */
export function composerFieldReplacement(request: unknown, context: Context): ComposerFieldReplacement {
  const refused = (reason: string): ComposerFieldReplacement => ({ kind: 'refused', reason });
  const { snapshot, mode } = context;
  if (!record(request) || request.op !== 'editorEdit' && request.op !== 'editorInsert') return refused('field command invalid');
  if (!snapshot.live) return refused('no composer');
  if (snapshot.composing) return refused('field composing');
  if (request.snapshotOwner !== undefined && request.snapshotOwner !== snapshot.snapshotOwner
    || request.fieldOwner !== undefined && request.fieldOwner !== snapshot.owner
    || request.value !== undefined && request.value !== snapshot.value) return refused('field source changed');
  if (request.owner !== undefined && (typeof request.owner !== 'string' || request.owner && request.owner !== snapshot.snapshotOwner)) return refused('another composer');
  if (request.text !== undefined && typeof request.text !== 'string') return refused('field text invalid');
  const source = snapshot.value, body = typeof request.text === 'string' ? request.text : '';
  const insert = request.op === 'editorInsert';
  for (const key of ['all', 'pad', 'extendSpace', 'focus']) {
    if (request[key] !== undefined && typeof request[key] !== 'boolean') return refused('field option invalid');
  }
  if (request.expect !== undefined && typeof request.expect !== 'string') return refused('field expectation invalid');
  if (request.kind !== undefined && typeof request.kind !== 'string') return refused('field trigger invalid');
  if (request.caret !== undefined && !integer(request.caret)) return refused('field caret invalid');
  if (request.history !== undefined && (!integer(request.history) || request.history < 0)) return refused('field history invalid');
  if (request.citation !== undefined && (!integer(request.citation) || request.citation < 0)) return refused('field citation invalid');
  if (request.completion !== undefined && (!integer(request.completion) || request.completion < 0)) return refused('field completion invalid');
  const then = request.then ?? '';
  if (then !== '' && then !== 'models' && then !== 'mode-plan' && then !== 'mode-default') return refused('field action invalid');
  const all = request.all === true;
  const requestedStart = insert ? snapshot.start : all ? 0 : request.start ?? source.length;
  const requestedEnd = insert ? requestedStart : all ? source.length : request.end ?? requestedStart;
  if (!integer(requestedStart) || !integer(requestedEnd) || requestedStart < 0 || requestedEnd < requestedStart || requestedEnd > source.length) return refused('range');
  const start = requestedStart;
  let end = requestedEnd, text = body;
  if (request.expect !== undefined && source.slice(start, end) !== request.expect) return refused('changed');
  if (request.kind !== undefined) {
    const trigger = mode === 'literal' ? null : composerTrigger(source, snapshot.start, [...context.chips], mode === 'rich', context.blockOrigins);
    if (!trigger || trigger.kind !== request.kind || trigger.start !== start || trigger.end !== end) return refused('changed');
  }
  if (insert) {
    end = source[start] === ' ' ? start + 1 : start;
    text = `${start > 0 && !blank(source[start - 1] ?? '') ? ' ' : ''}${body}${body.endsWith(' ') ? '' : ' '}`;
  } else {
    if (request.pad === true && start > 0 && !blank(source[start - 1] ?? '')) text = ' ' + text;
    if (request.extendSpace === true && text.endsWith(' ') && source[end] === ' ') end++;
  }
  const size = source.length - (end - start) + text.length;
  const caret = !insert && integer(request.caret) ? Math.max(0, Math.min(request.caret, size)) : start + text.length;
  return { kind: 'edit', focus: insert || request.focus !== false, then, history: typeof request.history === 'number' ? request.history : 0, citation: typeof request.citation === 'number' ? request.citation : 0, completion: typeof request.completion === 'number' ? request.completion : 0,
    edit: { generation: snapshot.generation, snapshotOwner: snapshot.snapshotOwner, owner: snapshot.owner,
      value: source, start, end, text, selectionStart: caret, selectionEnd: caret } };
}
