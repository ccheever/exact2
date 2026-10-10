// realinput-1010c-fixes: rows that failed under real input in the attended session of 2026-10-10 although agent drives
// passed. These read the Contract sources (as menu-keys.test.ts does); the drives and the next attended session prove
// the behavior. RC-3 is in menu-keys.test.ts, RC-4 in composer-chip-popover.test.ts and macos/tests/composer.
import { describe, expect, test } from 'bun:test';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
const line = (text: string, start: string) => text.split('\n').find(row => row.trimStart().startsWith(start)) ?? '';

describe('RC-2: the model picker trigger after a Shift+click removes a model', () => {
  test('its label is a new text node per label, so a shrinking label never keeps its old raster (X64)', async () => {
    const controls = await source('composer-controls.contract');
    // Keyed on the size as well: a resting composer draws the same text smaller (12/400 for 14/500), which shrinks a
    // two-model label below the raster size too.
    expect(controls).toContain('each shown in [(data.composer.fanout ? data.composer.fanoutLabel : data.modelLabel == "" ? "Choose model" : data.modelLabel)] key=`${xs}:${shown}`\n            text shown font-size=(xs ? "0.75rem" : "0.875rem") font-weight=(xs ? 400 : 500) ');
    expect(line(controls, 'text shown ')).toContain('line-clamp=1 min-width=0 flex-shrink=1 testId="model-picker-label"');
    // The trigger's accessible name was already right (the tree had it); only the painted text was stale.
    expect(controls).toContain('aria-label=(data.composer.fanout ? data.composer.fanoutAria : data.modelLabel == "" ? "Choose model" : data.modelLabel)');
  });
});

describe('RC-6: Add profile\'s keyboard walks every enabled row', () => {
  test('Blank profile, then each browser it can import from, as Base UI\'s list navigation', async () => {
    const profiles = await source('browser-profiles.contract');
    expect(profiles).toContain('fn bpAddItems(view: BrowserProfilesView): list<KmItem> = concat((view.hydrated and not view.atLimit ? [KmItem(id="browser-profiles-blank", label="Blank profile")] : []), (view.sourcesState == "ready" and view.canImport ? map(view.sources, (source) => KmItem(id=`browser-import-source-${source.id}`, label=source.name)) : []))');
    expect(profiles).toContain('SkPopup(menuId="browser-profiles-add-menu", label="Add profile", width=216, list=false, endAlign=116, items=bpAddItems(view), keyed=keyed)');
    // The rows the items name are the source rows' own ids (SkMenuItem `id=itemId`), enabled exactly when they are items.
    expect(profiles).toContain('itemId=`browser-import-source-${source.id}`, selected=false, check=false, destructive=false, disabled=(not view.canImport)');
    expect(profiles).toContain('itemId="browser-profiles-blank", selected=false, check=false, destructive=false, disabled=(not view.hydrated or view.atLimit)');
  });
});
