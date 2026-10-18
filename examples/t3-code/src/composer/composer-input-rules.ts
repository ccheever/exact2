// Live block gestures from ComposerPromptEditorTiptap.tsx at T3 Code 9c161462c9.
export type ComposerRuleContext = { kind: 'paragraph' } | { kind: 'list'; marker: string } | { kind: 'other' };
export type ComposerBlockRule =
  | { kind: 'list'; list: 'bullet' | 'ordered'; marker: string; space: string; carried: string }
  | { kind: 'task'; checked: boolean }
  | { kind: 'quote'; prefix: string }
  | { kind: 'heading'; level: number; space: string }
  | { kind: 'rule'; source: string };

/** These gestures differ from restoring Markdown. In particular a dash waits
 *  for its first non-`[` body character, and `***` waits for a space. */
export function composerBlockInputRule(line: string, context: ComposerRuleContext): ComposerBlockRule | null {
  const task = context.kind === 'list' ? /^\[([ xX])\] $/.exec(line) : /^- \[([ xX])\] $/.exec(line);
  if (task && (context.kind === 'paragraph' || context.kind === 'list' && ['-', '*', '+'].includes(context.marker))) {
    return { kind: 'task', checked: task[1]?.toLowerCase() === 'x' };
  }
  if (context.kind !== 'paragraph') return null;
  const rule = /^(---|\*\*\*|___)\s?$/.exec(line);
  if (rule && (line.startsWith('---') || /\s$/.test(line))) return { kind: 'rule', source: line };
  const heading = /^(#{1,6})(\s)$/.exec(line);
  if (heading?.[1] && heading[2]) return { kind: 'heading', level: heading[1].length, space: heading[2] };
  const quote = /^>(\s)$/.exec(line);
  if (quote?.[1]) return { kind: 'quote', prefix: '>' + quote[1] };
  const bullet = /^([*+])(\s)$/.exec(line) ?? /^(-)([ \t]+)([^\s[])$/.exec(line);
  if (bullet?.[1] && bullet[2]) return { kind: 'list', list: 'bullet', marker: bullet[1], space: bullet[2], carried: bullet[3] ?? '' };
  const ordered = /^(\d+[.)])(\s)$/.exec(line);
  return ordered?.[1] && ordered[2] ? { kind: 'list', list: 'ordered', marker: ordered[1], space: ordered[2], carried: '' } : null;
}
