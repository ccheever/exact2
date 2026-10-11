// 20261005-embedded-server-runtime: stage-runtime.mjs accepts the archive only when the pin, the
// release's SHA256SUMS and the computed hash agree (cliRelease.ts parseChecksums for the file).
import { describe, expect, test } from 'bun:test';
import { mkdtempSync, mkdirSync, writeFileSync, chmodSync, symlinkSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
// @ts-ignore: a Bun script without declarations
import { checkArchive, manifestOf, parseChecksums } from './stage-runtime.mjs';

const pin = JSON.parse(readFileSync(join(import.meta.dir, 'server-runtime/runtime-pin.json'), 'utf8'));
const sums = (hash: string) => `${'0'.repeat(64)}  t3-${pin.version}-linux-x64.tar.gz\n${hash}  ${pin.asset}\n`;

describe('stage-runtime', () => {
  test('parses sha256sum lines, binary marks included', () => {
    const map = parseChecksums(`${'A'.repeat(64)} *one.tar.gz\r\nnot a line\n${'b'.repeat(64)}  two.zip\n`);
    expect(map.get('one.tar.gz')).toBe('a'.repeat(64));
    expect(map.get('two.zip')).toBe('b'.repeat(64));
    expect(map.size).toBe(2);
  });

  test('accepts the pinned archive', () => {
    expect(checkArchive(pin, { size: pin.size, sha256: pin.sha256, checksums: sums(pin.sha256) })).toBeNull();
  });

  test('refuses a tampered archive with its hash', () => {
    const tampered = 'f'.repeat(64);
    expect(checkArchive(pin, { size: pin.size, sha256: tampered, checksums: sums(pin.sha256) })).toContain(tampered);
  });

  test('refuses a SHA256SUMS line that differs from the pin, even when it matches the file', () => {
    const other = 'e'.repeat(64);
    expect(checkArchive(pin, { size: pin.size, sha256: other, checksums: sums(other) })).toContain(`SHA256SUMS lists ${other}`);
  });

  test('refuses an archive SHA256SUMS does not list, and a wrong size', () => {
    expect(checkArchive(pin, { size: pin.size, sha256: pin.sha256, checksums: `${pin.sha256}  something-else.tar.gz\n` })).toContain('is not listed');
    expect(checkArchive(pin, { size: pin.size - 1, sha256: pin.sha256, checksums: sums(pin.sha256) })).toContain('size');
  });

  test('the manifest records type, mode, size, hash and link targets', () => {
    const root = mkdtempSync(join(tmpdir(), 't3-manifest-'));
    mkdirSync(join(root, 'node_modules/@scope/pkg'), { recursive: true });
    writeFileSync(join(root, 't3'), '#!/bin/sh\n'); chmodSync(join(root, 't3'), 0o755);
    writeFileSync(join(root, 'node_modules/@scope/pkg/index.js'), 'x');
    symlinkSync('index.js', join(root, 'node_modules/@scope/pkg/main.js'));
    const entries = manifestOf(root);
    expect(entries.map((entry: { path: string }) => entry.path)).toEqual(['node_modules', 'node_modules/@scope', 'node_modules/@scope/pkg', 'node_modules/@scope/pkg/index.js', 'node_modules/@scope/pkg/main.js', 't3']);
    expect(entries.find((entry: { path: string }) => entry.path === 't3')).toMatchObject({ type: 'file', mode: 0o755, size: 10 });
    expect(entries.find((entry: { path: string }) => entry.path.endsWith('main.js'))).toEqual({ path: 'node_modules/@scope/pkg/main.js', type: 'link', link: 'index.js' });
  });
});
