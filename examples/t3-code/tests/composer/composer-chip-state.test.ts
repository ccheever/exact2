import { expect, test } from 'bun:test';
import { ComposerChipPresses } from '../../src/composer/composer-chip-state';
import { composerChips, insertCitation } from '../../src/composer/composer-source-chips';
import { serializeAssistantCitation } from '../../src/timeline/diff-citations';

const snapshot = (value: string) => ({ generation: 1, snapshotOwner: 'thread', owner: 'prompt', value, start: 0, end: 0, composing: false, live: true });
const frame = [10, 20, 80, 18] as const;
const citation = serializeAssistantCitation({ environmentId: 'env', threadId: 'thread', messageId: 'message', start: 0, end: 5, text: 'hello' });

test('presses use exact admitted source and geometry, with one newest sequence across surfaces', () => {
  const state = new ComposerChipPresses(), value = '$skill @file.ts ', current = snapshot(value), chips = composerChips(value);
  const skill = state.open(current, 'composer', chips, chips[0]!, frame)!;
  expect(skill).toMatchObject({ open: true, kind: 'skill', name: 'skill', owner: 'thread', fieldOwner: 'prompt' });
  skill.frame[0] = 999;
  expect(state.read(current, 'composer')?.frame[0]).toBe(10);
  expect(state.open(current, 'composer', chips, { ...chips[0]!, source: 'wrong' }, frame)).toBeNull();
  expect(state.open(current, 'composer', chips, chips[0]!, [10, 20, NaN, 18])).toBeNull();
  expect(state.open({ ...current, composing: true }, 'composer', chips, chips[0]!, frame)).toBeNull();
  const preview = state.open(current, 'preview', chips, chips[0]!, frame)!;
  expect(preview.seq).toBeGreaterThan(skill.seq);
  expect(state.close(skill.seq)).toBe(false);
  expect(state.read({ ...current, owner: 'other' }, 'composer')?.surface).toBe('preview');
  expect(state.read({ ...current, generation: 2 }, 'preview')).toBeNull();
});

test('detail source changes close ordinary chips and stale owners cannot reopen them', () => {
  const state = new ComposerChipPresses(), current = snapshot('$skill '), chip = composerChips(current.value)[0]!;
  expect(state.open(current, 'composer', [chip], chip, frame)?.open).toBe(true);
  expect(state.read({ ...current, value: `x${current.value}` }, 'composer')).toBeNull();
  expect(state.open({ ...current, value: 'plain' }, 'composer', [chip], chip, frame)).toBeNull();
  const next = state.open(current, 'composer', [chip], chip, frame)!;
  expect(state.read({ ...current, live: false }, 'composer')).toBeNull();
  expect(state.close(next.seq)).toBe(false);
});

test('citation comment follows surrounding edits and fresh cancellation removes only its owned spaces', () => {
  const state = new ComposerChipPresses(), inserted = insertCitation('hello', 5, citation), current = snapshot(inserted.citation.value), chips = composerChips(current.value);
  const press = state.open(current, 'composer', chips, chips[0]!, frame, { comment: true, fresh: inserted.citation, sourceFrame: [0, 0, 100, 20] })!;
  expect(press).toMatchObject({ open: true, kind: 'citation', anchor: 'source', removeOnCancel: true });
  const prefixed = { ...current, value: `before ${current.value}` };
  expect(state.read(prefixed, 'composer')?.start).toBe(inserted.citation.start + 7);
  const changed = { ...current, value: `${prefixed.value}after` };
  expect(state.read(changed, 'composer')?.start).toBe(inserted.citation.start + 7);
  const cancel = state.cancel(changed, press.seq)!;
  expect(changed.value.slice(0, cancel.start) + cancel.text + changed.value.slice(cancel.end)).toBe('before helloafter');
  expect(state.cancel(changed, press.seq + 1)).toBeNull();
  const through = { ...changed, value: changed.value.replace('Assistant quote', 'different') };
  expect(state.read(through, 'composer')).toBeNull();
});

test('source activation is an action; preview actions and stale fresh anchors are refused', () => {
  const state = new ComposerChipPresses(), current = snapshot(citation), chips = composerChips(current.value), chip = chips[0]!;
  expect(state.open(current, 'composer', chips, chip, frame)).toMatchObject({ act: 'citation', open: false });
  expect(state.open(current, 'preview', chips, chip, frame)).toBeNull();
  expect(state.open(current, 'composer', chips, chip, frame, { comment: true, fresh: { ...insertCitation('', 0, citation).citation, value: 'stale' } })).toBeNull();
  const open = state.open(current, 'composer', chips, chip, frame, { comment: true })!;
  expect(state.cancel(current, open.seq)).toBeNull();
  expect(state.read({ ...current, owner: 'new prompt' }, 'composer')).toBeNull();
});

test('details toggle without replacing an active citation draft and actions are consumed once', () => {
  const state = new ComposerChipPresses(), current = snapshot('$skill @file.ts '), chips = composerChips(current.value);
  const skill = state.open(current, 'composer', chips, chips[0]!, frame)!;
  expect(state.open(current, 'composer', chips, chips[0]!, frame)).toBeNull();
  expect(state.read(current, 'composer')).toBeNull();
  const action = state.open(current, 'composer', chips, chips[1]!, frame)!;
  expect(action.seq).toBeGreaterThan(skill.seq);
  expect(state.take(current, action.seq)?.path).toBe('file.ts');
  expect(state.take(current, action.seq)).toBeNull();
  const source = snapshot(citation), citationChip = composerChips(source.value)[0]!;
  const first = state.open(source, 'composer', [citationChip], citationChip, frame, { comment: true })!;
  expect(state.open(source, 'composer', [citationChip], citationChip, frame, { comment: true })?.seq).toBe(first.seq);
});
