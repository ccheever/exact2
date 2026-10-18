// Exact's own Swift warnings leave an outside app's build output; the app's own, errors and
// everything else stay (quiet.mjs, LLP 1116.021 D3).
import { expect, test } from 'bun:test';
import { quiet, quietFor } from './quiet.mjs';

const ROOT = '/x/exact2', C = (code) => `\x1b[${code}m`;
const block = (file, kind, text, cite = 'NoUsage') => [
  `${file}:72:91: ${C('1;33')}${kind}: ${C('1;39')}${text}${C('0;0')}`,
  `${C('0;36')}71 |${C('0;0')}         }`,
  `   ${C('0;36')}|${C('0;0')}    \`- ${C('1;33')}${kind}: ${C('1;39')}${text}${C('0;0')} [#\x1b]8;;https://docs.swift.org/x\x1b\\${cite}\x1b]8;;\x1b\\]`,
  `${C('0;36')}72 |${C('0;0')}         f.go()`,
  '',
];

test('an Exact warning, its blank line and an uncited note go; the app\'s warning and an error stay', () => {
  const output = [
    '[129 / 137] ExactKit',
    ...block(`${ROOT}/host/apple/Sources/ExactKit/A.swift`, 'warning', 'result unused'),
    ...block(`${ROOT}/host/apple/Sources/ExactKit/B.swift`, 'warning', 'deprecated', 'DeprecatedDeclaration'),
    '[#DeprecatedDeclaration]: <https://docs.swift.org/compiler/documentation/diagnostics/deprecated-declaration>',
    ...block('/apps/mine/apple/Mine.swift', 'warning', 'mine unused'),
    ...block(`${ROOT}/host/apple/Sources/ExactKit/C.swift`, 'error', 'cannot find x'),
    '[#NoUsage]: <https://docs.swift.org/compiler/documentation/diagnostics/no-usage>',
    'Build complete! (5.53 sec)',
  ].join('\n');
  const out = quiet(output, `${ROOT}/`);
  expect(out).not.toContain('A.swift');
  expect(out).not.toContain('B.swift');
  expect(out).not.toContain('[#DeprecatedDeclaration]: <');
  expect(out).toContain('mine unused');
  expect(out).toContain('C.swift:72:91');
  expect(out).toContain('[#NoUsage]: <');
  expect(out.startsWith('[129 / 137] ExactKit\n/apps/mine/apple/Mine.swift')).toBe(true);
  expect(out.endsWith('Build complete! (5.53 sec)')).toBe(true);
});

test('an app in the checkout keeps every warning', () => {
  const output = block(`${ROOT}/host/apple/Sources/ExactKit/A.swift`, 'warning', 'result unused').join('\n');
  quietFor({ dir: `${ROOT}/apps/caltrain` }, ROOT);
  expect(quiet(output)).toBe(output);
  quietFor({ dir: '/elsewhere/groceries' }, ROOT);
  expect(quiet(output)).toBe('');
});
