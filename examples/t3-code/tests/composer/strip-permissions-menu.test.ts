// ComposerContextPermissions at T3 Code v0.1.7 (#30275): the new thread strip's Permissions control opens its own menu,
// ComposerPermissionsSection alone, above the control and aligned to its end, wherever composer.mode lives.
import { describe, expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';

const strip = contractComponent('composer-controls.contract', 'ComposerStrip');
const source = appText('src/composer/composer-controls.contract').split('\n');
const derive = (name: string) => source.find(line => line.startsWith(`  derive ${name} = `))!;

describe('the strip\'s Permissions menu', () => {
  test.each([[true, false], [false, true], [false, false]])('opens "strip" (picker owns mode %p, overflow %p)', (pickerOwnsMode, overflowPermissions) => {
    const calls: unknown[][] = [];
    strip.call('permissionsPress', strip.state(), { composer: { pickerOwnsMode, overflowPermissions }, openOptions: (name: string) => calls.push(['open', name]),
      modelOpenTo: (name: string) => calls.push(['model', name]) }, []);
    expect(calls).toEqual([['open', 'strip']]);
  });
  test('the menu is the Permissions section alone, measured from the control, above it and aligned to its end', () => {
    expect(appText('src/shell/layout-frames.contract')).toContain('name == "strip" ? "strip-permissions"');
    expect(source.find(line => line.includes('testId="strip-permissions"'))).toContain('id="strip-permissions"');
    expect(derive('permissions')).toContain('which == "strip"');
    expect(derive('strip')).toContain('which == "strip"');
    expect(derive('below')).toStartWith('  derive below = not strip and');
    expect(derive('x')).toContain('which == "implement" or which == "context" or which == "strip" ? max(5, actionsEnd - width)');
    expect(source.find(line => line.includes('StripPermissions(composer=composer'))).toContain('open=(open == "strip")');
  });
});
