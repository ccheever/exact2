import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

// The generated sources are built, not committed: the ones on disk are what the generator makes from
// the pinned packages and the committed inputs (`generate.mjs` writes them; this fails until it has).
test('the generated sources on disk are what the generator makes', () => {
  const run = spawnSync(process.execPath, [join(import.meta.dir, 'generate.mjs'), '--check'], { encoding: 'utf8' });
  expect(run.stderr.split('\n').filter(line => /^(missing|differs|stale): /.test(line))).toEqual([]);
  expect(run.status).toBe(0);
}, 120_000);

// Built, not committed (rules/RULES.md): .gitignore lists every file a generator writes, and git tracks none.
test('git ignores every generated source and tracks none', () => {
  const list = spawnSync(process.execPath, ['-e', `const { outputs } = await import(${JSON.stringify(join(import.meta.dir, 'generate.mjs'))});
console.log((await outputs()).files.map(({ file }) => file).join('\\n'));`], { encoding: 'utf8' });
  expect(list.stderr).toBe('');
  const files = list.stdout.trim().split('\n');
  expect(files.length).toBeGreaterThan(20);
  const git = (...args: string[]) => spawnSync('git', args, { cwd: import.meta.dir, encoding: 'utf8' }).stdout.trim().split('\n').filter(Boolean);
  expect(git('check-ignore', '--no-index', ...files).length).toBe(files.length);
  expect(git('ls-files', '--', ...files)).toEqual([]);
}, 120_000);

test('the generator pins its packages at the integrity values the reference locks', () => {
  const lock = readFileSync(join(import.meta.dir, 'bun.lock'), 'utf8');
  for (const [name, integrity] of [
    ['@shikijs/langs@4.2.0', 'bwrVRlJ0wUhZxAbVdvBbv2TTC9yLsh4C/IO5Ofz0T8MQntgDvyVnkbjw9vi50r1kx7RCIJdnJnjZAwmAsXFLZQ=='],
    ['@pierre/theme@1.1.0', 'GC2OWTAfTIIWWYhPCygwG8t2EtePQkRfON4MI2rwIkJylmiyqIttJID2dCL8sUD8cNdEvYkEyfEHHKMeCiDLoQ=='],
    ['oniguruma-to-es@4.3.6', 'csuQ9x3Yr0cEIs/Zgx/OEt9iBw9vqIunAPQkx19R/fiMq2oGVTgcMqO/V3Ybqefr1TBvosI6jU539ksaBULJyA=='],
    ['oniguruma-parser@0.12.2', '6HVa5oIrgMC6aA6WF6XyyqbhRPJrKR02L20+2+zpDtO5QAzGHAUGw5TKQvwi5vctNnRHkJYmjAhRVQF2EKdTQw=='],
    ['regex@6.1.0', '6VwtthbV4o/7+OaAF9I5L5V3llLEsoPyq9P1JVXkedTP33c7MfCG0/5NOPcSJn0TzXcG9YUrR0gQSWioew3LDg=='],
    ['regex-recursion@6.0.2', '0YCaSCq2VRIebiaUviZNs0cBz1kg5kVS2UKUfNIx8YVs1cN3AV7NTctO5FOKBA+UT2BPJIWZauYHPqJODG50cg=='],
    ['lucide@0.564.0', 'FasyXKHWon773WIl3HeCQpd5xS6E0aLjqxiQStlHNKktni+HDncc1sqY+6vRUbCfmDsIaKQz43EEQLAUDLZO0g=='],
    ['lucide-react@0.564.0', 'JJ8GVTQqFwuliifD48U6+h7DXEHdkhJ/E87kksGByII3qHxtPciVb8T8woQONHBQgHVOl7rSMrrip3SeVNy7Fg=='],
    ['@pierre/trees@1.0.0-beta.4', 'OfT1yk9ne8Te5+GB5zUY8yqE6B8BqjBHQJleH4lu8ltwNpoocZl4vXt1AzlEExpxI/pp+AFX5QG+lR3JjtTEag=='],
    ['morphicons@1.7.1', 'q5ylxy5/d7vBg0OAzanlooXf05PekovMYDuuQVpr6vAQZxl99lrJbaIi+jJ32PXQf9WEEaDO2pbNBsx1ZhEnFQ=='],
    ['pele@1.8.0', 'AYmHttE70cwNJVXdfPMCANGivFlmc3bGGpMG0GqH5S5vE3RgtEuLlkqbhRfe94Csv/ZEd1TUbv2dMUHpw4p1Bg=='],
    ['katex@0.16.47', 'Eeo8Ys1doU1z+x8AZsPpQu+p/QcZBI5PeOo7QGQdy2x2m0MU/hYagBbGOmXwr5KVbEfVuWv9LpnQWeehogurjg=='],
    ['dompurify@3.4.16', 'sqo+pNp3qRhCIpbgRi1y8Tgk27Bo2Ry7w0dC1NBeNTdZChWjz9Xb/KOoZbRP/R6pQZ80Qw8YhXw13hWWBbMRnQ=='],
    // The document viewers' page (gen-file-preview.mjs).
    ['@betteroffice/docx@0.4.3', 'CEErXJH3iCsi2a61gTNkAofLCzSTcktLmp710gGhMwbBfOqwQ2yI7dRFBV3ElGa9JxkS03IfejvLX0sAvlTrUQ=='],
    ['@betteroffice/fonts@0.4.0', 'x1BVwYDAgY5R6TWI4t9NLguSiAEbH5GItN2ZlpBIFoRXTIJgjBsXd6sUxPGpexXOyY6KdhRjG8XvHjyAYxEGCw=='],
    ['@betteroffice/pptx@0.2.0', 'pNPQ58tlA+pVibERbLdQNskvP7a0GIMf4jhkCU/gPp8043Ichf6gB3PSb4ra9sfqoIaWqOaZKCWyptovA2+Q0A=='],
    ['@betteroffice/xlsx@0.3.0', 'K2bHtdk/epAxsmvxJlIWBRndjP3ltQU0Oh3p4gcZd2n5/YgXglo3SmMYX4nQe8WEm/yvz91p+aIasKyXXQP9Wg=='],
    ['pdfjs-dist@6.4.299', 'AVl138zALtfaAPvADulE0PZThbYzCBS79nL4pOSL/6Sm/4AH5A21BD9VHt97OlCuzJuCpmeZtAtkinisF4Vb1g=='],
    ['sql.js@1.15.0', '54VT5h9/VGlLaQan5pX/Zx8k6Vbykwl5hRUtl4h4+VYnaZCTraUs4P1i2WLz21O6oPVp6Gvg+d1OyPo01ZG5ig=='],
    ['jszip@3.10.1', 'xXDvecyTpGLrqFrvkrUSoxxfJI5AH7U8zxxtVclpsUtMCq4JQ290LY8AW5c7Ggnr/Y/oK+bQMbqK2qmtk3pN4g=='],
    ['saxes@6.0.0', 'xAg7SOnEhrm5zI3puOOKyy1OMcMlIJZYNJY7xLBwSze0UjhPLnWfj2GF2EpT0jmzaJKIWKHLsaSSajf35bcYnA=='],
    ['fast-xml-parser@5.7.3', 'C0AaNuC+mscy6vrAQKAc/rMq+zAPHodfHGZu4sGVehvAQt/JLG1O5zEcYcXSY5zSqr4YVgxsB+pHXTq0i7eDlg=='],
    ['entities@8.1.0', 'kxL7msIffSuh9aaFAMD7rxAIuTRMAHMeBtgHW2yUdWw732ZNh4MehkF2gdjvtdmikkaIP9bFDDJOPlsvm7avrA=='],
    ['effect@4.0.5', 'BY4/28NlpOd99yzmV741xbVGmp+1U7tks4oB7F/Png64bTa5tm5eCA6W0RiXsnuLDDoJpjfVSo4Dnq4BZb/89Q=='],
  ]) {
    const entry = lock.split('\n').find(line => line.includes(`["${name}"`)) ?? '';
    expect(entry).toContain(`sha512-${integrity}`);
  }
});

// A prebuilt file carries its dependencies' code (jszip.min.js holds pako, lie, immediate and setimmediate), so the
// viewer page's notices follow each shipped package's dependencies, as the reference's third-party-licenses.ts does,
// less what a browser build replaces or leaves out: jszip's readable-stream (its `browser` field), pdfjs-dist's
// Node canvas (an optional dependency) and a type-only package.
test("the viewer page carries the notice of each package it ships and of each that one carries", async () => {
  const { productionDependencies } = await import('./gen-file-preview.mjs');
  const dir = (name: string) => join(import.meta.dir, 'node_modules', name);
  const walked: Map<string, string> = productionDependencies(new Map([['jszip', dir('jszip')], ['dompurify', dir('dompurify')], ['pdfjs-dist', dir('pdfjs-dist')]]));
  expect([...walked.keys()].sort()).toEqual(['dompurify', 'immediate', 'jszip', 'lie', 'pako', 'pdfjs-dist', 'setimmediate']);
  const { outputs } = await import('./generate.mjs');
  const notices = (await outputs()).files.map(({ file }: { file: string }) => file).filter(file => /assets\/file-preview\/vendor\/licen[cs]e-/i.test(file));
  expect(['pako', 'lie', 'immediate', 'setimmediate'].filter(name => !notices.some(file => file.toLowerCase().includes(`-${name}`)))).toEqual([]);
  expect(notices.filter(file => /napi-rs|readable-stream|trusted-types/.test(file))).toEqual([]);
}, 120_000);

// The Mermaid page's pele (assets/mermaid/vendor/) draws the diagram kinds the Mermaid build it replaced drew, the
// fixtures of the transcript, Files and attachment tests among them, and the math page's KaTeX typesets.
test('the vendored pele renders the fixtures the Mermaid build rendered, and the vendored KaTeX typesets', async () => {
  const assets = join(import.meta.dir, '..', '..', 'assets');
  const pele = await import(join(assets, 'mermaid', 'vendor', 'pele', 'lazy.js'));
  const fixtures: [string, string][] = [
    ['graph TD\n  A --> B', 'flowchart'],
    ['graph LR\n  Files --> Preview', 'flowchart'],
    ['flowchart TD\n  A[Start] --> B{Ready?}\n  B -->|yes| C[Ship]\n  B -->|no| A', 'flowchart'],
    ['sequenceDiagram\n  Alice->>Bob: Hi\n  Bob-->>Alice: Hello', 'sequence'],
    ['classDiagram\n  Animal <|-- Duck\n  Animal : +int age', 'class'],
    ['stateDiagram-v2\n  [*] --> Still\n  Still --> Moving\n  Moving --> [*]', 'state'],
    ['erDiagram\n  CUSTOMER ||--o{ ORDER : places', 'er'],
    ['gantt\n  title Plan\n  dateFormat YYYY-MM-DD\n  section A\n  Task :a1, 2026-01-01, 3d', 'gantt'],
    ['pie title Pets\n  "Dogs" : 386\n  "Cats" : 85', 'pie'],
    ['mindmap\n  root((Root))\n    A\n    B', 'mindmap'],
    ['gitGraph\n  commit\n  branch dev\n  commit\n  checkout main\n  merge dev', 'gitGraph'],
    ['journey\n  title Day\n  section Work\n    Code: 5: Me', 'journey'],
    ['timeline\n  title History\n  2025 : One\n  2026 : Two', 'timeline'],
  ];
  for (const [source, type] of fixtures) {
    const result = await pele.renderAsync(source, { links: false, images: false, autoDirection: false });
    expect([source, result.type]).toEqual([source, type]);
    expect(result.width).toBeGreaterThan(0);
    expect(result.svg).toStartWith('<svg');
  }
  await expect(pele.renderAsync('not a diagram')).rejects.toThrow('No diagram type detected');
  const katex = (await import(join(assets, 'math', 'vendor', 'katex.mjs'))).default;
  expect(katex.renderToString('x^2', { displayMode: true, output: 'htmlAndMathml', throwOnError: true, trust: false })).toContain('katex-display');
  expect(() => katex.renderToString('\\frac{', { throwOnError: true })).toThrow();
}, 60_000);
