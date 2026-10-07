// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): apps/web/src/components/chat/
// ModelPickerContent.test.ts "shouldOfferModelPickerSetup" (original names), then the
// picker's footer and rail as model-catalog.ts projects them.
import { describe, expect, test } from 'bun:test';
import type { Obj } from './domain';
import { shouldOfferModelPickerSetup, pickerSetupEntries } from './provider-picker-setup';
import { pickerCatalog } from './model-catalog';

function entry(status: string, driver = 'opencode', extra: Obj = {}): Obj {
  return { instanceId: `${driver}_work`, driver, enabled: true, installed: true, version: null, status, auth: { status: 'authenticated' },
    checkedAt: '2026-08-28T00:00:00.000Z', models: [], slashCommands: [], skills: [], ...extra };
}

describe('shouldOfferModelPickerSetup', () => {
  const availableModel = { slug: 'gemini-3.1-pro', name: 'Gemini 3.1 Pro' };
  test('offers setup before an Antigravity account has models', () => {
    expect(shouldOfferModelPickerSetup(entry('error', 'antigravity'), [])).toBe(true);
  });
  test('offers setup after sign-out even if a model remains cached', () => {
    expect(shouldOfferModelPickerSetup({ ...entry('ready', 'antigravity'), auth: { status: 'unauthenticated' } }, [availableModel])).toBe(true);
  });
  test('offers setup when the only model is an unavailable saved selection', () => {
    expect(shouldOfferModelPickerSetup(entry('ready', 'antigravity'), [{ ...availableModel, isUnavailable: true }])).toBe(true);
  });
  test('does not offer setup for a ready account with available models', () => {
    expect(shouldOfferModelPickerSetup(entry('ready', 'antigravity'), [availableModel])).toBe(false);
  });
  test('does not restore a disabled provider while its status snapshot is stale', () => {
    expect(shouldOfferModelPickerSetup({ ...entry('error', 'antigravity'), enabled: false }, [])).toBe(false);
  });
  test('keeps providers without integrated setup on their existing path', () => {
    expect(shouldOfferModelPickerSetup(entry('error', 'codex'), [])).toBe(false);
  });
  test("uses the environment's setup capability for other drivers", () => {
    expect(shouldOfferModelPickerSetup({ ...entry('error', 'custom_driver'), setup: { canAuthenticate: true, canInstall: false } }, [])).toBe(true);
  });
});

describe('the picker footer and rail', () => {
  const codex = { ...entry('ready', 'codex'), instanceId: 'codex', displayName: 'Codex', models: [{ slug: 'gpt', name: 'GPT' }] };
  const google = { ...entry('error', 'antigravity'), instanceId: 'google_work', displayName: 'Google work', auth: { status: 'unauthenticated' }, message: '' };
  const cursor = { ...entry('error', 'cursor'), instanceId: 'cursor', displayName: 'Cursor', installed: false, setup: { canAuthenticate: true, canInstall: false }, message: '' };
  const client = (providerId: string, favorites: string[] = [], providers = [codex, google, cursor]) =>
    ({ config: { providers, settings: {} }, local: { favoriteModels: favorites }, providerId, modelId: '', projection: {} });
  const noBadge = () => ({ providerBadge: '', providerBadgeColor: '' });

  test('an instance that needs setup opens selected and its footer offers "Open provider setup"', () => {
    const catalog = pickerCatalog(client('google_work', ['["codex","gpt"]']), '', '', noBadge);
    expect(catalog.provider).toBe('google_work');
    expect(catalog.setup).toEqual([{ id: 'google_work', message: 'Open provider setup to sign in with Google.', label: 'Open provider setup' }]);
    expect(catalog.providers.find(rail => rail.id === 'google_work')).toMatchObject({ ready: false, selectable: true });
  });
  test('an empty favorites view lists every instance that needs setup as "Set up <name>"', () => {
    const catalog = pickerCatalog(client('codex', ['["codex","missing"]']), 'favorites', '', noBadge);
    expect(catalog.count).toBe(0);
    expect(catalog.setup.map(item => item.label)).toEqual(['Set up Google work', 'Set up Cursor']);
    expect(catalog.setup[1]!.message).toBe('Open provider setup to install Cursor on this environment.');
  });
  test('a ready instance, a search, or a provider without setup shows no footer', () => {
    expect(pickerCatalog(client('codex'), 'codex', '', noBadge).setup).toEqual([]);
    expect(pickerCatalog(client('google_work'), 'google_work', 'gem', noBadge).setup).toEqual([]);
    const broken = { ...entry('error', 'codex'), instanceId: 'codex_broken' };
    const catalog = pickerCatalog(client('codex_broken', [], [codex, broken]), 'codex_broken', '', noBadge);
    expect(catalog.setup).toEqual([]);
    expect(catalog.providers.find(rail => rail.id === 'codex_broken')).toMatchObject({ ready: false, selectable: false });
  });
  test('pickerSetupEntries keeps the selected instance only', () => {
    expect(pickerSetupEntries([google, cursor], 'cursor', false, 0).map(item => item.id)).toEqual(['cursor']);
  });
});
