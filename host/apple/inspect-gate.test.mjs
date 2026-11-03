// Inspect mode and the agent's private API are compiled out of a production
// bake (LLP 1122 D5, T2c; theme2's ruling; LLP 1035.003 §3):
// `bun test host/apple/inspect-gate.test.mjs`; the async lane runs it
// (host/apple/tests/it/development.rs). Package.swift
// defines `EXACT_INSPECT` for every trust policy but production, and every
// Swift line naming inspect mode's types or the agent's private calls is
// inside an `#if` on it, so a production build compiles none of it. Shipped
// code reads no undocumented defaults key, notification or string table. A
// production Mac bake on disk (from
// `EXACT_UPDATE_TRUST=production bun host/apple/build.mjs theme-fixture-apple`)
// is checked for their strings too.
import { test, expect } from 'bun:test';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const root = resolve(import.meta.dir, '../..');
const inspectNames = /\b(Inspect|InspectHost|InspectOverlay)\b|\binspect(Command|Tapped)\b|\bleaveInspect\b|\bgroupId\b|\bshownHeader\b|\bitemNodes\b/;

function swiftFiles(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) => e.isDirectory() ? swiftFiles(join(dir, e.name)) : e.name.endsWith('.swift') ? [join(dir, e.name)] : []);
}

test('Package.swift defines EXACT_INSPECT for development and not for production', () => {
  const defines = (trust) => {
    const out = spawnSync('swift', ['package', 'dump-package'], { cwd: resolve(root, 'host/apple'), env: { ...process.env, EXACT_UPDATE_TRUST: trust }, encoding: 'utf8' });
    expect(out.status).toBe(0);
    return out.stdout.includes('"EXACT_INSPECT"');
  };
  expect(defines('development')).toBe(true);
  expect(defines('production')).toBe(false);
});

/** Each line of the Swift under `host/apple/Sources` that matches `names`
 * outside an `#if EXACT_INSPECT`, quoted strings left in when `strings`. */
function outsideInspect(names, { strings = false } = {}) {
  const outside = [];
  for (const file of swiftFiles(resolve(root, 'host/apple/Sources'))) {
    const stack = [];
    readFileSync(file, 'utf8').split('\n').forEach((line, i) => {
      const t = line.trim();
      if (t.startsWith('#if ')) stack.push(t.includes('EXACT_INSPECT') && !t.includes('!EXACT_INSPECT'));
      else if (t.startsWith('#else') || t.startsWith('#elseif')) stack[stack.length - 1] = false;
      else if (t.startsWith('#endif')) stack.pop();
      else if (!t.startsWith('//') && !t.startsWith('///')) {
        const code = line.replace(/\/\/.*$/, '');
        if (names.test(strings ? code : code.replace(/"[^"]*"/g, '')) && !stack.includes(true)) outside.push(`${file.slice(root.length + 1)}:${i + 1}: ${t}`);
      }
    });
  }
  return outside;
}

// The agent's private calls: an event made in a window (CoreGraphics' setter
// outside its headers, an undocumented field), a gesture event's raw type and
// fields, Core Animation's filter keys.
const agentPrivate = /CGEventSetWindowLocation|CGEventField\(rawValue|CGEventType\(rawValue|inputRadius|"captureOnly"/;

test('every agent private call is inside an #if on EXACT_INSPECT', () => {
  expect(outsideInspect(agentPrivate, { strings: true })).toEqual([]);
});

// Undocumented defaults keys, notifications and string tables, which shipped
// code does not read (LLP 1035.003 §3).
const undocumented = /"Apple(InterfaceStyle|InterfaceThemeChangedNotification|ICUForce24HourTime|ICUForce12HourTime|Persistence)"|table: "Placeholders"|"WKJavaScriptException/;

test('shipped code reads no undocumented defaults key, notification or string table', () => {
  const read = [resolve(root, 'host/apple/Sources'), resolve(root, 'host/apple/webarm')].flatMap(swiftFiles).flatMap((file) =>
    readFileSync(file, 'utf8').split('\n').map((line, i) => [line, i]).filter(([line]) => !line.trim().startsWith('//') && undocumented.test(line))
      .map(([line, i]) => `${file.slice(root.length + 1)}:${i + 1}: ${line.trim()}`));
  expect(read).toEqual([]);
});

test('every Swift line naming inspect mode is inside an #if on EXACT_INSPECT', () => {
  expect(outsideInspect(inspectNames)).toEqual([]);
});

const bake = resolve(root, 'target/clients');
const productionBinaries = () => !existsSync(bake) ? [] : readdirSync(bake).map((d) => join(bake, d, 'com.exact.theme-fixture/macos/aarch64-apple-darwin/embedded/production/standalone/Theme fixture')).filter(existsSync);

test.skipIf(productionBinaries().length === 0)('a production Mac bake has none of inspect mode\'s strings or symbols', () => {
  for (const binary of productionBinaries()) {
    const strings = spawnSync('strings', [binary], { encoding: 'utf8', maxBuffer: 1 << 28 }).stdout;
    expect(strings).not.toContain('not a view of the app');
    expect(strings).not.toContain('identity alone is larger than a report');
    for (const word of ['CGEventSetWindowLocation', 'inputRadius', 'captureOnly', 'AppleInterfaceStyle', 'AppleInterfaceThemeChangedNotification', 'AppleICUForce24HourTime', 'AppleICUForce12HourTime', 'ApplePersistence']) expect(strings).not.toContain(word);
    const symbols = spawnSync('nm', [binary], { encoding: 'utf8', maxBuffer: 1 << 28 }).stdout;
    expect(symbols.split('\n').filter((s) => /InspectHost|InspectOverlay|7Inspect/.test(s))).toEqual([]);
  }
});
