// The Linux desktop entry (LLP 1133.023 D17): the manifest's shortcuts as Actions, Exec quoted as the
// Desktop Entry Specification says, and checked by desktop-file-validate where it is installed.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { desktopEntry, execArgument } from './desktop-entry.mjs';

const shelf = { id: 'com.exact.shelf', displayName: 'Shelf', manifest: { shortcuts: [{ name: 'Add Book', url: '/add' }, { name: 'Add Book', url: '/add?from=menu&x=1' }] } };

test('each shortcut is an action running the app with its location', () => {
  const entry = desktopEntry(shelf, '/opt/shelf/shelf');
  expect(entry).toBe([
    '[Desktop Entry]', 'Type=Application', 'Name=Shelf', 'Exec=/opt/shelf/shelf', 'Terminal=false', 'Actions=add-book;add-book-2;',
    '', '[Desktop Action add-book]', 'Name=Add Book', 'Exec=/opt/shelf/shelf /add',
    '', '[Desktop Action add-book-2]', 'Name=Add Book', 'Exec=/opt/shelf/shelf "/add?from=menu&x=1"', ''].join('\n'));
});

test('an argument with a reserved character is quoted, `%` doubled, and the line escaped as a string', () => {
  expect(execArgument('/a/b')).toBe('/a/b');
  expect(execArgument('/My Apps/shelf')).toBe('"/My Apps/shelf"');
  expect(execArgument('/x?p=50%')).toBe('"/x?p=50%%"');
  expect(execArgument('/a"$`\\b')).toBe('"/a\\"\\$\\`\\\\b"');
  // A literal backslash in a quoted argument is four in the file (the spec's "The Exec key").
  expect(desktopEntry({ id: 'x', displayName: 'X', manifest: {} }, '/a b\\c')).toContain('Exec="/a b\\\\\\\\c"\n');
});

test('no shortcuts, no Actions key', () => {
  expect(desktopEntry({ id: 'x', displayName: 'X', manifest: {} }, '/x')).not.toContain('Actions=');
});

test('desktop-file-validate accepts it, where installed', () => {
  if (spawnSync('desktop-file-validate', ['--help']).error) return;
  const dir = mkdtempSync(resolve(tmpdir(), 'desktop-entry-')), file = resolve(dir, 'com.exact.shelf.desktop');
  writeFileSync(file, desktopEntry(shelf, '/opt/shelf/shelf'));
  const r = spawnSync('desktop-file-validate', [file], { encoding: 'utf8' });
  expect(r.stdout + r.stderr).toBe('');
  expect(r.status).toBe(0);
});
