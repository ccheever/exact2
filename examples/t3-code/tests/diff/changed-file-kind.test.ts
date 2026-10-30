// The test-file rule the Hide test files toggle uses (packages/shared/src/changedFileKind.ts at 6d450a4284), and the
// Changes panel hiding them (DiffPanel isFileShown): tests and test support hide, a turn diff's named file stays, and
// the choice is one saved preference.
import { describe, expect, test } from 'bun:test';
import { classifyChangedFile, isTestChangedFile } from '../../src/diff/changed-file-kind';
import { adoptHideTestFiles, hideTestFiles, shownDiffFiles } from '../../src/diff/hide-test-files';

describe('classifyChangedFile and isTestChangedFile', () => {
  test('tests and what only tests use', () => {
    for (const path of ['src/a.test.ts', 'src/a.spec.tsx', 'pkg/a_test.go', 'tests/helpers.ts', 'test_api.py', 'conftest.py', 'vitest.setup.ts', 'FooTest.java',
      'src/__mocks__/fs.ts', 'fixtures/data.json', 'src/__snapshots__/a.snap', 'src/a.fixture.ts', 'testdata/in.txt']) expect([path, isTestChangedFile(path)]).toEqual([path, true]);
    for (const path of ['src/a.ts', 'Latest.java', 'README.md', 'bun.lock', 'src/generated/api.ts', 'contest.py']) expect([path, isTestChangedFile(path)]).toEqual([path, false]);
  });
  test('the other kinds', () => {
    expect(['yarn.lock', 'app.min.js', 'docs/guide.md', 'requirements.txt', 'src/x.ts', 'e2e/login.ts', 'mocks/user.ts'].map(classifyChangedFile))
      .toEqual(['generated', 'generated', 'docs', 'code', 'code', 'tests', 'mocks']);
  });
});

describe('Hide test files (useHideTestFiles)', () => {
  test('hides tests but the file asked for; off by default and saved as one choice', () => {
    const files = ['src/a.ts', 'src/a.test.ts', 'src/b.test.ts'];
    expect(shownDiffFiles(files, path => path, false, '')).toEqual(files);
    expect(shownDiffFiles(files, path => path, true, 'src/b.test.ts')).toEqual(['src/a.ts', 'src/b.test.ts']);
    const next = {};
    adoptHideTestFiles(next, {});
    expect(hideTestFiles({ local: next })).toBe(false);
    adoptHideTestFiles(next, { diffHideTestFiles: true });
    expect(hideTestFiles({ local: next })).toBe(true);
  });
});
