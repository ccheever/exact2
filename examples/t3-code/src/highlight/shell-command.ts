// ShellCommandBlock and HighlightedTokens.withEmbeddedScripts from T3 Code
// 9c161462c9 (MIT; see LICENSE-T3), over the clone's shared tokens (highlights.ts).
import { highlight, type Token } from './timeline-highlight';
import { embeddedScripts } from './embedded-scripts';
import { commandHighlightLanguage, withVisibleControlCharacters } from '../timeline/command-label';

/** Map embedded-script token styles back through shell quoting to the original command. */
export function shellCommand(command: string): Token[] {
  const code = withVisibleControlCharacters(command.trim()), language = commandHighlightLanguage(code);
  const tokens = highlight(code, language);
  if (language !== 'shellscript') return tokens;
  try {
    const scripts = embeddedScripts(code);
    if (!scripts.length) return tokens;
    const styles: string[] = Array.from({ length: code.length }, () => '');
    let offset = 0;
    for (const token of tokens) { styles.fill(token.cls, offset, offset + token.text.length); offset += token.text.length; }
    for (const script of scripts) {
      offset = 0;
      for (const token of highlight(script.text, script.language)) {
        if (token.text) styles.fill(token.cls, script.starts[offset], script.ends[offset + token.text.length - 1]);
        offset += token.text.length;
      }
    }
    const result: Token[] = [];
    for (let at = 0; at < code.length;) {
      const cls = styles[at] ?? ''; let end = at + 1;
      while (end < code.length && styles[end] === cls) end++;
      result.push({ text: code.slice(at, end), cls }); at = end;
    }
    return result;
  } catch { return tokens; }
}
