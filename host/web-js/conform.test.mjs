import { test, expect } from 'bun:test';
import { conformArgs } from './conform-args.mjs';

test('conformance selects menus without treating the job count as an app', () => {
  const { options, named } = conformArgs(['menus', '--jobs', '1', '--build', '--strict']);
  expect(named).toEqual(['menus']);
  expect(options['--jobs']).toBe('1');
});

test('conformance consumes every value option between named apps', () => {
  const values = {
    '--wasm-root': 'wasm', '--out': 'report', '--steps': '0', '--browser': 'firefox',
    '--only': 'menus', '--label': 'probe', '--jobs': '2', '--js-from': 'earlier',
  };
  const { options, named } = conformArgs(['caltrain', ...Object.entries(values).flat(), 'menus']);
  expect(named).toEqual(['caltrain', 'menus']);
  expect(options).toEqual(values);
  const defaults = conformArgs([]);
  expect(defaults.named).toEqual([]);
  expect(defaults.options['--steps']).toBe(10);
  expect(defaults.options['--jobs']).toBeGreaterThanOrEqual(1);
  expect(defaults.options['--jobs']).toBeLessThanOrEqual(4);
});

test('URL comparison does not select its URLs or label as apps', () => {
  const { options, named } = conformArgs([
    '--urls', 'caltrain', 'http://localhost:8000', 'http://localhost:8001', '--label', 'preview',
  ]);
  expect(named).toEqual([]);
  expect(options['--label']).toBe('preview');
});
