// The thread details card's rows are ThreadDetailsControl's `text-[13px] font-medium text-foreground/80`
// (threadDetailsPanelStyles.ts), and the composer's controls ComposerControl's: size "sm" `font-medium`, the resting
// "xs" `font-normal`. The window draws them antialiased under Font smoothing (app-window.contract), as the reference's
// root does, so the weights are the reference's own.
import { describe, expect, test } from 'bun:test';
import { appText } from '../support/app-files';

/** The lines of `file` that start (indentation aside) with `start`. */
const lines = (file: string, start: string) => appText(file).split('\n').filter(line => line.trimStart().startsWith(start));

describe('the labels the reference draws font-medium', () => {
  test('every row label of the thread details card is 500', () => {
    const labels: Record<string, [string, number][]> = {
      'src/details/git.contract': [['text details.git.branch.labelHead ', 1], ['text details.git.branch.labelTail ', 1], ['text details.git.branch.label title=', 1],
        ['text details.git.initLabel ', 1], ['text details.git.progressStatus ', 1], ['text details.git.successTitle ', 1], ['text details.git.quickLabel ', 1],
        ['text "Changes" ', 1], ['text details.envLabel ', 2]],
      'src/details/shell-details.contract': [['text "Add project script" ', 1], ['text "Previous agents" ', 1], ['text details.envLabel ', 1], ['text details.editorLabel ', 1]],
      'src/details/project-scripts.contract': [['text details.scriptName ', 1], ['text "Actions" ', 1]],
    };
    for (const [file, starts] of Object.entries(labels)) for (const [start, count] of starts) {
      const found = lines(file, start);
      expect([file, start, found.length]).toEqual([file, start, count]);
      for (const line of found) expect([start, line.includes(' font-weight=500 ')]).toEqual([start, true]);
    }
    // "Actions" takes the row's ink, as the script name beside it does (text-foreground/80).
    expect(lines('src/details/project-scripts.contract', 'text "Actions" ')[0]).toContain('color="light-dark(#18181bcc, #f5f5f5cc)"');
  });

  test('the composer\'s model, traits, runtime and Plan/Build labels are 500 at size sm and 400 resting', () => {
    const file = 'src/composer/composer-controls.contract';
    const found = ['text (data.composer.fanout ', 'text label font-size=(xs ', 'text (active ? "Plan" : "Build") '].flatMap(start => lines(file, start));
    expect(found).toHaveLength(5);
    for (const line of found) expect(line).toContain(' font-weight=(xs ? 400 : 500) ');
  });
});
