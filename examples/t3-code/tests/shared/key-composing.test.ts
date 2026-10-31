import {describe, expect, test} from 'bun:test';
import {contractComponent, contractFns, type Env} from '../support/contract-action-fixture';

// The reference's text fields return early on `event.isComposing` before acting on Return or Escape, so the
// Return that confirms an input method's candidate neither submits nor closes (KeyboardEvent.isComposing).
const key = (isComposing: boolean, held: Env = {}) => ({isComposing, shiftKey: false, metaKey: false, ctrlKey: false, altKey: false, repeat: false, code: '', ...held});

describe('a composing key is not a Return or an Escape', () => {
  test('the diff find bar: Return finds next and Escape closes only once the candidate is in', () => {
    const bar = contractComponent('diff-find.contract', 'DiffFindBar');
    const ops: string[] = [];
    const press = (k: string, composing: boolean) => bar.call('key', {}, {op: (what: string) => ops.push(what)}, [k, key(composing)]);
    press('Enter', true); press('Escape', true);
    expect(ops).toEqual([]);
    press('Enter', false); press('Escape', false);
    expect(ops).toEqual(['next', 'close']);
  });

  test('the thread find bar and the citation comment', () => {
    const find = contractComponent('thread-find.contract', 'ThreadFindBar');
    const local: string[] = [];
    find.call('key', {}, {local: (what: string) => local.push(what)}, ['Enter', key(true)]);
    find.call('key', {}, {local: (what: string) => local.push(what)}, ['Enter', key(false)]);
    expect(local).toEqual(['find-next']);
    const comment = contractComponent('citation-comment.contract', 'CitationCommentLayer');
    const saves: unknown[] = [];
    const scope = {tooLong: false, save: (send: boolean) => saves.push(send), cancel: () => saves.push('cancel'), preventDefault: () => {}};
    comment.call('keys', {}, scope, ['Enter', key(true)]);
    comment.call('keys', {}, scope, ['Escape', key(true)]);
    expect(saves).toEqual([]);
    comment.call('keys', {}, scope, ['Enter', key(false)]);
    expect(saves).toEqual([false]);
  });

  test('the settings search hands on only keys that are not composing', () => {
    const nav = contractComponent('settings-core.contract', 'SettingsNav');
    const keys: string[] = [];
    nav.call('searchKey', {}, {onKey: (k: string) => keys.push(k)}, ['Enter', key(true)]);
    nav.call('searchKey', {}, {onKey: (k: string) => keys.push(k)}, ['Enter', key(false)]);
    expect(keys).toEqual(['Enter']);
  });

  test('a menu row\'s Return (kmReturn)', () => {
    const {kmReturn} = contractFns('src/shared/menu-keys.contract') as {kmReturn: (k: string, e: Env) => boolean};
    expect(kmReturn('Enter', key(false))).toBe(true);
    expect(kmReturn('Enter', key(true))).toBe(false);
  });
});
