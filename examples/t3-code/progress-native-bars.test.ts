// progress-native-bars (PB-1, PB-2): every progress UI is the reference's own element and says its value to assistive
// technology, with main's LLP 1116 D8 (`progress value=… max=…`, AppKit's bar; ARIA's range values on any node, a drawn
// `progressbar` or `slider` one AppKit element with its value, minimum and maximum). T3 Code 1e2ecbd975: the Antigravity
// download is ProviderSetupSection.tsx's `<progress aria-label="Antigravity download" value max>`, the context window bar
// ContextWindowMeter.tsx's drawn `role="progressbar"` with `aria-valuemin=0 aria-valuemax=100 aria-valuenow`, the two colour
// pickers' stops and hue sliders (the theme editor's ThemeColorPicker.tsx and the provider accent's
// ProviderAccentColorPicker.tsx, both over color-picker.tsx) its range values, each stop described by the plane's
// instructions. The Usage bars stay as the reference draws them (an
// image named with its figures, a button so named, `aria-hidden` decorations). The X49/X55 workaround (an invisible busy
// indicator named "<label>, <n>%", progress-value.contract) is gone. The AX reads are in tasks/closed/20261011-progress-native-bars.md.
import { describe, expect, test } from 'bun:test';
import { readdirSync } from 'node:fs';
import { providerRuntime } from './provider-install';
import { contextMeter } from './composer-controls-view';
import type { ProviderSetupEntry } from './provider-setup';

const dir = new URL('./', import.meta.url);
const files = readdirSync(dir).filter(name => name.endsWith('.contract')).sort();
const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
const code = async (file: string) => (await source(file)).split('\n').map((line, index) => ({ file, at: index + 1, line })).filter(row => !row.line.trimStart().startsWith('//'));
const indent = (line: string) => line.length - line.trimStart().length;
const stream = (state: unknown) => ({ state, error: '' }) as unknown as ProviderSetupEntry['install'];
const entry = (install: unknown): ProviderSetupEntry => ({ auth: stream(null), install: stream(install), authError: '', authPending: false, installError: '', installPending: '', cleared: 0, done: new Set() });
const runtime = (install: unknown) => providerRuntime({ instanceId: 'antigravity', setup: { canInstall: true } }, entry(install), 'This machine', '', true);
const meter = (usage: unknown) => contextMeter(({ threadId: 't', local: { clientSettings: { contextWindowMeterEnabled: true } }, config: { providers: [] }, providerId: '',
  projection: { providerTurns: [{ tokenUsage: usage, updatedAt: '' }] } }) as never, '');

describe('the values', () => {
  test('Antigravity download: the bytes downloaded of the total while the size is known, no bar otherwise', () => {
    expect(runtime({ phase: 'downloading', operationId: 'op', downloadedBytes: 12_345_678, totalBytes: 45_600_000 })).toMatchObject({ progressValue: 12_345_678, progressMax: 45_600_000 });
    expect(runtime({ phase: 'downloading', operationId: 'op', downloadedBytes: 1_000, totalBytes: null })).toMatchObject({ progressValue: 0, progressMax: 0 });
    expect(runtime({ phase: 'extracting', operationId: 'op' })).toMatchObject({ progressMax: 0 });
    // The workaround's fields are gone: the bar says its value itself.
    const row = runtime({ phase: 'downloading', operationId: 'op', downloadedBytes: 1, totalBytes: 2 }) as Record<string, unknown>;
    expect(row.progressPercent).toBeUndefined();
    expect(row.progressLabel).toBeUndefined();
  });

  test('context window: aria-valuenow is the used share held to 0–100 and rounded, as the reference', () => {
    expect(meter({ usedTokens: 84_200, maxTokens: 200_000 })).toMatchObject({ contextPercent: '42%', contextMax: '200k', contextValueNow: 42 });
    expect(meter({ usedTokens: 9_000, maxTokens: 200_000 })).toMatchObject({ contextPercent: '4.5%', contextValueNow: 5 });
    expect(meter({ usedTokens: 250_000, maxTokens: 200_000 })).toMatchObject({ contextValueNow: 100, contextOver: true });
    // No size: the reference draws no bar (the contract's `when data.composer.contextMax != ""`).
    expect(meter({ usedTokens: 9_000, maxTokens: null })).toMatchObject({ contextMax: '', contextValueNow: 0 });
    expect((meter({ usedTokens: 1, maxTokens: 2 }) as Record<string, unknown>).contextValueLabel).toBeUndefined();
  });
});

describe('the markup', () => {
  test('Antigravity: the platform determinate progress, named and sized as the reference, with no drawn track', async () => {
    const lines = await code('providers-setup.contract');
    const bar = lines.findIndex(row => row.line.includes('progress value=runtime.progressValue max=runtime.progressMax aria-label="Antigravity download"'));
    expect(bar).toBeGreaterThan(0);
    expect(lines[bar]!.line).toContain('display="block" width="100%" height="0.25rem"');
    expect(lines[bar]!.line).not.toContain('aria-hidden');
    expect(lines[bar - 1]!.line.trim()).toBe('when runtime.progressMax > 0');
    // The `h-1` slot keeps its place while nothing downloads.
    expect(lines[bar - 2]!.line.trim()).toBe('box width="100%" height="0.25rem"');
    expect(lines.some(row => /runtime\.progress(Percent|Label)/.test(row.line))).toBe(false);
  });

  test('context window: the drawn bar is the progressbar, with the reference range values, its fill inside', async () => {
    const lines = await code('composer-controls.contract');
    const bar = lines.findIndex(row => row.line.includes('testId="context-window-bar"'));
    expect(lines[bar]!.line).toContain('role="progressbar" aria-valuemin=0 aria-valuemax=100 aria-valuenow=data.composer.contextValueNow aria-label="Context window usage"');
    expect(lines[bar]!.line).not.toContain('aria-hidden');
    expect(lines[bar - 1]!.line.trim()).toBe('when data.composer.contextMax != ""');
    expect(indent(lines[bar + 1]!.line)).toBe(indent(lines[bar]!.line) + 2);
    expect(lines[bar + 1]!.line).toContain('width=`${data.composer.contextPercent == "" ? "0%" : data.composer.contextPercent}`');
  });

  // color-picker.tsx ColorSaturationValuePlane's sr-only text, which each stop's aria-describedby names.
  const instructions = 'Use arrow keys to adjust the focused value. Hold Shift for larger steps. Use Home and End for the minimum and maximum. Press Tab to move between saturation and brightness.';

  test('theme colour picker: the stops and the hue slider carry the reference range values, no value in a description', async () => {
    const lines = await code('theme-color-picker.contract');
    const of = (id: string) => lines.find(row => row.line.includes(`testId=\`theme-color-\${row.id}-${id}\``))!.line;
    expect(of('saturation')).toContain('role="slider" aria-label=`${row.label} saturation` aria-describedby=`theme-color-${row.id}-instructions` aria-valuemin=0 aria-valuemax=100 aria-valuenow=round(cs * 100) aria-valuetext=`${round(cs * 100)}%`');
    expect(of('brightness')).toContain('role="slider" aria-label=`${row.label} brightness` aria-describedby=`theme-color-${row.id}-instructions` aria-valuemin=0 aria-valuemax=100 aria-valuenow=round(cv * 100) aria-valuetext=`${round(cv * 100)}%`');
    expect(of('hue')).toContain('role="slider" aria-label=`${row.label} hue` aria-valuemin=0 aria-valuemax=360 aria-valuenow=round(ch) aria-valuetext=`${round(ch)}`');
    for (const id of ['saturation', 'brightness', 'hue']) expect(of(id)).not.toContain('aria-description');
    expect(lines.find(row => row.line.includes('id=`theme-color-${row.id}-instructions`'))!.line.trim()).toStartWith(`text "${instructions}"`);
  });

  test('provider accent picker: the reference stops and hue slider, focusable, named without their value', async () => {
    const lines = await code('settings-b-accent.contract');
    const of = (id: string) => lines.find(row => row.line.includes(`testId="provider-accent-${id}"`))!;
    const plane = of('plane'), saturation = of('saturation'), brightness = of('brightness'), hue = of('hue');
    expect(plane.line).toContain('role="group" aria-label="Accent color saturation and brightness"');
    // The stops are the plane's, the first autofocused (Base UI's initial focus), each a Tab stop with its keys.
    for (const stop of [saturation, brightness]) {
      expect(indent(stop.line)).toBe(indent(plane.line) + 2);
      expect(stop.at).toBeGreaterThan(plane.at);
      expect(stop.line).toContain('tabindex=0');
      expect(stop.line).toContain('aria-describedby=`provider-accent-${editorId}-instructions`');
    }
    expect(saturation.line).toContain('autofocus key=axisKey("s")');
    expect(brightness.line).toContain('key=axisKey("v")');
    expect(saturation.line).toContain('role="slider" aria-label="Accent color saturation" aria-describedby=`provider-accent-${editorId}-instructions` aria-valuemin=0 aria-valuemax=100 aria-valuenow=(cs * 100) aria-valuetext=`${round(cs * 100)}%`');
    expect(brightness.line).toContain('role="slider" aria-label="Accent color brightness" aria-describedby=`provider-accent-${editorId}-instructions` aria-valuemin=0 aria-valuemax=100 aria-valuenow=(cv * 100) aria-valuetext=`${round(cv * 100)}%`');
    expect(hue.line).toContain('tabindex=0 key=hueKey');
    expect(hue.line).toContain('role="slider" aria-label="Accent color hue" aria-valuemin=0 aria-valuemax=360 aria-valuenow=round(ch)');
    expect(lines.find(row => row.line.includes('id=`provider-accent-${editorId}-instructions`'))!.line.trim()).toStartWith(`text "${instructions}"`);
  });

  test('no workaround is left: every progress element and range box says its value, none in its name', async () => {
    const left: string[] = [];
    for (const file of files) {
      for (const { at, line } of await code(file)) {
        const where = `${file}:${at}`;
        if (/\bProgressValue\b/.test(line) || line.includes('progress-value.contract')) left.push(`${where} ProgressValue`);
        // A `progress` with no value is an activity indicator: it must not carry a percentage in its name.
        if (/^\s*progress(\s+[a-z-]+=|\s*$)/.test(line) && !/\bvalue=/.test(line) && !/aria-label="[^"%$]*"/.test(line)) left.push(`${where} an indeterminate progress named by a value`);
        if (/role="(progressbar|slider|meter|spinbutton)"/.test(line) && !/\baria-valuenow=/.test(line)) left.push(`${where} a range role with no aria-valuenow`);
        if (/role="(progressbar|slider|meter|spinbutton)"/.test(line) && /aria-description=/.test(line)) left.push(`${where} a range value in a description`);
        // A value in a name (`aria-label=\`Accent color hue ${floor(ch)}\``): the value belongs in aria-valuenow.
        if (/aria-label=`[^`]*(\$\{(floor|round)\(|%`)/.test(line)) left.push(`${where} a value in a name`);
      }
    }
    expect(left).toEqual([]);
    expect(files).not.toContain('progress-value.contract');
  });

  // Each drawn fill whose width is a percentage, against the reference's markup for it; a new one fails until listed.
  const fills: Record<string, string> = {
    'usage-bars.contract:${window.remaining}': 'UsageLimits.tsx WindowBar: inside role="img" named with the summary',
    'usage-pooled.contract:${seg.remaining}': 'UsageLimitsPooled.tsx: aria-hidden, in the segment button named with its figures',
    'usage-pooled.contract:${100 - seg.remaining}': 'UsageLimitsPooled.tsx: the hatch, aria-hidden',
    'pages-usage.contract:${model.bar}': 'UsagePage.tsx model breakdown: aria-hidden',
    'settings-a-telemetry.contract:${head.width}': 'a table column width, not a bar',
    'settings-a-telemetry.contract:${part.width}': 'a table column width, not a bar',
  };

  test('every drawn track and fill is classified against the reference', async () => {
    const found: string[] = [];
    for (const file of files) for (const { line } of await code(file)) for (const m of line.matchAll(/width=`(\$\{[^`]*\})%`/g)) found.push(`${file}:${m[1]}`);
    expect(found.filter(key => !(key in fills))).toEqual([]);
    expect(Object.keys(fills).filter(key => !found.includes(key))).toEqual([]);
  });

  test('the Usage bars keep the reference accessible markup', async () => {
    const bars = (await code('usage-bars.contract')).map(row => row.line);
    const windowBar = bars.findIndex(line => line.includes('testId=`usage-bar-${slot}-${window.id}`'));
    expect(bars[windowBar]).toContain('tabindex=0 role="img" aria-label=window.summary');
    expect(bars.findIndex(line => line.includes('testId=`usage-bar-fill-${slot}-${window.id}`'))).toBeGreaterThan(windowBar);
    const pooled = (await code('usage-pooled.contract')).map(row => row.line);
    expect(pooled.find(line => line.includes('testId=`usage-seg-${seg.id}`'))).toContain('aria-label=seg.label');
    for (const id of ['usage-seg-fill-${seg.id}', 'usage-seg-hatch-${seg.id}']) expect(pooled.find(line => line.includes(`testId=\`${id}\``))).toContain('aria-hidden=true');
    const usage = (await code('pages-usage.contract')).map(row => row.line);
    const model = usage.findIndex(line => line.includes('width=`${model.bar}%`'));
    expect(usage[model - 2]).toContain('aria-hidden=true');
    const browser = (await code('browser-surface.contract')).map(row => row.line);
    const loading = browser.findIndex(line => line.includes('testId="browser-loading-bar"'));
    expect(browser[loading - 1]).toContain('aria-hidden=true');
  });
});
