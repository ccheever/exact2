// `tamber import css` (LLP 1153 D6.2): a stylesheet's custom properties written into app.tamber between markers.
// The subset it translates, each thing it refuses by name, each scheme's value by CSS's cascade, an author's tokens
// kept outside the markers, a second import rewriting only its block, and one that would leave the author's tokens
// refused writing nothing. Runs the debug compiler the lane builds, as tamber-dtcg.test.mjs does.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

const ROOT = resolve(import.meta.dir, '..');
const compiler = resolve(process.env.CARGO_TARGET_DIR ?? resolve(ROOT, 'target'), 'debug', process.platform === 'win32' ? 'contract.exe' : 'contract');

/** An app directory holding `files`, the import of its tokens.css run in it, and what it printed and wrote. */
function imported(files, again = []) {
  const dir = mkdtempSync(resolve(tmpdir(), 'tamber-import-css-'));
  try {
    const run = (written) => {
      for (const [name, text] of Object.entries(written)) writeFileSync(resolve(dir, name), text);
      const r = spawnSync(compiler, ['tamber', 'import', 'css', 'tokens.css', '--app', '.'], { cwd: dir, encoding: 'utf8' });
      return { status: r.status, out: `${r.stdout}${r.stderr}`, design: readFileSync(resolve(dir, 'app.tamber'), 'utf8') };
    };
    return [run(files), ...again.map(run)];
  } finally { rmSync(dir, { recursive: true, force: true }); }
}

const SUBSET = `/* the brand */
:root {
  --ink: #1d1d1f;
  --paper: #ffffff;
  --space-row: 12px;
  --radius: 0.5rem;
  --heading: var(--ink);
  --accent: var(--ink, #0000ee) !important;
}
@media (prefers-color-scheme: dark) {
  :root { --ink: #f5f5f7; --paper: #000000; --space-row: 12px; }
}
`;

test('the subset: :root tokens, a dark redeclaration as light-dark(), a var() as a reference', () => {
  const [r] = imported({ 'tokens.css': SUBSET });
  expect(r.status).toBe(0);
  expect(r.design).toBe(`tamber app

theme
  // tamber:import tokens.css
  ink = light-dark(#1d1d1f, #f5f5f7)
  paper = light-dark(#ffffff, #000000)
  space-row = 12px
  radius = 0.5rem
  heading = ink
  accent = ink
  // tamber:end
`);
  // Each token written, with the kind the evaluator gave it; the fallback dropped is said.
  expect(r.out).toContain('6 tokens between `// tamber:import tokens.css` and `// tamber:end`');
  expect(r.out).toContain('  ink = light-dark(#1d1d1f, #f5f5f7)  color');
  expect(r.out).toContain('  space-row = 12px  length');
  expect(r.out).toMatch(/ {2}accent = ink {2}color {2}\(its fallback dropped/);
  expect(r.out).not.toContain('refused');
});

test('each thing outside the subset is refused by name, and nothing is written for it', () => {
  const [r] = imported({ 'tokens.css': `:root {
  --ink: #111111;
  --h: 220;
  --brand: hsl(var(--h) 50% 40%);
  --grid: 1fr 2fr;
  --1x: 4px;
  --orphan: var(--missing);
  --link: var(--ink);
  --edge: var(--ink, #000) var(--paper);
  ---ink: #000000;
}
.dark { --ink: #eeeeee; }
[data-theme="dark"] { --ink: #dddddd; }
@media (prefers-color-scheme: dark) { :root { --only-dark: #000000; } }
@media (min-width: 600px) { :root { --ink: #222222; } }
` });
  expect(r.status).toBe(1);
  const refused = r.out.split('\n').filter((l) => l.startsWith('  refused ')).map((l) => l.slice('  refused '.length));
  const why = (name) => refused.filter((l) => l.startsWith(`${name}: `)).join('\n');
  expect(why('--ink')).toContain('`.dark` is a scope the page switches');
  expect(why('--ink')).toContain('`[data-theme="dark"]` is a scope the page switches');
  expect(why('--ink')).toContain('`@media (min-width: 600px) { :root }` is neither block the import reads');
  expect(why('--brand')).toContain('composed around a `var()`');
  // A `var()` with a fallback is a reference only when that call is the whole value.
  expect(why('--edge')).toContain('composed around a `var()`');
  expect(why('--grid')).toContain('`grid = 1fr 2fr`');
  expect(why('--1x')).toContain('names no Tamber token');
  // Exactly the `--` goes: `---ink` is `-ink`, which no Tamber name is.
  expect(why('---ink')).toContain('names no Tamber token');
  expect(why('--orphan')).toContain('`orphan = missing`');
  // The evaluator types it and the compiler refuses it: `link` is drawn in the app's tint.
  expect(why('--link')).toContain('`link = ink`: [lower-');
  expect(why('--only-dark')).toContain('declared only under `prefers-color-scheme: dark`');
  // What translates is written, a number among it, and nothing refused is.
  expect(r.design).toContain('  ink = #111111  scheme=fixed\n  h = 220\n  // tamber:end');
  for (const name of ['brand', 'grid', '1x', 'orphan', 'only-dark', 'link', 'edge', '-ink']) expect(r.design).not.toContain(`${name} =`);
});

// A colour with no dark value is one colour in both schemes in CSS, so it is written `scheme=fixed` and the build does
// not ask for a dark side; a reference, a length and a system colour (which follows the scheme) carry no mark.
test('a colour the stylesheet gives one value is marked scheme=fixed', () => {
  const [r] = imported({ 'tokens.css': ':root { --bg: #f5f6f8; --text: var(--bg); --gap: 4px; --field: Canvas; --line: light-dark(#dddddd, #333333); }\n' });
  expect(r.status).toBe(0);
  expect(r.design).toContain('  bg = #f5f6f8  scheme=fixed\n  text = bg\n  gap = 4px\n  field = Canvas\n  line = light-dark(#dddddd, #333333)\n');
  expect(r.out).toContain('  bg = #f5f6f8  scheme=fixed  color');
  expect(r.out).toContain('1 colour has no dark value in the stylesheet, so it is marked `scheme=fixed`');
});

test("each scheme's value is the declaration CSS's cascade gives it", () => {
  const [r] = imported({ 'tokens.css': `:root { --ink: #111111; --line: #cccccc !important; --muted: #777777; }
@media (prefers-color-scheme: dark) { :root { --ink: #eeeeee; --line: #333333; --muted: #999999 !important; } }
:root { --ink: #333333; --line: #dddddd; --muted: #888888; }
html { --rule: #101010; }
:root { --rule: #202020; }
html { --rule: #303030; }
:root { --nested: #101010; @media (prefers-color-scheme: dark) { --nested: #efefef; } }
` });
  expect(r.status).toBe(0);
  // A later :root wins the dark scheme too; an !important one wins either scheme it applies in;
  // :root outranks html whatever their order; the dark query nested in :root, CSS Nesting's form,
  // is the dark side as the query around :root is.
  expect(r.design).toContain(`  // tamber:import tokens.css
  ink = #333333  scheme=fixed
  line = #cccccc  scheme=fixed
  muted = light-dark(#888888, #999999)
  rule = #202020  scheme=fixed
  nested = light-dark(#101010, #efefef)
  // tamber:end
`);
});

test("the author's tokens stay outside the markers, and a second import rewrites only its block", () => {
  const design = `tamber shop

axes
  density = regular | compact

theme
  brand = #c8102e  scheme=fixed
  ink = #000000
  card = light-dark(paper, #1c1c1e)
`;
  const [first, second, third] = imported(
    { 'app.tamber': design, 'tokens.css': ':root { --ink: #111111; --paper: #ffffff; --line: #dddddd; }\n' },
    [
      { 'tokens.css': ':root { --paper: #fafafa; --rule: var(--line2); --line2: #cccccc; }\n' },
      { 'tokens.css': ':root { --rule: var(--line2); --line2: #cccccc; }\n' },
    ],
  );
  // The author's `ink` stays theirs; the stylesheet's is refused, and `card` reads the imported `paper`.
  expect(first.status).toBe(1);
  expect(first.out).toContain('refused --ink: the design declares `ink` outside the import\'s markers');
  expect(first.design).toBe(`${design}  // tamber:import tokens.css
  paper = #ffffff  scheme=fixed
  line = #dddddd  scheme=fixed
  // tamber:end
`);
  expect(second.status).toBe(0);
  expect(second.design).toBe(`${design}  // tamber:import tokens.css
  paper = #fafafa  scheme=fixed
  rule = line2
  line2 = #cccccc  scheme=fixed
  // tamber:end
`);
  // Without `--paper`, the author's `card` would name no token: the file is left as it was.
  expect(third.status).toBe(1);
  expect(third.out).toContain("is refused outside the import's markers, so it is left as it was");
  expect(third.out).toMatch(/app\.tamber:\d+:\d+: .*`paper`/);
  expect(third.design).toBe(second.design);
});
