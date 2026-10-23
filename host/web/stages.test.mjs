import { expect, test } from 'bun:test';
import { normalizeDemangled, STAGES } from './stages.mjs';

test.each(['', '[3c1c0]', '[6acfc8ca64c5829]', '[0123456789abcdef]'])(
  'inspection seam accepts crate disambiguator %s', (hash) => {
    const name = `exact_runner${hash}::agent::handle::<caltrain_web${hash}::AppData>`;
    const normalized = normalizeDemangled(name);
    expect(normalized).toBe('exact_runner::agent::handle::<caltrain_web::AppData>');
    expect(STAGES.find(stage => stage.name === 'inspection').seam.test(normalized)).toBe(true);
  },
);

test.each([
  ['<alloc[2d4e00666e4053f6]::sync::Arc<[f64]>>::drop_slow', '<alloc::sync::Arc<[f64]>>::drop_slow'],
  ['exact_runner[3c1c0]::agent::handle::<alloc[abc]::sync::Arc<[f64]>>', 'exact_runner::agent::handle::<alloc::sync::Arc<[f64]>>'],
  ['<str>::trim_matches::<<exact_web[e1bdf407d33a72ab]::host::Host<caltrain_web[9c4056522a759cff]::AppData>>::canvas_image::{closure#1}>', '<str>::trim_matches::<<exact_web::host::Host<caltrain_web::AppData>>::canvas_image::{closure#1}>'],
  ['<impl exact_runner[3c1c0]::agent::Agent>::handle::{closure#0}', '<impl exact_runner::agent::Agent>::handle::{closure#0}'],
])('normalizes crate paths without changing types or closures: %s', (name, expected) => {
  expect(normalizeDemangled(name)).toBe(expected);
});
