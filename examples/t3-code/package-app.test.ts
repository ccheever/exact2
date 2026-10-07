// 20261005-portable-app-download item 1: the package step's pure parts (the full step is a build:
// `bun examples/t3-code/package-app.mjs`, recorded in the task record).
import { describe, expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { SWIFT_WRAPPER, archiveName, clientVersion, remapFlags, sandboxProfile } from './package-app.mjs';
import { CLIENT_VERSION } from './version-skew';

describe('package-app', () => {
  test('the archive is named by the client version, arm64 only', () => {
    expect(clientVersion()).toBe(CLIENT_VERSION);
    expect(archiveName(CLIENT_VERSION)).toBe(`T3-Code-${CLIENT_VERSION}-arm64.zip`);
  });

  test('the sandbox denies the named trees, and the build phase every outbound connection', () => {
    const stage = sandboxProfile(['/Users/a/exact2', '/Users/a/.t3', '/Users/a/exact2']);
    expect(stage).toBe('(version 1)\n(allow default)\n(deny file-read* file-write* (subpath "/Users/a/exact2") (subpath "/Users/a/.t3"))\n');
    expect(sandboxProfile(['/x'], { offline: true })).toContain('(deny network-outbound (remote ip "*:*"))');
  });

  test('the profile really denies: a denied file cannot be read, an allowed one can', () => {
    const profile = sandboxProfile([import.meta.dir]);
    expect(spawnSync('/usr/bin/sandbox-exec', ['-p', profile, '/bin/cat', `${import.meta.dir}/app.json`]).status).not.toBe(0);
    expect(spawnSync('/usr/bin/sandbox-exec', ['-p', profile, '/bin/cat', '/etc/hosts']).status).toBe(0);
  });

  test("Rust's remaps name Cargo's home, the toolchain's std and the export", () => {
    const flags = remapFlags('/tmp/t3-code-package/exact2', { ...process.env, CARGO_HOME: '/Users/a/.cargo' });
    expect(flags).toContain('--remap-path-prefix=/Users/a/.cargo=cargo');
    expect(flags).toMatch(/--remap-path-prefix=\S+\/lib\/rustlib\/src\/rust=\/rustc\/[0-9a-f]{40}/);
    expect(flags.endsWith('--remap-path-prefix=/tmp/t3-code-package/exact2=')).toBe(true);
  });

  test('the swift wrapper adds --disable-sandbox to SwiftPM commands only', () => {
    expect(spawnSync('/bin/sh', ['-n', '-c', SWIFT_WRAPPER]).status).toBe(0);
    expect(SWIFT_WRAPPER).toContain('build|package|test|run) shift; exec /usr/bin/xcrun swift "$command" --disable-sandbox "$@"');
    expect(SWIFT_WRAPPER).toContain('*) exec /usr/bin/xcrun swift "$@"');
  });
});
