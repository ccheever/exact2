// dialog-trap-and-progress-value (PV-1): a drawn progress bar's value reaches assistive technology. The reference's
// Antigravity download is `<progress aria-label="Antigravity download" value max>` and its context window a
// `role="progressbar"` box with `aria-valuenow`; VoiceOver reads the name and the rounded percentage. Contract carries
// no range value (X49) and macOS exposes no drawn `progressbar` (X55), so each drawn bar is `aria-hidden` and an
// invisible indeterminate `progress` beside it carries "<name>, <n>%" (progress-value.contract). The AX read is in
// tasks/20261011-dialog-trap-and-progress-value.md.
import { describe, expect, test } from 'bun:test';
import { providerRuntime } from './provider-install';
import { contextMeter } from './composer-controls-view';
import type { ProviderSetupEntry } from './provider-setup';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
const stream = (state: unknown) => ({ state, error: '' }) as unknown as ProviderSetupEntry['install'];
const entry = (install: unknown): ProviderSetupEntry => ({ auth: stream(null), install: stream(install), authError: '', authPending: false, installError: '', installPending: '', cleared: 0, done: new Set() });
const runtime = (install: unknown) => providerRuntime({ instanceId: 'antigravity', setup: { canInstall: true } }, entry(install), 'This machine', '', true);

describe('the values', () => {
  test('Antigravity download: the name and the rounded percentage while the size is known, nothing otherwise', () => {
    expect(runtime({ phase: 'downloading', operationId: 'op', downloadedBytes: 12_345_678, totalBytes: 45_600_000 })).toMatchObject({ progressPercent: 27.1, progressLabel: 'Antigravity download, 27%' });
    expect(runtime({ phase: 'downloading', operationId: 'op', downloadedBytes: 45_600_000, totalBytes: 45_600_000 }).progressLabel).toBe('Antigravity download, 100%');
    expect(runtime({ phase: 'downloading', operationId: 'op', downloadedBytes: 1_000, totalBytes: null })).toMatchObject({ progressMax: 0, progressLabel: '' });
    expect(runtime({ phase: 'extracting', operationId: 'op' }).progressLabel).toBe('');
  });

  test('context window: the name and aria-valuenow (rounded), only while the window has a size', () => {
    const client = (usage: unknown) => ({ threadId: 't', local: { clientSettings: { contextWindowMeterEnabled: true } }, config: { providers: [] }, providerId: '',
      projection: { providerTurns: [{ tokenUsage: usage, updatedAt: '' }] } }) as never;
    expect(contextMeter(client({ usedTokens: 84_200, maxTokens: 200_000 }), '')).toMatchObject({ contextPercent: '42%', contextValueLabel: 'Context window usage, 42%' });
    expect(contextMeter(client({ usedTokens: 9_000, maxTokens: 200_000 }), '')).toMatchObject({ contextPercent: '4.5%', contextValueLabel: 'Context window usage, 5%' });
    expect(contextMeter(client({ usedTokens: 9_000, maxTokens: null }), '').contextValueLabel).toBe('');
  });
});

describe('the markup', () => {
  test('ProgressValue is an invisible indeterminate progress over its box, named by its label', async () => {
    const value = await source('progress-value.contract');
    expect(value).toContain('progress aria-label=label position="absolute" left=0 top=0 width="100%" height="100%" opacity=0 pointer-events="none" testId=progressId');
  });

  test('each drawn bar is aria-hidden, keeps its look, and has its ProgressValue beside it, never inside', async () => {
    for (const [file, bar, value] of [
      ['providers-setup.contract', 'testId=`provider-runtime-progress-${instanceId}`', 'ProgressValue(label=runtime.progressLabel, progressId=`provider-runtime-progress-value-${instanceId}`)'],
      ['composer-controls.contract', 'background-color="light-dark(#f4f4f599, #ffffff0f)" aria-hidden=true', 'ProgressValue(label=data.composer.contextValueLabel, progressId="context-window-value")'],
    ] as const) {
      const lines = (await source(file)).split('\n');
      const at = lines.findIndex(line => line.includes(bar));
      const indent = (line: string) => line.length - line.trimStart().length;
      expect(lines[at]).toContain('aria-hidden=true');
      expect(lines[at]).not.toContain('role="progressbar"');
      expect(lines[at]).not.toContain('aria-description');
      // The fill is the bar's only child; ProgressValue is its sibling, in a positioned parent.
      expect(indent(lines[at + 1]!)).toBe(indent(lines[at]!) + 2);
      const sibling = lines.findIndex((line, index) => index > at && line.trim() === value);
      expect(indent(lines[sibling]!)).toBe(indent(lines[at]!));
      let parent = at - 1;
      while (indent(lines[parent]!) >= indent(lines[at]!) || lines[parent]!.trim().startsWith('//') || lines[parent]!.trim().startsWith('when ')) parent--;
      expect(lines[parent]).toContain('position="relative"');
    }
  });

  test('no drawn role="progressbar" box is left in the app', async () => {
    const { readdirSync } = await import('node:fs');
    const files = readdirSync(new URL('./', import.meta.url)).filter(name => name.endsWith('.contract'));
    const left: string[] = [];
    for (const file of files) for (const line of (await source(file)).split('\n')) if (!line.trimStart().startsWith('//') && line.includes('role="progressbar"')) left.push(`${file}: ${line.trim().slice(0, 80)}`);
    expect(left).toEqual([]);
  });
});
