import { describe, expect, test } from 'bun:test';
import { highlight, languageOf, messageCodeBlocks, type Cls } from './timeline-highlight';
import { fileIconToken, languageIconToken } from './timeline-files';

// Token colours the built reference painted (pierre-light) for the fixture's
// "fixture code" answer, one [text, colour] pair per Shiki span.
const reference: Record<string, [string, string][][]> = {"ts":[[["import","#d32a61"],[" {","#636363"],[" readFile","#d47628"],[" }","#636363"],[" from","#d32a61"],[" 'node:fs/promises'","#199f43"],[";","#636363"]],[],[["// Count the lines of a file.","#737373"]],[["export","#d32a61"],[" async","#d32a61"],[" function","#a631be"],[" countLines","#693acf"],["(path:","#636363"],[" string","#a631be"],["):","#636363"],[" Promise","#a631be"],["<","#636363"],["number","#a631be"],[">","#636363"],[" {","#636363"]],[["  const","#a631be"],[" text","#d5901c"],[" =","#08c0ef"],[" await","#d32a61"],[" readFile","#693acf"],["(","#636363"],["path","#d47628"],[",","#636363"],[" 'utf8'","#199f43"],[");","#636363"]],[["  return","#d32a61"],[" text","#d47628"],[".","#636363"],["split","#693acf"],["(","#636363"],["'","#199f43"],["\\n","#16a994"],["'","#199f43"],[").","#636363"],["length","#d47628"],[" +","#08c0ef"],[" 0x1","#1ca1c7"],[";","#636363"]],[["}","#636363"]],[]],"json":[[["{","#636363"]],[["  \"name\"","#d5512f"],[":","#636363"],[" \"fixture\"","#199f43"],[",","#636363"]],[["  \"version\"","#d5512f"],[":","#636363"],[" 2","#1ca1c7"],[",","#636363"]],[["  \"private\"","#d5512f"],[":","#636363"],[" true","#1ca1c7"],[",","#636363"]],[["  \"tags\"","#d5512f"],[":","#636363"],[" [","#636363"],["\"a\"","#199f43"],[",","#636363"],[" null","#08c0ef"],["]","#636363"]],[["}","#636363"]],[]],"bash":[[["# build then test","#737373"]],[["bun","#693acf"],[" install","#199f43"],[" --frozen-lockfile","#d5a910"],[" &&","#636363"],[" bun","#693acf"],[" test","#199f43"],[" \"","#199f43"],["$DIR","#d47628"],["\"","#199f43"]],[]],"python":[[["def","#a631be"],[" greet","#693acf"],["(","#636363"],["name","#d5a910"],[":","#636363"],[" str","#08c0ef"],[")","#636363"],[" ->","#636363"],[" str","#08c0ef"],[":","#636363"]],[["    \"\"\"Return a greeting.\"\"\"","#199f43"]],[["    return","#d32a61"],[" f","#a631be"],["'Hello, ","#199f43"],["{","#d5a910"],["name","#0a0a0a"],["}","#d5a910"],["!'","#199f43"],["  # done","#737373"]],[]]};
const LIGHT: Record<Cls, string> = { '': '#0a0a0a', kw: '#d32a61', decl: '#a631be', type: '#a631be', fn: '#693acf', var: '#d47628', const: '#d5901c',
  param: '#636363', pyparam: '#d5a910', str: '#199f43', esc: '#16a994', num: '#1ca1c7', nul: '#08c0ef', op: '#08c0ef', punct: '#636363', com: '#737373',
  key: '#d5512f', flag: '#d5a910', builtin: '#08c0ef', interp: '#d5a910', heading: '#d5512f', bold: '#d5a910', tag: '#d5512f', attr: '#18a46c' };
/** Colour per non-space character, so span boundaries and whitespace do not matter. */
function painted(spans: [string, string][]): string[] {
  return spans.flatMap(([text, color]) => [...text].filter(character => character.trim()).map(() => color));
}
function mine(line: string, lang: string): string[] {
  return painted(highlight(line, lang).map(token => [token.text, LIGHT[token.cls]] as [string, string]));
}

describe('syntax highlighting against the reference Shiki output', () => {
  const sources: Record<string, string> = {
    ts: "import { readFile } from 'node:fs/promises';\n\n// Count the lines of a file.\nexport async function countLines(path: string): Promise<number> {\n  const text = await readFile(path, 'utf8');\n  return text.split('\\n').length + 0x1;\n}",
    json: '{\n  "name": "fixture",\n  "version": 2,\n  "private": true,\n  "tags": ["a", null]\n}',
    bash: '# build then test\nbun install --frozen-lockfile && bun test "$DIR"',
    python: "def greet(name: str) -> str:\n    \"\"\"Return a greeting.\"\"\"\n    return f'Hello, {name}!'  # done",
  };
  for (const lang of Object.keys(sources)) {
    test(`${lang} paints every character as the reference does`, () => {
      const code = sources[lang]!;
      const expected = reference[lang]!.filter(line => line.length).flatMap(line => painted(line));
      const actual = painted(highlight(code, lang).map(token => [token.text, LIGHT[token.cls]] as [string, string]));
      expect(actual).toEqual(expected);
    });
  }
  test('languages come from fences and file names', () => {
    expect([languageOf('tsx'), languageOf('src/app.ts'), languageOf('README.md'), languageOf('zsh'), languageOf('notes.txt')]).toEqual(['ts', 'ts', 'md', 'sh', '']);
  });
  test('code blocks carry their exact text for the renderer to match', () => {
    const blocks = messageCodeBlocks('Intro\n\n```ts\nconst a = 1;\n```\n\n```\nplain\n```\n');
    expect(blocks.map(block => block.code)).toEqual(['const a = 1;', 'plain']);
    expect(blocks[0]!.tokens.map(token => token.cls)).toContain('decl');
    expect(blocks[1]!.tokens).toEqual([{ id: '0', text: 'plain', cls: '' }]);
    expect(blocks.map(block => block.icon)).toEqual(['typescript', '']);
  });
  test('fence languages draw their Pierre icon only when it is specific', () => {
    expect(['ts', 'json', 'bash', 'python', 'sh', 'diff', 'text', 'tsx', 'yaml', 'dockerfile', ''].map(languageIconToken))
      .toEqual(['typescript', 'json', 'bash', 'python', 'bash', '', '', 'react', 'yml', 'docker', '']);
    expect(['src/app.ts', 'package.json', 'README.md', 'docs/notes.md', 'Cargo.toml', 'x.unknownext', '.gitignore'].map(fileIconToken))
      .toEqual(['typescript', 'npm-plain', 'markdown', 'markdown', 'default', 'default', 'git']);
  });
  void mine;
});
