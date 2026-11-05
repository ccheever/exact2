// A reply's action row, a file chip's name and the focus after a decision, each as the reference draws it (T3 Code
// 5d3d78ab93: TimelineAssistantRows, MarkdownFileLink, workspaceFocus.ts).
import { describe, expect, test } from 'bun:test';
import { appText } from '../support/app-files';
import { contractComponent, value } from '../support/contract-action-fixture';

describe('a reply row and its controls', () => {
  // TimelineAssistantRows: the row is `opacity-0 focus-within:opacity-100 group-hover/assistant:opacity-100`. A copy shows
  // its check for a second, and the row hides once the pointer and the focus have left it.
  test('the action row shows while hovered or focused, a copy notwithstanding', () => {
    const text = appText('timeline.contract'), from = text.indexOf('component RowActions');
    const row = text.slice(from).split('\n').find(line => line.trimStart().startsWith('row width="100%" height="1.5rem"'))!;
    const opacity = /opacity=(\([^)]*\))/.exec(row)![1]!;
    expect([true, false].map(visible => value(opacity, { visible, message: { copied: 3 } }))).toEqual([1, 0]);
  });

  // MarkdownFileLink: the chip with a press of its own is an `<a>` named by its text; without one (the pull request
  // page), a button named "File options for <label>".
  test("a file chip is named by its text, not its path", () => {
    const line = appText('link-menus.contract').split('\n').find(entry => entry.includes('button press=press contextmenu'))!;
    const label = /aria-label=(\([^)]*\))/.exec(line)![1]!;
    const scope = { label: 'outline.ts', tip: '/work/notes/outline.ts' };
    expect(value(label, { ...scope, fileLinkPressMenu: false })).toBe('outline.ts');
    expect(value(label, { ...scope, fileLinkPressMenu: true })).toBe('File options for outline.ts');
    for (const [file, call] of [['markdown.contract', 'FileLinkButton(label=chip.label, tip=chip.tip'], ['markdown.contract', 'FileLinkButton(label=run.text, tip=chip.tip'], ['media-markdown.contract', 'FileLinkButton(label=label, tip=tip']] as const)
      expect(appText(file)).toContain(call);
  });

  // workspaceFocus.ts recover: a decision disables the focused button, and the focus it drops goes to its area, the
  // conversation, which rings. A Mac click focuses no button, so the focus stays where it was.
  test('a decision from the keyboard moves the focus to the conversation', () => {
    const decided = (name: string, focused: boolean) => {
      const calls: string[] = [];
      contractComponent('timeline-approval.contract', name).call('chosen', { focused }, { choose: () => calls.push('choose'), focus: (id: string) => calls.push(`focus ${id}`) }, []);
      return calls;
    };
    expect(decided('ApprovalChoiceButton', true)).toEqual(['choose', 'focus workspace-conversation']);
    expect(decided('ApprovalChoiceButton', false)).toEqual(['choose']);
    expect(decided('ApprovalMenuChoice', true)).toEqual(['choose', 'focus workspace-conversation']);
    const text = appText('timeline-approval.contract');
    expect(text).toContain('button press=chosen hover');
    expect(text).toContain('button id=testId press=chosen popovertarget=menuId');
    expect(appText('app-main.contract')).toContain('id="workspace-conversation" tabindex=-1 role="region" aria-label="Conversation"');
  });

  // index.css `[data-workspace-area]:focus-visible`: a 2px ring inside the area's edges, from the keyboard only.
  test('the conversation rings while it holds a keyboard focus itself', () => {
    const column = contractComponent('app-main.contract', 'ChatColumn');
    const ringed = (on: boolean, keyboardLast: boolean) => { const state = { conversationRinged: !on }; column.call('conversationFocus', state, { keyboardLast }, [on]); return state.conversationRinged; };
    expect([ringed(true, true), ringed(true, false), ringed(false, true)]).toEqual([true, false, false]);
    const text = appText('app-main.contract');
    expect(text).toContain('focus=conversationFocus(true) blur=conversationFocus(false) outline="none"');
    expect(text).toContain('if conversationRinged\n            box position="absolute" top=0 left=0 right=0 bottom=0 border-width=2');
  });
});
